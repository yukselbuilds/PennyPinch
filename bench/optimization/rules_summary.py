"""Read every matching held-out trial; never run a client or make a model call."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
PLAN = ROOT / "bench/optimization/rules-heldout-plan.json"
FIELDS = {"total_input_tokens", "output_tokens", "total_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"}


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def summarize(plan, records):
    required = {(case["id"], repeat, cache) for case in plan["cases"]
                for repeat in range(plan["repetitions_per_case_and_cache_start"]) for cache in plan["required_cache_starts"]}
    seen, duplicates, invalid, accepted = set(), [], [], []
    for record in records:
        try:
            observation = record["heldout_observations"]
            slot = (record["case"], record["repeat_index"], observation["cache_start_requested"])
            assert slot in required, "unexpected_trial"
            if slot in seen:
                duplicates.append(list(slot))
            seen.add(slot)
            assert record.get("inputs_match") is True, "inputs_changed_or_unverified"
            assert record["passed"] is True and record["failure"] is None, "trial_failed"
            assert record["arm_order"] == plan["pair_order_by_repetition"][slot[1]], "arm_order_changed"
            assert observation["cache_start_observed"] == slot[2] and observation["cache_start_matches"] is True, "cache_start_unconfirmed"
            assert record["prior_attempts"] == 0 and record["pair_attempt_limit"] == plan["live_execution"]["max_requests_per_pair"], "pair_budget_changed"
            rows = record["rows"]
            assert len(rows) == 2 and [row["arm"] for row in rows] == record["arm_order"], "arm_scope_changed"
            assert all(type(record[key]) is int and record[key] >= 0 for key in ("pair_attempts_total", "prior_attempts", "pair_attempt_limit")), "invalid_budget"
            assert record["pair_attempts_total"] <= record["pair_attempt_limit"], "pair_budget_exceeded"
            assert sum(row["forwarded_http_attempts"] for row in rows) == record["pair_attempts_total"], "request_scope_changed"
            assert len(record["stops"]) == 2 and all(item["shutdown"] == "graceful" and item["engine_exit_code"] == 0
                and item["owned_port_closed"] is True for item in record["stops"]), "owned_shutdown_failed"
            for row in rows:
                assert all(row[key] is True for key in ("turn_completed", "exact_answer", "fixed_tool_call")), "quality_failed"
                assert all(row["tool_checks"][key] is True for key in ("sequence_valid", "owned_server", "all_completed", "no_reported_errors")), "tool_scope_failed"
                assert row["request_count"] == row["usage_record_count"] == row["forwarded_http_attempts"] > 0, "usage_scope_changed"
                usage = row["total_usage"]
                assert set(usage) == FIELDS and all(type(value) is int and value >= 0 for value in usage.values()), "unknown_usage"
                assert usage["total_input_tokens"] + usage["output_tokens"] == usage["total_tokens"], "usage_sum_changed"
                assert max(usage["cache_read_input_tokens"], usage["cache_creation_input_tokens"]) <= usage["total_input_tokens"], "invalid_cache_usage"
                cached = row["first_upstream_cache_read_input_tokens"]
                assert type(cached) is int and (cached == 0 if slot[2] == "cold" else cached > 0), "cache_start_unconfirmed"
                resource = row["resources"]
                assert row["resource_error"] is None and all(type(resource[key]) in (int, float) and math.isfinite(resource[key])
                    and resource[key] >= 0 for key in ("local_cpu_ms", "observed_engine_private_bytes")), "unknown_resources"
                assert type(row["elapsed_ms"]) in (int, float) and math.isfinite(row["elapsed_ms"]) and row["elapsed_ms"] >= 0, "unknown_elapsed"
            accepted.append((slot, {row["arm"]: row for row in rows}))
        except (AssertionError, KeyError, TypeError, ValueError, IndexError) as error:
            reason = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z_]+", str(error)) else "invalid_trial_record"
            invalid.append({"trial_index": len(accepted) + len(invalid), "reason": reason})
    missing = sorted(required - seen)
    complete = len(records) == len(required) and not missing and not duplicates and not invalid
    groups = {}
    if complete:
        limits = plan["acceptance"]
        p95 = lambda values: sorted(values)[math.ceil(len(values) * .95) - 1]
        for cache in plan["required_cache_starts"]:
            pairs = [rows for slot, rows in accepted if slot[2] == cache]
            control = sum(rows["control"]["total_usage"]["total_tokens"] for rows in pairs)
            candidate = sum(rows["candidate"]["total_usage"]["total_tokens"] for rows in pairs)
            elapsed = p95([rows["candidate"]["elapsed_ms"] - rows["control"]["elapsed_ms"] for rows in pairs])
            cpu = p95([rows["candidate"]["resources"]["local_cpu_ms"] for rows in pairs])
            memory = max(row["resources"]["observed_engine_private_bytes"] for rows in pairs for row in rows.values())
            reduction = (control - candidate) / control if control else None
            passed = (reduction is not None and reduction >= limits["minimum_total_provider_usage_reduction_fraction_per_cache_start"]
                and elapsed <= limits["added_completed_task_elapsed_p95_ms"] and cpu <= limits["candidate_completed_task_cpu_p95_ms"]
                and memory <= limits["engine_observed_private_bytes"])
            groups[cache] = {"pairs": len(pairs), "control_total_tokens": control, "candidate_total_tokens": candidate,
                "total_tokens_saved": control - candidate, "percent_of_control": round(100 * reduction, 4) if reduction is not None else None,
                "added_completed_task_elapsed_p95_ms": elapsed, "candidate_completed_task_cpu_p95_ms": cpu,
                "engine_observed_private_bytes": memory, "passed": passed}
    return {"passed": complete and all(group["passed"] for group in groups.values()), "required_pairs": len(required),
        "records_seen": len(records), "missing_pairs": [list(slot) for slot in missing], "duplicate_pairs": duplicates,
        "invalid_pairs": invalid, "cache_groups": groups, "automatic_product_activation": False}


def verify_inputs(record, run):
    local_app = Path(os.environ["LOCALAPPDATA"]).resolve(strict=True)
    codex = local_app / "OpenAI/Codex/bin"
    profiles = local_app / "dev.headroom.desktop/desktop-live-proof"
    stable = {}
    snapshots = record["fingerprints"]
    assert set(snapshots) == {"control", "candidate"}, "missing_fingerprints"
    for snapshot in snapshots.values():
        assert snapshot["before"] == snapshot["after"], "inputs_changed"
        for filename, digest in snapshot["after"].items():
            path = Path(filename).resolve(strict=True)
            allowed_profile = path.name == "config.toml" and path.parent.parent == profiles
            assert path.is_relative_to(ROOT) or path.is_relative_to(codex) or allowed_profile, "input_path_outside_scope"
            assert sha(path) == digest, "inputs_changed"
            if not path.is_relative_to(run):
                assert path.as_posix() not in stable or stable[path.as_posix()] == digest, "conflicting_inputs"
                stable[path.as_posix()] = digest
            elif path.name == "models.json":
                stable["account_model_catalog"] = digest
    for path in (ROOT / "bench/optimization/rules_live.py", ROOT / "bench/optimization/rules_pilot.py", PLAN, Path(sys.executable).resolve()):
        assert stable.get(path.as_posix()) == sha(path), "required_input_missing"
    return stable


def main():
    assert Path.cwd().resolve(strict=True) == ROOT == Path(r"D:\Projeler\Headroom")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--provider", choices=("chatgpt", "loopback_fixture"), required=True)
    parser.add_argument("--model", required=True)
    parser.add_argument("--effort", required=True)
    args = parser.parse_args()
    sources = [Path(__file__).resolve(), PLAN, ROOT / "tests/runtime/test_rules_summary.py", Path(sys.executable).resolve()]
    before = {str(path): sha(path) for path in sources}
    plan = json.loads(PLAN.read_bytes())
    records, artifacts, signatures = [], [], []
    base = ROOT / "work/proofs/optimization/rules-live"
    for run in sorted(base.iterdir()):
        if not run.is_dir() or not re.fullmatch(r"[a-f0-9]{32}", run.name) or not (run / "result.json").is_file():
            continue
        path = run / "result.json"
        assert path.stat().st_size <= 2 * 1024 * 1024, "trial_report_too_large"
        raw = path.read_bytes()
        record = json.loads(raw)
        if (record.get("provider"), record.get("model"), record.get("effort")) != (args.provider, args.model, args.effort):
            continue
        if record.get("case") not in {case["id"] for case in plan["cases"]}:
            continue
        try:
            signature = verify_inputs(record, run)
            if signatures:
                assert signature == signatures[0], "inconsistent_trial_inputs"
            else:
                signatures.append(signature)
            record["inputs_match"] = True
        except (AssertionError, OSError, KeyError, TypeError):
            record["inputs_match"] = False
        records.append(record)
        artifacts.append({"path": path.relative_to(ROOT).as_posix(), "sha256": hashlib.sha256(raw).hexdigest()})
    result = summarize(plan, records)
    after = {str(path): sha(path) for path in sources}
    assert before == after, "summary_inputs_changed"
    result.update(provider=args.provider, model=args.model, effort=args.effort, artifacts=artifacts,
                  fingerprints={"before": before, "after": after}, model_calls=0,
                  scope="Live matrix acceptance" if args.provider == "chatgpt" else "Local protocol fixtures only")
    print(json.dumps(result, separators=(",", ":")))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())

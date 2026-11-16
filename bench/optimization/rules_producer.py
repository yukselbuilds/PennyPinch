"""Pair actual subprocess captures through the same pinned engine profile on loopback."""
import argparse
import hashlib
from http.server import ThreadingHTTPServer
import json
import os
from pathlib import Path
import platform
import re
import runpy
import site
import sys
import threading
import time
import uuid

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
CAPTURE_PATH = HERE.parent / "context_tools/targeted_query_replay.py"
capture_proof = runpy.run_path(str(CAPTURE_PATH))
replay = capture_proof["replay"]
require, sha, encoded = (replay[name] for name in ("require", "sha", "encoded"))
query = capture_proof["query"]
ACCEPTED = b"INFO batch=clover state=settled amount=9007199254740997\r\n"
HELD = b"WARN batch=clover state=held ratio=-17.2500\r\n"
AUDIT = ACCEPTED * 48 + HELD * 16 + ACCEPTED * 9 + b"SUMMARY batch=clover complete\r\n"
REPORT = b'{"receipt":9007199254740997,"ratio":-17.2500,"precision":0.00000000000000013}\r\n'
CCR_REFERENCE = re.compile(r"(?:Retrieve (?:more|original): hash=|<<ccr:)([a-fA-F0-9]{12,24})(?=[^a-fA-F0-9]|$)")
RETRIEVE_ID = "call-audit-retrieval"
CASES = {
    "audit": ("exec_command", AUDIT, b"", 0, "Count the settled and held receipts and report the exact amount and ratio."),
    "audit_retrieval": ("exec_command", AUDIT, b"", 0, "Count the settled and held receipts and report the exact amount and ratio."),
    "warning": ("exec_command", AUDIT, b"WARN audit metadata incomplete\r\n", 0, "Report the audit warning and its exact exit code."),
    "failure": ("exec_command", AUDIT, b"", 7, "Report whether the audit succeeded, using its actual exit code."),
    "exact_read": ("Read", REPORT, b"", 0, "Return the exact receipt, ratio and precision as written."),
}


class Fixture(replay["FixtureHandler"]):
    def do_POST(self):
        key = self.headers.get("X-Proof-Case", "")
        if "-audit_retrieval-" not in key:
            return super().do_POST()
        size = int(self.headers.get("Content-Length", "0"))
        auth = self.headers.get("Authorization") == "Bearer " + replay["SENTINEL"]
        if self.path != "/v1/responses" or not 0 < size <= 2 * 1024 * 1024 or not auth:
            self.send_error(400)
            return
        body = json.loads(self.rfile.read(size))
        if body.get("stream") is not False:
            self.send_error(400)
            return
        with self.server.capture_lock:
            attempts = self.server.captured.setdefault(key, [])
            attempts.append({"body": body, "auth_preserved": auth})
            first = len(attempts) == 1
        match = None
        if first and key.endswith("-t1"):
            match = CCR_REFERENCE.search(next(item["output"] for item in body["input"]
                if item.get("type") == "function_call_output" and item.get("call_id") == "call-audit"))
        item = {"type": "message", "id": "msg_audit", "role": "assistant", "status": "completed",
                "content": [{"type": "output_text", "text": "FIXTURE_OK", "annotations": []}]}
        if match:
            item = {"type": "function_call", "id": "fc_audit", "call_id": RETRIEVE_ID,
                    "name": "headroom_retrieve", "arguments": json.dumps({"hash": match[1]})}
        data = encoded({"object": "response", "id": "resp_audit", "model": body["model"],
                        "status": "completed", "output": [item]})
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


def compact(archive, owner, handle, case, report_rule, collapse, expand):
    output = archive.read_original(owner, handle)
    envelope = capture_proof["read_result"](output)
    arguments = {"cmd": "audit isolated receipts", "workdir": "."} if case != "exact_read" else capture_proof["READ_ARGS"]
    items = [
        {"type": "function_call", "name": output.tool, "call_id": output.call_id, "arguments": encoded(arguments).decode()},
        {"type": "function_call_output", "call_id": output.call_id, "output": envelope},
    ]
    raw = b"\n".join(encoded(row) for row in [
        {"type": "session_meta", "payload": {"id": handle}},
        *[{"type": "response_item", "payload": item} for item in items],
    ]) + b"\n"
    outcome = report_rule(raw)
    require(outcome["status"] == "trial_ready", "rule_did_not_read_capture")
    report = outcome["report"]
    eligible = int(case in ("audit", "audit_retrieval"))
    require(report["original_sha256"] == hashlib.sha256(raw).hexdigest()
            and report["round_trip_exact"] and report["checked"] == 1 and report["ambiguous"] == 0
            and (report["eligible"], report["compacted"], report["protected"]) == (eligible, eligible, 1 - eligible),
            "rule_capture_gate_changed")
    candidate = dict(envelope)
    if eligible:
        candidate["stdout"] = collapse(envelope["stdout"])
        restored = expand(candidate["stdout"])
        require(restored.encode() == output.stdout and {**candidate, "stdout": restored} == envelope,
                "recovered_capture_changed")
        require(report["before_bytes"] == len(output.stdout) and report["after_bytes"] == len(candidate["stdout"].encode()),
                "rule_byte_measurement_changed")
        lines = restored.encode().splitlines(keepends=True)
        require(lines.count(ACCEPTED) == 57 and lines.count(HELD) == 16 and len(lines) == 74,
                "audit_exact_counts_changed")
    return arguments, envelope, candidate, report


def observe(engine, server, key, case, body, previous, plan, count):
    sampler = replay["Resources"]([os.getpid(), *engine.pids], plan["memory_sample_interval_ms"])
    try:
        timing = replay["request"](engine.port, body, key, plan["thresholds"]["request_timeout_seconds"])
    finally:
        resources = sampler.finish()
    with server.capture_lock:
        attempts = server.captured.pop(key, [])
    continuation_due = case == "audit_retrieval" and key.endswith("-control-t1")
    require(len(attempts) == 1 + int(continuation_due) and all(row["auth_preserved"] for row in attempts),
            "unexpected_attempt_or_auth")
    forwarded = attempts[0]["body"]
    replay["validate_body"](body, forwarded, previous, False)
    require(forwarded["tools"] == body["tools"], "engine_tool_schema_changed")
    restored = 0
    for sent, received in zip(body["input"], forwarded["input"], strict=True):
        if sent == received:
            continue
        require(sent["type"] == "function_call_output" and isinstance(received.get("output"), str)
                and {**received, "output": sent["output"]} == sent, "engine_changed_non_output_data")
        references = set(CCR_REFERENCE.findall(received["output"]))
        require(len(references) == 1, "unverifiable_engine_output_" + case)
        original = json.loads(sent["output"])
        observed = json.loads(received["output"])
        require(isinstance(observed, dict) and observed.keys() == original.keys()
                and encoded({**observed, "stdout": original["stdout"]}) == encoded(original)
                and len(CCR_REFERENCE.findall(observed["stdout"])) == 1, "engine_changed_capture_envelope")
        replay["retrieval"](engine.port, references.pop(), original["stdout"], plan["thresholds"]["request_timeout_seconds"])
        restored += 1
    if continuation_due:
        continuation = attempts[1]["body"]
        require({**continuation, "input": forwarded["input"]} == forwarded, "continuation_changed_request_contract")
        previous_items = forwarded["input"]
        require(continuation["input"][:len(previous_items)] == previous_items, "continuation_changed_prefix")
        extra = continuation["input"][len(previous_items):]
        require(len(extra) == 2 and extra[0].get("name") == "headroom_retrieve"
                and all(item.get("call_id") == RETRIEVE_ID for item in extra)
                and extra[1].get("type") == "function_call_output"
                and json.loads(extra[1]["output"])["original_content"].encode() == AUDIT, "continuation_original_changed")
    tokens = [count(encoded(row["body"]).decode()) for row in attempts]
    return forwarded, {"primary_input_json_tokens": tokens[0], "request_input_json_tokens": sum(tokens),
                       "upstream_attempts": len(attempts), "local_ccr_readbacks_verified": restored,
                       "exact_ccr_continuations": int(continuation_due), **timing, **resources}


def paired(engine, server, repeat, case, plan, count, report_rule, collapse, expand, retrieve_tool):
    tool, stdout, stderr, code, prompt = CASES[case]
    original = capture_proof["capture"](engine, tool, "call-audit", stdout, stderr, code)
    archive, owner = query.Archive(), object()
    handle = archive.keep(owner, original)
    capture_proof["expect_error"](lambda: archive.read_original(object(), handle), LookupError, "archive_unavailable")
    started, cpu = time.perf_counter(), time.process_time()
    arguments, control, candidate, report = compact(archive, owner, handle, case, report_rule, collapse, expand)
    processing = {"prepare_ms": 1000 * (time.perf_counter() - started), "prepare_cpu_ms": 1000 * (time.process_time() - cpu)}
    require(archive.read_original(owner, handle) == original, "archive_original_changed")

    request = capture_proof["request_body"](prompt)
    request["model"] = plan["model"]
    request["reasoning"] = {"effort": plan["reasoning_effort"]}
    request["tools"] = [{"type": "function", "name": tool,
        "description": "Read the owned fixture. A repeat marker reports the total occurrences of the preceding line.",
        "parameters": {"type": "object", "properties": {key: {"type": "string"} for key in arguments},
                       "required": list(arguments), "additionalProperties": False}}]
    if case == "audit_retrieval":
        request["tools"].append(retrieve_tool)
    rows, previous = {"control": [], "candidate": []}, {}
    arms = ("control", "candidate") if repeat % 2 == 0 else ("candidate", "control")
    values = {"control": control, "candidate": candidate}
    histories = {arm: capture_proof["append_result"](request, tool, original.call_id, arguments, envelope)
                 for arm, envelope in values.items()}
    followup = {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "Confirm the exact result."}]}
    for turn, stage in enumerate(plan["turns"]):
        for arm in arms:
            body = request if turn == 0 else histories[arm]
            if turn == 2:
                body = {**body, "input": [*body["input"], followup]}
            key = f"r{repeat}-{case}-{arm}-t{turn}"
            previous[arm], row = observe(engine, server, key, case, body, previous.get(arm), plan, count)
            rows[arm].append({"stage": stage, **row})
    totals = {arm: sum(row["request_input_json_tokens"] for row in values) for arm, values in rows.items()}
    require(report["eligible"] or control == candidate, "protected_capture_modified_by_rule")
    return {"case": case, "repeat": repeat + 1, "archive_identity_verified": True,
            "paired_input_tokens": totals, "input_reduction_fraction": (totals["control"] - totals["candidate"]) / totals["control"],
            "compacted_outputs": report["compacted"], "protected_outputs": report["protected"],
            "before_bytes": len(original.stdout), "after_bytes": len(candidate["stdout"].encode()),
            "rows": rows, **processing}


def main():
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("--run", action="store_true", required=True)
    args.parse_args()
    require(os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_workspace")
    plan_path = HERE / "rules-producer-plan.json"
    plan = json.loads(plan_path.read_bytes())
    require(tuple(plan["cases"]) == tuple(CASES) and plan["turns"] == ["initial", "tool_result", "same_session_followup"], "plan_changed")
    runtime = ROOT / "runtime"
    controller = replay["load_module"]("rules_producer_controller", runtime / "desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(runtime / "manifest.json")
    require(sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "pinned_python_required")
    runner = payload / manifest["engine"]["runner"]
    supervisor = replay["load_module"]("rules_producer_supervisor", runner)
    cache = controller.verify_tokenizer_cache(payload)
    os.environ["TIKTOKEN_CACHE_DIR"] = str(cache)
    packages = payload / "Lib/site-packages"
    site.addsitedir(str(packages))
    import tiktoken
    from headroom.ccr.tool_injection import create_ccr_tool_definition
    from headroom.transforms.lossless_compaction import collapse_runs, expand_runs

    tokenizer = tiktoken.get_encoding(plan["token_encoding"])
    retrieve_tool = create_ccr_tool_definition("openai_responses")
    rule_path = runtime / "learn_preview.py"
    report_rule = replay["load_module"]("rules_producer_local_replay", rule_path).local_replay
    run = ROOT / "work/proofs/optimization/rules-producer" / uuid.uuid4().hex
    require(run.resolve().is_relative_to(ROOT), "proof_directory_outside_workspace")
    run.mkdir(parents=True)
    staged = replay["prepare_fixture_runtime"](run, payload)
    replay["write_new"](run / "frozen-plan.json", plan)
    expected_sources = json.loads((runtime / "optimization-sources.json").read_bytes())
    paths = [Path(__file__), plan_path, run / "frozen-plan.json", CAPTURE_PATH, CAPTURE_PATH.with_name("targeted_query.py"),
             HERE / "replay.py", HERE / "fixtures.py", rule_path, Path(sys.executable), runner,
             payload / "runtime-manifest.json", payload / "python313.dll", *staged,
             *[runtime / path.name for path in staged], *[packages / "headroom" / name for name in expected_sources],
             packages / "headroom/transforms/lossless_compaction.py", packages / "headroom/ccr/tool_injection.py",
             *[path for path in cache.iterdir() if path.is_file()],
             *list((packages / "tiktoken").glob("*.pyd"))]
    before = replay["fingerprints"](paths)
    server = ThreadingHTTPServer(("127.0.0.1", 0), Fixture)
    server.capture_lock, server.captured = threading.Lock(), {}
    threading.Thread(target=server.serve_forever, daemon=True).start()
    pairs, stops, failure = [], [], None
    try:
        for repeat in range(plan["repetitions"]):
            engine = replay["Engine"](controller, supervisor, payload, run / f"engine-{repeat}", server.server_port, plan)
            try:
                engine.start()
                for case in plan["cases"]:
                    row = paired(engine, server, repeat, case, plan,
                                 lambda value: len(tokenizer.encode(value, disallowed_special=())),
                                 report_rule, collapse_runs, expand_runs, retrieve_tool)
                    pairs.append(row)
                    print(json.dumps({key: row[key] for key in ("case", "repeat", "paired_input_tokens", "compacted_outputs")}), flush=True)
                require(not server.captured, "unattributed_attempt")
            finally:
                engine.close()
                stops.append(engine.stopped)
    except Exception as error:
        failure = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)) else type(error).__name__
    finally:
        server.shutdown()
        server.server_close()
    after = replay["fingerprints"](paths)
    network = replay["fixture_network_evidence"](run)
    passed = failure is None and before == after and network["huggingface_hub_payload_bytes"] == 0
    limits = plan["thresholds"]
    requests = [row for pair in pairs for sequence in pair["rows"].values() for row in sequence]
    audit = [pair for pair in pairs if pair["case"] == "audit"]
    p95 = replay["percentile"]
    local_gate = None
    if passed:
        added_p95 = p95([rule["total_ms"] - base["total_ms"] + (pair["prepare_ms"] if base["stage"] == "tool_result" else 0)
                         for pair in pairs for base, rule in zip(pair["rows"]["control"], pair["rows"]["candidate"], strict=True)])
        cpu_p95 = p95([row["harness_cpu_ms"] + row["engine_cpu_ms"] + (pair["prepare_cpu_ms"] if row["stage"] == "tool_result" else 0)
                       for pair in pairs for row in pair["rows"]["candidate"]])
        peak_private = max(row["observed_engine_private_bytes"] for row in requests)
        local_gate = {"minimum_audit_reduction": min(pair["input_reduction_fraction"] for pair in audit),
                      "minimum_audit_with_retrieval_reduction": min(pair["input_reduction_fraction"] for pair in pairs if pair["case"] == "audit_retrieval"),
                      "added_turn_p95_ms": added_p95, "candidate_turn_cpu_p95_ms": cpu_p95, "engine_peak_sampled_private_bytes": peak_private}
        local_gate["pass"] = (local_gate["minimum_audit_reduction"] >= limits["audit_min_full_sequence_reduction_fraction"]
                              and added_p95 <= limits["added_turn_p95_ms"] and cpu_p95 <= limits["candidate_turn_cpu_p95_ms"]
                              and peak_private <= limits["engine_observed_private_bytes"])
    proof = run / "result.json"
    replay["write_new"](proof, {"passed": passed, "failure": failure, "engine": manifest["engine"]["version"], "os": platform.platform(),
                               "pairs": pairs, "stops": stops, "local_gate": local_gate, "fixture_network": network,
                               "client_requests": len(requests), "upstream_requests": sum(row["upstream_attempts"] for row in requests),
                               "provider_model_calls": 0,
                               "provider_quality_cache_and_usage": "not_tested", "before": before, "after": after})
    print(json.dumps({"passed": passed, "failure": failure, "pairs": len(pairs), "local_gate": local_gate,
                      "proof": proof.relative_to(ROOT).as_posix()}), flush=True)
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

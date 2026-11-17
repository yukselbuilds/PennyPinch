"""One bounded Codex rule comparison. Default mode uses only a loopback provider."""
import argparse
from http.server import ThreadingHTTPServer
import json
import math
import os
from pathlib import Path
import platform
import re
import runpy
import shutil
import subprocess
import sys
import threading
import time
import uuid

PROGRAM = Path(__file__).resolve()
ROOT = PROGRAM.parents[2]
CASE = "counts"
RECOVERY_TOOL = "headroom_retrieve"
ATTEMPTS_PER_PAIR = 8
MIN_ATTEMPTS_PER_ARM = 2
ARM_TIMEOUT = 120
MAX_REQUEST_BYTES = 256 * 1024
ANSWER_SCHEMA = {
    "type": "object",
    "properties": {
        "passed": {"type": "integer"},
        "held": {"type": "integer"},
        "exact_limit": {"type": "string"},
        "signed_factor": {"type": "string"},
    },
    "required": ["passed", "held", "exact_limit", "signed_factor"],
    "additionalProperties": False,
}


def upstream_error_summary(status, content):
    text = content.decode("utf-8", errors="replace") if len(content) <= 16 * 1024 else ""
    try:
        body = json.loads(text) if text else None
    except (ValueError, RecursionError):
        body = text if not text.lstrip().startswith("<") else None
    error = body.get("error", body.get("detail", body.get("message"))) if isinstance(body, dict) else body
    if isinstance(error, dict):
        error = " ".join(error[key] for key in ("code", "type", "param", "message") if isinstance(error.get(key), str))
    # Emit fixed protocol labels only. Provider messages can contain account data.
    markers = {
        "unsupported": r"unsupported|not support(?:ed)?|not available",
        "invalid": r"(?<![.\w])invalid(?:_request|_value|\b)|not valid",
        "missing": r"missing|required|must be provided",
        "sign_in": r"login|sign.in|account|authentication",
        "quota": r"\bquota\b|usage.limit|rate.limit",
        "model": r"\bmodel\b",
        "instructions": r"\binstructions\b",
        "tools": r"\btools?\b",
        "function": r"\bfunction\b",
        "namespace": r"\bnamespace\b",
        "tool_search": r"\btool_search\b",
        "reasoning": r"\breasoning\b|\beffort\b",
        "service_tier": r"\bservice_tier\b",
        "store": r"\bstore\b",
        "stream": r"\bstream\b|\bwebsocket\b",
        "input": r"\binput\b",
        "include": r"\binclude\b",
        "verbosity": r"\bverbosity\b",
    }
    return {"status": status, "tags": [tag for tag, pattern in markers.items()
                                      if isinstance(error, str) and re.search(pattern, error, re.IGNORECASE)]}


def capture_evidence(tool, texts, capture):
    encoded = json.dumps(capture, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
    quoted = json.dumps(encoded, ensure_ascii=True)
    return {"tool": tool, "capture_exact": any(encoded in text or quoted in text for text in texts),
            "text_blocks": len(texts),
            "ccr_marker_present": any("<<ccr:" in value for value in texts)}


def forwarded_tool_evidence(body, capture):
    from headroom.config import unwrap_tool_call_name

    items = body.get("input", [])
    if not isinstance(items, list):
        return []
    calls = {}
    for item in items:
        if not isinstance(item, dict) or item.get("type") not in ("function_call", "custom_tool_call"):
            continue
        identity = item.get("call_id")
        if not isinstance(identity, str) or not identity:
            continue
        name = item.get("name")
        if name in ("exec", "functions.exec"):
            effective = unwrap_tool_call_name(name, item.get("input") or item.get("arguments"))
            calls[identity] = RECOVERY_TOOL if effective == RECOVERY_TOOL else "exec"
        for tool in ("inspect_result", RECOVERY_TOOL):
            if ((item.get("namespace") == "mcp__headroom_rule_pilot" and item.get("name") == tool)
                    or item.get("name") == "mcp__headroom_rule_pilot__" + tool):
                calls[identity] = tool
    results = []
    for item in items:
        if (not isinstance(item, dict) or item.get("type") not in ("function_call_output", "custom_tool_call_output")
                or not isinstance(item.get("call_id"), str) or item["call_id"] not in calls):
            continue
        value = item.get("output")
        texts = []
        if isinstance(value, str):
            texts = [value]
        elif isinstance(value, list):
            texts = [part["text"] for part in value if isinstance(part, dict)
                     and part.get("type") == "input_text" and isinstance(part.get("text"), str)]
        results.append(capture_evidence(calls[item.get("call_id")], texts, capture))
    return results


def guard_upstream(target, model, effort, counter, pair_counter, arm_limit, expected_capture=None):
    import httpx

    send = httpx.AsyncClient.send
    remaining = arm_limit

    def reject(request, reason):
        with Path(counter).with_name("guard-rejections.txt").open("a", encoding="ascii") as stream:
            stream.write(reason + "\n")
        return httpx.Response(400, request=request, json={"error": {"message": "Pilot request outside scope"}})

    async def bounded(client, request, **options):
        nonlocal remaining
        if Path(pair_counter).stat().st_size >= ATTEMPTS_PER_PAIR:
            return reject(request, "pair_request_limit")
        if remaining == 0:
            return reject(request, "arm_request_limit")
        body = None
        if (request.method == "POST" and str(request.url) == target
                and 0 < len(request.content) <= MAX_REQUEST_BYTES):
            try:
                body = json.loads(request.content)
            except (ValueError, RecursionError):
                pass
        reasoning = body.get("reasoning") if isinstance(body, dict) else None
        if (not isinstance(body, dict) or body.get("model") != model
                or not isinstance(reasoning, dict) or reasoning.get("effort") != effort or type(body.get("stream")) is not bool):
            return reject(request, "request_outside_scope")
        remaining -= 1
        with Path(pair_counter).open("ab") as stream:
            stream.write(b"1")
        with Path(counter).open("ab") as stream:
            stream.write(b"1\n")
        if expected_capture is not None:
            entry = {"outputs": forwarded_tool_evidence(body, expected_capture)}
            with Path(counter).with_name("tool-delivery.jsonl").open("a", encoding="utf-8") as stream:
                stream.write(json.dumps(entry) + "\n")
        response = await send(client, request, **{**options, "follow_redirects": False})
        if response.is_error:
            read = response.aread

            async def observe_error():
                content = await read()
                result = upstream_error_summary(response.status_code, content)
                with Path(counter).with_name("upstream-errors.jsonl").open("a", encoding="utf-8") as stream:
                    stream.write(json.dumps(result) + "\n")
                return content

            response.aread = observe_error
        return response

    httpx.AsyncClient.send = bounded


def stage_arm(replay, directory, payload, target, model, effort, arm_limit, expected_capture=None):
    staged = replay.prepare_fixture_runtime(directory, payload)
    helper = directory / "fixture-runtime/desktop_helper.py"
    source = helper.read_bytes()
    anchor = b'"from headroom.cli import main; main()"'
    assert source.count(anchor) == 1, "engine_bootstrap_changed"
    counter = directory / "http-attempts.txt"
    guard_args = (target, model, effort, str(counter), str(directory.parent / "pair-attempts.txt"), arm_limit, expected_capture)
    guard = f"import runpy; runpy.run_path({str(PROGRAM)!r})['guard_upstream']{guard_args!r}; "
    helper.write_bytes(source.replace(anchor, repr(guard).encode() + b" " + anchor, 1))
    return staged, counter


def tool_reported_error(item):
    result = item.get("result")
    return item.get("error") is not None or isinstance(result, dict) and (
        result.get("isError") is True or result.get("is_error") is True)


def check_turn(exit_code, stdout, expected, *, expected_capture=None):
    events = [json.loads(line) for line in stdout.splitlines() if line.startswith(b"{")]
    items = [event["item"] for event in events if event.get("type") == "item.completed"]
    messages = [item["text"] for item in items if item.get("type") == "agent_message"]
    calls = [item for item in items if item.get("type") == "mcp_tool_call"]
    tools = [item.get("tool") for item in calls]
    try:
        answer = json.loads(messages[-1]) if messages else None
    except (TypeError, ValueError, RecursionError):
        answer = None
    exact_fields = {key: isinstance(answer, dict) and key in answer and type(answer[key]) is type(value) and answer[key] == value
                    for key, value in expected.items()}
    exact_keys = isinstance(answer, dict) and set(answer) == set(expected)
    tool_checks = {
        "call_count": len(calls),
        "sequence_valid": tools[:1] == ["inspect_result"] and all(tool == RECOVERY_TOOL for tool in tools[1:]),
        "owned_server": all(item.get("server") == "headroom_rule_pilot" for item in calls),
        "all_completed": all(item.get("status") == "completed" for item in calls),
        "no_reported_errors": not any(tool_reported_error(item) for item in calls),
    }
    # JSON equality alone would accept 48.0 as an exact integer result.
    result = {
        "turn_completed": exit_code == 0 and sum(event.get("type") == "turn.completed" for event in events) == 1
            and not any(event.get("type") in ("error", "turn.failed") for event in events),
        "exact_answer": exact_keys and all(exact_fields.values()),
        "fixed_tool_call": all(tool_checks[key] for key in ("sequence_valid", "owned_server", "all_completed", "no_reported_errors")),
        "tool_checks": tool_checks,
        "recovery_tool_calls": tools.count(RECOVERY_TOOL),
    }
    if not result["exact_answer"]:
        fields = answer if isinstance(answer, dict) else {}
        result["answer_checks"] = {"message_count": len(messages), "json_object": isinstance(answer, dict),
                                   "expected_keys_only": exact_keys, "exact_fields": exact_fields,
                                   "type_matches": {key: key in fields and type(fields[key]) is type(value)
                                                    for key, value in expected.items()},
                                   "integer_string_matches": {key: fields.get(key) == str(value)
                                                              for key, value in expected.items() if type(value) is int}}
    if expected_capture is not None:
        delivery = []
        for item in calls:
            if item.get("server") != "headroom_rule_pilot" or item.get("tool") not in ("inspect_result", RECOVERY_TOOL):
                continue
            value = item.get("result")
            content = value.get("content", []) if isinstance(value, dict) else []
            if not isinstance(content, list):
                content = []
            texts = [part["text"] for part in content if isinstance(part, dict)
                     and part.get("type") == "text" and isinstance(part.get("text"), str)]
            delivery.append({**capture_evidence(item["tool"], texts, expected_capture),
                             "error_reported": tool_reported_error(item)})
        result["producer_delivery"] = delivery
    return result


def measure(engine, usage, sent):
    deadline = time.monotonic() + 3
    while True:
        ids = set(usage["probe"]["request_logs"](engine))
        rows = usage["records"](engine)
        matched = bool(ids) and ids == rows.keys() and sum(len(row["attempts"]) for row in rows.values()) == sent
        if matched or time.monotonic() >= deadline:
            break
        time.sleep(0.1)
    fields = ("total_input_tokens", "output_tokens", "total_tokens", "cache_read_input_tokens", "cache_creation_input_tokens")
    result = {"request_count": len(ids), "usage_record_count": len(rows), "total_usage": None,
              "first_upstream_cache_read_input_tokens": first_cache_read(rows) if matched else None}
    if matched:
        result.update(request_ids=sorted(ids), attempts_per_request=[len(rows[key]["attempts"]) for key in sorted(ids)])
    if matched and all(row["surface"] == "openai_responses" and row["client_status"] == 200 and not row["truncated"]
                       and row["usage"]["total_tokens"] is not None for row in rows.values()):
        totals = {}
        for key in fields:
            counts = [row["usage"][key] for row in rows.values()]
            totals[key] = sum(counts) if all(value is not None for value in counts) else None
        result["total_usage"] = totals
    else:
        result["observed_usage_fields"] = [
            {"request_id": key, "client_status": row["client_status"], "truncated": row["truncated"],
             "attempt_http_statuses": [attempt["status"] for attempt in row["attempts"]],
             "available": {field: row["usage"].get(field) is not None for field in fields}}
            for key, row in sorted(rows.items())
        ]
    return result


def first_cache_read(rows):
    identities = [(re.fullmatch(r"hr_[0-9]{1,20}_([0-9]{6,20})", key), key) for key in rows]
    if not identities or any(match is None for match, _ in identities):
        return None
    _, key = min(identities, key=lambda item: int(item[0][1]))
    row = rows[key]
    attempts = row.get("attempts", [])
    if row.get("truncated") or not attempts or attempts[0].get("status") != 200:
        return None
    value = (attempts[0].get("usage") or {}).get("cache_read_input_tokens")
    return value if type(value) is int and value >= 0 else None


def read_rule_cpu(path):
    assert path.stat().st_size <= 16 * 1024, "rule_resource_report_limit"
    records = [json.loads(line) for line in path.read_text(encoding="ascii").splitlines()]
    assert records and all(set(item) == {"rule_prepare_cpu_ms"} and type(item["rule_prepare_cpu_ms"]) in (int, float)
                           and math.isfinite(item["rule_prepare_cpu_ms"]) and item["rule_prepare_cpu_ms"] >= 0
                           for item in records), "rule_resource_report_invalid"
    total = sum(item["rule_prepare_cpu_ms"] for item in records)
    assert math.isfinite(total), "rule_resource_report_invalid"
    return total


def heldout_observations(rows, requested):
    cached = [row.get("first_upstream_cache_read_input_tokens") for row in rows]
    observed = "unconfirmed"
    if len(cached) == 2 and all(type(value) is int and value >= 0 for value in cached):
        if all(value == 0 for value in cached):
            observed = "cold"
        elif all(value > 0 for value in cached):
            observed = "warm"
    fields = {"total_input_tokens", "output_tokens", "total_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"}
    counters_known = len(rows) == 2 and all(isinstance(row.get("total_usage"), dict) and set(row["total_usage"]) == fields
        and all(type(value) is int and value >= 0 for value in row["total_usage"].values()) for row in rows)
    resources_known = len(rows) == 2 and all(row.get("resource_error") is None and row.get("resources") is not None
        and type(row["resources"].get("resource_samples")) is int and row["resources"]["resource_samples"] >= 2
        and all(type(row["resources"].get(key)) in (int, float) and math.isfinite(row["resources"][key])
                and row["resources"][key] >= 0 for key in ("local_cpu_ms", "observed_engine_private_bytes")) for row in rows)
    metrics = None
    if resources_known and {row["arm"] for row in rows} == {"control", "candidate"}:
        by_arm = {row["arm"]: row for row in rows}
        metrics = {"added_completed_task_elapsed_ms": by_arm["candidate"]["elapsed_ms"] - by_arm["control"]["elapsed_ms"],
                   "candidate_completed_task_cpu_ms": by_arm["candidate"]["resources"]["local_cpu_ms"],
                   "engine_observed_private_bytes": max(row["resources"]["observed_engine_private_bytes"] for row in rows)}
    return {"cache_start_requested": requested, "cache_start_observed": observed, "cache_start_matches": observed == requested,
            "all_provider_counters_known": counters_known, "resource_data_complete": resources_known,
            "pair_metrics": metrics, "matrix_acceptance": "not_tested"}


def arm_budget(sent, index):
    assert type(sent) is int and 0 <= sent <= ATTEMPTS_PER_PAIR and index in (0, 1), "invalid_pair_budget"
    available = ATTEMPTS_PER_PAIR - sent - (MIN_ATTEMPTS_PER_ARM if index == 0 else 0)
    assert available >= MIN_ATTEMPTS_PER_ARM, "paired_request_budget_exhausted"
    return available


def run_arm(pilot, cli, supervisor, engine, directory, model, effort, fixture, catalog, counter, usage, auth_home, arm_limit, *, case=CASE):
    details = pilot["HELDOUT_CASES"].get(case)
    schema = details["answer_schema"] if details else ANSWER_SCHEMA
    transient = directory / "client"
    work, temp = directory.parent / "client-workspace", transient / "temp"
    temp.mkdir(parents=True)
    env = pilot["client_environment"]()
    env.update(TEMP=str(temp), TMP=str(temp), CODEX_SQLITE_HOME=str(transient / "state"))
    config = {"model_provider": "pilot", "model_providers.pilot.name": "Headroom Rules Pilot",
              "model_providers.pilot.base_url": f"http://127.0.0.1:{engine.port}/v1",
              "model_providers.pilot.wire_api": "responses", "model_providers.pilot.supports_websockets": False,
              "model_providers.pilot.requires_openai_auth": fixture is None, "model_reasoning_effort": effort,
              "model_catalog_json": str(catalog), "log_dir": str(transient / "logs"), "project_doc_max_bytes": 0,
              "features.memories": False, "features.multi_agent": False, "features.multi_agent_v2": False}
    if fixture is not None:
        home = transient / "fixture-home"
        home.mkdir()
        env.update(CODEX_HOME=str(home), RULES_PILOT_TOKEN=fixture.auth.removeprefix("Bearer "))
        config["model_providers.pilot.env_key"] = "RULES_PILOT_TOKEN"
        fixture.case, fixture.requests, fixture.failure = case, [], None
        fixture.recovery_arm = directory.name
    else:
        env["CODEX_HOME"] = str(auth_home)
        config["cli_auth_credentials_store"] = "file"
    prompt = ("Call the fixed inspect_result tool once. If it returns a Headroom reference, pass its hash to headroom_retrieve. "
              "Return only a JSON object. "
              + pilot["case_data"](case)[3])
    rule_report = directory / "rule-resources.jsonl" if details else None
    command = pilot["client_args"](cli, work, case, directory.name, model, config, prompt=prompt, retrieval_port=engine.port,
                                   output_schema=directory.parent / "answer-schema.json", resource_report=rule_report)
    sampler = usage["replay"].Resources([os.getpid(), *engine.pids], pilot["HELDOUT_PLAN"]["acceptance"]["resource_sample_interval_ms"]) if details else None
    resources, resource_error, run_failure = None, None, None
    start = time.monotonic()
    try:
        exit_code, stdout = pilot["run_client"](command, work, env, supervisor, ARM_TIMEOUT)
    except subprocess.TimeoutExpired:
        exit_code, stdout, run_failure = -1, b"", "arm_timeout"
    finally:
        elapsed = time.monotonic() - start
        if sampler is not None:
            try:
                resources = sampler.finish()
            except Exception:
                resource_error = "resource_sampling_failed"
        assert transient.resolve(strict=True) == transient and transient.parent == directory, "temporary_client_scope"
        shutil.rmtree(transient)
    sent = counter.read_bytes().count(b"\n") if counter.exists() else 0
    assert sent <= arm_limit, "request_budget_exceeded"
    row = {"arm": directory.name, "forwarded_http_attempts": sent, "elapsed_ms": round(elapsed * 1000),
           "attempt_limit": arm_limit,
           **check_turn(exit_code, stdout, pilot["case_data"](case)[4], expected_capture=pilot["envelope"](case, directory.name)),
           **measure(engine, usage, sent)}
    if run_failure:
        row["run_failure"] = run_failure
    if details:
        if resources is not None:
            try:
                resources["rule_prepare_cpu_ms"] = read_rule_cpu(rule_report)
                resources["local_cpu_ms"] = resources["harness_cpu_ms"] + resources["engine_cpu_ms"] + resources["rule_prepare_cpu_ms"]
            except Exception:
                resource_error = "rule_prepare_cpu_unavailable"
        row.update(resources=resources, resource_error=resource_error)
    delivery = directory / "tool-delivery.jsonl"
    if delivery.is_file():
        row["forwarded_delivery"] = [json.loads(line) for line in delivery.read_text(encoding="utf-8").splitlines()]
        assert len(row["forwarded_delivery"]) == sent, "tool_delivery_request_scope_changed"
    failures = directory / "upstream-errors.jsonl"
    if failures.is_file():
        row["upstream_errors"] = [json.loads(line) for line in failures.read_text(encoding="utf-8").splitlines()]
    denied = directory / "guard-rejections.txt"
    if denied.is_file():
        row["guard_rejections"] = denied.read_text(encoding="ascii").splitlines()
    if fixture is not None and row["turn_completed"]:
        assert fixture.failure is None and len(fixture.requests) == sent and sent in (2, 3), fixture.failure or "fixture_wire_scope_changed"
        assert all(body.get("text", {}).get("format", {}).get("schema") == schema
                   and body["text"]["format"].get("type") == "json_schema" for body in fixture.requests), "answer_schema_wire_mismatch"
        row["provider_answer_schema_exact"] = True
        assert all("# Headroom Desktop çalışma kuralları" not in json.dumps(body, ensure_ascii=False)
                   for body in fixture.requests), "repository_instructions_leaked"
        matches = pilot["inspect_tool_output"](engine, fixture.requests[1], case, directory.name, usage)
        assert matches["exact_native_ccr_readback"] == (row["recovery_tool_calls"] == 1) == (sent == 3), "fixture_recovery_mismatch"
        row.update(matches)
    return row


def account_catalog_allows(catalog_bytes, model, effort):
    return any(row["slug"] == model and row.get("visibility") == "list"
               and any(level["effort"] == effort for level in row["supported_reasoning_levels"])
               for row in json.loads(catalog_bytes)["models"])


def main():
    assert os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom")
    os.environ["LITELLM_LOCAL_MODEL_COST_MAP"] = "True"
    pilot = runpy.run_path(str(PROGRAM.with_name("rules_pilot.py")))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex", required=True, type=Path)
    parser.add_argument("--live", action="store_true", help="Use the existing ChatGPT login and its quota")
    parser.add_argument("--codex-home", type=Path, help="Existing Headroom test login profile")
    parser.add_argument("--model")
    parser.add_argument("--effort")
    parser.add_argument("--case", choices=[CASE, *pilot["HELDOUT_CASES"]], default=CASE)
    parser.add_argument("--repeat-index", type=int, choices=range(3))
    parser.add_argument("--cache-start", choices=("cold", "warm"))
    parser.add_argument("--prior-attempts", type=int, choices=range(ATTEMPTS_PER_PAIR), default=0,
                        help="Previously forwarded requests against this paired pilot's limit")
    args = parser.parse_args()
    supplied = (args.model, args.effort, args.codex_home)
    if (args.live and not all(supplied)) or (not args.live and (any(supplied) or args.prior_attempts)):
        parser.error("Pass --model, --effort and --codex-home together with --live")
    heldout = args.case in pilot["HELDOUT_CASES"]
    if heldout and (args.repeat_index is None or args.cache_start is None) or not heldout and (args.repeat_index is not None or args.cache_start is not None):
        parser.error("Held-out cases require --repeat-index and --cache-start; the preliminary pilot does not use them")
    plan = pilot["HELDOUT_PLAN"] if heldout else None
    order = plan["pair_order_by_repetition"][args.repeat_index] if heldout else ["control", "candidate"]
    assert sorted(order) == ["candidate", "control"], "invalid_arm_order"
    schema = pilot["HELDOUT_CASES"][args.case]["answer_schema"] if heldout else ANSWER_SCHEMA
    usage_path = ROOT / "tests/runtime/anthropic_usage_capture.py"
    usage = runpy.run_path(str(usage_path))
    replay = usage["replay"]
    if heldout:
        assert plan["live_execution"]["max_requests_per_pair"] == plan["live_execution"]["max_requests_per_batch"] == ATTEMPTS_PER_PAIR
        assert plan["acceptance"]["arm_timeout_seconds"] == ARM_TIMEOUT
        for filename, digest in plan["rule_sources_frozen"].items():
            path = (ROOT / filename).resolve(strict=True)
            assert path.is_relative_to(ROOT) and replay.sha(path) == digest, "frozen_rule_changed"
    runtime = ROOT / "runtime"
    controller = replay.load_module("rules_live_controller", runtime / "desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(runtime / "manifest.json")
    assert replay.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"]
    runner = payload / manifest["engine"]["runner"]
    supervisor = replay.load_module("rules_live_supervisor", runner)
    cli = args.codex.resolve(strict=True)
    assert cli.name == "codex.exe" and cli.is_relative_to((Path(os.environ["LOCALAPPDATA"]) / "OpenAI/Codex/bin").resolve())
    version = subprocess.run([str(cli), "--version"], capture_output=True, timeout=10, check=True).stdout.decode().strip()
    assert version == "codex-cli 0.169.0-alpha.11.1", "cli_fingerprint_changed"
    env = pilot["client_environment"]()
    catalog_command = [str(cli), "debug", "models", "--bundled"]
    auth_home = None
    if args.live:
        auth_home = args.codex_home.resolve(strict=True)
        owned_profiles = Path(os.environ["LOCALAPPDATA"]).resolve(strict=True) / "dev.headroom.desktop/desktop-live-proof"
        assert auth_home.parent == owned_profiles and auth_home.is_dir(), "owned_test_profile_required"
        env["CODEX_HOME"] = str(auth_home)
        authenticated_cli = [str(cli), "-c", 'cli_auth_credentials_store="file"']
        status = subprocess.run([*authenticated_cli, "login", "status"], capture_output=True, timeout=10, env=env)
        assert status.returncode == 0 and b"chatgpt" in (status.stdout + status.stderr).lower(), "existing_chatgpt_login_required"
        catalog_command = [*authenticated_cli, "debug", "models"]
    catalog_bytes = subprocess.run(catalog_command, capture_output=True, timeout=20, check=True, env=env).stdout
    if args.live and not account_catalog_allows(catalog_bytes, args.model, args.effort):
        parser.error("Selected model or effort is unavailable in the Headroom test account catalog")
    model, effort = (args.model, args.effort) if args.live else (pilot["MODEL"], "high")
    run = ROOT / "work/proofs/optimization/rules-live" / uuid.uuid4().hex
    assert run.resolve().is_relative_to(ROOT)
    run.mkdir(parents=True)
    pair_counter = run / "pair-attempts.txt"
    pair_counter.write_bytes(b"1" * args.prior_attempts)
    work = run / "client-workspace"
    work.mkdir()
    git_env = {key: value for key, value in pilot["client_environment"]().items() if not key.upper().startswith("GIT_")}
    subprocess.run(["git", "init", "--quiet", "--template="], cwd=work, env=git_env,
                   capture_output=True, check=True, timeout=10)
    catalog = run / "models.json"
    catalog.write_bytes(catalog_bytes)
    answer_schema = run / "answer-schema.json"
    replay.write_new(answer_schema, schema)
    fixture, upstream_port = None, 1
    if args.live:
        from headroom.providers.codex.responses import codex_responses_http_url

        target = codex_responses_http_url()
        assert target == "https://chatgpt.com/backend-api/codex/responses", "unexpected_chatgpt_origin"
    else:
        fixture = ThreadingHTTPServer(("127.0.0.1", 0), pilot["Provider"])
        fixture.auth = "Bearer " + replay.SENTINEL
        fixture.cache_start = args.cache_start
        upstream_port = fixture.server_port
        target = f"http://127.0.0.1:{upstream_port}/v1/responses"
        threading.Thread(target=fixture.serve_forever, daemon=True).start()
    rows, stops, failure = [], [], None
    fingerprints = {}
    try:
        for index, arm in enumerate(order):
            arm_limit = arm_budget(pair_counter.stat().st_size, index)
            directory = run / arm
            directory.mkdir()
            staged, counter = stage_arm(replay, directory, payload, target, model, effort, arm_limit,
                                        expected_capture=pilot["envelope"](args.case, arm))
            sources = json.loads((runtime / "optimization-sources.json").read_bytes())
            files = [PROGRAM, PROGRAM.with_name("rules_pilot.py"), pilot["HELDOUT_PATH"],
                     ROOT / "tests/runtime/test_rules_pilot_contract.py", ROOT / "tests/runtime/test_rules_heldout.py",
                     usage_path, cli, catalog, answer_schema, runner, work / ".git/config", Path(sys.executable),
                     payload / "python313.dll", payload / "runtime-manifest.json", runtime / "learn_preview.py",
                     *staged, *[runtime / path.name for path in staged],
                     *[PROGRAM.with_name(name) for name in ("replay.py", "anthropic_usage.py", "ccr_policy.py", "surfaces.py", "fixtures.py")],
                     *[ROOT / "bench/context_tools" / name for name in ("targeted_query.py", "targeted_query_replay.py")],
                     *[payload / "Lib/site-packages/headroom" / name for name in sources],
                     payload / "Lib/site-packages/headroom/providers/codex/endpoints.py",
                     payload / "Lib/site-packages/headroom/transforms/lossless_compaction.py",
                     payload / "Lib/site-packages/mcp-2.3.0.dist-info/RECORD"]
            if auth_home is not None:
                files.append(auth_home / "config.toml")
            before = {str(path): replay.sha(path) for path in files}
            engine = usage["probe"]["UsageEngine"](controller, supervisor, payload, directory / "engine", upstream_port, pilot["PLAN"])
            engine.environment["HEADROOM_DESKTOP_PROTECTED_TOOLS"] = json.dumps(sorted([
                pilot["RECOVERY_TOOL"], pilot["NAMESPACE"] + "__" + pilot["RECOVERY_TOOL"]]))
            try:
                engine.start()
                assert pilot["usage_ids"](engine, usage) == (set(), set()), "engine_not_empty"
                case_options = {"case": args.case} if heldout else {}
                row = run_arm(pilot, cli, supervisor, engine, directory, model, effort, fixture, catalog, counter, usage, auth_home, arm_limit, **case_options)
                rows.append(row)
                assert row["turn_completed"], "arm_turn_incomplete"
                if heldout:
                    cached = row["first_upstream_cache_read_input_tokens"]
                    assert type(cached) is int and (cached == 0 if args.cache_start == "cold" else cached > 0), "cache_start_unconfirmed"
                    assert row["resource_error"] is None and row["resources"] is not None, "resource_data_unavailable"
            finally:
                engine.close()
                stops.append(engine.stopped)
                after = {str(path): replay.sha(path) for path in files}
                fingerprints[arm] = {"before": before, "after": after}
            assert before == after, "inputs_changed"
    except Exception as error:
        code = str(error)
        failure = code if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", code) else type(error).__name__
    finally:
        if fixture is not None:
            fixture.shutdown()
            fixture.server_close()
    report = run / "result.json"
    observations = heldout_observations(rows, args.cache_start) if heldout else None
    if failure is None and any(not row["exact_answer"] or not row["fixed_tool_call"] or row["total_usage"] is None for row in rows):
        failure = "arm_acceptance_failed"
    if failure is None and heldout and not all(observations[key] for key in ("cache_start_matches", "all_provider_counters_known", "resource_data_complete")):
        failure = "heldout_measurement_incomplete"
    passed = failure is None and len(rows) == 2
    difference = None
    if passed and args.live:
        totals = {row["arm"]: row["total_usage"]["total_tokens"] for row in rows}
        control, candidate = totals["control"], totals["candidate"]
        saved = control - candidate
        difference = {"total_tokens_saved": saved, "percent_of_control": round(100 * saved / control, 4) if control else None}
    replay.write_new(report, {"passed": passed, "failure": failure, "provider": "chatgpt" if args.live else "loopback_fixture",
                              "model": model, "effort": effort, "case": args.case, "cli": version, "platform": platform.platform(),
                              "arm_order": order, "repeat_index": args.repeat_index, "heldout_observations": observations,
                              "general_acceptance": "not_tested",
                              "pair_attempt_limit": ATTEMPTS_PER_PAIR, "arm_timeout_seconds": ARM_TIMEOUT,
                              "prior_attempts": args.prior_attempts, "pair_attempts_total": pair_counter.stat().st_size,
                              "rows": rows, "provider_delta": difference, "stops": stops, "fingerprints": fingerprints})
    print(pilot["encode"]({"passed": passed, "failure": failure, "rows": rows, "provider_delta": difference, "report": str(report)}))
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())

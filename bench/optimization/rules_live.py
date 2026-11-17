"""One bounded Codex rule comparison. Default mode uses only a loopback provider."""
import argparse
from http.server import ThreadingHTTPServer
import json
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


def guard_upstream(target, model, effort, counter, pair_counter, arm_limit):
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


def stage_arm(replay, directory, payload, target, model, effort, arm_limit):
    staged = replay.prepare_fixture_runtime(directory, payload)
    helper = directory / "fixture-runtime/desktop_helper.py"
    source = helper.read_bytes()
    anchor = b'"from headroom.cli import main; main()"'
    assert source.count(anchor) == 1, "engine_bootstrap_changed"
    counter = directory / "http-attempts.txt"
    guard_args = (target, model, effort, str(counter), str(directory.parent / "pair-attempts.txt"), arm_limit)
    guard = f"import runpy; runpy.run_path({str(PROGRAM)!r})['guard_upstream']{guard_args!r}; "
    helper.write_bytes(source.replace(anchor, repr(guard).encode() + b" " + anchor, 1))
    return staged, counter


def check_turn(exit_code, stdout, expected):
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
    # JSON equality alone would accept 48.0 as an exact integer result.
    result = {
        "turn_completed": exit_code == 0 and sum(event.get("type") == "turn.completed" for event in events) == 1
            and not any(event.get("type") in ("error", "turn.failed") for event in events),
        "exact_answer": exact_keys and all(exact_fields.values()),
        "fixed_tool_call": tools in (["inspect_result"], ["inspect_result", "recover_result"])
            and all(item.get("server") == "headroom_rule_pilot" and item.get("status") == "completed" for item in calls),
        "recovery_tool_calls": tools.count("recover_result"),
    }
    if not result["exact_answer"]:
        fields = answer if isinstance(answer, dict) else {}
        result["answer_checks"] = {"message_count": len(messages), "json_object": isinstance(answer, dict),
                                   "expected_keys_only": exact_keys, "exact_fields": exact_fields,
                                   "type_matches": {key: key in fields and type(fields[key]) is type(value)
                                                    for key, value in expected.items()},
                                   "integer_string_matches": {key: fields.get(key) == str(value)
                                                              for key, value in expected.items() if type(value) is int}}
    return result


def measure(engine, usage, sent):
    deadline = time.monotonic() + 3
    while True:
        ids = set(usage["probe"]["request_logs"](engine))
        rows = usage["records"](engine)
        matched = len(ids) >= 2 and ids == rows.keys() and sum(len(row["attempts"]) for row in rows.values()) == sent
        if matched or time.monotonic() >= deadline:
            break
        time.sleep(0.1)
    fields = ("total_input_tokens", "output_tokens", "total_tokens", "cache_read_input_tokens", "cache_creation_input_tokens")
    result = {"request_count": len(ids), "usage_record_count": len(rows), "total_usage": None}
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


def run_arm(pilot, cli, supervisor, engine, directory, model, effort, fixture, catalog, counter, usage, auth_home, arm_limit):
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
        fixture.case, fixture.requests, fixture.failure = CASE, [], None
        fixture.recovery_arm = directory.name
    else:
        env["CODEX_HOME"] = str(auth_home)
        config["cli_auth_credentials_store"] = "file"
    prompt = ("Call the fixed inspect_result tool once. If it returns a Headroom reference, pass its hash to recover_result. "
              "A repeat marker gives the total occurrences of the preceding line in that run. Return only a JSON object. "
              + pilot["CASES"][CASE][3])
    command = pilot["client_args"](cli, work, CASE, directory.name, model, config, prompt=prompt, retrieval_port=engine.port,
                                   output_schema=directory.parent / "answer-schema.json")
    try:
        start = time.monotonic()
        exit_code, stdout = pilot["run_client"](command, work, env, supervisor, ARM_TIMEOUT)
        elapsed = time.monotonic() - start
    finally:
        assert transient.resolve(strict=True) == transient and transient.parent == directory, "temporary_client_scope"
        shutil.rmtree(transient)
    sent = counter.read_bytes().count(b"\n") if counter.exists() else 0
    assert sent <= arm_limit, "request_budget_exceeded"
    row = {"arm": directory.name, "forwarded_http_attempts": sent, "elapsed_ms": round(elapsed * 1000),
           "attempt_limit": arm_limit, **check_turn(exit_code, stdout, pilot["CASES"][CASE][4]), **measure(engine, usage, sent)}
    failures = directory / "upstream-errors.jsonl"
    if failures.is_file():
        row["upstream_errors"] = [json.loads(line) for line in failures.read_text(encoding="utf-8").splitlines()]
    denied = directory / "guard-rejections.txt"
    if denied.is_file():
        row["guard_rejections"] = denied.read_text(encoding="ascii").splitlines()
    if fixture is not None:
        assert fixture.failure is None and len(fixture.requests) == sent and sent in (2, 3), fixture.failure or "fixture_wire_scope_changed"
        assert all(body.get("text", {}).get("format", {}).get("schema") == ANSWER_SCHEMA
                   and body["text"]["format"].get("type") == "json_schema" for body in fixture.requests), "answer_schema_wire_mismatch"
        row["provider_answer_schema_exact"] = True
        assert all("# Headroom Desktop çalışma kuralları" not in json.dumps(body, ensure_ascii=False)
                   for body in fixture.requests), "repository_instructions_leaked"
        matches = pilot["inspect_tool_output"](engine, fixture.requests[1], CASE, directory.name, usage)
        assert matches["exact_native_ccr_readback"] == (row["recovery_tool_calls"] == 1) == (sent == 3), "fixture_recovery_mismatch"
        row.update(matches)
    return row


def account_catalog_allows(catalog_bytes, model, effort):
    return any(row["slug"] == model and row.get("visibility") == "list"
               and any(level["effort"] == effort for level in row["supported_reasoning_levels"])
               for row in json.loads(catalog_bytes)["models"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex", required=True, type=Path)
    parser.add_argument("--live", action="store_true", help="Use the existing ChatGPT login and its quota")
    parser.add_argument("--codex-home", type=Path, help="Existing Headroom test login profile")
    parser.add_argument("--model")
    parser.add_argument("--effort")
    parser.add_argument("--prior-attempts", type=int, choices=range(ATTEMPTS_PER_PAIR), default=0,
                        help="Previously forwarded requests against this paired pilot's limit")
    args = parser.parse_args()
    supplied = (args.model, args.effort, args.codex_home)
    if (args.live and not all(supplied)) or (not args.live and (any(supplied) or args.prior_attempts)):
        parser.error("Pass --model, --effort and --codex-home together with --live")
    assert os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom")
    os.environ["LITELLM_LOCAL_MODEL_COST_MAP"] = "True"
    pilot = runpy.run_path(str(PROGRAM.with_name("rules_pilot.py")))
    usage_path = ROOT / "tests/runtime/anthropic_usage_capture.py"
    usage = runpy.run_path(str(usage_path))
    replay = usage["replay"]
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
    replay.write_new(answer_schema, ANSWER_SCHEMA)
    fixture, upstream_port = None, 1
    if args.live:
        from headroom.providers.codex.responses import codex_responses_http_url

        target = codex_responses_http_url()
        assert target == "https://chatgpt.com/backend-api/codex/responses", "unexpected_chatgpt_origin"
    else:
        fixture = ThreadingHTTPServer(("127.0.0.1", 0), pilot["Provider"])
        fixture.auth = "Bearer " + replay.SENTINEL
        upstream_port = fixture.server_port
        target = f"http://127.0.0.1:{upstream_port}/v1/responses"
        threading.Thread(target=fixture.serve_forever, daemon=True).start()
    rows, stops, failure = [], [], None
    fingerprints = {}
    try:
        for arm in ("control", "candidate"):
            reserved = MIN_ATTEMPTS_PER_ARM if arm == "control" else 0
            arm_limit = ATTEMPTS_PER_PAIR - pair_counter.stat().st_size - reserved
            assert arm_limit >= MIN_ATTEMPTS_PER_ARM, "paired_request_budget_exhausted"
            directory = run / arm
            directory.mkdir()
            staged, counter = stage_arm(replay, directory, payload, target, model, effort, arm_limit)
            sources = json.loads((runtime / "optimization-sources.json").read_bytes())
            files = [PROGRAM, PROGRAM.with_name("rules_pilot.py"), usage_path, cli, catalog, answer_schema, runner, work / ".git/config", Path(sys.executable),
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
                row = run_arm(pilot, cli, supervisor, engine, directory, model, effort, fixture, catalog, counter, usage, auth_home, arm_limit)
                rows.append(row)
                assert row["turn_completed"], "arm_turn_incomplete"
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
    if failure is None and any(not row["exact_answer"] or not row["fixed_tool_call"] or row["total_usage"] is None for row in rows):
        failure = "arm_acceptance_failed"
    passed = failure is None and len(rows) == 2
    difference = None
    if passed and args.live:
        control, candidate = (row["total_usage"]["total_tokens"] for row in rows)
        saved = control - candidate
        difference = {"total_tokens_saved": saved, "percent_of_control": round(100 * saved / control, 4) if control else None}
    replay.write_new(report, {"passed": passed, "failure": failure, "provider": "chatgpt" if args.live else "loopback_fixture",
                              "model": model, "effort": effort, "case": CASE, "cli": version, "platform": platform.platform(),
                              "pair_attempt_limit": ATTEMPTS_PER_PAIR, "arm_timeout_seconds": ARM_TIMEOUT,
                              "prior_attempts": args.prior_attempts, "pair_attempts_total": pair_counter.stat().st_size,
                              "rows": rows, "provider_delta": difference, "stops": stops, "fingerprints": fingerprints})
    print(pilot["encode"]({"passed": passed, "failure": failure, "rows": rows, "provider_delta": difference, "report": str(report)}))
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())

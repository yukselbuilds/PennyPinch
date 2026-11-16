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
ATTEMPTS_PER_ARM = 4
ARM_TIMEOUT = 120
MAX_REQUEST_BYTES = 256 * 1024


def guard_upstream(target, model, effort, counter):
    import httpx

    send = httpx.AsyncClient.send
    remaining = ATTEMPTS_PER_ARM

    async def bounded(client, request, **options):
        nonlocal remaining
        body = None
        if (remaining and request.method == "POST" and str(request.url) == target
                and 0 < len(request.content) <= MAX_REQUEST_BYTES):
            try:
                body = json.loads(request.content)
            except (ValueError, RecursionError):
                pass
        reasoning = body.get("reasoning") if isinstance(body, dict) else None
        if (not isinstance(body, dict) or body.get("model") != model
                or not isinstance(reasoning, dict) or reasoning.get("effort") != effort or type(body.get("stream")) is not bool):
            return httpx.Response(400, request=request, json={"error": {"message": "Pilot request outside scope"}})
        remaining -= 1
        with Path(counter).open("ab") as stream:
            stream.write(b"1\n")
        return await send(client, request, **{**options, "follow_redirects": False})

    httpx.AsyncClient.send = bounded


def stage_arm(replay, directory, payload, target, model, effort):
    staged = replay.prepare_fixture_runtime(directory, payload)
    helper = directory / "fixture-runtime/desktop_helper.py"
    source = helper.read_bytes()
    anchor = b'"from headroom.cli import main; main()"'
    assert source.count(anchor) == 1, "engine_bootstrap_changed"
    counter = directory / "http-attempts.txt"
    guard = f"import runpy; runpy.run_path({str(PROGRAM)!r})['guard_upstream']({target!r}, {model!r}, {effort!r}, {str(counter)!r}); "
    helper.write_bytes(source.replace(anchor, repr(guard).encode() + b" " + anchor, 1))
    return staged, counter


def check_turn(exit_code, stdout, expected):
    events = [json.loads(line) for line in stdout.splitlines() if line.startswith(b"{")]
    items = [event["item"] for event in events if event.get("type") == "item.completed"]
    messages = [item["text"] for item in items if item.get("type") == "agent_message"]
    calls = [item for item in items if item.get("type") == "mcp_tool_call"]
    tools = [item.get("tool") for item in calls]
    try:
        answer = json.loads(messages[0]) if len(messages) == 1 else None
    except (ValueError, RecursionError):
        answer = None
    # JSON equality alone would accept 48.0 as an exact integer result.
    return {
        "turn_completed": exit_code == 0 and sum(event.get("type") == "turn.completed" for event in events) == 1
            and not any(event.get("type") in ("error", "turn.failed") for event in events),
        "exact_answer": isinstance(answer, dict) and answer == expected
            and all(type(answer[key]) is type(value) for key, value in expected.items()),
        "fixed_tool_call": tools in (["inspect_result"], ["inspect_result", "recover_result"])
            and all(item.get("server") == "headroom_rule_pilot" and item.get("status") == "completed" for item in calls),
        "recovery_tool_calls": tools.count("recover_result"),
    }


def measure(engine, usage, sent):
    deadline = time.monotonic() + 3
    while True:
        ids = set(usage["probe"]["request_logs"](engine))
        rows = usage["records"](engine)
        if len(ids) >= 2 and ids == rows.keys() and sum(len(row["attempts"]) for row in rows.values()) == sent:
            break
        if time.monotonic() >= deadline:
            return {"request_count": len(ids), "usage_record_count": len(rows), "total_usage": None}
        time.sleep(0.1)
    complete = all(row["surface"] == "openai_responses" and row["client_status"] == 200 and not row["truncated"]
                   and row["usage"]["total_tokens"] is not None for row in rows.values())
    totals = None
    if complete:
        totals = {}
        for key in ("total_input_tokens", "output_tokens", "total_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"):
            counts = [row["usage"][key] for row in rows.values()]
            totals[key] = sum(counts) if all(value is not None for value in counts) else None
    return {"request_count": len(ids), "usage_record_count": len(rows), "total_usage": totals,
            "request_ids": sorted(ids), "attempts_per_request": [len(rows[key]["attempts"]) for key in sorted(ids)]}


def run_arm(pilot, cli, supervisor, engine, directory, model, effort, fixture, catalog, counter, usage, auth_home):
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
    command = pilot["client_args"](cli, work, CASE, directory.name, model, config, prompt=prompt, retrieval_port=engine.port)
    try:
        start = time.monotonic()
        exit_code, stdout = pilot["run_client"](command, work, env, supervisor, ARM_TIMEOUT)
        elapsed = time.monotonic() - start
    finally:
        assert transient.resolve(strict=True) == transient and transient.parent == directory, "temporary_client_scope"
        shutil.rmtree(transient)
    sent = counter.read_bytes().count(b"\n") if counter.exists() else 0
    assert sent <= ATTEMPTS_PER_ARM, "request_budget_exceeded"
    row = {"arm": directory.name, "forwarded_http_attempts": sent, "elapsed_ms": round(elapsed * 1000),
           **check_turn(exit_code, stdout, pilot["CASES"][CASE][4]), **measure(engine, usage, sent)}
    if fixture is not None:
        assert fixture.failure is None and len(fixture.requests) == sent and sent in (2, 3), fixture.failure or "fixture_wire_scope_changed"
        assert all("# Headroom Desktop çalışma kuralları" not in json.dumps(body, ensure_ascii=False)
                   for body in fixture.requests), "repository_instructions_leaked"
        matches = pilot["inspect_tool_output"](engine, fixture.requests[1], CASE, directory.name, usage)
        assert matches["exact_native_ccr_readback"] == (row["recovery_tool_calls"] == 1) == (sent == 3), "fixture_recovery_mismatch"
        row.update(matches)
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex", required=True, type=Path)
    parser.add_argument("--live", action="store_true", help="Use the existing ChatGPT login and its quota")
    parser.add_argument("--codex-home", type=Path, help="Existing Headroom test login profile")
    parser.add_argument("--model")
    parser.add_argument("--effort")
    args = parser.parse_args()
    supplied = (args.model, args.effort, args.codex_home)
    if (args.live and not all(supplied)) or (not args.live and any(supplied)):
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
    catalog_bytes = subprocess.run([str(cli), "debug", "models", "--bundled"], capture_output=True, timeout=15, check=True).stdout
    auth_home = None
    if args.live:
        models = json.loads(catalog_bytes)["models"]
        assert any(row["slug"] == args.model and args.effort in {level["effort"] for level in row["supported_reasoning_levels"]}
                   for row in models), "model_or_effort_not_in_bundled_catalog"
        env = pilot["client_environment"]()
        auth_home = args.codex_home.resolve(strict=True)
        owned_profiles = Path(os.environ["LOCALAPPDATA"]).resolve(strict=True) / "dev.headroom.desktop/desktop-live-proof"
        assert auth_home.parent == owned_profiles and auth_home.is_dir(), "owned_test_profile_required"
        env["CODEX_HOME"] = str(auth_home)
        status = subprocess.run([str(cli), "-c", 'cli_auth_credentials_store="file"', "login", "status"],
                                capture_output=True, timeout=10, env=env)
        assert status.returncode == 0 and b"chatgpt" in (status.stdout + status.stderr).lower(), "existing_chatgpt_login_required"
    model, effort = (args.model, args.effort) if args.live else (pilot["MODEL"], "high")
    run = ROOT / "work/proofs/optimization/rules-live" / uuid.uuid4().hex
    assert run.resolve().is_relative_to(ROOT)
    run.mkdir(parents=True)
    work = run / "client-workspace"
    work.mkdir()
    git_env = {key: value for key, value in pilot["client_environment"]().items() if not key.upper().startswith("GIT_")}
    subprocess.run(["git", "init", "--quiet", "--template="], cwd=work, env=git_env,
                   capture_output=True, check=True, timeout=10)
    catalog = run / "bundled-models.json"
    catalog.write_bytes(catalog_bytes)
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
            directory = run / arm
            directory.mkdir()
            staged, counter = stage_arm(replay, directory, payload, target, model, effort)
            sources = json.loads((runtime / "optimization-sources.json").read_bytes())
            files = [PROGRAM, PROGRAM.with_name("rules_pilot.py"), usage_path, cli, catalog, runner, work / ".git/config", Path(sys.executable),
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
                row = run_arm(pilot, cli, supervisor, engine, directory, model, effort, fixture, catalog, counter, usage, auth_home)
                rows.append(row)
                assert all(row[key] for key in ("turn_completed", "exact_answer", "fixed_tool_call")) and row["total_usage"] is not None, "arm_acceptance_failed"
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
    passed = failure is None and len(rows) == 2
    difference = None
    if passed and args.live:
        control, candidate = (row["total_usage"]["total_tokens"] for row in rows)
        saved = control - candidate
        difference = {"total_tokens_saved": saved, "percent_of_control": round(100 * saved / control, 4) if control else None}
    replay.write_new(report, {"passed": passed, "failure": failure, "provider": "chatgpt" if args.live else "loopback_fixture",
                              "model": model, "effort": effort, "case": CASE, "cli": version, "platform": platform.platform(),
                              "attempt_limit_per_arm": ATTEMPTS_PER_ARM, "arm_timeout_seconds": ARM_TIMEOUT,
                              "rows": rows, "provider_delta": difference, "stops": stops, "fingerprints": fingerprints})
    print(pilot["encode"]({"passed": passed, "failure": failure, "rows": rows, "provider_delta": difference, "report": str(report)}))
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())

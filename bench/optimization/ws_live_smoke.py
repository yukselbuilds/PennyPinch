"""Verify one Responses WebSocket generation through the owned Codex/Headroom route."""
import argparse
from http.server import ThreadingHTTPServer
import json
import os
from pathlib import Path
import platform
import re
import runpy
import subprocess
import sys
import threading
import time
import uuid


PROGRAM = Path(__file__).resolve()
ROOT = PROGRAM.parents[2]
COMPARE = ROOT / "bench/context_tools/targeted_query_compare.py"
WS_CAPTURE = ROOT / "tests/runtime/ws_usage_capture.py"
CHATGPT_WS = "wss://chatgpt.com/backend-api/codex/responses"
MARKER = "HEADROOM_WS_OK"
FRAME_LIMIT = 256 * 1024


def request_kind(message, sent, model, effort):
    if not isinstance(message, (str, bytes)) or not 0 < len(message) <= FRAME_LIMIT:
        return None
    try:
        frame = json.loads(message)
    except (ValueError, RecursionError):
        return None
    if not isinstance(frame, dict) or frame.get("type") != "response.create" or frame.get("model") != model:
        return None
    reasoning = frame.get("reasoning")
    generate = frame.get("generate", True)
    if not isinstance(reasoning, dict) or reasoning.get("effort") != effort or type(generate) is not bool:
        return None
    if generate:
        return "generate" if "generate" not in sent else None
    return "warmup" if not sent else None


def install_guard(target, model, effort, trace, sentinel):
    import httpx
    import websockets
    from websockets.exceptions import InvalidStatus, InvalidURI

    sent = []
    attempted = False

    class NoRedirect(websockets.connect):
        def process_redirect(self, error):
            return error

    def record(event, **fields):
        with Path(trace).open("a", encoding="ascii") as file:
            file.write(json.dumps({"event": event, **fields}) + "\n")

    async def blocked_http(client, request, **options):
        if request.url.path.endswith("/responses"):
            record("blocked_http_responses")
        return httpx.Response(403, request=request, json={"error": {"message": "WebSocket smoke only"}})

    async def bounded_connect(uri, **options):
        nonlocal attempted
        headers = {key.lower(): value for key, value in (options.get("additional_headers") or {}).items()}
        bearer = headers.get("authorization", "")
        authorized = (bool(headers.get("chatgpt-account-id")) and bearer.startswith("Bearer ") and len(bearer) > 7
                      if target == CHATGPT_WS else bearer == "Bearer " + sentinel)
        if uri != target or attempted or not authorized:
            record("blocked_ws_connect", target_matched=uri == target, authorization_matched=authorized)
            raise InvalidURI("wss://blocked.invalid", "probe route, auth or connection limit")
        attempted = True
        record("ws_connect")
        try:
            socket = await NoRedirect(uri, **{**options, "proxy": None, "open_timeout": 15})
        except InvalidStatus as error:
            record("ws_http_status", status=error.response.status_code)
            raise
        except (OSError, TimeoutError):
            record("ws_transport_error")
            raise
        # Codex may prewarm the connection. Cap all subsequent response.create sends here.
        send = socket.send

        async def bounded_send(message, **options):
            kind = request_kind(message, sent, model, effort)
            if kind is None:
                record("blocked_ws_frame")
                raise ValueError("probe frame, preset or generation limit")
            sent.append(kind)
            record("ws_frame", kind=kind)
            return await send(message, **options)

        socket.send = bounded_send
        record("ws_opened")
        return socket

    httpx.AsyncClient.send = blocked_http
    websockets.connect = bounded_connect


def matched_usage(rows, cli):
    if not 1 <= len(rows) <= 2 or not isinstance(cli, dict):
        return None
    if any(row["surface"] != "openai_responses_ws" or row["truncated"] or row["failed"]
           or len(row["attempts"]) != 1 or row["attempts"][0].get("completed") is not True for row in rows):
        return None
    generated = [row["usage"] for row in rows if (row["usage"]["output_tokens"] or 0) > 0]
    if len(generated) != 1:
        return None
    usage = generated[0]
    pairs = (("total_input_tokens", "input_tokens"), ("output_tokens", "output_tokens"),
             ("cache_read_input_tokens", "cached_input_tokens"))
    for provider, client in pairs:
        count = usage.get(provider)
        if type(count) is not int or type(cli.get(client)) is not int or count < 0 or count != cli[client]:
            return None
    if (usage["cache_read_input_tokens"] <= usage["total_input_tokens"]
            and usage["total_tokens"] == usage["total_input_tokens"] + usage["output_tokens"]):
        return usage
    return None


def inspect_cli(code, stdout):
    entries = [json.loads(line) for line in stdout.splitlines() if line.startswith(b"{")]
    turns = [entry.get("usage") for entry in entries if entry.get("type") == "turn.completed"]
    messages = [entry["item"]["text"].strip() for entry in entries if entry.get("type") == "item.completed"
                and entry.get("item", {}).get("type") == "agent_message"]
    completed = code == 0 and len(turns) == 1 and not any(row.get("type") in ("error", "turn.failed") for row in entries)
    counters = ("input_tokens", "cached_input_tokens", "output_tokens")
    usage = {name: turns[0].get(name) for name in counters} if len(turns) == 1 and isinstance(turns[0], dict) else None
    return {"exit_code": code, "completed": completed, "answer_exact": messages == [MARKER], "usage": usage}


def start_fixture(ws):
    class Fixture(ws["Fixture"]):
        def emit(self, request, identity, counters):
            warming = request.get("generate", True) is False
            response = ws["http"]["response"](request, (0, 0, 0, 0) if warming else (2400, 2000, 0, 5),
                                               output=[] if warming else None)
            response["id"] = identity
            write = ws["surface"]["ws_write"]
            write(self.wfile, 1, {"type": "response.created", "response": {**response, "status": "in_progress", "usage": None}})
            if not warming:
                response["output"][0]["content"][0]["text"] = MARKER
                write(self.wfile, 1, {"type": "response.output_item.done", "output_index": 0, "item": response["output"][0]})
            write(self.wfile, 1, {"type": "response.completed", "response": response})

    server = ThreadingHTTPServer(("127.0.0.1", 0), Fixture)
    server.capture_lock, server.calls, server.handshakes = threading.Lock(), {}, {}
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def read_usage(engine, ws, seen):
    deadline = time.monotonic() + 3
    while True:
        logged = set(ws["probe"]["request_logs"](engine)) - seen
        records = {key: value for key, value in ws["capture"]["records"](engine).items() if key not in seen}
        if (logged and logged == records.keys()) or time.monotonic() >= deadline:
            return list(records.values()), bool(logged) and logged == records.keys()
        time.sleep(0.05)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex", required=True, type=Path)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--live", action="store_true")
    mode.add_argument("--local-catalog", type=Path)
    args = parser.parse_args()
    assert os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_workspace"
    compare = runpy.run_path(str(COMPARE))
    cli = args.codex.resolve(strict=True)
    app_bin = (Path(os.environ["LOCALAPPDATA"]) / "OpenAI/Codex/bin").resolve(strict=True)
    assert cli.name == "codex.exe" and cli.is_relative_to(app_bin), "cli_scope"
    version = subprocess.run([str(cli), "--version"], capture_output=True, timeout=10, check=True).stdout.decode().strip()
    assert version == compare["CLI_VERSION"], "cli_pin_changed"
    os.environ["LITELLM_LOCAL_MODEL_COST_MAP"] = "True"
    pilot = runpy.run_path(str(ROOT / "bench/optimization/rules_pilot.py"))
    rules = runpy.run_path(str(compare["LIVE_PATH"]))
    ws = runpy.run_path(str(WS_CAPTURE))
    replay = ws["replay"]
    runtime = ROOT / "runtime"
    controller = replay.load_module("live_ws_controller", runtime / "desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(runtime / "manifest.json")
    assert replay.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "python_pin_changed"
    runner = payload / manifest["engine"]["runner"]
    supervisor = replay.load_module("live_ws_supervisor", runner)
    run = compare["PROOF_ROOT"] / "websocket-smoke" / uuid.uuid4().hex
    assert run.resolve().is_relative_to(ROOT), "proof_scope"
    run.mkdir(parents=True)
    model, effort = compare["LIVE_MODEL"], compare["LIVE_EFFORT"]
    plan = {"cli": version, "engine": manifest["engine"]["version"], "os": platform.platform(),
            "mode": "chatgpt_free" if args.live else "loopback", "model": model, "effort": effort,
            "websocket_connection_limit": 1, "warmup_limit": 1, "generation_limit": 1, "http_upstream_limit": 0}
    replay.write_new(run / "registration.json", plan)
    before, observed, failure, matched, engine, fixture = {}, {}, None, None, None, None
    trace = run / "transport.jsonl"
    try:
        if args.live:
            home, catalog = compare["account_catalog"](cli, run, pilot, rules, supervisor)
            target, port = CHATGPT_WS, 1
        else:
            source = args.local_catalog.resolve(strict=True)
            assert source.name == "models.json" and source.parent.parent == compare["PROOF_ROOT"], "catalog_scope"
            assert json.loads(source.with_name("registration.json").read_bytes())["mode"] == "live", "catalog_origin"
            catalog = source.read_bytes()
            assert rules["account_catalog_allows"](catalog, model, effort), "matched_model_unavailable"
            home = run / "client/home"
            home.mkdir(parents=True)
            fixture = start_fixture(ws)
            port = fixture.server_port
            target = f"ws://127.0.0.1:{port}/v1/responses"
        catalog_path = run / "models.json"
        catalog_path.write_bytes(catalog)
        staged = replay.prepare_fixture_runtime(run, payload)
        helper = run / "fixture-runtime/desktop_helper.py"
        source = helper.read_bytes()
        anchor = b'"from headroom.cli import main; main()"'
        assert source.count(anchor) == 1, "engine_bootstrap_changed"
        args_for_guard = (target, model, effort, str(trace), replay.SENTINEL)
        hook = f"import runpy; runpy.run_path({str(PROGRAM)!r})['install_guard']{args_for_guard!r}; "
        helper.write_bytes(source.replace(anchor, repr(hook).encode("ascii") + b" " + anchor, 1))
        runtime_sources = json.loads((runtime / "optimization-sources.json").read_bytes())
        inputs = [PROGRAM, COMPARE, WS_CAPTURE, cli, runner, Path(sys.executable), payload / "python313.dll",
                  catalog_path, run / "registration.json", payload / "runtime-manifest.json", runtime / "manifest.json",
                  *staged, *[runtime / path.name for path in staged],
                  *[ROOT / "bench/optimization" / name for name in ("rules_pilot.py", "rules_live.py", "replay.py", "ccr_policy.py", "surfaces.py")],
                  *[ROOT / "tests/runtime" / name for name in ("responses_usage_capture.py", "anthropic_usage_capture.py")],
                  *[payload / "Lib/site-packages/websockets" / name for name in ("asyncio/client.py", "asyncio/connection.py", "__init__.py")],
                  *[payload / "Lib/site-packages/headroom" / name for name in runtime_sources]]
        before = {str(path): replay.sha(path) for path in inputs}
        engine = ws["probe"]["UsageEngine"](controller, supervisor, payload, run / "engine", port, pilot["PLAN"])
        engine.start()
        seen, existing = pilot["usage_ids"](engine, {"probe": ws["probe"], "records": ws["capture"]["records"]})
        assert not seen and not existing, "engine_not_empty"
        transient = run / "client"
        work, temp = transient / "workspace", transient / "temp"
        for path in (work, temp):
            path.mkdir(parents=True)
        env = pilot["client_environment"]()
        env.update(CODEX_HOME=str(home), CODEX_SQLITE_HOME=str(transient / "state"), TEMP=str(temp), TMP=str(temp))
        config = {"model_provider": "probe", "model_providers.probe.name": "Headroom WS Smoke",
                  "model_providers.probe.base_url": f"http://127.0.0.1:{engine.port}/v1",
                  "model_providers.probe.wire_api": "responses", "model_providers.probe.supports_websockets": True,
                  "model_providers.probe.requires_openai_auth": args.live,
                  "model_providers.probe.request_max_retries": 0, "model_providers.probe.stream_max_retries": 0,
                  "cli_auth_credentials_store": "file", "model_catalog_json": str(catalog_path),
                  "model_reasoning_effort": effort, "log_dir": str(transient / "logs"), "project_doc_max_bytes": 0,
                  "features.memories": False, "features.multi_agent": False, "features.multi_agent_v2": False,
                  "mcp_servers.headroom_rule_pilot.enabled": False}
        if not args.live:
            env["HEADROOM_WS_SMOKE_TOKEN"] = replay.SENTINEL
            config["model_providers.probe.env_key"] = "HEADROOM_WS_SMOKE_TOKEN"
            config["model_providers.probe.http_headers.X-Proof-Case"] = "sequence"
        command = pilot["client_args"](cli, work, "counts", "control", model, config,
                                       prompt=f"Reply exactly {MARKER} using no tools.")
        try:
            exit_code, stdout = pilot["run_client"](command, work, env, supervisor, 45)
        except subprocess.TimeoutExpired:
            exit_code, stdout = -1, b""
        observed = inspect_cli(exit_code, stdout)
        rows, joined = read_usage(engine, ws, seen)
        observed["usage_joined_by_request"] = joined
        observed["engine_rows"] = [{"surface": row["surface"], "truncated": row["truncated"], "failed": row.get("failed", False),
                                    "attempts": len(row["attempts"]), "usage": row["usage"]} for row in rows]
        matched = matched_usage(rows, observed["usage"]) if joined else None
    except Exception as error:
        failure = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)) else type(error).__name__
    finally:
        try:
            if engine is not None:
                engine.close()
        except Exception:
            failure = failure or "engine_shutdown_failed"
        finally:
            if fixture is not None:
                fixture.shutdown()
                fixture.server_close()
            transient = run / "client"
            if transient.is_dir():
                compare["remove_client_state"](transient)
    transport = [json.loads(line) for line in trace.read_bytes().splitlines()] if trace.is_file() else []
    events = [item["event"] for item in transport]
    creates = [item["kind"] for item in transport if item["event"] == "ws_frame"]
    after = {path: replay.sha(Path(path)) for path in before}
    passed = (failure is None and bool(before) and before == after and observed.get("completed") and observed.get("answer_exact")
              and matched is not None and engine.stopped["owned_port_closed"] and events.count("ws_opened") == 1
              and creates in (["generate"], ["warmup", "generate"]))
    replay.write_new(run / "result.json", {**plan, "passed": bool(passed), "failure": failure, "transport": transport,
        "client": observed, "matched_usage": matched, "engine_stop": engine.stopped if engine else None,
        "before": before, "after": after, "generated_responses_sent": creates.count("generate"),
        "provider_model_requests": creates.count("generate") if args.live else 0})
    print(json.dumps({"passed": bool(passed), "failure": failure, "events": events, "frames": creates,
                      "client_completed": observed.get("completed"), "answer_exact": observed.get("answer_exact"),
                      "matched_usage": matched, "proof": (run / "result.json").relative_to(ROOT).as_posix()}))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

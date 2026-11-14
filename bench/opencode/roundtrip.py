"""Opt-in OpenCode 1.19.3 Chat tool -> Headroom -> CCR fixture proof."""
from __future__ import annotations

import argparse
import http.client
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import uuid

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("opencode_common_proof", ROOT / "bench/claude/roundtrip.py")
common = importlib.util.module_from_spec(spec)
spec.loader.exec_module(common)
fixture = common.module("opencode_chat_tool_fixture", ROOT / "tests/fixtures/opencode/tool_fixture.py")
CLI = ROOT / "work/toolchains/opencode/v1.19.3/runtime/opencode.exe"
CLI_SHA256 = "cab0c9a8480f71fbb7cbc88ad91057712ecb500ab41fb9c01dd586cb9d8ea375"


class IngressHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_args):
        pass

    def do_POST(self):
        connection = None
        try:
            expected_path = "/v1/responses" if self.server.surface == "responses" else "/v1/chat/completions"
            assert self.path.split("?", 1)[0] == expected_path, "unexpected_client_route"
            size = int(self.headers.get("content-length", "0"))
            assert 0 < size <= 4 * 1024 * 1024, "native_request_size_invalid"
            raw = self.rfile.read(size)
            body = fixture.normalize_request(json.loads(raw), self.server.surface)
            matches = [message for message in body.get("messages", [])
                       if message.get("role") == "tool" and message.get("tool_call_id") == fixture.CLIENT_ID]
            if matches:
                assert len(matches) == 1, "duplicate_native_tool_result"
                wire = fixture.wire_text(matches[0]["content"])
                assert fixture.tool_text(wire) == fixture.LOGS, "native_tool_output_changed_or_offloaded"
                if self.server.rows and self.server.original_wire is not None:
                    assert wire == self.server.original_wire, "native_history_changed"
                self.server.original_wire = wire
                self.server.conversation.original_wire_result = wire
            self.server.rows.append({"wire_model_preserved": body.get("model") == self.server.conversation.model,
                                     "native_stream": body.get("stream") is True,
                                     "native_log_present_byte_exact": bool(matches),
                                     "original_wire_bytes": len(wire.encode()) if matches else None})
            connection = http.client.HTTPConnection("127.0.0.1", self.server.engine_port, timeout=40)
            headers = {key: value for key, value in self.headers.items()
                       if key.lower() not in {"host", "connection", "transfer-encoding", "content-length"}}
            connection.request("POST", self.path, raw, headers)
            response = connection.getresponse()
            self.send_response(response.status)
            for key, value in response.getheaders():
                if key.lower() not in {"connection", "transfer-encoding", "content-length"}:
                    self.send_header(key, value)
            self.send_header("Transfer-Encoding", "chunked")
            self.send_header("Connection", "close")
            self.end_headers()
            total = 0
            while chunk := response.read1(16384):
                total += len(chunk)
                assert total <= 4 * 1024 * 1024, "native_response_size_invalid"
                self.wfile.write(f"{len(chunk):x}\r\n".encode() + chunk + b"\r\n")
                self.wfile.flush()
            self.wfile.write(b"0\r\n\r\n")
            self.wfile.flush()
        except (AssertionError, ValueError, OSError) as error:
            if self.server.error is None:
                self.server.error = str(error) if isinstance(error, AssertionError) else "local_relay_io_failed"
            if connection is None:
                self.send_error(400, "fixture_rejected")
            self.close_connection = True
        finally:
            if connection is not None:
                connection.close()


class Ingress(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = False

    def __init__(self, engine_port, conversation, surface):
        super().__init__(("127.0.0.1", 0), IngressHandler)
        self.engine_port, self.conversation = engine_port, conversation
        self.surface = surface
        self.rows, self.error, self.original_wire = [], None, None


def environment(directory, config, sink):
    home = directory / "client-home"
    env = {key: os.environ[key] for key in ("SystemRoot", "WINDIR", "SystemDrive", "COMSPEC") if key in os.environ}
    env.update(PATH=os.pathsep.join((str(CLI.parent), str(Path(env["SystemRoot"]) / "System32"))),
               HOME=str(home), USERPROFILE=str(home), HOMEDRIVE=home.drive, HOMEPATH=str(home)[len(home.drive):],
               APPDATA=str(directory / "roaming"), LOCALAPPDATA=str(directory / "local"),
               TEMP=str(directory / "temp"), TMP=str(directory / "temp"), ProgramData=str(directory / "program-data"),
               XDG_CONFIG_HOME=str(directory / "xdg-config"), XDG_DATA_HOME=str(directory / "xdg-data"),
               XDG_CACHE_HOME=str(directory / "xdg-cache"), XDG_STATE_HOME=str(directory / "xdg-state"),
               OPENCODE_CONFIG_DIR=str(directory / "client-config"), OPENCODE_CONFIG_CONTENT=json.dumps(config),
               OPENCODE_DISABLE_MODELS_FETCH="1", OPENCODE_DISABLE_AUTOUPDATE="1", OPENCODE_DISABLE_PROJECT_CONFIG="1",
               OPENAI_API_KEY="headroom-local-fixture-only", GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=str(home / ".gitconfig"))
    for key in ("HOME", "APPDATA", "LOCALAPPDATA", "TEMP", "ProgramData", "XDG_CONFIG_HOME", "XDG_DATA_HOME",
                "XDG_CACHE_HOME", "XDG_STATE_HOME", "OPENCODE_CONFIG_DIR"):
        Path(env[key]).mkdir(parents=True, exist_ok=True)
    for key in ("HTTP_PROXY", "HTTPS_PROXY", "http_proxy", "https_proxy"):
        env[key] = f"http://127.0.0.1:{sink}"
    env["NO_PROXY"] = env["no_proxy"] = "127.0.0.1,localhost"
    return env


def configuration(port, receipt, surface, retrieval_port=None):
    provider, sdk, model = ("openai", "@ai-sdk/openai", fixture.RESPONSES_MODEL) if surface == "responses" else (
        "lab", "@ai-sdk/openai-compatible", fixture.MODEL)
    value = {"$schema": "https://opencode.ai/config.json", "model": provider + "/slot", "small_model": provider + "/slot",
            "enabled_providers": [provider], "provider": {provider: {"npm": sdk,
                "name": "Local tool fixture", "options": {"baseURL": f"http://127.0.0.1:{port}/v1",
                    "apiKey": "{env:OPENAI_API_KEY}"},
                "models": {"slot": {"id": model, "name": "Local fixture", "tool_call": True}}}},
            "autoupdate": False, "plugin": [],
            "mcp": {"headroom_fixture": {"type": "local", "enabled": True,
                "command": [sys.executable, "-I", "-B", str(Path(__file__).resolve()), "--mcp-receipt", str(receipt)]}},
            "permission": {"*": "deny", fixture.CLIENT_TOOL: "allow"},
            "tools": {name: False for name in ("bash", "edit", "write", "read", "grep", "glob", "lsp", "apply_patch",
                                               "skill", "webfetch", "websearch", "question", "task", "todowrite", "todoread")}}
    if retrieval_port is not None:
        value["mcp"]["headroom"] = {"type": "local", "enabled": True,
            "command": [sys.executable, "-I", "-B", str(Path(__file__).resolve()),
                        "--mcp-receipt", str(receipt.with_name("retrieval-receipt.json")), "--retrieve-port", str(retrieval_port)]}
        value["permission"]["headroom_retrieve"] = "allow"
    return value


def run(surface, retrieval_bridge):
    assert os.name == "nt" and Path.cwd().resolve() == ROOT, "windows_project_root_required"
    assert common.sha(CLI) == CLI_SHA256, "pinned_opencode_hash_required"
    replay = common.module("opencode_replay", ROOT / "bench/optimization/replay.py")
    controller = common.module("opencode_controller", ROOT / "runtime/desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(ROOT / "runtime/manifest.json")
    assert common.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "pinned_python_required"
    supervisor_path = payload / manifest["engine"]["runner"]
    supervisor = common.module("opencode_supervisor", supervisor_path)
    cache = controller.verify_tokenizer_cache(payload)
    directory = ROOT / "work/proofs/opencode-tools/runs" / str(uuid.uuid4())
    directory.mkdir(parents=True)
    selector, model = ("openai/slot", fixture.RESPONSES_MODEL) if surface == "responses" else ("lab/slot", fixture.MODEL)
    plan = {"schema_version": 1, "task": "H010-b", "selector": selector, "wire_model": model,
            "protocol": surface, "client_timeout_seconds": 180,
            "explicit_local_retrieval_bridge": retrieval_bridge,
            "thresholds": {"startup_timeout_seconds": 100}, "expected_mcp_calls": ["logs", "witness"]}
    replay.write_new(directory / "plan.json", plan)
    sources = [Path(__file__), ROOT / "tests/fixtures/opencode/tool_fixture.py",
               ROOT / "tests/fixtures/opencode/test_tool_fixture.py", ROOT / "bench/claude/roundtrip.py",
               ROOT / "tests/fixtures/claude/tool_fixture.py", ROOT / "bench/optimization/replay.py",
               ROOT / "bench/optimization/fixtures.py", CLI, Path(sys.executable), supervisor_path, directory / "plan.json"]
    sources.extend(ROOT / "runtime" / name for name in ("manifest.json", "desktop_controller.py", "desktop_helper.py",
                   "cache_control_shim.py", "optimization_control.py", "optimization-sources.json"))
    sources.extend((payload / "Lib/site-packages/headroom").rglob("*.py"))
    sources.extend(path for path in cache.iterdir() if path.is_file())
    sources.extend(replay.prepare_fixture_runtime(directory, payload))
    before = replay.fingerprints(sources)
    server = fixture.Upstream(replay.SENTINEL, surface)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    engine = replay.Engine(controller, supervisor, payload, directory / "engine", server.server_port, plan)
    job = supervisor.KillOnCloseJob()
    cli, ingress = None, None
    result = {"schema_version": 1, "task": "H010-b", "passed": False}
    print(json.dumps({"started": directory.relative_to(ROOT).as_posix(), "provider": "loopback_only"}), flush=True)
    try:
        result["engine"] = engine.start()
        ingress = Ingress(engine.port, server.conversation, surface)
        threading.Thread(target=ingress.serve_forever, daemon=True).start()
        config = configuration(ingress.server_port, directory / "mcp-receipt.json", surface,
                               engine.port if retrieval_bridge else None)
        config_before = replay.encoded(config)
        (directory / "engine/workspace/.git").mkdir()
        command = [str(CLI), "run", "--pure", "--format", "json", "--title", "H010 local tool fixture",
                   "--dir", str(directory / "engine/workspace"), "Use the synthetic fixture tool, then return the final marker."]
        cli = subprocess.Popen(command, cwd=directory / "engine/workspace",
                               env=environment(directory, config, server.server_port), stdin=subprocess.DEVNULL,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, creationflags=subprocess.CREATE_NO_WINDOW)
        job.assign(cli.pid)
        output, errors = common.BoundedOutput(cli.stdout), common.BoundedOutput(cli.stderr)
        cli.wait(timeout=plan["client_timeout_seconds"])
        stdout, stderr = output.finish(), errors.finish()
        events = [json.loads(line) for line in stdout.splitlines() if line.strip()]
        tool_calls = [event["part"] for event in events if event.get("type") == "tool_use"]
        texts = [event["part"].get("text") for event in events if event.get("type") == "text"]
        result["cli"] = {"version": "1.19.3", "sha256": CLI_SHA256, "exit_code": cli.returncode,
                         "stdout_bytes": len(stdout), "stderr_bytes": len(stderr),
                         "event_types": sorted({event.get("type", "unknown") for event in events}),
                         "native_tool_names": [call.get("tool") for call in tool_calls],
                         "native_call_ids": [call.get("callID") for call in tool_calls],
                         "native_tool_states": [call.get("state", {}).get("status") for call in tool_calls],
                         "raw_output_retained": False}
        assert cli.returncode == 0 and not any(event.get("type") == "error" for event in events), "native_cli_failed"
        assert texts == [fixture.FINAL], "final_marker_changed"
        assert server.conversation.error is None and server.conversation.complete, "upstream_conversation_incomplete"
        expected_native_count = 4 if surface == "chat" and retrieval_bridge else 3
        assert ingress.error is None and len(ingress.rows) == expected_native_count, "native_request_count_changed"
        assert all(row["wire_model_preserved"] and row["native_stream"] for row in ingress.rows), "native_settings_changed"
        assert ingress.rows[1]["native_log_present_byte_exact"], "native_original_log_missing"
        expected_calls = [(fixture.CLIENT_ID, fixture.CLIENT_TOOL), (fixture.SECOND_ID, fixture.CLIENT_TOOL)]
        if surface == "chat" and retrieval_bridge:
            expected_calls.insert(1, (fixture.RETRIEVE_ID, "headroom_retrieve"))
            retrieval_receipt = json.loads((directory / "retrieval-receipt.json").read_text())
            assert retrieval_receipt["calls"] == 1, "native_retrieval_call_count_changed"
            result["client_retrieval"] = retrieval_receipt
        elif surface == "responses":
            assert not (directory / "retrieval-receipt.json").exists(), "responses_retrieval_leaked_to_client"
        assert [(call.get("callID"), call.get("tool")) for call in tool_calls] == expected_calls, "native_call_ids_changed"
        assert all(call.get("state", {}).get("status") == "completed" for call in tool_calls), "native_tool_state_changed"
        assert json.loads((directory / "mcp-receipt.json").read_text())["calls"] == plan["expected_mcp_calls"], "mcp_calls_changed"
        assert replay.encoded(config) == config_before, "selector_config_changed"
        result["selector_config_preserved"] = True
        result["passed"] = True
    except BaseException as error:
        result["failure"] = {"class": type(error).__name__, "code": str(error) if isinstance(error, AssertionError) else "bounded_roundtrip_failed"}
    finally:
        job.close()
        if cli is not None and cli.poll() is None:
            cli.wait(timeout=10)
        if ingress is not None:
            ingress.shutdown()
            ingress.server_close()
            result["native_ingress"] = {"rows": ingress.rows, "error": ingress.error}
        try:
            engine.close()
            result["engine_stop"] = engine.stopped
        except BaseException as error:
            result["passed"], result["cleanup_error_class"] = False, type(error).__name__
        server.shutdown()
        server.server_close()
        after = replay.fingerprints(sources)
        result.update(before=before, after=after, inputs_unchanged=before == after, protocol=surface, selector=selector,
                      upstream_rows=server.conversation.rows, upstream_error=server.conversation.error,
                      reasoning_fields=server.conversation.reasoning, blocked_external_connects=server.blocked_connects,
                      fixture_network=replay.fixture_network_evidence(directory),
                      environment={"workspace": str(ROOT), "windows_build": list(sys.getwindowsversion()[:3]), "python": sys.version})
        result["passed"] = result["passed"] and result["inputs_unchanged"] and result["fixture_network"]["huggingface_hub_payload_bytes"] == 0
        replay.write_new(directory / "result.json", result)
    print(json.dumps({key: result[key] for key in ("passed", "upstream_rows", "upstream_error", "inputs_unchanged")}
                     | {"failure": result.get("failure"), "cli": result.get("cli"), "native_ingress": result.get("native_ingress"),
                        "engine_stop": result.get("engine_stop"), "proof": directory.relative_to(ROOT).as_posix()}), flush=True)
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--run", action="store_true")
    parser.add_argument("--surface", choices=("chat", "responses"), default="chat")
    parser.add_argument("--mcp-receipt", type=Path)
    parser.add_argument("--retrieve-port", type=int)
    parser.add_argument("--with-retrieval-bridge", action="store_true")
    args = parser.parse_args()
    if args.mcp_receipt is not None:
        receipt = args.mcp_receipt.resolve()
        assert receipt.is_relative_to(ROOT / "work/proofs/opencode-tools/runs") and receipt.parent.is_dir()
        if args.retrieve_port is not None:
            assert 1 <= args.retrieve_port <= 65535
            fixture.serve_retrieval(receipt, args.retrieve_port)
        else:
            fixture.data.serve_mcp(receipt)
    else:
        assert args.run, "explicit_run_required"
        raise SystemExit(run(args.surface, args.with_retrieval_bridge))

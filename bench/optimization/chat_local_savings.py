"""Compare cold Chat Completions usage with and without the pinned Windows engine."""

from copy import deepcopy
import ctypes
from hashlib import file_digest, sha256
import http.client
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import os
from pathlib import Path, PurePosixPath
import re
import runpy
import socket
import subprocess
import sys
import threading
import time
import uuid
from zipfile import ZipFile


PROGRAM = Path(__file__).resolve()
ROOT = PROGRAM.parents[2]
ORIGINAL = PROGRAM.with_name("chat_real_inference.py")
LOCAL = runpy.run_path(str(ORIGINAL))
CAPTURE = LOCAL["CAPTURE"]
PATH = LOCAL["PATH"]
PROOFS = LOCAL["PROOFS"]
FIXTURES = runpy.run_path(str(PROGRAM.with_name("fixtures.py")))
LOG_LINES = 24
REQUEST_TIMEOUT = 120


def build_request():
    body = deepcopy(LOCAL["BODY"])
    messages = [{"role": "user", "content": "Inspect the four tool logs and report the final result."}]
    for index in range(1, 5):
        call = f"log-{index}"
        lines = FIXTURES["output"]("repetitive_logs", index).splitlines()[:LOG_LINES]
        if index == 4:
            lines.append("FINAL_RESULT = 4")
        messages.extend([
            {"role": "assistant", "content": None, "tool_calls": [{
                "id": call, "type": "function", "function": {"name": "inspect_workspace", "arguments": "{}"},
            }]},
            {"role": "tool", "tool_call_id": call, "content": "\n".join(lines)},
        ])
    messages.append({"role": "user", "content": 'Reply with JSON only. Use the key "answer" and the integer next to FINAL_RESULT.'})
    schema = body["response_format"]["json_schema"]["schema"]
    schema["properties"]["answer"]["enum"] = list(range(1, 10))
    return {
        **body, "messages": messages, "tool_choice": "none",
        "tools": [{"type": "function", "function": {
            "name": "inspect_workspace", "description": "Read the fixture log",
            "parameters": {"type": "object", "properties": {}},
        }}],
    }


def exchange(port, headers, body):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=REQUEST_TIMEOUT)
    try:
        connection.request("POST", PATH, headers=headers, body=body)
        response = connection.getresponse()
        data = response.read(262145)
        assert len(data) <= 262144, "local_response_limit"
        return response.status, response.getheader("Content-Type", ""), data
    finally:
        connection.close()


def model_bridge(original, port, result):
    invariants = {key: value for key, value in original.items() if key not in ("messages", "tools")}
    def tool_sizes(messages):
        return {row["tool_call_id"]: len(row["content"].encode("utf-8"))
                for row in messages if row["role"] == "tool"}

    class Bridge(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_HEAD(self):
            self.send_response(200)
            self.end_headers()

        def do_POST(self):
            if self.path != PATH or "tool_delivery" in result:
                self.send_error(403)
                return
            try:
                size = int(self.headers.get("Content-Length", 0))
                assert 0 < size <= 65536, "forwarded_size"
                request = self.rfile.read(size)
                body = json.loads(request)
                assert isinstance(body, dict), "forwarded_message_shape"
                assert all(body.get(key) == value for key, value in invariants.items()), "changed_request_fields"
                source, forwarded = original["messages"], body["messages"]
                assert [row for row in source if row["role"] != "tool"] == [
                    row for row in forwarded if row["role"] != "tool"], "changed_non_tool_history"
                before, after = tool_sizes(source), tool_sizes(forwarded)
                assert before.keys() == after.keys(), "changed_tool_ids"
                result["tool_delivery"] = {"tool_bytes_direct": sum(before.values()), "tool_bytes_forwarded": sum(after.values())}
                status, media, response = exchange(port, {"Content-Type": "application/json"}, request)
            except (AssertionError, OSError, ValueError, KeyError) as error:
                result["bridge_error"] = str(error) if isinstance(error, AssertionError) else type(error).__name__
                self.send_error(400)
                return
            self.send_response(status)
            self.send_header("Content-Type", media)
            self.send_header("Content-Length", str(len(response)))
            self.end_headers()
            self.wfile.write(response)

    return HTTPServer(("127.0.0.1", 0), Bridge)


def pinned_model(replay):
    assets = PROOFS / "assets"
    assert assets.resolve(strict=True) == assets, "asset_scope"
    for name, pin in LOCAL["PINS"].items():
        assert replay.sha(assets / name) == pin, "asset_pin_changed"
    binary = assets / "llama-b12359-bin-win-cpu-x64/llama-server.exe"
    binaries = [binary, *sorted(binary.parent.glob("*.dll"))]
    archive_path = assets / LOCAL["ARCHIVE"]
    with ZipFile(archive_path) as archive:
        members = {PurePosixPath(name).name: name for name in archive.namelist()
                   if name.endswith((".dll", "/llama-server.exe")) or name == "llama-server.exe"}
        assert members.keys() == {path.name for path in binaries}, "server_binary_set_changed"
        for path in binaries:
            assert path.resolve(strict=True) == path, "server_binary_path_changed"
            with archive.open(members[path.name]) as stream:
                assert file_digest(stream, "sha256").hexdigest() == replay.sha(path), "server_binary_changed"
    return binary, assets / LOCAL["WEIGHTS"], [archive_path, *binaries]


def main():
    assert os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_workspace"
    chat = runpy.run_path(str(CAPTURE))
    capture, replay, probe = (chat[key] for key in ("capture", "base", "probe"))
    plan_path = capture["ccr"]["PLAN"]
    plan = json.loads(plan_path.read_bytes())
    runtime = ROOT / "runtime"
    controller = replay.load_module("paired_chat_controller", runtime / "desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(runtime / "manifest.json")
    assert replay.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "python_pin_changed"
    runner = payload / manifest["engine"]["runner"]
    supervisor = replay.load_module("paired_chat_supervisor", runner)
    binary, weights, binaries = pinned_model(replay)
    run = PROOFS / "pairs" / uuid.uuid4().hex
    assert run.resolve().is_relative_to(PROOFS), "proof_scope"
    run.mkdir(parents=True)
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]

    body = build_request()
    request = replay.encoded(body)
    registration = {"scope": "one synthetic log-reading pair on an actual local CPU model",
                    "engine": manifest["engine"]["version"], "model": LOCAL["MODEL"],
                    "prompt_cache": False, "context_size": 8192, "max_model_requests": 2, "lines_per_log": LOG_LINES,
                    "request_timeout_seconds": REQUEST_TIMEOUT,
                    "request_sha256": sha256(request).hexdigest(),
                    "acceptance": "both answers exactly 4, zero cache reads, matching routed usage, both processes closed",
                    "expected": {"answer": 4}}
    replay.write_new(run / "registration.json", registration)
    fixture_sources = replay.prepare_fixture_runtime(run, payload)
    sources = json.loads((runtime / "optimization-sources.json").read_bytes())
    inputs = [PROGRAM, ORIGINAL, CAPTURE, CAPTURE.with_name("anthropic_usage_capture.py"), plan_path,
              *(PROGRAM.with_name(name) for name in ("anthropic_usage.py", "ccr_policy.py", "fixtures.py", "replay.py", "surfaces.py")),
              *binaries, weights, runner, Path(sys.executable), *fixture_sources,
              *(runtime / name for name in ("manifest.json", "desktop_controller.py", "desktop_helper.py",
                                            "optimization_control.py", "optimization-sources.json")),
              *(payload / "Lib/site-packages/headroom" / name for name in sources)]
    before = replay.fingerprints(inputs)
    iphlpapi = ctypes.WinDLL("iphlpapi")
    job = supervisor.KillOnCloseJob()
    provider = engine = bridge = bridge_thread = None
    result = {**registration, "external_model_calls": 0}
    failure = None
    try:
        os.environ["LITELLM_LOCAL_MODEL_COST_MAP"] = "True"
        cwd = run / "model-server"
        cwd.mkdir()
        env = supervisor.safe_environment(cwd, cwd, cwd, port)
        provider = subprocess.Popen(
            [str(binary), "--model", str(weights), "--alias", LOCAL["MODEL"], "--host", "127.0.0.1",
             "--port", str(port), "--no-webui", "--ctx-size", "8192", "--parallel", "1", "--no-cache-prompt",
             "--n-gpu-layers", "0", "--threads", "2", "--threads-batch", "4", "--n-predict", "24", "--jinja"],
            cwd=cwd, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL, creationflags=subprocess.CREATE_NO_WINDOW,
        )
        try:
            job.assign(provider.pid)
        except OSError:
            provider.kill()
            raise
        LOCAL["ready"](port, provider, supervisor, iphlpapi)
        headers = {"Content-Type": "application/json", "Authorization": "Bearer " + replay.SENTINEL}
        for arm in ("direct", "headroom"):
            if arm == "headroom":
                bridge = model_bridge(body, port, result)
                bridge_thread = threading.Thread(target=bridge.serve_forever, daemon=True)
                bridge_thread.start()
                engine = probe["UsageEngine"](controller, supervisor, payload, run / "engine", bridge.server_port, plan)
                engine.start()
                assert not capture["records"](engine), "engine_not_empty"
            endpoint = port if arm == "direct" else engine.port
            status, media, response = exchange(endpoint, headers, request)
            result[arm] = {"status": status}
            if status >= 400 and arm == "direct":
                try:
                    message = json.loads(response).get("error", {}).get("message", "")
                    if isinstance(message, str):
                        print("Local provider rejected the synthetic input:", message.splitlines()[0][:140], file=sys.stderr)
                except (AttributeError, IndexError, ValueError):
                    pass
            assert status == 200 and media.startswith("text/event-stream"), f"{arm}_route_failed"
            usage = LOCAL["inspect_stream"](response)
            result[arm].update(usage=usage, answer_exact=True)
            assert usage["cache_read_input_tokens"] == 0, f"{arm}_cache_hit"
        deadline = time.monotonic() + 3
        while True:
            rows = capture["records"](engine)
            logged = set(probe["request_logs"](engine))
            matched = len(rows) == 1 and set(rows) == logged
            if matched or time.monotonic() >= deadline:
                break
            time.sleep(0.05)
        assert matched, "usage_row_join"
        chat["check"](next(iter(rows.values())), [{"status": 200, "usage": result["headroom"]["usage"]}], 200, 1)
    except Exception as error:
        failure = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)) else type(error).__name__
    finally:
        try:
            if engine is not None:
                engine.close()
        except Exception:
            failure = failure or "engine_shutdown_failed"
        finally:
            if bridge is not None:
                if bridge_thread is not None and bridge_thread.is_alive():
                    bridge.shutdown()
                bridge.server_close()
            job.close()
            if provider is not None:
                provider.wait(timeout=10)
    after = replay.fingerprints(inputs)
    provider_closed = provider is not None and not supervisor.listener_pids(port, iphlpapi)
    engine_stopped = bool(engine and engine.stopped and engine.stopped["owned_port_closed"])
    passed = failure is None and before == after and provider_closed and engine_stopped
    result.update(passed=passed, failure=failure, provider_closed=provider_closed, engine_stopped=engine_stopped,
                  inputs_before=before, inputs_after=after)
    if passed:
        direct = result["direct"]["usage"]
        routed = result["headroom"]["usage"]
        result["input_tokens_saved"] = direct["input_tokens"] - routed["input_tokens"]
        result["total_tokens_saved"] = direct["input_tokens"] + direct["output_tokens"] - routed["input_tokens"] - routed["output_tokens"]
    proof = run / "result.json"
    replay.write_new(proof, result)
    print(json.dumps({key: result[key] for key in ("passed", "failure", "bridge_error", "direct", "headroom", "input_tokens_saved")
                      if key in result} | {"proof": proof.relative_to(ROOT).as_posix()}))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

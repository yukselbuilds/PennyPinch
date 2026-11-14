"""Opt-in V2 Windows CLI protocol preflight; synthetic loopback inference only."""
from __future__ import annotations

import ctypes
from ctypes import wintypes
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import threading
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("v2_opencode_shared", Path(__file__).with_name("roundtrip.py"))
shared = importlib.util.module_from_spec(spec)
spec.loader.exec_module(shared)
common, fixture = shared.common, shared.fixture
CLI = ROOT / "work/toolchains/opencode/v2.3.0/runtime/opencode.exe"
CLI_SHA256 = "2359208e2f81539fd16eb755a4d8a747e87c6fd5cd59a49bb552f8a610d33eb9"
SOURCE_COMMIT = "a6b15d33daf6ab7d35569f6d711fdb630b9f04e7"
FINAL = "HEADROOM_V2_PROTOCOL_OK"
PROMPT = "Return the synthetic fixture final marker. Do not use tools."
SENTINEL = "headroom-local-fixture-only"
LIMIT = 4 * 1024 * 1024


def configuration(port, surface):
    assert surface in ("chat", "responses"), "unknown_protocol"
    provider = "lab" if surface == "chat" else "openai"
    model = fixture.MODEL if surface == "chat" else fixture.RESPONSES_MODEL
    return {
        "model": provider + "/slot", "update": "disable", "share": "disabled", "plugins": [],
        "warming": False, "snapshots": False, "compaction": {"auto": False},
        "agents": {"title": {"disabled": True}},
        "permissions": [{"action": "*", "resource": "*", "effect": "deny"}],
        "policies": [
            *({"action": action, "resource": "*", "effect": "deny"} for action in
              ("provider.use", "tool.use", "integration.connect", "integration.use", "mcp.use", "skill.use", "plugin.use", "websearch.use")),
            {"action": "provider.use", "resource": provider, "effect": "allow"},
            {"action": "integration.use", "resource": provider, "effect": "allow"},
        ],
        "providers": {provider: {
            "package": "@opencode/ai/providers/openai-compatible" if surface == "chat" else "@opencode/ai/providers/openai",
            "env": ["OPENAI_API_KEY"],
            "settings": {"baseURL": f"http://127.0.0.1:{port}/v1", "apiKey": "{env:OPENAI_API_KEY}"},
            "models": {"slot": {"modelID": model, "name": "Synthetic V2 fixture",
                                "capabilities": {"tools": False, "reasoning": {"supported": False}}}},
        }},
    }


def environment(directory, sink):
    env = shared.environment(directory, {}, sink)
    for key in ("OPENCODE_CONFIG", "OPENCODE_CONFIG_CONTENT", "OPENCODE_CONFIG_DIR"):
        env.pop(key, None)
    env["PATH"] = os.pathsep.join((str(CLI.parent), str(Path(env["SystemRoot"]) / "System32")))
    return env


def summarize(stdout):
    assert len(stdout) <= LIMIT, "cli_output_too_large"
    records = [json.loads(line) for line in stdout.splitlines() if line.strip()]
    assert records and all(isinstance(row, dict) for row in records), "invalid_cli_events"
    kinds = [row.get("type") for row in records]
    assert not set(kinds).intersection(("error", "retry", "tool_use", "background_wait")), "unexpected_cli_event"
    text = "".join(row.get("part", {}).get("text", "") for row in records if row.get("type") == "text")
    assert text == FINAL and kinds.count("step_finish") == 1, "cli_final_or_step_missing"
    return {"event_types": kinds, "final_marker_exact": True, "steps": 1, "usage": "synthetic_not_a_savings_measurement"}


def validate_request(path, headers, body, surface):
    expected = "/v1/chat/completions" if surface == "chat" else "/v1/responses"
    assert path == expected, "unexpected_route"
    assert headers.get("Authorization") == "Bearer " + SENTINEL, "synthetic_auth_changed"
    assert body.get("model") == (fixture.MODEL if surface == "chat" else fixture.RESPONSES_MODEL), "wire_model_changed"
    assert body.get("stream") is True, "stream_not_requested"
    assert PROMPT in json.dumps(body), "fixture_prompt_missing"
    return {"protocol": surface, "wire_model_preserved": True, "synthetic_auth_preserved": True, "stream_requested": True}


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_args):
        pass

    def do_CONNECT(self):
        self.server.blocked_connects += 1
        self.send_error(502, "local_fixture_only")

    def do_POST(self):
        try:
            length = int(self.headers.get("content-length", "0"))
            assert 0 < length <= LIMIT, "request_size_invalid"
            body = json.loads(self.rfile.read(length))
            row = validate_request(self.path, self.headers, body, self.server.surface)
            assert not self.server.rows, "unexpected_extra_request"
            self.server.rows.append(row)
            message = {"role": "assistant", "content": FINAL}
            raw = fixture.response(message, 0, True) if self.server.surface == "chat" else fixture.responses_response(message, 0, True)
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Content-Length", str(len(raw)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(raw)
            self.wfile.flush()
        except (AssertionError, ValueError, TypeError, OSError) as error:
            self.server.error = str(error) if isinstance(error, AssertionError) else "fixture_request_failed"
            self.close_connection = True


def active_processes(job):
    class Accounting(ctypes.Structure):
        _fields_ = [(name, ctypes.c_longlong) for name in ("user", "kernel", "period_user", "period_kernel")] + [
            (name, wintypes.DWORD) for name in ("faults", "total", "active", "terminated")]
    info = Accounting()
    query = job._kernel32.QueryInformationJobObject
    query.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD, ctypes.c_void_p]
    query.restype = wintypes.BOOL
    assert query(job._handle, 1, ctypes.byref(info), ctypes.sizeof(info), None), "owned_job_query_failed"
    return info.active


def cli_call(args, workspace, env, supervisor):
    job, process = supervisor.KillOnCloseJob(), None
    try:
        process = subprocess.Popen([str(CLI), *args], cwd=workspace, env=env, stdin=subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, creationflags=subprocess.CREATE_NO_WINDOW)
        job.assign(process.pid)
        output, errors = common.BoundedOutput(process.stdout), common.BoundedOutput(process.stderr)
        process.wait(timeout=120)
        stdout, stderr = output.finish(), errors.finish()
        deadline = time.monotonic() + 10
        while active_processes(job) and time.monotonic() < deadline:
            time.sleep(.1)
        row = {"args": args, "exit_code": process.returncode, "stdout_bytes": len(stdout), "stderr_bytes": len(stderr),
               "owned_processes_after_exit": active_processes(job)}
        row["passed"] = row["exit_code"] == 0 and row["owned_processes_after_exit"] == 0 and not stderr
        if not row["passed"]:
            diagnostic = re.sub(r"\x1b\[[0-9;]*m", "", (stdout + stderr).decode("utf-8", errors="replace"))
            diagnostic = re.sub(r"https?://\S+|[A-Za-z0-9_-]{40,}", "[redacted]", diagnostic)
            print(json.dumps({"cli_failure": row, "bounded_diagnostic": diagnostic[-1600:]}), flush=True)
        return row, stdout
    finally:
        job.close()
        if process is not None and process.poll() is None:
            process.kill()
            process.wait(timeout=10)


def validate_paths(stdout, directory):
    rows = [line.split(None, 1) for line in stdout.decode("utf-8").splitlines() if line.strip()]
    assert rows and all(len(row) == 2 for row in rows), "invalid_paths_output"
    assert {row[0] for row in rows}.issuperset(("home", "config", "state", "data", "cache", "db")), "global_paths_missing"
    for _, value in rows:
        assert Path(value).resolve().is_relative_to(directory.resolve()), "global_path_outside_fixture"
    return {"all_paths_inside_fixture": True, "path_names": [row[0] for row in rows]}


def run():
    assert os.name == "nt" and Path.cwd().resolve() == ROOT, "windows_project_root_required"
    assert common.sha(CLI) == CLI_SHA256, "pinned_v2_cli_required"
    replay = common.module("v2_replay", ROOT / "bench/optimization/replay.py")
    controller = common.module("v2_controller", ROOT / "runtime/desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(ROOT / "runtime/manifest.json")
    assert common.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "pinned_python_required"
    supervisor_path = payload / manifest["engine"]["runner"]
    supervisor = common.module("v2_supervisor", supervisor_path)
    cache = controller.verify_tokenizer_cache(payload)
    directory = ROOT / "work/proofs/opencode-v2/runs" / uuid.uuid4().hex[:12]
    directory.mkdir(parents=True)
    plan = {"task": "H010-e", "version": "2.3.0", "source_commit": SOURCE_COMMIT,
            "protocols": ["chat", "responses"], "client_timeout_seconds": 120,
            "thresholds": {"startup_timeout_seconds": 100}, "live_inference": False, "native_product_integration": False}
    replay.write_new(directory / "plan.json", plan)
    sources = [Path(__file__), ROOT / "tests/fixtures/opencode/test_v2.py", Path(shared.__file__),
               Path(common.__file__), Path(fixture.__file__), ROOT / "tests/fixtures/claude/tool_fixture.py",
               ROOT / "bench/optimization/replay.py", ROOT / "bench/optimization/fixtures.py",
               CLI, Path(sys.executable), supervisor_path, directory / "plan.json"]
    sources.extend(ROOT / "runtime" / name for name in ("manifest.json", "desktop_controller.py", "desktop_helper.py",
                   "cache_control_shim.py", "optimization_control.py", "optimization-sources.json"))
    sources.extend((payload / "Lib/site-packages/headroom").rglob("*.py"))
    sources.extend(path for path in cache.iterdir() if path.is_file())
    sources.extend((CLI.parent.parent / name) for name in ("receipt.json", "SHA256SUMS", "opencode-windows-x64-baseline.zip"))
    research = ROOT / "work/research/h010-v2"
    sources.extend(research / name for name in ("release.raw.json", "source-pin.json", "download.py"))
    for name in ("manifest.json", "runtime-source.json", "probe-source.json", "format-source.json", "route-source.json", "policy-source.json"):
        source_manifest = research / name
        value = json.loads(source_manifest.read_text())
        assert value["commit"] == SOURCE_COMMIT, "source_commit_changed"
        sources.append(source_manifest)
        for local, record in value["files"].items():
            assert common.sha(research / local) == record["sha256"], "source_pin_changed"
            sources.append(research / local)
    sources.extend(replay.prepare_fixture_runtime(directory, payload))
    clients = []
    for surface in plan["protocols"]:
        client = directory / surface
        env = environment(client, 9)
        workspace = Path(env["XDG_CONFIG_HOME"]) / "opencode"
        workspace.mkdir()
        clients.append((surface, client, env, workspace))
    before = replay.fingerprints(sources)
    result = {"schema_version": 1, **plan, "passed": False, "environment": {"os": platform.platform(),
              "windows": list(platform.win32_ver()), "python": sys.version, "cli_sha256": CLI_SHA256}, "cases": []}
    print(json.dumps({"started": directory.relative_to(ROOT).as_posix(), "provider": "loopback_synthetic"}), flush=True)
    try:
        for surface, client, env, workspace in clients:
            server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
            server.daemon_threads = True
            server.surface, server.rows, server.error, server.blocked_connects = surface, [], None, 0
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            engine = replay.Engine(controller, supervisor, payload, directory / ("engine-" + surface), server.server_port, plan)
            case = {"protocol": surface, "passed": False}
            result["cases"].append(case)
            try:
                case["engine"] = engine.start()
                config = workspace / "opencode.jsonc"
                replay.write_new(config, configuration(engine.port, surface))
                case["config_before"] = common.sha(config)
                case["environment_before"] = hashlib.sha256(replay.encoded(env)).hexdigest()
                case["commands"] = []
                for args in (["--version"], ["--help"], ["debug", "paths"]):
                    row, stdout = cli_call(args, workspace, env, supervisor)
                    case["commands"].append(row)
                    assert row["passed"], "cli_exit_or_service_cleanup_failed"
                    if args == ["--version"]:
                        assert stdout.decode().strip() == "opencode v2.3.0", "cli_version_changed"
                        row["version_exact"] = True
                    elif args == ["--help"]:
                        assert b"--standalone" in stdout, "standalone_flag_missing"
                        row["standalone_documented"] = True
                    else:
                        row.update(validate_paths(stdout, client))
                row, stdout = cli_call(["run", "--standalone", "--format", "json", "--model",
                                       configuration(engine.port, surface)["model"], "--title", "H010 V2 local fixture", PROMPT], workspace, env, supervisor)
                case["commands"].append(row)
                assert row["passed"], "cli_exit_or_service_cleanup_failed"
                row.update(summarize(stdout))
                assert server.error is None and len(server.rows) == 1, "upstream_validation_failed"
                case.update(upstream=server.rows, blocked_external_connects=server.blocked_connects,
                            config_after=common.sha(config))
                assert case["config_before"] == case["config_after"], "client_config_changed"
                case["passed"] = True
            finally:
                try:
                    if "config_before" in case:
                        case["config_after"] = common.sha(config)
                        case["environment_after"] = hashlib.sha256(replay.encoded(env)).hexdigest()
                        assert case["config_before"] == case["config_after"] and case["environment_before"] == case["environment_after"], "client_inputs_changed"
                finally:
                    try:
                        engine.close()
                        case["shutdown"] = engine.stopped
                    finally:
                        server.shutdown()
                        server.server_close()
                        thread.join(timeout=3)
        result["passed"] = True
    except Exception as error:
        result["failure"] = str(error) if isinstance(error, AssertionError) else type(error).__name__
    finally:
        after = replay.fingerprints(sources)
        result.update(before=before, after=after, inputs_unchanged=before == after)
        result["passed"] = result["passed"] and before == after
        replay.write_new(directory / "result.json", result)
    print(json.dumps({"proof": (directory / "result.json").relative_to(ROOT).as_posix(), "passed": result["passed"],
                      "failure": result.get("failure")}), flush=True)
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(run())

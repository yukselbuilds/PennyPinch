"""Opt-in pinned Claude CLI -> local MCP -> Headroom -> CCR fixture loop."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import uuid

ROOT = Path(__file__).resolve().parents[2]
CLAUDE_SHA256 = "10fa305545b20baf6e074a086762bd252b89dfe7035848a5d41385503b1a6c74"


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    sys.modules[name] = value
    spec.loader.exec_module(value)
    return value


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


class BoundedOutput:
    def __init__(self, pipe):
        self.pipe, self.data, self.overflow = pipe, bytearray(), False
        self.thread = threading.Thread(target=self.read, daemon=True)
        self.thread.start()

    def read(self):
        while chunk := self.pipe.read(65536):
            if len(self.data) + len(chunk) <= 4 * 1024 * 1024:
                self.data.extend(chunk)
            else:
                self.overflow = True

    def finish(self):
        self.thread.join(timeout=3)
        assert not self.thread.is_alive() and not self.overflow, "cli_output_not_bounded"
        return bytes(self.data)


def cli_environment(run, executable, port, sink):
    home, temp = run / "cli-home", run / "temp"
    for path in (home, temp, home / "AppData/Roaming", home / "AppData/Local"):
        path.mkdir(parents=True, exist_ok=True)
    system_root = os.environ.get("SystemRoot", r"C:\Windows")
    env = {"SystemRoot": system_root, "WINDIR": system_root, "SystemDrive": home.drive,
           "PATH": os.pathsep.join((str(executable.parent), str(Path(system_root) / "System32"))),
           "USERPROFILE": str(home), "HOME": str(home), "HOMEDRIVE": home.drive,
           "HOMEPATH": str(home)[len(home.drive):], "TEMP": str(temp), "TMP": str(temp),
           "APPDATA": str(home / "AppData/Roaming"), "LOCALAPPDATA": str(home / "AppData/Local"),
           "CLAUDE_CONFIG_DIR": str(home / ".claude"), "ANTHROPIC_API_KEY": "headroom-local-fixture-only",
           "ANTHROPIC_BASE_URL": f"http://127.0.0.1:{port}",
           "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1", "DISABLE_AUTOUPDATER": "1"}
    for key in ("HTTP_PROXY", "HTTPS_PROXY", "http_proxy", "https_proxy"):
        env[key] = f"http://127.0.0.1:{sink}"
    env["NO_PROXY"] = env["no_proxy"] = "127.0.0.1,localhost"
    return env


def run(executable):
    assert os.name == "nt" and Path.cwd().resolve() == ROOT, "windows_project_root_required"
    executable = executable.resolve(strict=True)
    assert sha(executable) == CLAUDE_SHA256, "pinned_claude_hash_required"
    replay = module("claude_tools_replay", ROOT / "bench/optimization/replay.py")
    fixture_path = ROOT / "tests/fixtures/claude/tool_fixture.py"
    fixture = module("claude_tools_fixture", fixture_path)
    controller = module("claude_tools_controller", ROOT / "runtime/desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(ROOT / "runtime/manifest.json")
    assert sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "pinned_python_required"
    supervisor_path = payload / manifest["engine"]["runner"]
    supervisor = module("claude_tools_supervisor", supervisor_path)
    cache = controller.verify_tokenizer_cache(payload)
    directory = ROOT / "work/proofs/claude-tools/runs" / str(uuid.uuid4())
    directory.mkdir(parents=True)
    plan = {"schema_version": 1, "task": "H009-b", "model": fixture.MODEL, "effort": fixture.EFFORT,
            "client_timeout_seconds": 180, "thresholds": {"startup_timeout_seconds": 100},
            "expected_mcp_calls": ["logs", "witness"], "expected_upstream_messages": 4,
            "scope": "Synthetic provider and MCP only; no real login, model or subagent"}
    replay.write_new(directory / "plan.json", plan)
    paths = [Path(__file__), fixture_path, fixture_path.with_name("test_tool_fixture.py"),
             ROOT / "bench/optimization/replay.py", ROOT / "bench/optimization/fixtures.py",
             ROOT / "runtime/manifest.json", ROOT / "runtime/desktop_controller.py",
             ROOT / "runtime/desktop_helper.py", ROOT / "runtime/cache_control_shim.py",
             ROOT / "runtime/optimization_control.py", ROOT / "runtime/optimization-sources.json",
             ROOT / "src-tauri/Cargo.toml", ROOT / "src-tauri/Cargo.lock", ROOT / "package.json",
             ROOT / "pnpm-lock.yaml", directory / "plan.json", Path(sys.executable), supervisor_path]
    paths.extend((payload / "Lib/site-packages/headroom").rglob("*.py"))
    paths.extend(path for path in cache.iterdir() if path.is_file())
    paths.extend(replay.prepare_fixture_runtime(directory, payload))
    before = replay.fingerprints(paths)
    before["external_pinned_claude_executable"] = sha(executable)
    server = fixture.Upstream(replay.SENTINEL)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    engine = replay.Engine(controller, supervisor, payload, directory / "engine", server.server_port, plan)
    job = supervisor.KillOnCloseJob()
    cli = None
    ingress = None
    result = {"schema_version": 1, "task": "H009-b", "passed": False}
    print(json.dumps({"started": directory.relative_to(ROOT).as_posix(), "provider": "loopback_only"}), flush=True)
    try:
        result["engine"] = engine.start()
        ingress = fixture.Ingress(engine.port)
        threading.Thread(target=ingress.serve_forever, daemon=True).start()
        mcp_path = directory / "mcp.json"
        replay.write_new(mcp_path, {"mcpServers": {"headroom_fixture": {"type": "stdio",
            "command": str(Path(sys.executable)),
            "args": ["-I", "-B", str(fixture_path), "--mcp-receipt", str(directory / "mcp-receipt.json")]}}})
        command = [str(executable), "--bare", "--print", "--verbose", "--output-format", "stream-json",
                   "--include-partial-messages", "--no-session-persistence", "--tools", "",
                   "--allowedTools", fixture.CLIENT_TOOL, "--strict-mcp-config", "--mcp-config", str(mcp_path),
                   "--permission-mode", "dontAsk", "--model", fixture.MODEL, "--effort", fixture.EFFORT,
                   "--max-turns", "5", "--", "Use the synthetic fixture tool, then return the fixture final marker."]
        cli = subprocess.Popen(command, cwd=directory / "engine/workspace",
                               env=cli_environment(directory, executable, ingress.server_port, server.server_port),
                               stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               creationflags=subprocess.CREATE_NO_WINDOW)
        job.assign(cli.pid)
        output, errors = BoundedOutput(cli.stdout), BoundedOutput(cli.stderr)
        cli.wait(timeout=plan["client_timeout_seconds"])
        stdout, stderr = output.finish(), errors.finish()
        events = [json.loads(line) for line in stdout.splitlines() if line.strip()]
        final = [event for event in events if event.get("type") == "result"]
        tool_calls = [block for event in events if event.get("type") == "assistant"
                      for block in event.get("message", {}).get("content", []) if block.get("type") == "tool_use"]
        result["cli"] = {"version": "2.1.158", "sha256": CLAUDE_SHA256, "exit_code": cli.returncode,
                         "stdout_bytes": len(stdout), "stderr_bytes": len(stderr),
                         "event_types": sorted({event.get("type", "unknown") for event in events}),
                         "result_subtypes": [event.get("subtype") for event in final],
                         "client_tool_names": [block.get("name") for block in tool_calls],
                         "client_call_ids": [block.get("id") for block in tool_calls],
                         "raw_output_retained": False}
        assert cli.returncode == 0, "cli_exit_failed"
        assert len(final) == 1 and final[0].get("subtype") == "success" and not final[0].get("is_error"), "cli_final_failed"
        assert final[0].get("result") == fixture.FINAL, "final_marker_changed"
        assert ingress.error is None and len(ingress.rows) == 3, "native_client_request_count_changed"
        assert all(row["model_preserved"] and row["effort_preserved"] and row["native_stream"] for row in ingress.rows), "native_client_settings_changed"
        assert ingress.rows[1]["original_log_present_byte_exact"], "native_log_output_changed"
        assert server.conversation.error is None and server.conversation.complete, "upstream_conversation_incomplete"
        assert json.loads((directory / "mcp-receipt.json").read_text())["calls"] == plan["expected_mcp_calls"], "mcp_calls_changed"
        assert [(block.get("name"), block.get("id")) for block in tool_calls] == [
            (fixture.CLIENT_TOOL, fixture.CLIENT_ID), (fixture.CLIENT_TOOL, fixture.SECOND_ID)], "client_tool_identity_changed"
        result["passed"] = True
    except BaseException as error:
        result["failure"] = {"class": type(error).__name__,
                             "code": str(error) if isinstance(error, AssertionError) else "bounded_roundtrip_failed"}
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
            result["passed"] = False
            result["cleanup_error_class"] = type(error).__name__
        server.shutdown()
        server.server_close()
        after = replay.fingerprints(paths)
        after["external_pinned_claude_executable"] = sha(executable)
        result.update(before=before, after=after, inputs_unchanged=before == after,
                      upstream_rows=server.conversation.rows, upstream_error=server.conversation.error,
                      rejected_observation=server.conversation.rejected_observation,
                      blocked_external_connects=server.blocked_connects,
                      fixture_network=replay.fixture_network_evidence(directory),
                      environment={"workspace": str(ROOT), "windows_build": list(sys.getwindowsversion()[:3]),
                                   "python": sys.version, "cli_timeout_seconds": plan["client_timeout_seconds"]})
        result["passed"] = result["passed"] and result["inputs_unchanged"] and result["fixture_network"]["huggingface_hub_payload_bytes"] == 0
        replay.write_new(directory / "result.json", result)
    print(json.dumps({key: result[key] for key in ("passed", "upstream_rows", "upstream_error", "inputs_unchanged")}
                     | {"failure": result.get("failure"), "cli": result.get("cli"),
                        "rejected_observation": result.get("rejected_observation"), "native_ingress": result.get("native_ingress"),
                        "engine_stop": result.get("engine_stop"), "proof": directory.relative_to(ROOT).as_posix()}), flush=True)
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--claude-exe", required=True, type=Path)
    parser.add_argument("--run", action="store_true")
    args = parser.parse_args()
    assert args.run, "explicit_run_required"
    raise SystemExit(run(args.claude_exe))

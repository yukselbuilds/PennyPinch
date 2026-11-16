"""Observe actual Codex tool-result framing using an owned scripted loopback endpoint."""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import platform
import runpy
import subprocess
import sys
import threading
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
helpers = runpy.run_path(str(HERE / "replay.py"))
require, sha, write_new = (helpers[name] for name in ("require", "sha", "write_new"))
MARKER = "LOCAL_PROTOCOL_PROBE_COMPLETE"


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        server = self.server
        size = int(self.headers.get("Content-Length", "0"))
        if self.path != "/v1/responses" or not 0 < size <= server.limit or self.headers.get("Authorization"):
            self.send_error(400)
            return
        body = json.loads(self.rfile.read(size))
        with server.lock:
            server.requests.append(body)
            turn = len(server.requests)
        call = {"type": "function_call", "id": "fc_local_probe", "call_id": "call_local_probe",
                "name": "exec_command", "arguments": json.dumps(server.arguments)}
        item = call if turn == 1 else {
            "type": "message", "id": "msg_local_probe", "role": "assistant", "status": "completed",
            "content": [{"type": "output_text", "text": MARKER, "annotations": []}],
        }
        response = {"id": "resp_local_" + str(turn), "object": "response", "status": "completed",
                    "model": "local-protocol-fixture", "output": [item],
                    "usage": {"input_tokens": 0, "output_tokens": 0, "total_tokens": 0}}
        events = [("response.created", {"response": {**response, "status": "in_progress", "output": []}}),
                  ("response.output_item.done", {"output_index": 0, "item": item}),
                  ("response.completed", {"response": response})]
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Connection", "close")
        self.end_headers()
        for name, value in events:
            data = json.dumps({"type": name, **value}).encode()
            self.wfile.write(b"event: " + name.encode() + b"\ndata: " + data + b"\n\n")
            self.wfile.flush()
        self.close_connection = True


def observe(cli, python, payload, supervisor, directory, case, plan):
    directory.mkdir()
    workspace, home, temporary = (directory / name for name in ("workspace", "codex-home", "temp"))
    for path in (workspace, home, temporary):
        path.mkdir()
    source = workspace / "owned-logs.txt"
    source.write_text("INFO owned protocol probe amount=18446744073709551617\n" * plan["log_lines"], encoding="ascii")
    source_before = sha(source)
    command = f"Get-Content -LiteralPath '{source}'"
    if case == "stderr_failed_read":
        command += f",'{workspace / 'missing-fixture.txt'}'"
    arguments = {"cmd": command, "workdir": str(workspace), "login": False,
                 "shell": str(Path(os.environ["SystemRoot"]) / "System32/WindowsPowerShell/v1.0/powershell.exe"),
                 "max_output_tokens": 128 if case == "truncated_output" else 12000, "yield_time_ms": plan["tool_yield_ms"]}
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    server.arguments, server.requests, server.lock = arguments, [], threading.Lock()
    server.limit = plan["limits"]["request_bytes"]
    threading.Thread(target=server.serve_forever, daemon=True).start()
    environment = {key: value for key, value in os.environ.items() if not key.upper().startswith(
        ("OPENAI_", "ANTHROPIC_", "CODEX_", "CLAUDE_", "OPENCODE_", "HEADROOM_", "OTEL_"))}
    environment.update(CODEX_HOME=str(home), TEMP=str(temporary), TMP=str(temporary))
    config = {"model_provider": '"fixture"', "model_providers.fixture.name": '"Owned protocol fixture"',
              "model_providers.fixture.base_url": json.dumps(f"http://127.0.0.1:{server.server_port}/v1"),
              "model_providers.fixture.wire_api": '"responses"', "model_providers.fixture.requires_openai_auth": "false",
              "analytics.enabled": "false", "feedback.enabled": "false", "otel.exporter": '"none"',
              "otel.metrics_exporter": '"none"', "web_search": '"disabled"',
              "windows.sandbox": json.dumps(plan["windows_sandbox"]), "approval_policy": json.dumps(plan["approval_policy"])}
    command = [str(cli), "exec", "--ignore-user-config", "--ephemeral", "--skip-git-repo-check",
               "--sandbox", plan["sandbox"], "--json", "--color", "never", "-C", str(workspace),
               "--model", "local-protocol-fixture"]
    for key, value in config.items():
        command.extend(["-c", key + "=" + value])
    command.append("Run the owned protocol probe. Return its completion marker.")
    job, child = supervisor.KillOnCloseJob(), None
    started = time.monotonic()
    try:
        child = subprocess.Popen(command, cwd=workspace, env=environment, stdin=subprocess.DEVNULL,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE, creationflags=subprocess.CREATE_NO_WINDOW)
        job.assign(child.pid)
        stdout, stderr = child.communicate(timeout=plan["limits"]["cli_seconds"])
        require(child.returncode == plan["expected"]["cli_exit"] and MARKER.encode() in stdout, "cli_probe_not_completed")
        require(len(server.requests) == plan["expected"]["posts_per_run"], "unexpected_client_request_count")
        first, second = server.requests
        definitions = [child for tool in first["tools"] for child in tool.get("tools", [tool])]
        schemas = [tool["parameters"]["properties"] for tool in definitions if tool.get("name") == "exec_command"]
        require(len(schemas) == 1 and set(arguments) <= schemas[0].keys(), "client_tool_schema_changed")
        calls = [item for item in second["input"] if item.get("type") == "function_call" and item.get("call_id") == "call_local_probe"]
        outputs = [item for item in second["input"] if item.get("type") == "function_call_output" and item.get("call_id") == "call_local_probe"]
        require(len(calls) == len(outputs) == 1 and json.loads(calls[0]["arguments"]) == arguments, "tool_identity_changed")
        output = outputs[0]["output"]
        require(isinstance(output, str), "unexpected_client_output_type")
        count = output.count("INFO owned protocol probe")
        truncated = case == "truncated_output"
        if not truncated and count != plan["log_lines"]:
            print(json.dumps({"diagnostic": "client_output_incomplete", "policy_rejected": "Rejected" in output,
                              "log_lines": count, "truncated": "truncat" in output.lower(),
                              "still_running": "running" in output.lower(), "output_bytes": len(output.encode())}), flush=True)
        require((count < plan["log_lines"] and "truncat" in output.lower()) if truncated else count == plan["log_lines"], "client_log_output_incomplete")
        require(("missing-fixture.txt" in output) == (case == "stderr_failed_read"), "stderr_observation_changed")
        if not truncated:
            require([line for line in output.splitlines() if line.startswith("INFO ")]
                    == source.read_text(encoding="ascii").splitlines(), "exact_log_value_changed")
        require(sha(source) == source_before, "read_only_fixture_changed")
        events = [json.loads(line) for line in stdout.splitlines() if line.startswith(b"{")]
        sessions = [event["thread_id"] for event in events if event.get("type") == "thread.started"]
        require(len(sessions) == 1, "cli_scope_unavailable")
        transcript = [{"type": "session_meta", "payload": {"id": sessions[0]}},
                      *[{"type": "response_item", "payload": item} for item in (calls[0], outputs[0])]]
        raw = b"\n".join(json.dumps(item).encode() for item in transcript) + b"\n"
        preview = subprocess.run([str(python), "-I", "-S", "-B", "-c",
            (ROOT / "runtime/learn_preview.py").read_text(encoding="utf-8"), "replay"],
            cwd=payload, env={"SystemRoot": os.environ["SystemRoot"]}, input=raw, capture_output=True, timeout=15)
        require(preview.returncode == 0 and not preview.stderr and len(preview.stdout) <= plan["limits"]["reply_bytes"], "local_replay_failed")
        result = json.loads(preview.stdout)
        report = result["report"]
        require(result["status"] == "trial_ready" and report["checked"] == report["protected"] == 1
                and report["eligible"] == report["compacted"] == report["ambiguous"] == 0, "merged_channel_not_protected")
        return {"case": case, "cli_exit": child.returncode, "posts": len(server.requests),
                "wire_output_type": "string", "log_lines_observed": count, "client_truncation": truncated,
                "stderr_in_same_text": case == "stderr_failed_read", "protected": 1, "eligible": 0,
                "source_before_sha256": source_before, "source_after_sha256": sha(source),
                "tool_schema_matched": True, "seconds": round(time.monotonic() - started, 3),
                "raw_wire_content_exported": False}
    finally:
        job.close()
        if child is not None and child.poll() is None:
            child.wait(timeout=15)
        server.shutdown()
        server.server_close()
        # This home was created exclusively for this probe; discard CLI private logs and state.
        require(home.resolve().is_relative_to(directory.resolve()), "private_state_scope_changed")
        for path in home.rglob("*"):
            require(path.resolve().is_relative_to(home.resolve()) and not path.is_symlink(), "private_state_path_changed")
            if path.is_file():
                path.unlink()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true", required=True)
    parser.add_argument("--codex", type=Path, required=True)
    args = parser.parse_args()
    require(os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_windows_workspace")
    cli = args.codex.resolve(strict=True)
    install = Path(os.environ["LOCALAPPDATA"]) / "OpenAI/Codex/bin"
    require(cli.is_relative_to(install.resolve(strict=True)) and cli.name == "codex.exe", "installed_cli_required")
    plan_path = HERE / "rules-client-plan.json"
    plan = json.loads(plan_path.read_bytes())
    version = subprocess.run([str(cli), "--version"], capture_output=True, timeout=10, check=True).stdout.decode().strip()
    require(version == plan["client_version"], "client_version_changed")
    controller = helpers["load_module"]("rules_client_controller", ROOT / "runtime/desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(ROOT / "runtime/manifest.json")
    python = payload / "python.exe"
    require(sha(Path(sys.executable)) == sha(python) == manifest["engine"]["python_sha256"], "pinned_python_required")
    supervisor = helpers["load_module"]("rules_client_supervisor", payload / manifest["engine"]["runner"])
    sources = [Path(__file__), plan_path, HERE / "replay.py", HERE / "fixtures.py", ROOT / "AGENTS.md", ROOT / "runtime/learn_preview.py",
               ROOT / "runtime/manifest.json", ROOT / "runtime/desktop_controller.py", payload / "runtime-manifest.json",
               payload / manifest["engine"]["runner"], python, payload / "python313.dll",
               payload / "Lib/site-packages/headroom/transforms/lossless_compaction.py", cli,
               Path(os.environ["SystemRoot"]) / "System32/WindowsPowerShell/v1.0/powershell.exe"]
    sources.extend(path for path in (ROOT / ".codex/config.toml", ROOT.parent / "AGENTS.md", ROOT / "runtime/optimization-sources.json") if path.is_file())
    snapshot = lambda: {str(path): sha(path) for path in sources}
    before = snapshot()
    proof_root = ROOT / "work/proofs/optimization/rules-client"
    require(proof_root.resolve().is_relative_to(ROOT), "proof_scope_changed")
    directory = proof_root / uuid.uuid4().hex
    directory.mkdir(parents=True)
    write_new(directory / "frozen-plan.json", plan)
    results, failure = [], None
    try:
        for repeat in range(plan["repetitions"]):
            for case in plan["cases"]:
                row = observe(cli, python, payload, supervisor, directory / f"{repeat}-{case}", case, plan)
                results.append({"repeat": repeat + 1, **row})
                print(json.dumps(results[-1]), flush=True)
    except Exception as error:
        failure = str(error) if isinstance(error, AssertionError) else type(error).__name__
    after = snapshot()
    passed = before == after and failure is None and len(results) == plan["repetitions"] * len(plan["cases"])
    record = {"passed": passed, "failure": failure, "stage": plan["stage"], "os": platform.platform(),
              "client_version": version, "rows": results, "before": before, "after": after,
              "provider_model_calls": 0, "live_task_quality": "not_tested", "provider_usage": "not_measured",
              "rule_applicability": "no_eligible_trusted_stdout_in_observed_client_text" if passed else "unknown",
              "raw_wire_content_exported": False, "cli_private_state_removed": True}
    write_new(directory / "result.json", record)
    print(json.dumps({"passed": passed, "failure": failure, "proof": str((directory / "result.json").relative_to(ROOT))}), flush=True)
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

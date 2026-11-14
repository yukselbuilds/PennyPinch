"""Opt-in native Headroom/OpenCode foreground task and model-change acceptance."""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import threading
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


bench = load("subagent_opencode_bench", Path(__file__).with_name("roundtrip.py"))
fixture = load("native_subagent_fixture", ROOT / "tests/fixtures/opencode/subagent_fixture.py")


def configuration():
    config = bench.configuration(0, ROOT / "unused-fixture-receipt.json", "chat")
    config["mcp"]["headroom_fixture"]["command"] = [sys.executable, "-I", "-B", str(Path(__file__).resolve()), "--mcp"]
    config["provider"]["lab"]["options"]["baseURL"] = "https://api.openai.com/v1"
    models = config["provider"]["lab"]["models"]
    for case in ("switch", "mixed"):
        _, selector, model = fixture.CASES[case]["child"]
        models[selector.split("/", 1)[1]] = {"id": model, "name": "Synthetic fixture", "tool_call": True}
    responses = bench.configuration(0, ROOT / "unused-fixture-receipt.json", "responses")
    config["provider"]["openai"] = responses["provider"]["openai"]
    config["provider"]["openai"]["options"]["baseURL"] = "https://api.openai.com/v1"
    config["enabled_providers"] = ["lab", "openai"]
    config["tools"].pop("task")
    config["permission"].update(headroom_retrieve="allow", task={"*": "deny", "headroom_inherit": "allow", "headroom_other": "allow"})
    child_permission = {"*": "deny", bench.fixture.CLIENT_TOOL: "allow", "headroom_retrieve": "allow", "task": "deny"}
    config["agent"] = {name: {"disable": True} for name in ("build", "plan", "explore", "general")}
    config["agent"].update({
        "headroom_primary": {"mode": "primary", "steps": 8, "description": "Synthetic acceptance parent"},
        "headroom_inherit": {"mode": "subagent", "steps": 6, "description": "Read fixed synthetic data only", "permission": child_permission},
        "headroom_other": {"mode": "subagent", "steps": 6, "model": "lab/child",
                           "description": "Read fixed synthetic data with explicit model", "permission": child_permission},
    })
    config.update(default_agent="headroom_primary", subagent_depth=1, compaction={"auto": False})
    return config


def cli_call(command, workspace, env, job, timeout):
    process = subprocess.Popen(command, cwd=workspace, env=env, stdin=subprocess.DEVNULL,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, creationflags=subprocess.CREATE_NO_WINDOW)
    job.assign(process.pid)
    output, errors = bench.common.BoundedOutput(process.stdout), bench.common.BoundedOutput(process.stderr)
    try:
        process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        process.terminate()
        process.wait(timeout=5)
        raise AssertionError("bounded_cli_timeout") from None
    stdout, stderr = output.finish(), errors.finish()
    assert process.returncode == 0, "native_cli_exit_failed"
    return stdout, stderr


def assistant_models(exported):
    return [(item["info"]["providerID"], item["info"]["modelID"], item["info"]["agent"])
            for item in exported["messages"] if item["info"]["role"] == "assistant"]


def validate_sessions(case, parent, child, task, parent_id):
    expected = fixture.CASES[case]
    metadata = task["state"]["metadata"]
    child_id = metadata["sessionId"]
    assert child_id != parent_id and parent["info"]["id"] == parent_id, "session_identity_changed"
    assert metadata["parentSessionId"] == parent_id, "task_parent_changed"
    assert child["info"]["id"] == child_id and child["info"]["parentID"] == parent_id, "child_parent_changed"
    provider, model = expected["child"][1].split("/", 1)
    assert metadata["model"] == {"providerID": provider, "modelID": model}, "task_selector_changed"
    child_models = assistant_models(child)
    assert child_models and all(row == (provider, model, expected["agent"]) for row in child_models), "child_model_agent_changed"
    parent_models = assistant_models(parent)
    assert parent_models and parent_models[-1] == (*expected["parent"][1].split("/", 1), "headroom_primary"), "parent_model_changed"
    tools = [part for item in child["messages"] for part in item["parts"] if part["type"] == "tool"]
    expected_calls = [(fixture.shared.CLIENT_TOOL, "logs"), (fixture.shared.CLIENT_TOOL, "witness")]
    if expected["child"][0] == "chat":
        expected_calls.insert(1, ("headroom_retrieve", "retrieve"))
    assert [(part["tool"], part["callID"]) for part in tools] == [
        (name, f"call_{case}_child_{kind}") for name, kind in expected_calls], "child_tools_changed"
    assert all(part["state"]["status"] == "completed" for part in tools), "child_tool_not_completed"
    texts = [part["text"] for item in child["messages"] if item["info"]["role"] == "assistant"
             for part in item["parts"] if part["type"] == "text"]
    assert texts == [f"{fixture.marker(case, 'child')}_OK"], "child_export_final_changed"
    return {"parent_session_id": parent_id, "child_session_id": child_id, "child_parent_id": child["info"]["parentID"],
            "parent_selector": expected["parent"][1], "child_selector": expected["child"][1],
            "child_agent": expected["agent"], "child_assistant_messages": len(child_models),
            "native_child_tools": [{"name": part["tool"], "call_id": part["callID"], "status": part["state"]["status"]} for part in tools],
            "child_final_exact": True}


def run_cases(server, workspace, env, job, config_path, owner_path, directory):
    rows, parent_id = {}, None
    saved_config, saved_owner = bench.common.sha(config_path), bench.common.sha(owner_path)
    for case, expected in fixture.CASES.items():
        command = [str(bench.CLI), "run", "--pure", "--format", "json", "--agent", "headroom_primary",
                   "--model", expected["parent"][1], "--title", "H010 synthetic foreground task", "--dir", str(workspace)]
        if case == "switch":
            assert parent_id, "parent_session_required"
            command.extend(["--session", parent_id])
        command.append(fixture.marker(case, "parent"))
        stdout, stderr = cli_call(command, workspace, env, job, 120)
        events = [json.loads(line) for line in stdout.splitlines() if line.strip()]
        assert not stderr and not any(event.get("type") == "error" for event in events), "native_cli_error_event"
        texts = [event["part"].get("text") for event in events if event.get("type") == "text"]
        assert texts == [f"{fixture.marker(case, 'parent')}_OK"], "parent_final_changed"
        tools = [event["part"] for event in events if event.get("type") == "tool_use"]
        assert tools and all(tool["state"]["status"] == "completed" for tool in tools), "parent_tool_not_completed"
        tasks = [tool for tool in tools if tool["tool"] == "task"]
        assert len(tasks) == 1, "exactly_one_foreground_child_required"
        task = tasks[0]
        current_id = task["sessionID"]
        assert case != "switch" or current_id == parent_id, "model_switch_created_new_parent"
        if case == "same":
            parent_id = current_id
        child_id = task["state"]["metadata"]["sessionId"]
        exported = []
        for session_id in (current_id, child_id):
            raw, export_stderr = cli_call([str(bench.CLI), "export", session_id], workspace, env, job, 30)
            assert len(export_stderr) < 512, "export_error_output"
            exported.append(json.loads(raw))
        row = validate_sessions(case, *exported, task, current_id)
        row.update(exit_code=0, stderr_bytes=0, parent_final_exact=True,
                   parent_tools=[{"name": tool["tool"], "call_id": tool["callID"], "status": tool["state"]["status"]} for tool in tools],
                   upstream={role: server.conversations[case, role].rows for role in ("parent", "child")})
        assert all(server.conversations[case, role].complete for role in ("parent", "child")), "incomplete_role"
        assert bench.common.sha(config_path) == saved_config and bench.common.sha(owner_path) == saved_owner, "connection_changed_during_cli"
        assert not server.errors, "upstream_fixture_error"
        row.update(saved_config_and_owner_unchanged=True, passed=True)
        rows[case] = row
        with (directory / (case + ".json")).open("x", encoding="utf-8") as stream:
            json.dump(row, stream, indent=2)
            stream.write("\n")
        print(json.dumps({"case": case, "passed": True, "parent": current_id, "child": child_id,
                          "selectors": [row["parent_selector"], row["child_selector"]]}), flush=True)
    return rows


def remove_connection(exe, state, workspace, env):
    """Use the installed native keep-state Undo entry after the GUI exits normally."""
    import ctypes
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.CreateFileW.argtypes = [ctypes.c_wchar_p, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_void_p,
                                  ctypes.c_uint32, ctypes.c_uint32, ctypes.c_void_p]
    kernel.CreateFileW.restype = ctypes.c_void_p
    kernel.CloseHandle.argtypes = [ctypes.c_void_p]
    gate = state.with_name(state.name + ".desktop-use")
    handle = kernel.CreateFileW(str(gate), 0x80000000, 0, None, 3, 0x00200000, None)
    assert handle != ctypes.c_void_p(-1).value, "exclusive_fixture_gate_unavailable"
    try:
        result = subprocess.run([str(exe), "--headroom-uninstall", "keep"], cwd=workspace, env=env,
                                stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                creationflags=subprocess.CREATE_NO_WINDOW, timeout=30)
        assert result.returncode == 0, "native_undo_entry_failed"
        return {"exit_code": result.returncode, "keep_state": state.is_dir()}
    finally:
        kernel.CloseHandle(handle)


def run_native():
    assert os.name == "nt" and Path.cwd().resolve() == ROOT, "windows_project_root_required"
    accepted = json.loads((ROOT / "docs/proofs/opencode-retrieval.json").read_text())
    assert accepted["accepted"] and bench.common.sha(bench.CLI) == bench.CLI_SHA256
    exe_ref = accepted["fingerprints"]["release_binary"]
    exe = ROOT / exe_ref["path"]
    assert bench.common.sha(exe) == exe_ref["sha256"], "accepted_native_executable_required"
    qa = load("subagent_quality", ROOT / "work/proofs/codex-current/quality.py")
    quality = json.loads((ROOT / accepted["evidence"]["local_quality_gate"]["path"]).read_text())
    current = qa.fingerprint()
    assert all(current.get(name) == digest for name, digest in quality["after"].items()), "product_gate_inputs_changed"
    package_ref = accepted["fingerprints"]["staged_package"]
    assert bench.common.sha(ROOT / package_ref["path"]) == package_ref["sha256"], "staged_package_manifest_changed"
    source_manifest = json.loads((ROOT / "work/research/h010-subagents/manifest.json").read_text())
    assert all(bench.common.sha(ROOT / "work/research/h010-subagents" / name) == row["sha256"]
               for name, row in source_manifest["files"].items()), "pinned_source_changed"
    supervisor = load("subagent_owned_job", ROOT / "work/proofs/packaging/headroom_runtime.py")
    replay = load("subagent_replay", ROOT / "bench/optimization/replay.py")
    identifier = "dev.headroom.desktop.h010-ce8ea443ed"
    state = Path(os.environ["LOCALAPPDATA"]) / identifier
    owner_path = state / "ownership/opencode_v1.json"
    assert json.loads(owner_path.read_text())["state"] == "released", "previous_fixture_connection_still_active"
    run = ROOT / "work/proofs/opencode-subagents/runs" / uuid.uuid4().hex[:12]
    run.mkdir(parents=True)
    workspace = run / "project"
    workspace.mkdir()
    (workspace / ".git").mkdir()
    client = state / ("test-subagents-" + run.name)
    config_path = client / "xdg-config/opencode/opencode.jsonc"
    config_path.parent.mkdir(parents=True)
    original = ("// Disposable OpenCode native subagent acceptance\n" + json.dumps(configuration(), indent=2) + "\n").encode()
    config_path.write_bytes(original)
    settings = state / "runtime/workspace/settings.json"
    original_settings = settings.read_bytes()
    endpoint = state / "runtime/connection-endpoint.json"
    server = fixture.Upstream()
    threading.Thread(target=server.serve_forever, daemon=True).start()
    env = bench.environment(run, {}, server.server_port)
    for name in ("OPENCODE_CONFIG", "OPENCODE_CONFIG_CONTENT", "OPENCODE_CONFIG_DIR"):
        env.pop(name, None)
    env["XDG_CONFIG_HOME"] = str(client / "xdg-config")
    env["PATH"] += os.pathsep + r"C:\Program Files\Git\cmd"
    gui_env = env.copy()
    for name in ("USERPROFILE", "APPDATA", "LOCALAPPDATA", "HOMEDRIVE", "HOMEPATH"):
        if name in os.environ:
            gui_env[name] = os.environ[name]
        else:
            gui_env.pop(name, None)
    gui_env.update(CODEX_HOME=str(run / "client-home/codex"), CLAUDE_CONFIG_DIR=str(run / "client-home/claude"))
    paths = [Path(__file__), ROOT / "tests/fixtures/opencode/subagent_fixture.py", exe, bench.CLI,
             ROOT / "work/research/h010-subagents/manifest.json", ROOT / package_ref["path"], Path(sys.executable),
             exe.parent / "runtime/payload/Lib/site-packages/headroom/ccr/response_handler.py"]
    paths.extend((ROOT / "work/research/h010-subagents").glob("*.ts"))
    def fingerprint():
        state_files = {"fixture-state/" + path.relative_to(state).as_posix(): bench.common.sha(path)
                       for path in (settings, config_path)}
        return {**qa.fingerprint(), **replay.fingerprints(paths), **state_files}
    before = fingerprint()
    proof = {"task": "H010-c", "passed": False, "provider": "local_fixture", "auth": "synthetic",
             "before": before, "parent_task_accepted": False, "token_and_billing_savings": "not_measured",
             "windows_build": list(sys.getwindowsversion()[:3]), "python": sys.version,
             "product_quality_gate_reused": len(quality["after"]), "raw_transcript_retained": False}
    job, gui = supervisor.KillOnCloseJob(), None
    try:
        settings.write_text(json.dumps({"openai_base_url": f"http://127.0.0.1:{server.server_port}/v1"}) + "\n")
        gui = subprocess.Popen([str(exe)], cwd=workspace, env=gui_env, stdout=subprocess.DEVNULL,
                               stderr=subprocess.DEVNULL, creationflags=subprocess.CREATE_NO_WINDOW)
        job.assign(gui.pid)
        active = {"pid": gui.pid, "run": str(run), "title": "Headroom H010-b3 ce8ea443ed", "config_path": str(config_path)}
        (run.parent.parent / "active-native.json").write_text(json.dumps(active) + "\n")
        print(json.dumps(active), flush=True)
        deadline = time.monotonic() + 900
        attempted = False
        while gui.poll() is None and time.monotonic() < deadline:
            if not attempted and (run / "run-all").exists():
                attempted = True
                try:
                    assert b"retrieval_bridge.py" in config_path.read_bytes(), "native_bridge_apply_required"
                    proof["cases"] = run_cases(server, workspace, env, job, config_path, owner_path, run)
                except Exception as error:
                    proof["failure"] = str(error) if isinstance(error, AssertionError) else type(error).__name__
                    proof["cases"] = {case: json.loads((run / (case + ".json")).read_text())
                                      for case in fixture.CASES if (run / (case + ".json")).is_file()}
                (run / "cases-result.json").write_text(json.dumps(proof, indent=2) + "\n")
                print(json.dumps({"cases_finished": True, "cases": len(proof.get("cases", {})), "failure": proof.get("failure"),
                                  "upstream_errors": server.errors}), flush=True)
            time.sleep(.2)
        if gui.poll() == 0 and json.loads(owner_path.read_text())["state"] == "active":
            proof["native_undo"] = remove_connection(exe, state, workspace, gui_env)
        proof.update(gui_exit_code=gui.poll(), config_restored=config_path.read_bytes() == original,
                     owner_released=json.loads(owner_path.read_text())["state"] == "released", upstream_errors=server.errors,
                     blocked_external_connects=server.blocked_connects)
        if endpoint.is_file():
            with socket.socket() as check:
                check.settimeout(.5)
                proof["owned_listener_closed"] = check.connect_ex(("127.0.0.1", json.loads(endpoint.read_text())["port"])) != 0
        proof["passed"] = (len(proof.get("cases", {})) == 3 and not proof.get("failure") and gui.poll() == 0
                           and proof["config_restored"] and proof["owner_released"] and proof.get("owned_listener_closed") is True)
    finally:
        if gui is not None and gui.poll() is None:
            proof["forced_gui_fallback"] = True
            proof["passed"] = False
        job.close()
        if gui is not None:
            gui.wait(timeout=15)
        settings.write_bytes(original_settings)
        server.shutdown()
        server.server_close()
        proof["after"] = fingerprint()
        proof["inputs_unchanged"] = proof["before"] == proof["after"]
        proof["passed"] = proof["passed"] and proof["inputs_unchanged"]
        replay.write_new(run / "result.json", proof)
        print(json.dumps({"passed": proof["passed"], "proof": str(run / "result.json")}), flush=True)
    return 0 if proof["passed"] else 1


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--run", action="store_true")
    parser.add_argument("--mcp", action="store_true")
    args = parser.parse_args()
    if args.mcp:
        fixture.serve_mcp()
    else:
        assert args.run, "explicit_run_required"
        raise SystemExit(run_native())

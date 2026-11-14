"""Opt-in full Windows TUI acceptance, using the existing native fixture lifecycle."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
import importlib.util
spec = importlib.util.spec_from_file_location("tui_native_shared", Path(__file__).with_name("subagents.py"))
native = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = native
spec.loader.exec_module(native)
fixture = native.fixture
BASE = ROOT / "work/proofs/opencode-tui"
MODEL_NAMES = {"same": "Headroom base fixture", "switch": "Headroom switch fixture", "mixed": "Headroom Responses fixture"}


def configuration():
    config = native.configuration()
    config["agent"]["title"] = {"disable": True}
    for case, label in MODEL_NAMES.items():
        provider, model = fixture.CASES[case]["parent"][1].split("/", 1)
        config["provider"][provider]["models"][model]["name"] = label
    config["provider"]["lab"]["models"]["child"]["name"] = "Headroom child fixture"
    return config


def validate_exports(case, parent, child):
    prefix = "call_" + case + "_parent_"
    tools = [part for message in parent["messages"] for part in message["parts"]
             if part["type"] == "tool" and part["callID"].startswith(prefix)]
    expected = [(fixture.shared.CLIENT_TOOL, "logs"), (fixture.shared.CLIENT_TOOL, "witness"), ("task", "task")]
    if fixture.CASES[case]["parent"][0] == "chat":
        expected.insert(1, ("headroom_retrieve", "retrieve"))
    assert [(part["tool"], part["callID"]) for part in tools] == [(name, prefix + kind) for name, kind in expected], "tui_parent_tools_changed"
    assert all(part["state"]["status"] == "completed" for part in tools), "tui_parent_tool_incomplete"
    task = next(part for part in tools if part["tool"] == "task")
    parent_id = parent["info"]["id"]
    row = native.validate_sessions(case, parent, child, task, parent_id)
    user_texts = [part["text"] for message in parent["messages"] if message["info"]["role"] == "user"
                  for part in message["parts"] if part["type"] == "text"]
    assert any(text.strip() == fixture.marker(case, "parent") for text in user_texts), "terminal_prompt_missing"
    assistant_texts = [part["text"] for message in parent["messages"] if message["info"]["role"] == "assistant"
                       for part in message["parts"] if part["type"] == "text"]
    assert assistant_texts and assistant_texts[-1] == fixture.marker(case, "parent") + "_OK", "tui_parent_final_changed"
    row.update(parent_final_exact=True, terminal_prompt_exported=True,
               parent_tools=[{"name": part["tool"], "call_id": part["callID"], "status": part["state"]["status"]} for part in tools])
    return row


def verify_cases(server, workspace, env, job, config_path, owner_path, directory):
    config_before, owner_before = native.bench.common.sha(config_path), native.bench.common.sha(owner_path)
    rows, parent_id = {}, None
    def exported(session_id):
        raw, errors = native.cli_call([str(native.bench.CLI), "export", session_id], workspace, env, job, 30)
        assert len(errors) < 512, "tui_export_error_output"
        return json.loads(raw)
    for case in fixture.CASES:
        receipt = directory / ("terminal-" + case + ".json")
        deadline = time.monotonic() + 200
        while not receipt.is_file() and time.monotonic() < deadline:
            time.sleep(.2)
        assert receipt.is_file(), "terminal_receipt_timeout"
        terminal = json.loads(receipt.read_text())
        assert terminal["exit_code"] == 0 and not terminal["forced"], "tui_exit_failed"
        assert terminal["job_bound"], "tui_owned_process_job_required"
        assert terminal["before"] == terminal["after"] and terminal["before"]["tty"] == [True, True, True], "tui_terminal_or_input_changed"
        assert terminal["full_tui"] and terminal["selector"] == fixture.CASES[case]["parent"][1], "tui_mode_or_selector_changed"
        raw, errors = native.cli_call([str(native.bench.CLI), "session", "list", "--format", "json", "--max-count", "4"], workspace, env, job, 30)
        assert not errors, "tui_session_list_error"
        roots = {item["id"] for item in json.loads(raw)}
        if case == "same":
            assert len(roots) == 1, "unexpected_initial_root_count"
            parent_id = next(iter(roots))
            selected = parent_id
        elif case == "switch":
            assert roots == {parent_id} and terminal["resume_session_id"] == parent_id, "tui_model_switch_created_new_session"
            selected = parent_id
        else:
            assert parent_id in roots and len(roots) == 2, "unexpected_mixed_root_count"
            selected = next(iter(roots - {parent_id}))
        parent = exported(selected)
        task = next(part for message in parent["messages"] for part in message["parts"]
                    if part["type"] == "tool" and part["callID"] == f"call_{case}_parent_task")
        child = exported(task["state"]["metadata"]["sessionId"])
        row = validate_exports(case, parent, child)
        assert all(server.conversations[case, role].complete for role in ("parent", "child")), "tui_role_incomplete"
        assert not server.errors, "tui_upstream_fixture_failed"
        assert native.bench.common.sha(config_path) == config_before and native.bench.common.sha(owner_path) == owner_before, "tui_saved_connection_changed"
        row.update(passed=True, terminal=terminal, saved_config_and_owner_unchanged=True,
                   upstream={role: server.conversations[case, role].rows for role in ("parent", "child")})
        with (directory / (case + ".json")).open("x", encoding="utf-8") as stream:
            json.dump(row, stream, indent=2)
            stream.write("\n")
        rows[case] = row
        print(json.dumps({"case": case, "passed": True, "surface": "full_tui", "parent": selected,
                          "child": row["child_session_id"]}), flush=True)
    return rows


def terminal(case):
    assert os.name == "nt" and Path.cwd().resolve() == ROOT
    assert all(os.isatty(fd) for fd in (0, 1, 2)), "real_terminal_required"
    active = json.loads((BASE / "active-native.json").read_text())
    directory = Path(active["run"]).resolve()
    assert directory.parent == BASE / "runs" and directory.is_dir(), "owned_tui_fixture_required"
    workspace = directory / "project"
    config_path = Path(active["config_path"])
    state = Path(os.environ["LOCALAPPDATA"]) / "dev.headroom.desktop.h010-ce8ea443ed"
    assert config_path.is_relative_to(state / ("test-subagents-" + directory.name)), "owned_client_config_required"
    owner_path = state / "ownership/opencode_v1.json"
    assert b"retrieval_bridge.py" in config_path.read_bytes(), "native_ui_apply_required"
    assert native.bench.common.sha(native.bench.CLI) == native.bench.CLI_SHA256
    env = native.bench.environment(directory, {}, active["upstream_port"])
    for name in ("OPENCODE_CONFIG", "OPENCODE_CONFIG_CONTENT", "OPENCODE_CONFIG_DIR"):
        env.pop(name, None)
    env["XDG_CONFIG_HOME"] = str(config_path.parents[1])
    env["PATH"] += os.pathsep + r"C:\Program Files\Git\cmd"
    resume = None
    command = [str(native.bench.CLI), str(workspace), "--pure", "--model", fixture.CASES[case]["parent"][1], "--agent", "headroom_primary"]
    if case == "switch":
        resume = json.loads((directory / "same.json").read_text())["parent_session_id"]
        command.extend(["--session", resume])
    def fingerprint():
        return {"tty": [os.isatty(fd) for fd in (0, 1, 2)], "cli_sha256": native.bench.common.sha(native.bench.CLI),
                "launcher_sha256": native.bench.common.sha(Path(__file__)), "config_sha256": native.bench.common.sha(config_path),
                "owner_sha256": native.bench.common.sha(owner_path)}
    before = fingerprint()
    print(json.dumps({"surface": "full_tui", "case": case, "manual_prompt": fixture.marker(case, "parent"),
                      "select_in_tui": MODEL_NAMES[case], "tty": before["tty"]}), flush=True)
    supervisor = native.load("tui_owned_terminal_job", ROOT / "work/proofs/packaging/headroom_runtime.py")
    job = supervisor.KillOnCloseJob()
    process = None
    started = time.monotonic()
    forced, job_bound = False, False
    try:
        process = subprocess.Popen(command, cwd=workspace, env=env)
        job.assign(process.pid)
        job_bound = True
        process.wait(timeout=180)
    finally:
        if process is not None and process.poll() is None:
            forced = True
            process.terminate()
            process.wait(timeout=10)
        job.close()
        result = {"case": case, "full_tui": True, "selector": fixture.CASES[case]["parent"][1],
                  "model_selection": "tui_model_dialog",
                  "resume_session_id": resume, "exit_code": process.returncode if process else None, "forced": forced, "job_bound": job_bound,
                  "duration_seconds": round(time.monotonic() - started, 3), "before": before, "after": fingerprint()}
        with (directory / ("terminal-" + case + ".json")).open("x", encoding="utf-8") as stream:
            json.dump(result, stream, indent=2)
            stream.write("\n")
    # The native renderer owns console cleanup; avoid flushing its inherited handles again.
    os._exit(0 if process.returncode == 0 and not forced else 1)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--run", action="store_true")
    parser.add_argument("--case", choices=tuple(fixture.CASES))
    args = parser.parse_args()
    if args.case:
        terminal(args.case)
    else:
        assert args.run, "explicit_run_required"
        extra = [Path(__file__), ROOT / "work/proofs/packaging/headroom_runtime.py"]
        for filename in ("manifest.json", "interaction-manifest.json"):
            manifest_path = ROOT / "work/research/h010-tui" / filename
            extra.append(manifest_path)
            manifest = json.loads(manifest_path.read_text())
            for name, row in manifest["files"].items():
                path = manifest_path.parent / name
                assert native.bench.common.sha(path) == row["sha256"], "pinned_tui_source_changed"
                extra.append(path)
        raise SystemExit(native.run_native(task="H010-d", configuration_factory=configuration,
                                          case_runner=verify_cases, extra_sources=extra))

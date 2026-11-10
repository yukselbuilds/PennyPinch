"""Observe explicit PowerShell use and isolated RTK hook contracts on Windows."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import uuid

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("rtk_replay_for_surfaces", HERE / "replay.py")
replay = importlib.util.module_from_spec(spec)
spec.loader.exec_module(replay)
ROOT = replay.ROOT
base = replay.base


def exercise(run, git, rtk, shell, node, plugin, user_paths):
    env = replay.environment(run, git)
    workspace = run / "workspace with spaces"
    replay.fixtures.create(workspace, env, git)
    claude = run / "claude-fixture-config"
    claude.mkdir()
    env.update(CLAUDE_CONFIG_DIR=str(claude), HEADROOM_RTK_FIXTURE_BIN=str(rtk), HEADROOM_RTK_FIXTURE_GIT=str(git))

    def powershell(script):
        replay.require_no_user_rtk(user_paths)
        return replay.command([str(shell), "-NoLogo", "-NoProfile", "-NonInteractive", "-Command", script],
                              workspace, env, 30)

    original = powershell("& $env:HEADROOM_RTK_FIXTURE_GIT status; exit $LASTEXITCODE")
    filtered = powershell("& $env:HEADROOM_RTK_FIXTURE_BIN git status; exit $LASTEXITCODE")
    bypass = powershell("& $env:HEADROOM_RTK_FIXTURE_BIN bypass $env:HEADROOM_RTK_FIXTURE_GIT status; exit $LASTEXITCODE")
    base.require(original["exit_code"] == filtered["exit_code"] == 0 and original["stdout"] != filtered["stdout"]
                 and not original["stderr"] and not filtered["stderr"], "powershell_explicit_filter_not_observed")
    base.require(all(original[key] == bypass[key] for key in ("stdout", "stderr", "exit_code")), "powershell_bypass_changed")

    def hook(client, tool, tool_input, mode="default"):
        replay.require_no_user_rtk(user_paths)
        event = {"hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": tool_input,
                 "permission_mode": mode, "cwd": str(workspace)}
        result = subprocess.run([str(rtk), "hook", client], input=base.encoded(event), capture_output=True,
                                cwd=workspace, env=env, timeout=30, creationflags=subprocess.CREATE_NO_WINDOW)
        base.require(result.returncode == 0 and not result.stderr, "hook_fixture_failed")
        return json.loads(result.stdout)["hookSpecificOutput"] if result.stdout.strip() else None

    # Inspect returned arguments only. Native client authorization is outside this fixture.
    tool_input = {"command": "git status", "description": "Read owned fixture status"}
    codex = hook("codex", "Bash", tool_input)
    base.require(codex and codex["updatedInput"] == {**tool_input, "command": "rtk git status"}, "codex_bash_rewrite_absent")
    for tool in ("shell_command", "Read", "Grep"):
        base.require(hook("codex", tool, tool_input) is None, "codex_non_bash_rewrite")
    base.require(hook("codex", "Bash", tool_input, "unknown-fixture-mode") is None, "codex_unknown_mode_rewrite")

    claude_hook = hook("claude", "Bash", tool_input)
    base.require(claude_hook and claude_hook["updatedInput"]["command"] == "rtk git status"
                 and claude_hook.get("permissionDecision") != "allow", "claude_unattested_hook_result")
    base.require(hook("claude", "Read", {"file_path": "source.py"}) is None, "claude_read_rewrite")

    # These early plugin returns occur before locating RTK or reading host rules.
    js = ("import {tryRewriteCommand} from './rtk.ts';"
          "for (const tool of ['read','grep']) {if (await tryRewriteCommand(tool,'git status') !== null) process.exit(2)}"
          "process.env.RTK_DISABLED='1';"
          "for (const tool of ['bash','shell']) {if (await tryRewriteCommand(tool,'git status') !== null) process.exit(3)}")
    plugin_check = replay.command([str(node), "--input-type=module", "--eval", js], plugin.parent, env, 30)
    base.require(plugin_check["exit_code"] == 0, "opencode_plugin_gate_fixture_failed")
    return {"powershell_explicit_git_status": {"exit_codes_preserved": True, "bypass_byte_exact": True,
                                              "control": replay.metadata(original), "rtk": replay.metadata(filtered)},
            "codex_native_hook_fixture": {"bash_rewrite": True, "protocol_permission_decision": codex.get("permissionDecision"),
                                         "ignored_tools": ["shell_command", "Read", "Grep"], "unknown_permission_mode_passthrough": True},
            "claude_native_hook_fixture": {"bash_rewrite": True, "permission_decision": claude_hook.get("permissionDecision"),
                                          "native_read_without_command_passthrough": True},
            "opencode_plugin_fixture": {"ignored_tools": ["read", "grep"], "disabled_passthrough": ["bash", "shell"]}}


def main():
    base.require(os.name == "nt" and Path.cwd().resolve() == ROOT and ROOT == Path(r"D:\Projeler\Headroom"), "canonical_windows_root_required")
    base.require(len(sys.argv) == 3 and sys.argv[1] == "--benchmark-result", "explicit_benchmark_result_required")
    evidence = Path(sys.argv[2]).resolve(strict=True)
    base.require(evidence.is_relative_to(ROOT / "work/proofs/rtk/runs") and evidence.name == "result.json", "owned_result_required")
    benchmark = json.loads(evidence.read_text(encoding="utf-8"))
    base.require(benchmark["passed"] and benchmark["inputs_unchanged"] and not benchmark["diagnostic_only"], "complete_benchmark_required")
    plan = json.loads((HERE / "plan.json").read_text(encoding="utf-8"))
    toolchain = ROOT / "work/proofs/rtk/toolchain" / plan["version"]
    rtk = toolchain / "rtk.exe"
    plugin = toolchain / "source/hooks/opencode/rtk.ts"
    for path in (rtk, plugin, HERE / "plan.json", HERE / "replay.py", HERE / "fixtures.py", Path(sys.executable)):
        base.require(base.sha(path) == benchmark["after"][str(path)], "benchmark_input_changed")
    user_paths = replay.real_rtk_directories()
    replay.require_no_user_rtk(user_paths)
    git, node = (Path(shutil.which(name)).resolve(strict=True) for name in ("git.exe", "node.exe"))
    shell = Path(os.environ["SystemRoot"]) / "System32/WindowsPowerShell/v1.0/powershell.exe"
    run = ROOT / "work/proofs/rtk/surfaces" / str(uuid.uuid4())
    run.mkdir(parents=True)
    inputs = (Path(__file__).resolve(), HERE / "plan.json", HERE / "replay.py", HERE / "fixtures.py", Path(base.__file__),
              Path(sys.executable), evidence, rtk, plugin, git, node, shell)
    fingerprint = lambda: {str(path): base.sha(path) for path in inputs}
    before = fingerprint()
    result = {"passed": False}
    try:
        result["observations"] = exercise(run, git, rtk, shell, node, plugin, user_paths)
        result["passed"] = True
    except (AssertionError, OSError, ValueError, subprocess.TimeoutExpired) as error:
        result.update(error_class=type(error).__name__,
                      error_code=str(error) if isinstance(error, AssertionError) else "bounded_surface_fixture_failure")
    result.update(before=before, after=fingerprint(), windows_build=list(sys.getwindowsversion()[:3]),
                  scope="Explicit Windows PowerShell 5.1 invocation, RTK hook stdin and OpenCode plugin gates; no client launch or provider calls",
                  user_rtk_directories_absent=all(not os.path.lexists(path) for path in user_paths))
    result["inputs_unchanged"] = before == result["after"]
    result["passed"] = result["passed"] and result["inputs_unchanged"] and result["user_rtk_directories_absent"]
    base.write_new(run / "result.json", result)
    print(json.dumps({"passed": result["passed"], "run": run.relative_to(ROOT).as_posix(), "error_code": result.get("error_code")}))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

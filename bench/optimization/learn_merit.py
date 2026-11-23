"""Compare Learn previews with observed source changes and project guidance."""
import argparse
import json
import os
from pathlib import Path
import platform
import re
import site
import subprocess
import sys
import uuid

import runpy


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
replay = runpy.run_path(str(HERE / "replay.py"))
require, sha = replay["require"], replay["sha"]
CANARY = b"SYNTHETIC_LEARN_MERIT_PRIVATE_B712"
READ = "from pathlib import Path; print(Path('value.txt').read_text(encoding='utf-8'))"
CHECK_REVISION = """from pathlib import Path
revision = Path('revision.txt').read_text(encoding='ascii')
assert revision in ('a', 'b', 'c', 'ready')
print(Path('value-' + revision + '.txt').read_text(encoding='utf-8'))
"""


def run_child(python, cwd, environment, script, data=None):
    flags = ["-I", "-S", "-B", "-c", script]
    return subprocess.run([str(python), *flags], cwd=cwd, env=environment, input=data,
                          capture_output=True, timeout=15, creationflags=subprocess.CREATE_NO_WINDOW)


def observed_call(python, cwd, environment, script, input_names, *, success):
    def snapshot():
        return {name: sha(cwd / name) if (cwd / name).is_file() else None for name in input_names}

    before = snapshot()
    child = run_child(python, cwd, environment, script)
    after = snapshot()
    require(before == after, "read_only_fixture_mutated_source")
    require((child.returncode == 0 and child.stdout.strip() == CANARY and not child.stderr) if success
            else (child.returncode == 1 and not child.stdout and b"FileNotFoundError" in child.stderr),
            "unexpected_fixture_process_status")
    return {"cwd": str(cwd), "script": script, "exit": child.returncode,
            "output": (child.stdout + child.stderr).decode("utf-8"), "before": before, "after": after}


def observe_project_tasks(python, run, environment):
    approved, wrong, revised = (run / name for name in ("approved", "wrong-workdir", "revised"))
    for fixture in (approved, wrong, revised):
        fixture.mkdir()
    (approved / "value.txt").write_bytes(CANARY)
    (revised / "value-ready.txt").write_bytes(CANARY)
    cwd_calls = [observed_call(python, wrong, environment, READ, ["value.txt"], success=False) for _ in range(3)]
    cwd_calls.append(observed_call(python, approved, environment, READ, ["value.txt"], success=True))
    require(all(call["before"] == cwd_calls[0]["before"] for call in cwd_calls[:3])
            and cwd_calls[0]["cwd"] != cwd_calls[3]["cwd"], "scope_fixture_not_paired")
    revisions = []
    revision_path = revised / "revision.txt"
    for name in ("a", "b", "c", "ready"):
        draft = revision_path.with_suffix(".next")
        with draft.open("xb") as stream:
            stream.write(name.encode("ascii"))
        os.replace(draft, revision_path)
        revisions.append(observed_call(python, revised, environment, CHECK_REVISION,
                                       ["revision.txt", f"value-{name}.txt"], success=name == "ready"))
    require(len({call["before"]["revision.txt"] for call in revisions}) == 4
            and len({(call["script"], call["cwd"]) for call in revisions}) == 1, "revision_fixture_not_paired")
    return cwd_calls, revisions


def transcript(source, calls, *, visible_edits=False):
    codex = source == "codex"
    session_id = "fixture-not-a-real-session"
    rows = [{"type": "session_meta", "payload": {"id": session_id}}] if codex else []

    def append(identity, tool, call):
        if codex:
            arguments = {"cmd": call["script"], "workdir": call["cwd"]}
            items = [{"type": "function_call", "call_id": identity, "name": tool,
                      "arguments": json.dumps(arguments)},
                     {"type": "function_call_output", "call_id": identity,
                      "output": {"exit_code": call["exit"], "output": call["output"]}}]
            rows.extend({"type": "response_item", "payload": item} for item in items)
        else:
            items = [("assistant", {"type": "tool_use", "id": identity, "name": tool,
                                     "input": {"command": call["script"]}}),
                     ("user", {"type": "tool_result", "tool_use_id": identity, "is_error": call["exit"] != 0,
                               "content": call["output"]})]
            rows.extend({"type": role, "sessionId": session_id, "cwd": call["cwd"], "message": {"content": [item]}}
                        for role, item in items)

    for index, call in enumerate(calls):
        if visible_edits and index:
            append(f"edit-{index}", "apply_patch" if codex else "Edit",
                   {"script": "Apply the owned fixture revision", "cwd": call["cwd"], "exit": 0, "output": "updated"})
        append(f"call-{index}", "exec_command" if codex else "Bash", call)
    return b"\n".join(replay["encoded"](row) for row in rows) + b"\n"


def analyze(python, payload, environment, script, data, source, expected):
    child = run_child(python, payload, environment, script, data)
    require(child.returncode == 0 and not child.stderr and 0 < len(child.stdout) <= 2048, "learn_preview_process_failed")
    require(CANARY not in child.stdout and all(marker not in child.stdout for marker in
            (b"fixture-not-a-real-session", b"value.txt", b"revision.txt", b"call-0", b"wrong-workdir")),
            "learn_preview_exposed_fixture_content")
    actual = json.loads(child.stdout)
    require(actual == {"status": "ready", "source": source, "suggestions": expected}, "unexpected_learn_preview_result")
    return actual["suggestions"]


def policy_line(path, snippet):
    matches = [i for i, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1) if snippet in line]
    require(len(matches) == 1, "project_rule_reference_changed")
    return {"file": path.relative_to(ROOT).as_posix(), "line": matches[0]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true", required=True)
    parser.parse_args()
    require(os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_workspace")
    runtime = ROOT / "runtime"
    controller = replay["load_module"]("learn_merit_controller", runtime / "desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(runtime / "manifest.json")
    require(sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "pinned_python_required")
    cache = controller.verify_tokenizer_cache(payload)
    os.environ["TIKTOKEN_CACHE_DIR"] = str(cache)
    packages = payload / "Lib/site-packages"
    site.addsitedir(str(packages))
    import tiktoken

    tokenizer = tiktoken.get_encoding("o200k_base")
    preview = runtime / "learn_preview.py"
    ui = ROOT / "src/features/settings/Learn.tsx"
    guidance = ROOT / "AGENTS.md"
    workflow = ROOT / "docs/development/WORKFLOW.md"
    paths = [Path(__file__), HERE / "replay.py", HERE / "fixtures.py", preview, ui, guidance, workflow,
             runtime / "manifest.json", runtime / "desktop_controller.py", payload / "runtime-manifest.json",
             payload / "python313.dll", Path(sys.executable),
             *[packages / "headroom/learn" / name for name in ("__init__.py", "_shared.py", "loops.py", "models.py")],
             *[path for path in cache.iterdir() if path.is_file()], *list((packages / "tiktoken").glob("*.pyd"))]
    before = replay["fingerprints"](paths)
    script = preview.read_text(encoding="utf-8")
    labels = re.findall(r'^      path: "([^"\r\n]+)",$', ui.read_text(encoding="utf-8"), re.M)
    require(len(labels) == 2, "learn_path_label_changed")
    advice = {language: {"text": label, "tokens_o200k_base": len(tokenizer.encode(label, disallowed_special=()))}
              for language, label in zip(("tr", "en"), labels, strict=True)}
    policies = {"workspace": policy_line(guidance, "Use the root of this Git checkout as the workspace."),
                "changed_inputs": policy_line(guidance, "If inputs changed, the PASS is evidence"),
                "workflow": policy_line(workflow, "For new code acceptance, run the final candidate gate")}
    environment = {"SystemRoot": os.environ["SystemRoot"]}
    run = ROOT / "work/proofs/optimization/learn-merit" / uuid.uuid4().hex
    run.mkdir(parents=True)
    results, observed, failure = {}, None, None
    try:
        cwd, revisions = observe_project_tasks(Path(sys.executable), run, environment)
        observed = {case: [{"exit_code": call["exit"], "read_inputs_before": call["before"],
                            "read_inputs_after": call["after"], "same_invocation": call["cwd"] == calls[0]["cwd"]
                            and call["script"] == calls[0]["script"]} for call in calls]
                    for case, calls in (("workdir_repair", cwd), ("required_retests", revisions))}
        for source in ("codex", "claude_code"):
            path_advice = {"kind": "path", "failures": 3, "recoveries": 0, "first_line": 3 if source == "codex" else 2}
            variants = (
                ("stable_failure_then_workdir_repair", cwd, False, [path_advice]),
                ("repair_after_first_failure", [cwd[0], cwd[-1]], False, []),
                ("required_retests_with_edits", revisions, True, []),
                ("required_retests_with_edits_missing", revisions, False, [{**path_advice, "recoveries": 1}]),
            )
            results[source] = {}
            for name, calls, with_edits, expected in variants:
                data = transcript(source, calls, visible_edits=with_edits)
                results[source][name] = analyze(Path(sys.executable), payload, environment, script, data, source, expected)
    except Exception as error:
        failure = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)) else type(error).__name__
    after = replay["fingerprints"](paths)
    passed = failure is None and before == after
    artifact = run / "result.json"
    replay["write_new"](artifact, {"passed": passed, "failure": failure, "os": platform.platform(),
                                   "python": platform.python_version(), "engine": manifest["engine"]["version"],
                                   "scenarios": results, "observed_calls": observed, "displayed_guidance": advice,
                                   "project_policy_references": policies, "before": before, "after": after})
    print(json.dumps({"passed": passed, "failure": failure, "previews": sum(map(len, results.values())),
                      "proof": artifact.relative_to(ROOT).as_posix()}))
    return not passed


if __name__ == "__main__":
    raise SystemExit(main())

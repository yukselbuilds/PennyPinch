"""Probe Codex's native file policy and archive only an accepted fixture read."""
import argparse
import base64
import csv
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import runpy
import subprocess
import sys
import uuid


ENTRY = Path(__file__).resolve()
ROOT = ENTRY.parents[2]
COMPARE = runpy.run_path(str(ENTRY.with_name("targeted_query_compare.py")))
PROOFS = COMPARE["PROOF_ROOT"] / "file-agent-scope"
DATA = COMPARE["REPORT"]
READER = """import base64, json, sys
from pathlib import Path
result = {"started": True, "allowed": None, "outside": "not_requested"}
try:
    with Path(sys.argv[1]).open("rb") as stream:
        result["allowed"] = base64.b64encode(stream.read(8193)).decode("ascii")
except OSError:
    pass
if len(sys.argv) == 3:
    try:
        with Path(sys.argv[2]).open("rb"):
            result["outside"] = "opened"
    except PermissionError:
        result["outside"] = "access_denied"
    except OSError:
        result["outside"] = "other_error"
print(json.dumps(result))
"""


def observe_read(exit_code, stdout, stderr, expected, *, check_outside):
    row = {"exit_code": exit_code, "status": "native_probe_failed", "command_started": False,
           "inside_exact": False, "outside_denied": False}
    if exit_code != 0:
        errors = (stdout + b"\n" + stderr).lower()
        if b"restricted read-only access requires the elevated windows sandbox backend" in errors:
            row["status"] = "restricted_backend_required"
        elif b"native mxc is unavailable on this executor" in errors:
            row["status"] = "mxc_unavailable"
        elif b"approval" in errors and any(v in errors for v in (b"rejected", b"denied")):
            row["status"] = "approval_rejected"
        return row, None
    if len(stdout) > 16384:
        return row, None
    try:
        result = json.loads(stdout)
        if not isinstance(result, dict) or result.keys() != {"started", "allowed", "outside"} or result["started"] is not True:
            return row, None
        row["command_started"] = True
        inside = base64.b64decode(result["allowed"], validate=True)
    except (TypeError, ValueError, RecursionError):
        return row, None
    row["inside_exact"] = inside == expected
    row["outside_denied"] = check_outside and result["outside"] == "access_denied"
    if check_outside:
        row["status"] = "boundary_verified" if row["inside_exact"] and row["outside_denied"] else "boundary_unverified"
    elif row["inside_exact"] and result["outside"] == "not_requested":
        row["status"] = "approved_read_verified"
    return row, inside if row["status"] in ("approved_read_verified", "boundary_verified") else None


def sandbox_read(cli, supervisor, work, env, engine, profile, paths):
    cmd = [str(cli), "-c", f'windows.sandbox="{engine}"', "-c", 'approval_policy="never"', "sandbox",
           "--include-managed-config", "--permission-profile", profile, "-C", str(work), "--",
           sys.executable, "-I", "-B", "-c", READER, *map(str, paths)]
    job = supervisor.KillOnCloseJob()
    child = None
    try:
        child = subprocess.Popen(cmd, cwd=work, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, creationflags=subprocess.CREATE_NO_WINDOW)
        job.assign(child.pid)
        stdout, stderr = child.communicate(timeout=25)
        return child.returncode, stdout, stderr
    except subprocess.TimeoutExpired:
        return -1, b"", b""
    finally:
        job.close()
        if child is not None and child.poll() is None:
            child.wait(timeout=10)


def acl_denial_probe(cli, supervisor, work, env, inside, acl_file):
    assert (work.parent.parent == PROOFS and acl_file == work.parent / "acl-control.jsonl"
            and acl_file.resolve(strict=True) == acl_file), "acl_fixture_scope"
    system = Path(os.environ["SystemRoot"]) / "System32"

    def command(executable, *args):
        return subprocess.run([str(system / executable), *args], cwd=work, env=env,
                              capture_output=True, timeout=10, creationflags=subprocess.CREATE_NO_WINDOW)

    identity = command("whoami.exe", "/user", "/fo", "csv", "/nh")
    assert identity.returncode == 0, "fixture_identity_unavailable"
    _, sid = next(csv.reader(identity.stdout.decode(errors="replace").splitlines()))
    assert re.fullmatch(r"S-1-(?:[0-9]+-)+[0-9]+", sid), "fixture_identity_unavailable"
    original = command("icacls.exe", str(acl_file))
    assert original.returncode == 0, "fixture_acl_unavailable"
    try:
        changed = command("icacls.exe", str(acl_file), "/deny", f"*{sid}:(RD)")
        assert changed.returncode == 0, "fixture_acl_setup_failed"
        observed, _ = observe_read(*sandbox_read(cli, supervisor, work, env, "unelevated", ":read-only", (inside, acl_file)),
                                  DATA, check_outside=True)
    finally:
        restored = command("icacls.exe", str(acl_file), "/remove:d", f"*{sid}")
        after = command("icacls.exe", str(acl_file))
        assert restored.returncode == after.returncode == 0 and after.stdout == original.stdout, "fixture_acl_restore_failed"
    if observed["status"] == "boundary_verified":
        observed["status"] = "windows_acl_denial_verified"
    return {**observed, "acl_restored": True}


def archive_read(data):
    client = runpy.run_path(str(ENTRY.with_name("targeted_query_client.py")))
    output = client["query"].ToolOutput(COMPARE["CAPTURE"], "", data, b"", 0)
    session = client["ReportSession"](output)
    reference = session.capture("codex-sandbox:approved-file")
    selected = session.select({"archive_id": reference["archive_id"], "record_id": COMPARE["TARGET_ID"]})
    assert selected["status"] == "selected" and selected["stdout"].encode("utf-8") == COMPARE["RECORD"], "selected_record_changed"
    assert selected["record_count"] == len(COMPARE["ROWS"]) and selected["source_call_id"] == reference["source_call_id"], "selected_provenance_changed"
    original = session.read_part(reference["archive_id"], "stdout", 0)
    assert original["text"].encode("utf-8") == data and original["next_offset"] == len(data), "original_read_changed"
    assert original["source_call_id"] == reference["source_call_id"], "original_provenance_changed"
    return {"record_exact": True, "original_exact": True, "bytes": len(data), "record_count": selected["record_count"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex", required=True, type=Path)
    args = parser.parse_args()
    assert os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_workspace"
    cli = args.codex.resolve(strict=True)
    assert cli.name == "codex.exe" and cli.is_relative_to((Path(os.environ["LOCALAPPDATA"]) / "OpenAI/Codex/bin").resolve(strict=True)), "cli_scope"
    version = subprocess.run([str(cli), "--version"], capture_output=True, timeout=10, check=True).stdout.decode().strip()
    assert version == COMPARE["CLI_VERSION"], "cli_version"
    pilot_path = ROOT / "bench/optimization/rules_pilot.py"
    pilot = runpy.run_path(str(pilot_path))
    runtime_path = Path(sys.executable).parent
    runner = runtime_path / "headroom_runtime.py"
    core = runpy.run_path(str(ROOT / "tests/runtime/anthropic_usage_capture.py"))["replay"]
    supervisor = core.load_module("native_files_supervisor", runner)
    run = PROOFS / ("native-files-" + uuid.uuid4().hex)
    assert run.resolve().is_relative_to(ROOT), "owned_proof_root"
    work, state = run / "workspace", run / "client"
    home, temp = state / "home", state / "temp"
    for path in (work, home, temp):
        path.mkdir(parents=True)
    inside, outside, acl_file = work / "approved.jsonl", run / "outside.jsonl", run / "acl-control.jsonl"
    inside.write_bytes(DATA)
    outside.write_bytes(b'{"id":"outside","label":"HEADROOM_FILE_PERMISSION_FIXTURE"}\n')
    acl_file.write_bytes(b'{"id":"acl-control","label":"HEADROOM_FILE_ACL_FIXTURE"}\n')
    config = home / "config.toml"
    config.write_text("\n".join(("[permissions.headroom-file-scope.filesystem]", '":minimal" = "read"', '":workspace_roots" = "read"',
        json.dumps(str(runtime_path)) + ' = "read"', json.dumps(str(outside)) + ' = "deny"')) + "\n", encoding="utf-8")
    env = pilot["client_environment"]()
    env.update(CODEX_HOME=str(home), CODEX_SQLITE_HOME=str(state / "db"), TEMP=str(temp), TMP=str(temp))
    files = (ENTRY, COMPARE["PROGRAM"], ENTRY.with_name("targeted_query_client.py"), ENTRY.with_name("targeted_query.py"),
             ENTRY.with_name("targeted_query_replay.py"), pilot_path, runner, Path(sys.executable), runtime_path / "python313.dll", cli, config, inside, outside, acl_file)
    before = {str(path): core.sha(path) for path in files}
    checks, archive = {}, None
    cases = (("restricted_unelevated", "unelevated", "headroom-file-scope", (inside, outside)),
             ("restricted_mxc", "mxc", "headroom-file-scope", (inside, outside)),
             ("approved_only", "unelevated", ":read-only", (inside,)))
    try:
        for name, backend, profile, paths in cases:
            observed, data = observe_read(*sandbox_read(cli, supervisor, work, env, backend, profile, paths), DATA, check_outside=len(paths) == 2)
            checks[name] = observed
            if data is not None and archive is None:
                archive = archive_read(data)
            if observed["status"] == "approval_rejected":
                break
        if checks.get("approved_only", {}).get("status") == "approved_read_verified":
            checks["windows_acl_control"] = acl_denial_probe(cli, supervisor, work, env, inside, acl_file)
        after = {str(path): core.sha(path) for path in files}
    finally:
        COMPARE["remove_client_state"](state)
    boundary = any(check["status"] == "boundary_verified" for name, check in checks.items() if name.startswith("restricted_"))
    complete = boundary and archive is not None and before == after
    result = {"native_file_scope_verified": complete, "cli": version, "os": platform.platform(), "checks": checks,
              "accepted_read_archive": archive, "inputs_match": before == after, "before": before, "after": after,
              "external_model_requests": 0}
    core.write_new(run / "result.json", result)
    print(json.dumps({**{key: result[key] for key in ("native_file_scope_verified", "checks", "accepted_read_archive", "inputs_match")},
                      "proof": (run / "result.json").relative_to(ROOT).as_posix()}))
    return 0 if complete else 1


if __name__ == "__main__":
    raise SystemExit(main())

"""Exercise the narrow named Codex file profile on an existing Ubuntu WSL2 distro."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import runpy
import stat
import subprocess
import sys
import tarfile
import uuid


PROGRAM = Path(__file__).resolve()
ROOT = PROGRAM.parents[2]
NATIVE_PROBE = PROGRAM.with_name("targeted_query_native_files.py")
PROFILE = "headroom-file-scope"
TAG = "rust-v0.169.0-alpha.11.1"
CLI_NAME = "codex-x86_64-unknown-linux-musl"


def linux_path(path):
    relative = path.relative_to(ROOT)
    return "/mnt/d/Projeler/Headroom/" + relative.as_posix()


def pin_cli(asset_root, sha):
    archive, manifest, binary = (asset_root / name for name in (CLI_NAME + ".tar.gz", "asset.json", CLI_NAME))
    for file in (manifest, archive, binary):
        assert file.resolve(strict=True) == file and file.is_file() and not file.lstat().st_file_attributes & stat.FILE_ATTRIBUTE_REPARSE_POINT, "wsl_codex_path"
    pin = json.loads(manifest.read_bytes())
    assert pin["version"] == TAG and archive.stat().st_size == pin["size"] and sha(archive) == pin["sha256"], "wsl_codex_archive_pin"
    assert pin["source"] == f"https://github.com/openai/codex/releases/download/{TAG}/{archive.name}", "wsl_codex_source"
    with tarfile.open(archive, "r:gz") as package:
        member, = package.getmembers()
        assert member.isfile() and member.name == CLI_NAME and member.size <= 512 * 1024 * 1024
        with package.extractfile(member) as source:
            assert hashlib.file_digest(source, "sha256").hexdigest() == sha(binary), "wsl_codex_binary_pin"
    return binary, [archive, manifest, binary]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true", required=True)
    parser.parse_args()
    assert os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_workspace"
    native = runpy.run_path(str(NATIVE_PROBE))
    replay = runpy.run_path(str(ROOT / "tests/runtime/anthropic_usage_capture.py"))["replay"]
    base = native["PROOFS"].resolve(strict=True)
    assert base.is_relative_to(ROOT), "wsl_proof_scope"
    asset_root = base / TAG.replace("rust-v", "wsl-codex-")
    binary, assets = pin_cli(asset_root, replay.sha)
    wrapper, deb, source = (asset_root / name for name in ("bubblewrap-stage/usr/bin/bwrap", "bubblewrap.deb", "bubblewrap-pin.json"))
    for file in (wrapper, deb, source):
        assert file.resolve(strict=True) == file and file.is_file() and not file.lstat().st_file_attributes & stat.FILE_ATTRIBUTE_REPARSE_POINT, "bubblewrap_path"
    wrapper_pin = json.loads(source.read_bytes())
    assert replay.sha(deb) == wrapper_pin["sha256"] and deb.stat().st_size == wrapper_pin["bytes"], "bubblewrap_pin"
    wsl = Path(os.environ["SystemRoot"]) / "System32/wsl.exe"
    run = base / ("wsl2-files-" + uuid.uuid4().hex)
    work, home = run / "workspace", run / "client-home"
    work.mkdir(parents=True)
    home.mkdir()
    inside, outside = work / "approved.jsonl", run / "excluded.jsonl"
    data = native["DATA"]
    inside.write_bytes(data)
    outside.write_bytes(b'{"id":"excluded","label":"HEADROOM_WSL_SCOPE_FIXTURE"}\n')
    cfg = home / "config.toml"
    cfg.write_text(f'''default_permissions = "{PROFILE}"
[permissions.{PROFILE}.filesystem]
":root" = "deny"
":minimal" = "read"
":workspace_roots" = "read"
"{linux_path(run)}" = "read"
"{linux_path(binary)}" = "read"
[permissions.{PROFILE}.network]
enabled = false
''', encoding="utf-8")
    deny_layer = home / "headroom-deny.config.toml"
    deny_layer.write_text(f'[permissions.{PROFILE}.filesystem]\n"{linux_path(outside)}" = "deny"\n', encoding="utf-8")
    clean_env = [f"PATH={linux_path(wrapper.parent)}:/usr/bin:/bin", "LC_ALL=C",
                 f"HOME={linux_path(home)}", f"CODEX_HOME={linux_path(home)}"]

    def execute(*command):
        args = [str(wsl), "--distribution", "Ubuntu", "--cd", linux_path(work), "--exec",
                "/usr/bin/env", "-i", *clean_env, "/usr/bin/timeout", "--kill-after=5s", "20s", *command]
        return subprocess.run(args, cwd=ROOT, capture_output=True, timeout=35, stdin=subprocess.DEVNULL,
                              creationflags=subprocess.CREATE_NO_WINDOW)

    def metadata(*command):
        response = execute(*command)
        assert response.returncode == 0 and 0 < len(response.stdout) <= 512, "wsl_environment_probe"
        return response.stdout.decode("ascii").strip()

    exe = linux_path(binary)
    version = metadata(exe, "--version")
    assert version == native["COMPARE"]["CLI_VERSION"], "wsl_codex_version"
    kernel = metadata("/usr/bin/uname", "-srmo")
    assert "WSL2" in kernel and "x86_64" in kernel, "wsl2_x64_required"
    registration = {"cli": version, "kernel": kernel, "python": metadata("/usr/bin/python3", "--version"),
                    "bubblewrap": metadata(linux_path(wrapper), "--version"), "bubblewrap_package": wrapper_pin,
                    "python_sha256": metadata("/usr/bin/sha256sum", "/usr/bin/python3").split()[0],
                    "host": platform.platform(), "backend": "Ubuntu WSL2", "profile": PROFILE,
                    "model_calls": 0, "policy": "root deny, readable synthetic run and CLI binary, one exact file denial"}
    registration_path = run / "registration.json"
    replay.write_new(registration_path, registration)
    inputs = [PROGRAM, NATIVE_PROBE, wsl, registration_path, cfg, deny_layer, inside, outside, wrapper, deb, source, *assets,
              *[PROGRAM.with_name(name) for name in ("targeted_query.py", "targeted_query_compare.py", "targeted_query_client.py")]]
    before = {str(path): replay.sha(path) for path in inputs}
    observations, archive, failure = {}, None, None
    try:
        for phase in ("control", "restricted", "control_after"):
            options = ("--profile", "headroom-deny") if phase == "restricted" else ()
            reply = execute(exe, "-c", 'approval_policy="never"', "sandbox", *options,
                "--include-managed-config", "--permission-profile", PROFILE, "-C", linux_path(work), "--",
                "/usr/bin/python3", "-I", "-B", "-c", native["READER"], linux_path(inside), linux_path(outside))
            row, accepted = native["observe_read"](reply.returncode, reply.stdout, reply.stderr, data, check_outside=True)
            if row["command_started"]:
                outside_state = json.loads(reply.stdout)["outside"]
                row["outside"] = outside_state if outside_state in ("opened", "access_denied", "other_error") else "other_error"
            else:
                err = (reply.stdout + b"\n" + reply.stderr).lower()
                row["diagnostics"] = [tag for tag, fragment in (
                    ("argument", b"unexpected argument"), ("subcommand", b"unrecognized subcommand"),
                    ("profile", b"permission profile"), ("namespace", b"namespace"), ("bubblewrap", b"bwrap"),
                    ("config", b"configuration"), ("default_permissions", b"default_permissions"),
                    ("timeout", b"timed out"), ("bind", b"bind"),
                    ("permission", b"permission denied")) if fragment in err]
            observations[phase] = row
            if phase == "restricted":
                assert row["status"] == "boundary_verified", "wsl_named_scope_not_verified"
                archive = native["archive_read"](accepted)
            else:
                assert row["inside_exact"] and row.get("outside") == "opened", "wsl_control_not_verified"
    except Exception as error:
        failure = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)) else type(error).__name__
    after = {str(path): replay.sha(path) for path in inputs}
    same_environment = (metadata("/usr/bin/uname", "-srmo") == kernel and
                        metadata("/usr/bin/sha256sum", "/usr/bin/python3").split()[0] == registration["python_sha256"])
    passed = failure is None and archive is not None and before == after and same_environment
    result = {**registration, "passed": passed, "failure": failure, "observations": observations,
              "accepted_archive": archive, "same_environment": same_environment, "matched_inputs": len(before) if before == after else 0}
    proof = run / "result.json"
    replay.write_new(proof, {**result, "before": before, "after": after})
    print(json.dumps({**result, "proof": proof.relative_to(ROOT).as_posix()}))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())

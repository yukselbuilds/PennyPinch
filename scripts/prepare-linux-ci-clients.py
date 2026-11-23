#!/usr/bin/env python3
"""Prepare pinned Linux x64 CLI binaries without executing them."""

from __future__ import annotations

import argparse
import base64
import hashlib
import importlib.util
import json
import os
import stat
import struct
import sys
import tarfile
import time
import uuid
from pathlib import Path, PurePosixPath


ROOT = Path(__file__).resolve().parent.parent
WORK_BASE = ROOT / "work"
MAX_UNPACKED_BYTES = 768 * 1024 * 1024
MAX_MEMBERS = 50_000
MAX_EXECUTABLE_BYTES = 512 * 1024 * 1024
ELF_X86_64 = 62

ARTIFACTS = (
    {
        "client": "codex",
        "version": "0.163.0-alpha.11.1-linux-x64",
        "url": "https://registry.npmjs.org/@openai/codex/-/codex-0.163.0-alpha.11.1-linux-x64.tgz",
        "archive_size": 159_391_723,
        "archive_sri": "sha512-3TMCWdaqX0UyE3+GUR3PUUU1sIVd5IHOXxQiZWdjX3QHxNeOjM6JrzXIM6wi9hMowY5OJSZUdLRbfAABwgiT6g==",
        "member": "package/vendor/x86_64-unknown-linux-musl/bin/codex",
        "binary_name": "codex",
        "binary_size": 294_498_248,
        "binary_sha256": "ce74d935f61c14d931dd62a79736a964f741af47688ae119b17a1fd973a08399",
    },
    {
        "client": "claude",
        "version": "2.1.158",
        "url": "https://registry.npmjs.org/@anthropic-ai/claude-code-linux-x64/-/claude-code-linux-x64-2.1.158.tgz",
        "archive_size": 73_309_197,
        "archive_sri": "sha512-yIQwKyz9g66GfFnp/ydz2wq1HuRY7Gq3TY9/yyf5hEqEwAdj2ycP8/jMPGi+9UeKrc4GTcJ/BCBv/GqbTD9N4g==",
        "member": "package/claude",
        "binary_name": "claude",
        "binary_size": 240_666_320,
        "binary_sha256": "dd27008acd42700bac5762652ec83ff604bf9ae0786d4dde55d57a6866017fbe",
    },
    {
        "client": "opencode",
        "version": "1.19.3",
        "url": "https://registry.npmjs.org/opencode-linux-x64/-/opencode-linux-x64-1.19.3.tgz",
        "archive_size": 60_356_012,
        "archive_sri": "sha512-wz+kXi7ZkvLW04hEIocW3RoEznALJ3YGnvNlZdXFarYF0v1jyDyAwpPGZkxb4JZDrw4oy48/LDZ8wCFIQDTxhw==",
        "member": "package/bin/opencode",
        "binary_name": "opencode",
        "binary_size": 186_013_824,
        "binary_sha256": "179ca421d61eefeadfa0c50e084a6e79c1623734732f0f4593f81ccfd3ab7ee9",
    },
)


class PrepareError(Exception):
    """Stable error code with no external response details."""

    def __init__(self, code: str):
        super().__init__(code)
        self.code = code


def fail(code: str) -> None:
    raise PrepareError(code)


def is_reparse(path: Path) -> bool:
    return path.is_symlink() or getattr(path, "is_junction", lambda: False)()


def inside(path: Path, parent: Path) -> bool:
    return path == parent or parent in path.parents


def canonical_parent_for_new_root(raw: str) -> tuple[Path, Path]:
    supplied = Path(raw)
    if not supplied.is_absolute() or supplied.name in ("", ".", ".."):
        fail("output_root_not_absolute")
    try:
        workspace = ROOT.resolve(strict=True)
        base = WORK_BASE.resolve(strict=True)
        if base != WORK_BASE or not inside(base, workspace):
            fail("work_root_not_canonical")
        cursor = workspace
        for part in base.relative_to(workspace).parts:
            cursor = cursor / part
            if is_reparse(cursor):
                fail("proof_base_reparse")
        parent = supplied.parent.resolve(strict=True)
    except PrepareError:
        raise
    except (OSError, ValueError):
        fail("output_parent_unavailable")
    raw_absolute = Path(os.path.abspath(os.fspath(supplied)))
    if os.path.normcase(os.fspath(raw_absolute)) != os.path.normcase(os.fspath(supplied)):
        fail("output_root_not_canonical")
    if parent != supplied.parent or not inside(parent, base):
        fail("output_parent_outside")
    cursor = base
    for part in parent.relative_to(base).parts:
        cursor = cursor / part
        if is_reparse(cursor):
            fail("output_parent_reparse")
    if is_reparse(parent) or not parent.is_dir():
        fail("output_parent_not_directory")
    if os.name != "nt":
        details = parent.stat(follow_symlinks=False)
        if details.st_uid != os.getuid() or stat.S_IMODE(details.st_mode) & 0o077:
            fail("output_parent_not_private")
    root = parent / supplied.name
    if os.path.lexists(root):
        fail("output_exists")
    return parent, root


def mac_helpers():
    helper_path = ROOT / "scripts" / "prepare-macos-ci-clients.py"
    if not helper_path.is_file() or is_reparse(helper_path):
        fail("shared_download_helper_unavailable")
    try:
        sys.dont_write_bytecode = True
        spec = importlib.util.spec_from_file_location("headroom_macos_client_preparer", helper_path)
        if spec is None or spec.loader is None:
            fail("shared_download_helper_unavailable")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module
    except PrepareError:
        raise
    except Exception:
        fail("shared_download_helper_unavailable")


def valid_member_name(name: str) -> bool:
    candidate = PurePosixPath(name)
    return not candidate.is_absolute() and all(part not in ("", ".", "..") for part in candidate.parts)


def is_elf_x86_64(prefix: bytes) -> bool:
    return (
        len(prefix) >= 20
        and prefix[:4] == b"\x7fELF"
        and prefix[4] == 2
        and prefix[5] == 1
        and prefix[6] == 1
        and struct.unpack("<H", prefix[18:20])[0] == ELF_X86_64
    )


def same_file(path: Path, identity: tuple[int, int]) -> bool:
    try:
        item = path.stat(follow_symlinks=False)
        return stat.S_ISREG(item.st_mode) and (item.st_dev, item.st_ino) == identity
    except OSError:
        return False


def same_directory(path: Path, identity: tuple[int, int]) -> bool:
    try:
        item = path.stat(follow_symlinks=False)
        return stat.S_ISDIR(item.st_mode) and (item.st_dev, item.st_ino) == identity
    except OSError:
        return False


def remove_owned_file(path: Path, identity: tuple[int, int] | None) -> None:
    if identity is not None and same_file(path, identity):
        try:
            path.unlink()
        except OSError:
            pass


def extract_binary(artifact: dict, archive_path: Path, destination: Path, helpers) -> dict:
    count = 0
    unpacked = 0
    found = 0
    written = 0
    digest = hashlib.sha256()
    prefix = bytearray()
    identity: tuple[int, int] | None = None
    succeeded = False
    deadline = time.monotonic() + 180
    try:
        with tarfile.open(archive_path, mode="r|gz") as archive:
            with destination.open("xb") as output:
                item = os.fstat(output.fileno())
                identity = (item.st_dev, item.st_ino)
                for member in archive:
                    if time.monotonic() > deadline:
                        fail("archive_read_timeout")
                    count += 1
                    if count > MAX_MEMBERS:
                        fail("archive_member_limit")
                    if member.size < 0:
                        fail("archive_member_size_invalid")
                    unpacked += member.size
                    if unpacked > MAX_UNPACKED_BYTES:
                        fail("archive_unpacked_limit")
                    if member.name != artifact["member"]:
                        continue
                    found += 1
                    if found > 1:
                        fail("archive_member_duplicate")
                    if not valid_member_name(member.name) or not member.isfile():
                        fail("archive_member_not_regular")
                    if member.size != artifact["binary_size"] or member.size > MAX_EXECUTABLE_BYTES:
                        fail("archive_member_size_mismatch")
                    source = archive.extractfile(member)
                    if source is None:
                        fail("archive_member_unreadable")
                    with source:
                        while True:
                            chunk = source.read(1024 * 1024)
                            if not chunk:
                                break
                            written += len(chunk)
                            if written > artifact["binary_size"]:
                                fail("archive_member_size_mismatch")
                            if len(prefix) < 64:
                                prefix.extend(chunk[: 64 - len(prefix)])
                            digest.update(chunk)
                            output.write(chunk)
                output.flush()
                os.fsync(output.fileno())
        if found != 1:
            fail("archive_member_missing")
        if written != artifact["binary_size"]:
            fail("archive_member_size_mismatch")
        if not is_elf_x86_64(bytes(prefix)):
            fail("archive_member_not_elf_x86_64")
        binary_sha256 = digest.hexdigest()
        if binary_sha256 != artifact["binary_sha256"]:
            fail("binary_sha256_mismatch")
        os.chmod(destination, 0o700)
        size, checked_sha256 = helpers.sha256_file(destination)
        if size != written or checked_sha256 != binary_sha256:
            fail("binary_recheck_mismatch")
        succeeded = True
        return {
            "executable_bytes": size,
            "executable_sha256": checked_sha256,
            "elf": "ELF64 little-endian x86_64",
            "path": f"bin/{artifact['binary_name']}",
        }
    except PrepareError:
        raise
    except Exception:
        fail("archive_invalid")
    finally:
        if not succeeded:
            remove_owned_file(destination, identity)


def prepare(raw_output: str) -> dict:
    parent, output_root = canonical_parent_for_new_root(raw_output)
    try:
        output_root.mkdir(mode=0o700, exist_ok=False)
    except FileExistsError:
        fail("output_exists")
    except OSError:
        fail("output_create_failed")
    os.chmod(output_root, 0o700)
    root_stat = output_root.stat(follow_symlinks=False)
    root_identity = (root_stat.st_dev, root_stat.st_ino)
    stage = output_root / f".stage-{uuid.uuid4().hex}"
    stage_identity: tuple[int, int] | None = None
    staged_bin = stage / "bin"
    helpers = None
    files: dict[Path, tuple[int, int]] = {}
    published = False
    result = {"schema_version": 1, "status": "prepared", "clients": []}
    current = "setup"
    try:
        try:
            stage.mkdir(mode=0o700, exist_ok=False)
            stage_stat = stage.stat(follow_symlinks=False)
            stage_identity = (stage_stat.st_dev, stage_stat.st_ino)
            os.chmod(stage, 0o700)
            staged_bin.mkdir(mode=0o700, exist_ok=False)
            helpers = mac_helpers()
        except PrepareError:
            raise
        except Exception:
            fail("stage_create_failed")
        for artifact in ARTIFACTS:
            current = artifact["client"]
            archive_path = stage / f"{current}.tgz"
            if os.path.lexists(archive_path):
                fail("archive_output_exists")
            try:
                try:
                    archive_bytes, archive_sha256, archive_sha512_b64 = helpers.download(
                        artifact["url"], archive_path, artifact["archive_size"]
                    )
                except Exception:
                    fail("archive_download_failed")
            finally:
                if os.path.lexists(archive_path) and not archive_path.is_symlink():
                    archive_stat = archive_path.stat(follow_symlinks=False)
                    if stat.S_ISREG(archive_stat.st_mode):
                        files[archive_path] = (archive_stat.st_dev, archive_stat.st_ino)
            archive_sri = "sha512-" + archive_sha512_b64
            if archive_sri != artifact["archive_sri"]:
                fail("archive_sri_mismatch")
            archive_stat = archive_path.stat(follow_symlinks=False)
            if not stat.S_ISREG(archive_stat.st_mode) or archive_stat.st_size != artifact["archive_size"]:
                fail("archive_size_mismatch")
            destination = staged_bin / artifact["binary_name"]
            binary = extract_binary(artifact, archive_path, destination, helpers)
            binary_stat = destination.stat(follow_symlinks=False)
            if not stat.S_ISREG(binary_stat.st_mode):
                fail("binary_not_regular")
            files[destination] = (binary_stat.st_dev, binary_stat.st_ino)
            result["clients"].append(
                {
                    "client": current,
                    "version": artifact["version"],
                    "archive_bytes": archive_bytes,
                    "archive_sha256": archive_sha256,
                    "archive_sri": archive_sri,
                    "archive_member": artifact["member"],
                    **binary,
                }
            )
        bin_path = output_root / "bin"
        if os.path.lexists(bin_path):
            fail("output_bin_exists")
        os.rename(staged_bin, bin_path)
        published = True
        result["output"] = "bin"
        return result
    except PrepareError as error:
        raise PrepareError(f"{current}:{error.code}") from None
    finally:
        if stage_identity is not None and same_directory(stage, stage_identity):
            for artifact in ARTIFACTS:
                archive_path = stage / f"{artifact['client']}.tgz"
                remove_owned_file(archive_path, files.get(archive_path))
                binary_path = staged_bin / artifact["binary_name"]
                remove_owned_file(binary_path, files.get(binary_path))
            try:
                if staged_bin.exists() and not any(staged_bin.iterdir()):
                    staged_bin.rmdir()
                if not any(stage.iterdir()):
                    stage.rmdir()
            except OSError:
                pass
        if not published:
            try:
                current_root = output_root.stat(follow_symlinks=False)
                if stat.S_ISDIR(current_root.st_mode) and (current_root.st_dev, current_root.st_ino) == root_identity and not any(output_root.iterdir()):
                    output_root.rmdir()
            except OSError:
                pass


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("output_root", help="absolute new private directory below the project work directory")
    args = parser.parse_args()
    try:
        result = prepare(args.output_root)
    except PrepareError as error:
        print(json.dumps({"schema_version": 1, "status": "error", "code": error.code}, sort_keys=True))
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())

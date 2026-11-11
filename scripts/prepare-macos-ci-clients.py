#!/usr/bin/env python3
"""Fetch three pinned macOS ARM64 CLI releases into a private proof directory."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import shutil
import stat
import struct
import sys
import tarfile
import time
import urllib.parse
import urllib.request
import uuid
import zipfile
from pathlib import Path, PurePosixPath


ROOT = Path(__file__).resolve().parent.parent
PROOF_BASE = ROOT / "work" / "proofs" / "macos" / "h048-c1"
MAX_ARCHIVE_BYTES = 512 * 1024 * 1024
MAX_EXECUTABLE_BYTES = 256 * 1024 * 1024
MAX_MEMBERS = 20_000
ARM64 = 0x0100000C
ARCHIVE_HOSTS = {
    "github.com",
    "release-assets.githubusercontent.com",
    "objects.githubusercontent.com",
    "registry.npmjs.org",
}


ARTIFACTS = (
    {
        "client": "codex",
        "version": "0.163.0-alpha.11.1",
        "format": "tar.gz",
        "url": "https://github.com/openai/codex/releases/download/rust-v0.163.0-alpha.11.1/codex-aarch64-apple-darwin.tar.gz",
        "archive_size": 97_306_649,
        "archive_sha256": "f37985a88b8438cabefaf46392ad10f7390384209ae88b8bf315cd8d6e9e21dd",
        "archive_sha512_sri": None,
        "member_path": "codex-aarch64-apple-darwin",
        "binary_name": "codex",
        "binary_size": 249_449_504,
        "binary_sha256": "c43dfdedd0f4c6f9e2c8e03070233db63ad041cc299f53b57a325f06494d1954",
    },
    {
        "client": "claude",
        "version": "2.1.158",
        "format": "tar.gz",
        "url": "https://registry.npmjs.org/@anthropic-ai/claude-code-darwin-arm64/-/claude-code-darwin-arm64-2.1.158.tgz",
        "archive_size": 63_594_392,
        "archive_sha256": "a822fd6958bb01dd8f0ee2e88981a7fd9ca78319827f6ec96a4e89e10ca0df16",
        "archive_sha512_sri": "sha512-QKvnpz04kUuV3/SPjiAytmKD8yOo91AVckZBNdw5gUw5r81S4fqjUYKUyxesi8Y3O6aSJT8DkDHhDJlhmecvow==",
        "member_path": "package/claude",
        "binary_name": "claude",
        "binary_size": 215_233_824,
        "binary_sha256": "536a0517fa64d48ddcbc8eb511a3d08027d47e06d148872332a8041d72c22768",
    },
    {
        "client": "opencode",
        "version": "1.19.3",
        "format": "zip",
        "url": "https://github.com/anomalyco/opencode/releases/download/v1.19.3/opencode-darwin-arm64.zip",
        "archive_size": 45_587_801,
        "archive_sha256": "c2d0fecd27acf6e08be02c1f5d384614d07495587766bcc4a6bff3971749bb64",
        "archive_sha512_sri": None,
        "member_path": "opencode",
        "binary_name": "opencode",
        "binary_size": 144_634_848,
        "binary_sha256": "6d3e6458fce4b649edf974c19959d5dbfe1e7f255a503dcfc01215b24ad3ea5b",
    },
)


class PrepareError(Exception):
    """Fixed-label preparation failure without external response details."""

    def __init__(self, code: str):
        super().__init__(code)
        self.code = code


def fail(code: str) -> None:
    raise PrepareError(code)


def sha256_file(path: Path) -> tuple[int, str]:
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            size += len(chunk)
            digest.update(chunk)
    return size, digest.hexdigest()


def private_existing_root(raw: str) -> Path:
    supplied = Path(raw)
    if not supplied.is_absolute():
        fail("proof_root_not_absolute")
    try:
        workspace = ROOT.resolve(strict=True)
        base = workspace
        for part in Path("work/proofs/macos/h048-c1").parts:
            base = base / part
            is_junction = getattr(base, "is_junction", lambda: False)()
            if base.is_symlink() or is_junction:
                fail("proof_base_reparse")
        if base.resolve(strict=True) != base:
            fail("proof_base_not_canonical")
        supplied_absolute = Path(os.path.abspath(supplied))
        root = supplied.resolve(strict=True)
    except OSError:
        fail("proof_root_unavailable")
    if supplied_absolute != root:
        fail("proof_root_not_canonical")
    if root != base and base not in root.parents:
        fail("proof_root_outside_h048_c1")
    if not root.is_dir():
        fail("proof_root_not_directory")
    relative = root.relative_to(workspace)
    cursor = workspace
    for part in relative.parts:
        cursor = cursor / part
        is_junction = getattr(cursor, "is_junction", lambda: False)()
        if cursor.is_symlink() or is_junction:
            fail("proof_root_reparse")
    if os.name != "nt":
        details = root.stat(follow_symlinks=False)
        if details.st_uid != os.getuid() or stat.S_IMODE(details.st_mode) & 0o077:
            fail("proof_root_not_private")
    return root


def download(url: str, path: Path, expected_size: int | None) -> tuple[int, str, str]:
    parsed = urllib.parse.urlparse(url)
    if parsed.scheme != "https" or parsed.hostname not in ARCHIVE_HOSTS:
        fail("download_url_not_allowlisted")
    request = urllib.request.Request(url, headers={"User-Agent": "Headroom-macos-ci-client-preparer/1"})
    digest256 = hashlib.sha256()
    digest512 = hashlib.sha512()
    total = 0
    started = time.monotonic()
    try:
        with urllib.request.urlopen(request, timeout=20) as response, path.open("xb") as output:
            final = urllib.parse.urlparse(response.geturl())
            if final.scheme != "https" or final.hostname not in ARCHIVE_HOSTS:
                fail("download_redirect_not_allowlisted")
            length = response.headers.get("Content-Length")
            if length is not None:
                try:
                    announced = int(length)
                except ValueError:
                    fail("download_length_invalid")
                if announced < 1 or announced > MAX_ARCHIVE_BYTES or (expected_size is not None and announced != expected_size):
                    fail("download_length_mismatch")
            while True:
                if time.monotonic() - started > 180:
                    fail("download_timeout")
                chunk = response.read(1024 * 1024)
                if not chunk:
                    break
                total += len(chunk)
                if total > MAX_ARCHIVE_BYTES or (expected_size is not None and total > expected_size):
                    fail("download_size_limit")
                digest256.update(chunk)
                digest512.update(chunk)
                output.write(chunk)
            output.flush()
            os.fsync(output.fileno())
    except PrepareError:
        raise
    except Exception:
        fail("download_failed")
    if total == 0 or (expected_size is not None and total != expected_size):
        fail("download_size_mismatch")
    return total, digest256.hexdigest(), base64.b64encode(digest512.digest()).decode("ascii")


def is_arm64_macho(prefix: bytes) -> bool:
    if len(prefix) < 8:
        return False
    magic = prefix[:4]
    if magic == b"\xcf\xfa\xed\xfe":
        return struct.unpack("<I", prefix[4:8])[0] == ARM64
    if magic == b"\xfe\xed\xfa\xcf":
        return struct.unpack(">I", prefix[4:8])[0] == ARM64
    if magic in (b"\xca\xfe\xba\xbe", b"\xbe\xba\xfe\xca") and len(prefix) >= 8:
        endian = ">" if magic == b"\xca\xfe\xba\xbe" else "<"
        count = struct.unpack(f"{endian}I", prefix[4:8])[0]
        return 0 < count <= 64 and any(
            struct.unpack(f"{endian}I", prefix[8 + index * 20:12 + index * 20])[0] == ARM64
            for index in range(count)
            if len(prefix) >= 8 + (index + 1) * 20
        )
    return False


def valid_member_name(name: str) -> bool:
    candidate = PurePosixPath(name)
    return not candidate.is_absolute() and all(part not in ("", ".", "..") for part in candidate.parts)


def extract_tar_binary(archive_path: Path, member_path: str, destination: Path) -> str:
    try:
        with tarfile.open(archive_path, mode="r:gz") as archive:
            members = archive.getmembers()
            if len(members) > MAX_MEMBERS:
                fail("archive_member_limit")
            candidates = [member for member in members if member.name == member_path]
            if not candidates:
                fail("archive_member_missing")
            if len(candidates) > 64:
                fail("archive_member_ambiguous")
            eligible = []
            for candidate in candidates:
                if not candidate.isfile() or candidate.size <= 0 or candidate.size > MAX_EXECUTABLE_BYTES or not valid_member_name(candidate.name):
                    continue
                probe = archive.extractfile(candidate)
                if probe is not None and is_arm64_macho(probe.read(8 + 64 * 20)):
                    eligible.append(candidate)
            if not eligible:
                fail("archive_arm64_member_missing")
            if len(eligible) != 1:
                fail("archive_member_ambiguous")
            member = eligible[0]
            stream = archive.extractfile(member)
            if stream is None:
                fail("archive_member_unreadable")
            copy_member(stream, destination, member.size)
            return member.name
    except PrepareError:
        raise
    except Exception:
        fail("archive_invalid")


def extract_zip_binary(archive_path: Path, member_path: str, destination: Path) -> str:
    try:
        with zipfile.ZipFile(archive_path) as archive:
            infos = archive.infolist()
            if len(infos) > MAX_MEMBERS:
                fail("archive_member_limit")
            candidates = [info for info in infos if info.filename == member_path]
            if not candidates:
                fail("archive_member_missing")
            if len(candidates) > 64:
                fail("archive_member_ambiguous")
            eligible = []
            for candidate in candidates:
                mode = (candidate.external_attr >> 16) & 0xFFFF
                if candidate.is_dir() or stat.S_ISLNK(mode) or (mode and not stat.S_ISREG(mode)):
                    continue
                if candidate.file_size <= 0 or candidate.file_size > MAX_EXECUTABLE_BYTES or not valid_member_name(candidate.filename):
                    continue
                with archive.open(candidate, "r") as probe:
                    if is_arm64_macho(probe.read(8 + 64 * 20)):
                        eligible.append(candidate)
            if not eligible:
                fail("archive_arm64_member_missing")
            if len(eligible) != 1:
                fail("archive_member_ambiguous")
            info = eligible[0]
            with archive.open(info, "r") as stream:
                copy_member(stream, destination, info.file_size)
            return info.filename
    except PrepareError:
        raise
    except Exception:
        fail("archive_invalid")


def copy_member(stream, destination: Path, expected_size: int) -> None:
    written = 0
    with destination.open("xb") as output:
        while True:
            chunk = stream.read(1024 * 1024)
            if not chunk:
                break
            written += len(chunk)
            if written > expected_size:
                fail("archive_member_size_mismatch")
            output.write(chunk)
        output.flush()
        os.fsync(output.fileno())
    if written != expected_size:
        fail("archive_member_size_mismatch")
    with destination.open("rb") as stream:
        if not is_arm64_macho(stream.read(8 + 64 * 20)):
            fail("archive_member_not_arm64_macho")
    os.chmod(destination, 0o700)


def prepare(raw_root: str) -> dict:
    proof_root = private_existing_root(raw_root)
    clients_path = proof_root / "clients"
    if clients_path.exists() or clients_path.is_symlink():
        fail("clients_output_exists")

    nonce = uuid.uuid4().hex
    stage_path = proof_root / f".clients-stage-{nonce}"
    download_path = proof_root / f".downloads-{nonce}"
    stage_path.mkdir(mode=0o700)
    download_path.mkdir(mode=0o700)
    os.chmod(stage_path, 0o700)
    os.chmod(download_path, 0o700)
    stage_identity = stage_path.stat(follow_symlinks=False)
    result = {"schema_version": 1, "clients": []}
    current_client = "setup"
    try:
        for artifact in ARTIFACTS:
            current_client = artifact["client"]
            archive_path = download_path / f"{artifact['client']}.{artifact['format']}"
            archive_size, archive_sha256, archive_sha512 = download(
                artifact["url"], archive_path, artifact["archive_size"]
            )
            if artifact["archive_sha256"] and archive_sha256 != artifact["archive_sha256"]:
                fail("archive_sha256_mismatch")
            if artifact["archive_sha512_sri"]:
                observed_sri = "sha512-" + archive_sha512
                if observed_sri != artifact["archive_sha512_sri"]:
                    fail("archive_sri_mismatch")

            binary_dir = stage_path / "bin"
            binary_dir.mkdir(mode=0o700, exist_ok=True)
            os.chmod(binary_dir, 0o700)
            binary_path = binary_dir / artifact["binary_name"]
            if artifact["format"] == "zip":
                member_name = extract_zip_binary(archive_path, artifact["member_path"], binary_path)
            else:
                member_name = extract_tar_binary(archive_path, artifact["member_path"], binary_path)
            binary_size, binary_sha256 = sha256_file(binary_path)
            if binary_size != artifact["binary_size"] or binary_sha256 != artifact["binary_sha256"]:
                fail("binary_pin_mismatch")
            result["clients"].append({
                "client": artifact["client"],
                "version": artifact["version"],
                "archive_size": archive_size,
                "archive_sha256": archive_sha256,
                "archive_member": member_name,
                "binary_size": binary_size,
                "binary_sha256": binary_sha256,
                "macho_arm64": True,
                "binary_path": binary_path.relative_to(stage_path).as_posix(),
            })
            archive_path.unlink()

        if clients_path.exists():
            fail("clients_output_exists")
        stage_clients = stage_path / "clients"
        stage_clients.mkdir(mode=0o700)
        os.replace(stage_path / "bin", stage_clients / "bin")
        os.chmod(stage_clients, 0o700)
        try:
            clients_path.mkdir(mode=0o700)
        except FileExistsError:
            fail("clients_output_race")
        os.chmod(clients_path, 0o700)
        clients_identity = clients_path.stat(follow_symlinks=False)
        try:
            os.replace(stage_clients / "bin", clients_path / "bin")
        except Exception:
            try:
                current = clients_path.stat(follow_symlinks=False)
                if (current.st_dev, current.st_ino) == (clients_identity.st_dev, clients_identity.st_ino):
                    clients_path.rmdir()
            except OSError:
                pass
            fail("clients_publish_failed")
        result["output"] = str((clients_path / "bin").relative_to(ROOT).as_posix())
    except PrepareError as error:
        raise PrepareError(f"{current_client}:{error.code}") from None
    finally:
        for artifact in ARTIFACTS:
            try:
                (download_path / f"{artifact['client']}.{artifact['format']}").unlink()
            except OSError:
                pass
        try:
            download_path.rmdir()
        except OSError:
            pass
        try:
            current = stage_path.stat(follow_symlinks=False)
            if (current.st_dev, current.st_ino) == (stage_identity.st_dev, stage_identity.st_ino):
                shutil.rmtree(stage_path)
        except OSError:
            pass
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("proof_root", help="absolute existing private directory below work/proofs/macos/h048-c1")
    args = parser.parse_args()
    try:
        result = prepare(args.proof_root)
    except PrepareError as error:
        print(json.dumps({"status": "error", "code": error.code}, sort_keys=True))
        return 2
    print(json.dumps({"status": "prepared", **result}, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())

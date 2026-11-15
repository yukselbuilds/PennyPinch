"""Verify a packaged macOS runtime against its staged source without writing."""

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import sys


ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT / "work"
OVERLAYS = (
    "macos_engine.py",
    "cache_control_shim.py",
    "optimization_control.py",
    "optimization-sources.json",
)
MANIFEST_KEYS = {
    "schema_version", "platform", "engine_version", "python_version",
    "dependency_lock_sha256", "python", "site_packages", "root_mode",
    "entries", "overlays",
}
MAX_MANIFEST_BYTES = 16 * 1024 * 1024
MAX_OVERLAY_BYTES = 8 * 1024 * 1024
HEX_SHA256 = re.compile(r"[0-9a-f]{64}\Z")


class VerificationError(ValueError):
    def __init__(self, code):
        super().__init__(code)
        self.code = code


def _load_inventory():
    spec = importlib.util.spec_from_file_location("_headroom_macos_payload", ROOT / "scripts/macos-payload.py")
    if spec is None or spec.loader is None:
        raise RuntimeError("payload_inventory_unavailable")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


macos_payload = _load_inventory()


def _norm(path):
    return os.path.normcase(os.path.normpath(os.fspath(path)))


def _is_within(path, parent):
    try:
        return _norm(os.path.commonpath((os.fspath(path), os.fspath(parent)))) == _norm(parent)
    except ValueError:
        return False


def _reject_reparse_chain(path):
    current = path
    while True:
        info = current.lstat()
        if stat.S_ISLNK(info.st_mode) or getattr(info, "st_file_attributes", 0) & 0x400:
            raise VerificationError("unsafe_path")
        if current.parent == current:
            return
        current = current.parent


def _existing_directory(path):
    raw = os.fspath(path)
    path = Path(raw)
    if not path.is_absolute() or os.path.normcase(os.path.abspath(raw)) != os.path.normcase(raw):
        raise VerificationError("unsafe_path")
    _reject_reparse_chain(path)
    info = macos_payload.trusted_entry(path)
    if not stat.S_ISDIR(info.st_mode) or _norm(path.resolve(strict=True)) != _norm(path):
        raise VerificationError("unsafe_path")
    return path, info


def _identity(info):
    return macos_payload.identity(info)


def _entry_rows(rows):
    entries = {}
    for name, kind, *fields in rows:
        if kind == "file":
            mode, size, digest = fields
            entries[name] = {"kind": "file", "mode": mode, "size": size, "sha256": digest}
        elif kind == "directory":
            (mode,) = fields
            entries[name] = {"kind": "directory", "mode": mode}
        elif kind == "symlink":
            (target,) = fields
            entries[name] = {"kind": "symlink", "target": target}
        else:
            raise VerificationError("invalid_manifest")
    return entries


def _json_bytes(value):
    return json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":")).encode("ascii")


def _no_duplicate_keys(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise VerificationError("invalid_manifest")
        result[key] = value
    return result


def _reject_constant(_value):
    raise VerificationError("invalid_manifest")


def _read_regular(path, max_bytes=None):
    try:
        info = macos_payload.trusted_entry(path)
        if not stat.S_ISREG(info.st_mode) or (max_bytes is not None and info.st_size > max_bytes):
            raise VerificationError("unsafe_or_changed_input")
        digest = macos_payload.regular_hash(path, info)
        flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_BINARY", 0)
        with os.fdopen(os.open(path, flags), "rb") as stream:
            opened = os.fstat(stream.fileno())
            if not stat.S_ISREG(opened.st_mode) or _identity(opened) != _identity(info):
                raise VerificationError("unsafe_or_changed_input")
            data = stream.read((max_bytes + 1) if max_bytes is not None else -1)
            if max_bytes is not None and len(data) > max_bytes:
                raise VerificationError("unsafe_or_changed_input")
            if _identity(os.fstat(stream.fileno())) != _identity(info):
                raise VerificationError("unsafe_or_changed_input")
        if _identity(path.lstat()) != _identity(info):
            raise VerificationError("unsafe_or_changed_input")
        if len(data) != info.st_size or hashlib.sha256(data).hexdigest() != digest:
            raise VerificationError("unsafe_or_changed_input")
        return {
            "data": data,
            "mode": stat.S_IMODE(info.st_mode),
            "size": info.st_size,
            "sha256": digest,
            "identity": _identity(info),
        }
    except VerificationError:
        raise
    except (OSError, ValueError, RuntimeError):
        raise VerificationError("unsafe_or_changed_input") from None


def _tree_snapshot(root):
    try:
        summary, rows = macos_payload.inventory(root)
        root_info = macos_payload.trusted_entry(root)
        identities = {"": (_identity(root_info), stat.S_IMODE(root_info.st_mode), "directory")}
        for row in rows:
            name, kind = row[:2]
            path = root / Path(name)
            info = macos_payload.trusted_entry(path)
            actual = "directory" if stat.S_ISDIR(info.st_mode) else "symlink" if stat.S_ISLNK(info.st_mode) else "file"
            if actual != kind:
                raise VerificationError("unsafe_or_changed_input")
            value = (_identity(info), stat.S_IMODE(info.st_mode), kind)
            if kind == "symlink":
                value += (os.readlink(path),)
            identities[name] = value
        return summary, rows, identities, stat.S_IMODE(root_info.st_mode)
    except VerificationError:
        raise
    except (OSError, ValueError, RuntimeError):
        raise VerificationError("unsafe_or_changed_input") from None


def _runtime_snapshot(root):
    root, root_info = _existing_directory(root)
    expected_names = {"payload", "manifest.json", *OVERLAYS}
    try:
        children = list(root.iterdir())
    except OSError:
        raise VerificationError("unsafe_or_changed_input") from None
    names = [child.name for child in children]
    if len(names) != len(expected_names) or set(names) != expected_names or len({name.casefold() for name in names}) != len(names):
        raise VerificationError("bundle_mismatch")

    payload = root / "payload"
    payload_info = macos_payload.trusted_entry(payload)
    if not stat.S_ISDIR(payload_info.st_mode):
        raise VerificationError("bundle_mismatch")
    payload_snapshot = _tree_snapshot(payload)
    manifest = _read_regular(root / "manifest.json", MAX_MANIFEST_BYTES)
    overlays = {}
    for name in OVERLAYS:
        overlays[name] = _read_regular(root / name, MAX_OVERLAY_BYTES)
    return {
        "root_identity": _identity(root_info),
        "root_mode": stat.S_IMODE(root_info.st_mode),
        "payload": payload_snapshot,
        "manifest": manifest,
        "overlays": overlays,
    }


def _directory_chain(path, stop):
    chain = []
    current = path
    while True:
        info = macos_payload.trusted_entry(current)
        if not stat.S_ISDIR(info.st_mode):
            raise VerificationError("unsafe_path")
        chain.append((current, _identity(info), stat.S_IMODE(info.st_mode)))
        if _norm(current) == _norm(stop):
            return tuple(chain)
        if current.parent == current:
            raise VerificationError("unsafe_path")
        current = current.parent


def _validate_manifest(data, expected_digest, payload_snapshot, overlay_snapshot):
    if not isinstance(expected_digest, str) or not HEX_SHA256.fullmatch(expected_digest):
        raise VerificationError("invalid_input")
    digest = hashlib.sha256(data).hexdigest()
    if digest != expected_digest:
        raise VerificationError("manifest_mismatch")
    try:
        manifest = json.loads(
            data.decode("utf-8"), object_pairs_hook=_no_duplicate_keys, parse_constant=_reject_constant
        )
    except VerificationError:
        raise
    except (UnicodeDecodeError, json.JSONDecodeError):
        raise VerificationError("invalid_manifest") from None
    if type(manifest) is not dict or set(manifest) != MANIFEST_KEYS:
        raise VerificationError("invalid_manifest")
    if (
        type(manifest["schema_version"]) is not int or manifest["schema_version"] != 1
        or manifest["platform"] != "darwin-arm64"
        or manifest["engine_version"] != "0.44.0"
        or manifest["python_version"] != "3.13.13"
        or manifest["python"] != "python/bin/python3.13"
        or manifest["site_packages"] != "site-packages"
        or type(manifest["root_mode"]) is not int
        or manifest["root_mode"] != payload_snapshot[3]
        or type(manifest["dependency_lock_sha256"]) is not str
        or HEX_SHA256.fullmatch(manifest["dependency_lock_sha256"]) is None
    ):
        raise VerificationError("invalid_manifest")
    actual_entries = manifest["entries"]
    expected_entries = _entry_rows(payload_snapshot[1])
    actual_overlays = manifest["overlays"]
    expected_overlays = {name: record["sha256"] for name, record in overlay_snapshot.items()}
    if type(actual_entries) is not dict or _json_bytes(actual_entries) != _json_bytes(expected_entries):
        raise VerificationError("bundle_mismatch")
    if type(actual_overlays) is not dict or _json_bytes(actual_overlays) != _json_bytes(expected_overlays):
        raise VerificationError("bundle_mismatch")
    return digest


def verify(bundle_root: Path, runtime_root: Path, expected_manifest_sha256: str) -> dict:
    """Compare a staged runtime with its .app copy and recheck both stayed unchanged."""
    try:
        work, _ = _existing_directory(WORK)
        bundle, _ = _existing_directory(bundle_root)
        runtime, _ = _existing_directory(runtime_root)
        if not bundle.name.casefold().endswith(".app"):
            raise VerificationError("unsafe_path")
        if not _is_within(bundle, work) or not _is_within(runtime, work):
            raise VerificationError("unsafe_path")
        if _is_within(bundle, runtime) or _is_within(runtime, bundle):
            raise VerificationError("unsafe_path")
        for path in (bundle, runtime):
            _reject_reparse_chain(path)
        package_runtime = bundle / "Contents/Resources/runtime"
        package_runtime, _ = _existing_directory(package_runtime)
        if not _is_within(package_runtime, bundle):
            raise VerificationError("unsafe_path")

        bundle_chain_before = _directory_chain(package_runtime, bundle)
        source_chain_before = _directory_chain(runtime, WORK)
        source_before = _runtime_snapshot(runtime)
        bundle_before = _runtime_snapshot(package_runtime)
        if (
            source_before["payload"][:2] != bundle_before["payload"][:2]
            or source_before["payload"][3] != bundle_before["payload"][3]
        ):
            raise VerificationError("bundle_mismatch")
        source_manifest = source_before["manifest"]
        bundle_manifest = bundle_before["manifest"]
        if (
            source_manifest["data"] != bundle_manifest["data"]
            or source_manifest["mode"] != bundle_manifest["mode"]
        ):
            raise VerificationError("bundle_mismatch")
        for name in OVERLAYS:
            left, right = source_before["overlays"][name], bundle_before["overlays"][name]
            if (left["sha256"], left["mode"], left["size"]) != (right["sha256"], right["mode"], right["size"]):
                raise VerificationError("bundle_mismatch")

        digest = _validate_manifest(
            source_manifest["data"], expected_manifest_sha256,
            source_before["payload"], source_before["overlays"],
        )
        source_after = _runtime_snapshot(runtime)
        bundle_after = _runtime_snapshot(package_runtime)
        if (
            source_before != source_after
            or bundle_before != bundle_after
            or source_chain_before != _directory_chain(runtime, WORK)
            or bundle_chain_before != _directory_chain(package_runtime, bundle)
        ):
            raise VerificationError("unsafe_or_changed_input")
        payload_summary = source_before["payload"][0]
        return {
            "schema_version": 1,
            "status": "verified",
            "manifest_sha256": digest,
            "payload_files": payload_summary["files"],
            "payload_bytes": payload_summary["bytes"],
            "payload_inventory_sha256": payload_summary["tree_sha256"],
            "source_unchanged": True,
            "bundle_unchanged": True,
        }
    except VerificationError:
        raise
    except (OSError, ValueError, RuntimeError):
        raise VerificationError("unsafe_or_changed_input") from None


class _ArgumentParser(argparse.ArgumentParser):
    def error(self, _message):
        raise VerificationError("invalid_arguments")


def main(argv=None):
    parser = _ArgumentParser(add_help=False)
    parser.add_argument("--bundle", required=True, type=Path)
    parser.add_argument("--runtime", required=True, type=Path)
    parser.add_argument("--expected-manifest", required=True)
    try:
        args = parser.parse_args(argv)
        result = verify(args.bundle, args.runtime, args.expected_manifest)
    except VerificationError as error:
        print(json.dumps({"error": error.code}, sort_keys=True, separators=(",", ":")), file=sys.stderr)
        return 2
    except (OSError, ValueError, RuntimeError):
        print('{"error":"verification_failed"}', file=sys.stderr)
        return 2
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

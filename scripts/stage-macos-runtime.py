"""Stage a pinned, relocatable macOS engine payload without replacing output."""

import argparse
import ctypes
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
WORK = ROOT / "work"
RUNTIME = ROOT / "runtime"
OVERLAY_NAMES = (
    "macos_engine.py",
    "cache_control_shim.py",
    "optimization_control.py",
    "optimization-sources.json",
)


def _load_inventory():
    name = "_headroom_macos_payload"
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts/macos-payload.py")
    if spec is None or spec.loader is None:
        raise RuntimeError("payload_inventory_unavailable")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


macos_payload = _load_inventory()


def _canonical_absolute(path):
    value = Path(path)
    if not value.is_absolute():
        raise ValueError("absolute_path_required")
    raw = os.fspath(value)
    if os.path.normcase(os.path.abspath(raw)) != os.path.normcase(raw):
        raise ValueError("noncanonical_path")
    return value


def _reject_reparse_chain(path):
    current = path
    while True:
        info = current.lstat()
        if stat.S_ISLNK(info.st_mode) or getattr(info, "st_file_attributes", 0) & 0x400:
            raise ValueError("reparse_path_rejected")
        if current.parent == current:
            return
        current = current.parent


def _is_within(path, parent):
    try:
        return os.path.normcase(os.path.commonpath((os.fspath(path), os.fspath(parent)))) == os.path.normcase(
            os.fspath(parent)
        )
    except ValueError:
        return False


def _existing_directory(path):
    path = _canonical_absolute(path)
    _reject_reparse_chain(path)
    info = path.lstat()
    if not stat.S_ISDIR(info.st_mode) or path.resolve(strict=True) != path:
        raise ValueError("canonical_directory_required")
    return path, info


def _file_snapshot(path):
    info = macos_payload.trusted_entry(path)
    if not stat.S_ISREG(info.st_mode):
        raise ValueError("regular_source_required")
    digest = macos_payload.regular_hash(path, info)
    current = macos_payload.trusted_entry(path)
    if macos_payload.identity(current) != macos_payload.identity(info):
        raise ValueError("source_changed")
    return info, digest


def _entries(rows):
    result = {}
    for row in rows:
        name, kind, *fields = row
        if kind == "file":
            mode, size, digest = fields
            result[name] = {"kind": "file", "mode": mode, "size": size, "sha256": digest}
        elif kind == "directory":
            (mode,) = fields
            result[name] = {"kind": "directory", "mode": mode}
        elif kind == "symlink":
            (target,) = fields
            result[name] = {"kind": "symlink", "target": target}
        else:
            raise ValueError("invalid_inventory_row")
    return result


def _publish_exclusive(source, destination):
    if os.name == "nt":
        # Windows rename fails if the destination exists.
        os.rename(source, destination)
        return
    source_bytes = os.fsencode(source)
    destination_bytes = os.fsencode(destination)
    libc = ctypes.CDLL(None, use_errno=True)
    if sys.platform == "darwin":
        rename_exclusive = libc.renamex_np
        rename_exclusive.argtypes = (ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint)
        rename_exclusive.restype = ctypes.c_int
        result = rename_exclusive(source_bytes, destination_bytes, 0x00000004)
    elif hasattr(libc, "renameat2"):
        rename_exclusive = libc.renameat2
        rename_exclusive.argtypes = (
            ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint
        )
        rename_exclusive.restype = ctypes.c_int
        result = rename_exclusive(-100, source_bytes, -100, destination_bytes, 1)
    else:
        raise OSError("atomic_exclusive_rename_unavailable")
    if result != 0:
        code = ctypes.get_errno()
        raise OSError(code, "atomic_exclusive_rename_failed")


def stage(payload_root: Path, output_root: Path) -> dict:
    work, _ = _existing_directory(WORK)
    payload = _canonical_absolute(payload_root)
    _reject_reparse_chain(payload)
    payload_info = payload.lstat()
    if not stat.S_ISDIR(payload_info.st_mode) or payload.resolve(strict=True) != payload:
        raise ValueError("canonical_payload_required")
    if not _is_within(payload, work):
        raise ValueError("payload_outside_work")

    output = _canonical_absolute(output_root)
    if os.path.lexists(output):
        raise ValueError("output_already_exists")
    parent, _ = _existing_directory(output.parent)
    if not _is_within(parent, work):
        raise ValueError("output_outside_work")
    if _is_within(parent, payload):
        raise ValueError("output_inside_payload")

    source_summary, source_rows = macos_payload.inventory(payload)
    root_identity = macos_payload.identity(payload_info)
    root_mode = stat.S_IMODE(payload_info.st_mode)
    lock_path, _ = _existing_directory(RUNTIME / "engine-deps")
    lock = lock_path / "uv.lock"
    _reject_reparse_chain(lock)
    _, lock_hash = _file_snapshot(lock)

    overlay_sources = {}
    overlay_before = {}
    for name in OVERLAY_NAMES:
        source = RUNTIME / name
        _reject_reparse_chain(source)
        info, digest = _file_snapshot(source)
        overlay_sources[name] = source
        overlay_before[name] = (
            macos_payload.identity(info),
            digest,
            stat.S_IMODE(info.st_mode),
            info.st_size,
        )

    temporary = Path(tempfile.mkdtemp(prefix=".macos-stage-", dir=parent))
    if os.name == "posix":
        os.chmod(temporary, 0o700)
    staged_payload = temporary / "payload"
    shutil.copytree(payload, staged_payload, symlinks=True, copy_function=shutil.copy2)
    copied_summary, copied_rows = macos_payload.inventory(staged_payload)
    if (copied_summary, copied_rows) != (source_summary, source_rows):
        raise ValueError("payload_copy_mismatch")

    overlays = {}
    for name, source in overlay_sources.items():
        _, before_hash, before_mode, before_size = overlay_before[name]
        destination = temporary / name
        shutil.copy2(source, destination, follow_symlinks=False)
        copied_info, copied_hash = _file_snapshot(destination)
        source_info, after_hash = _file_snapshot(source)
        if (
            copied_hash != before_hash
            or copied_info.st_size != before_size
            or stat.S_IMODE(copied_info.st_mode) != before_mode
            or after_hash != before_hash
            or macos_payload.identity(source_info) != overlay_before[name][0]
            or stat.S_IMODE(source_info.st_mode) != before_mode
        ):
            raise ValueError("overlay_copy_mismatch")
        overlays[name] = copied_hash

    after_summary, after_rows = macos_payload.inventory(payload)
    final_payload_info = payload.lstat()
    if (
        (after_summary, after_rows) != (source_summary, source_rows)
        or macos_payload.identity(final_payload_info) != root_identity
    ):
        raise ValueError("payload_source_changed")
    _, final_lock_hash = _file_snapshot(lock)
    if final_lock_hash != lock_hash:
        raise ValueError("dependency_lock_changed")

    manifest = {
        "schema_version": 1,
        "platform": "darwin-arm64",
        "engine_version": "0.44.0",
        "python_version": "3.13.13",
        "dependency_lock_sha256": lock_hash,
        "python": "python/bin/python3.13",
        "site_packages": "site-packages",
        "root_mode": root_mode,
        "entries": _entries(source_rows),
        "overlays": overlays,
    }
    manifest_bytes = json.dumps(manifest, ensure_ascii=True, indent=2, sort_keys=True).encode("utf-8") + b"\n"
    manifest_path = temporary / "manifest.json"
    with manifest_path.open("xb") as stream:
        stream.write(manifest_bytes)
    if os.name == "posix":
        os.chmod(manifest_path, 0o600)

    _publish_exclusive(temporary, output)
    return {
        "manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
        "payload_files": source_summary["files"],
        "payload_bytes": source_summary["bytes"],
        "source_payload_unchanged": True,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--payload", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        result = stage(args.payload, args.output)
    except (OSError, ValueError, RuntimeError):
        print('{"error":"staging_failed"}', file=sys.stderr)
        return 2
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

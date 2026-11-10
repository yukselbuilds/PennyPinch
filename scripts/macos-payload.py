"""Read-only, relocatable inventory of the macOS engine and its file links."""

import hashlib
import json
import os
from pathlib import Path
import stat
import sys


MAX_FILES = 100_000
MAX_BYTES = 2 * 1024 * 1024 * 1024


def identity(info):
    stable = (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns)
    # Windows stat and fstat expose different ctime values after a rewrite.
    return stable + ((info.st_ctime_ns,) if os.name == "posix" else ())


def trusted_entry(path):
    info = path.lstat()
    if getattr(info, "st_file_attributes", 0) & 0x400 and not stat.S_ISLNK(info.st_mode):
        raise ValueError("reparse_point_rejected")
    if os.name == "posix" and not stat.S_ISLNK(info.st_mode):
        if info.st_uid != os.geteuid() or info.st_mode & 0o022:
            raise ValueError("untrusted_payload_entry")
    if stat.S_ISREG(info.st_mode) and info.st_nlink != 1:
        raise ValueError("hardlink_payload_entry")
    return info


def regular_hash(path, expected):
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_BINARY", 0)
    with os.fdopen(os.open(path, flags), "rb") as stream:
        opened = os.fstat(stream.fileno())
        if not stat.S_ISREG(opened.st_mode) or identity(opened) != identity(expected):
            raise ValueError("payload_changed_during_read")
        digest = hashlib.sha256()
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
        if identity(os.fstat(stream.fileno())) != identity(expected):
            raise ValueError("payload_changed_during_read")
    if identity(path.lstat()) != identity(expected):
        raise ValueError("payload_replaced_during_read")
    return digest.hexdigest()


def measure(root):
    root = Path(root)
    if not root.is_absolute():
        raise ValueError("payload_root_must_be_absolute")
    root_info = trusted_entry(root)
    if not stat.S_ISDIR(root_info.st_mode):
        raise ValueError("payload_root_is_not_a_directory")
    for ancestor in root.parents:
        if ancestor.is_symlink():
            raise ValueError("symlink_payload_ancestor")
    root = root.resolve(strict=True)
    pending = [root]
    directories = []
    names = set()
    rows = []
    file_count = 0
    byte_count = 0
    while pending:
        directory = pending.pop()
        info = trusted_entry(directory)
        if not stat.S_ISDIR(info.st_mode):
            raise ValueError("payload_directory_replaced")
        directories.append((directory, identity(info)))
        for path in directory.iterdir():
            name = path.relative_to(root).as_posix()
            key = name.casefold()
            if key in names:
                raise ValueError("case_collision")
            names.add(key)
            private = path.name.casefold()
            if private in ("auth.json", "credentials.json", ".env") or private.startswith(".env."):
                raise ValueError("private_input_rejected")
            entry = trusted_entry(path)
            mode = stat.S_IMODE(entry.st_mode)
            if stat.S_ISDIR(entry.st_mode):
                pending.append(path)
                rows.append((name, "directory", mode))
                continue
            if stat.S_ISLNK(entry.st_mode):
                target = os.readlink(path)
                try:
                    resolved = path.resolve(strict=True)
                except (OSError, RuntimeError) as error:
                    raise ValueError("invalid_payload_link") from error
                if Path(target).is_absolute() or not resolved.is_relative_to(root) or not resolved.is_file():
                    raise ValueError("payload_link_outside_regular_file")
                if identity(path.lstat()) != identity(entry):
                    raise ValueError("payload_link_replaced")
                rows.append((name, "symlink", target))
            elif stat.S_ISREG(entry.st_mode):
                if entry.st_size > MAX_BYTES - byte_count:
                    raise ValueError("payload_too_large")
                rows.append((name, "file", mode, entry.st_size, regular_hash(path, entry)))
                byte_count += entry.st_size
            else:
                raise ValueError("non_regular_input")
            file_count += 1
            if file_count > MAX_FILES:
                raise ValueError("too_many_payload_files")
        if len(names) > MAX_FILES:
            raise ValueError("too_many_payload_entries")
    for directory, expected in directories:
        current = trusted_entry(directory)
        if not stat.S_ISDIR(current.st_mode) or identity(current) != expected:
            raise ValueError("payload_directory_changed")
    encoded = json.dumps(sorted(rows), ensure_ascii=True, separators=(",", ":")).encode("ascii")
    return {
        "tree_sha256": hashlib.sha256(encoded).hexdigest(),
        "files": file_count,
        "bytes": byte_count,
    }


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: macos-payload.py <absolute-payload-root>")
    try:
        print(json.dumps(measure(Path(sys.argv[1])), sort_keys=True, separators=(",", ":")))
    except (OSError, ValueError):
        sys.exit("macOS payload inventory failed: unsafe or changed input")

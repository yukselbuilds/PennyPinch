"""Read-only, relocatable inventory of a Unix engine payload and its file links."""

import argparse
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


def trusted_entry(path, *, owner_uid=None):
    info = path.lstat()
    if getattr(info, "st_file_attributes", 0) & 0x400 and not stat.S_ISLNK(info.st_mode):
        raise ValueError("reparse_point_rejected")
    if os.name == "posix" and not stat.S_ISLNK(info.st_mode):
        expected_uid = os.geteuid() if owner_uid is None else owner_uid
        if info.st_uid != expected_uid or info.st_mode & 0o022:
            raise ValueError(
                f"untrusted_payload_entry owner_match={info.st_uid == expected_uid} "
                f"mode={stat.S_IMODE(info.st_mode):04o}"
            )
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


def inventory(root, *, case_sensitive=False, owner_uid=None):
    root = Path(root)
    if not root.is_absolute():
        raise ValueError("payload_root_must_be_absolute")
    root_info = trusted_entry(root, owner_uid=owner_uid)
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
        info = trusted_entry(directory, owner_uid=owner_uid)
        if not stat.S_ISDIR(info.st_mode):
            raise ValueError("payload_directory_replaced")
        directories.append((directory, identity(info)))
        for path in directory.iterdir():
            name = path.relative_to(root).as_posix()
            key = name if case_sensitive else name.casefold()
            if key in names:
                raise ValueError("case_collision")
            names.add(key)
            private = path.name.casefold()
            if private in ("auth.json", "credentials.json", ".env") or private.startswith(".env."):
                raise ValueError("private_input_rejected")
            entry = trusted_entry(path, owner_uid=owner_uid)
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
        current = trusted_entry(directory, owner_uid=owner_uid)
        if not stat.S_ISDIR(current.st_mode) or identity(current) != expected:
            raise ValueError("payload_directory_changed")
    ordered = sorted(rows)
    encoded = json.dumps(ordered, ensure_ascii=True, separators=(",", ":")).encode("ascii")
    return {
        "tree_sha256": hashlib.sha256(encoded).hexdigest(),
        "files": file_count,
        "bytes": byte_count,
    }, ordered


def measure(root, *, case_sensitive=False, owner_uid=None):
    return inventory(root, case_sensitive=case_sensitive, owner_uid=owner_uid)[0]


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("payload", type=Path)
    parser.add_argument("--case-sensitive", action="store_true")
    parser.add_argument("--root-owned", action="store_true")
    args = parser.parse_args()
    if args.root_owned and sys.platform != "linux":
        parser.error("Use --root-owned on Linux.")
    try:
        owner_uid = 0 if args.root_owned else None
        print(json.dumps(measure(args.payload, case_sensitive=args.case_sensitive, owner_uid=owner_uid),
                         sort_keys=True, separators=(",", ":")))
    except ValueError as error:
        sys.exit(f"Payload inventory failed: {error}")
    except OSError:
        sys.exit("Payload inventory failed: unsafe or changed input")

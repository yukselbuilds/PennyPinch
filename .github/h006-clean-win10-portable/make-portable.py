from hashlib import sha256
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
from zipfile import ZIP_DEFLATED, ZipFile

root = Path.cwd().resolve(strict=True)
work = root / "portable-build"
work.mkdir()
bundle = work / "files"
bundle.mkdir()
archive = root / "preview.zip"
installer = work / "preview8-setup.exe"
destination = root / "answer"
overlay = Path(sys.argv[1]).resolve(strict=True)
expected_helper = sys.argv[2]
expected_installer = "1bafc5fb7404bc9bd61f134a0072f571e184ce4c5a8621331430155361d83ea1"
expected_archive = "6fc4ae34a1a2b9404f0db1b7ea2763fd75b148cce0c0b2fd4fb0b528c85374be"
expected_executable = "3b9e27afc01d07efd13d0aa7602209da5ab25dadbd659a09849dc3b92c0f5cd2"
exe_name = "headroom-desktop.exe"
helper_name = "runtime/desktop_helper.py"
receipt_name = "package-inputs.json"


def digest(file):
    with file.open("rb") as source:
        from hashlib import file_digest
        return file_digest(source, "sha256").hexdigest()


def read(file):
    return json.loads(file.read_bytes())


assert digest(archive) == expected_archive
with ZipFile(archive) as zipped:
    entry, = (v for v in zipped.infolist() if v.filename == "PennyPinch_0.1.1-preview.8_x64-setup.exe")
    assert entry.file_size == 125356612
    with zipped.open(entry) as original, installer.open("xb") as target:
        shutil.copyfileobj(original, target)
assert digest(installer) == expected_installer
extract = shutil.which("7z") or shutil.which("7zz")
assert extract
subprocess.run([extract, "x", "-y", f"-o{bundle}", "-x!$PLUGINSDIR", "-x!$TEMP", str(installer)],
               capture_output=True, check=True, timeout=85)
members = {}
for item in bundle.rglob("*"):
    assert not item.is_symlink()
    if item.is_file():
        members[item.relative_to(bundle).as_posix()] = item
receipt = read(members[receipt_name])
pinned = receipt["resources"]
assert len(pinned) == 19491
assert members.keys() == pinned.keys() | {exe_name, receipt_name}
assert all(digest(members[k]) == v for k, v in pinned.items())
assert digest(members.pop(exe_name)) == expected_executable
helper_sha = digest(overlay)
assert re.fullmatch(r"[0-9a-f]{64}", expected_helper) and helper_sha == expected_helper
shutil.copyfile(overlay, members[helper_name])
assert digest(members[helper_name]) == helper_sha
receipt["resources"][helper_name] = helper_sha
receipt["inputs"]["runtime/desktop_helper.py"] = helper_sha
members[receipt_name].write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
output = destination / "portable.zip"
with ZipFile(output, "x", compression=ZIP_DEFLATED, compresslevel=1) as zipped:
    for name, path in sorted(members.items()):
        zipped.write(path, name)
facts = {"source_archive_sha256": expected_archive, "source_installer_sha256": expected_installer,
         "archive_scope": "engine_only", "candidate_helper_sha256": helper_sha,
         "portable_archive_sha256": digest(output), "portable_archive_bytes": output.stat().st_size,
         "resource_receipt_sha256": digest(members[receipt_name]), "file_count": len(members)}
(destination / "bundle.json").write_text(json.dumps(facts, separators=(",", ":")) + "\n", encoding="utf-8")
print("::notice title=H037 verified portable package::" + json.dumps(facts, separators=(",", ":")))

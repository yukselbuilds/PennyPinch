from hashlib import file_digest
import json
from pathlib import Path
import subprocess
from zipfile import ZIP_DEFLATED, ZipFile


def sha(path):
    with path.open("rb") as file:
        return file_digest(file, "sha256").hexdigest()


answer = Path("answer").resolve(strict=True)
stock = json.loads((answer / "candidate.json").read_bytes())
bundle = Path("extracted-portable").resolve()
assert not bundle.exists()
installer = answer / "setup.exe"
assert sha(installer) == stock["installer_sha256"]
subprocess.run(["7z", "x", "-y", f"-o{bundle}", "-x!$PLUGINSDIR", "-x!$TEMP", str(installer)],
               check=True, capture_output=True, timeout=105)
members = {}
for item in bundle.rglob("*"):
    assert not item.is_symlink()
    if item.is_file():
        members[item.relative_to(bundle).as_posix()] = item
receipt = members["package-inputs.json"]
assert sha(receipt) == stock["resource_receipt_sha256"]
resources = json.loads(receipt.read_bytes())["resources"]
assert len(resources) == stock["resource_count"]
assert set(members) == set(resources) | {"headroom-desktop.exe", "package-inputs.json"}
assert sha(members.pop("headroom-desktop.exe")) == stock["executable_sha256"]
assert all(sha(members[name]) == digest for name, digest in resources.items())
helper_sha = sha(members["runtime/desktop_helper.py"])
assert helper_sha == stock["helper_sha256"]
output = answer / "portable.zip"
with ZipFile(output, "x", compression=ZIP_DEFLATED, compresslevel=1) as zip_file:
    for name, source in sorted(members.items()):
        zip_file.write(source, name)
    assert zip_file.testzip() is None
descriptor = {"archive_scope": "engine_only", "candidate_helper_sha256": helper_sha,
              "resource_receipt_sha256": sha(receipt), "portable_archive_bytes": output.stat().st_size,
              "portable_archive_sha256": sha(output), "file_count": len(members)}
(answer / "bundle.json").write_text(json.dumps(descriptor) + "\n", encoding="utf-8")
installer.unlink()
print("::notice title=Windows 11 verified portable bundle::" + json.dumps(descriptor, separators=(",", ":")))

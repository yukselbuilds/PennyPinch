from hashlib import file_digest
import json
from pathlib import Path
import shutil
import subprocess
import sys
from zipfile import ZipFile


root = Path.cwd().resolve(strict=True)
assert root == Path(__file__).resolve(strict=True).parents[2]
work = root / "work" / "h037-clean-build"
assert work.resolve().is_relative_to(root / "work")
assert work.is_dir()
zip_path = (work / "preview.zip").resolve(strict=True)
assert zip_path.is_relative_to(work)


def sha(path):
    with path.open("rb") as stream:
        return file_digest(stream, "sha256").hexdigest()


assert sha(zip_path) == "6fc4ae34a1a2b9404f0db1b7ea2763fd75b148cce0c0b2fd4fb0b528c85374be"
installer = work / "baseline-setup.exe"
with ZipFile(zip_path) as archive:
    with archive.open("PennyPinch_0.1.1-preview.8_x64-setup.exe") as source, installer.open("xb") as target:
        shutil.copyfileobj(source, target)
assert sha(installer) == "1bafc5fb7404bc9bd61f134a0072f571e184ce4c5a8621331430155361d83ea1"
extract = Path(sys.argv[1]).resolve(strict=True)
unpacked = work / "unpacked"
subprocess.run([str(extract), "x", "-y", f"-o{unpacked}", "-x!$PLUGINSDIR", "-x!$TEMP", str(installer)],
               capture_output=True, check=True, timeout=240)
source_payload = (unpacked / "runtime" / "payload").resolve(strict=True)
assert source_payload.is_relative_to(work)
manifest = json.loads((root / "runtime/manifest.json").read_bytes())["engine"]
assert manifest["payload_root"] == "../work/proofs/packaging/artifacts/headroom-engine-0.45.0/runtime"
destination = (root / "runtime" / manifest["payload_root"]).resolve()
assert destination.is_relative_to(root / "work") and not destination.exists()
assert sha(source_payload / manifest["python"]) == manifest["python_sha256"]
shutil.copytree(source_payload, destination)
subprocess.run([str(destination / manifest["python"]), "-I", "-B", "scripts/package-windows.py"],
               cwd=root, check=True, timeout=420)

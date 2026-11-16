"""Fetch pinned public candidates into one owned pilot directory; never install hooks."""
import base64
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import tarfile
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
RESEARCH = ROOT / "work/proofs/research/context-tools-2026-11-16"
DIET_COMMIT = "3899b48dbf41ecc9c0e29aaec8634900101c52d2"
MODE_VERSION = "2.2.0"


def fetch(url):
    with urllib.request.urlopen(url, timeout=30) as response:
        return response.read(16 * 1024 * 1024 + 1)


def prepare(directory):
    directory = Path(directory).resolve()
    directory.relative_to(ROOT / "work/proofs/context-tools")
    directory.mkdir(parents=True, exist_ok=False)
    (directory / "tasks-before.bin").write_bytes((ROOT / "TASKS.md").read_bytes())
    diet = directory / "diet"
    metadata = {"diet": {"commit": DIET_COMMIT, "version": "0.5.0", "files": {}}, "mode": {}}
    files = [f"src/{name}.ts" for name in ("tokens", "dedupe", "extract", "summarize", "pack", "completeness")]
    files += [f"tests/{name}.test.js" for name in ("tokens", "dedupe", "extract", "summarize", "pack", "completeness")]
    files += ["src/cli.ts", "src/server.ts", "LICENSE", "package.json"]
    for name in files:
        cached = RESEARCH / "context-diet/source" / name
        url = f"https://raw.githubusercontent.com/techdevfun/context-diet/{DIET_COMMIT}/{name}"
        data = cached.read_bytes() if cached.is_file() else fetch(url)
        if len(data) > 1024 * 1024:
            raise ValueError("diet_source_size")
        target = diet / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        metadata["diet"]["files"][name] = {"url": url, "sha256": hashlib.sha256(data).hexdigest()}
    registry = json.loads(fetch(f"https://registry.npmjs.org/context-mode/{MODE_VERSION}"))
    if registry["name"] != "context-mode" or registry["version"] != MODE_VERSION:
        raise ValueError("package_identity")
    dist = registry["dist"]
    data = fetch(dist["tarball"])
    if len(data) > 16 * 1024 * 1024:
        raise ValueError("mode_package_size")
    integrity = "sha512-" + base64.b64encode(hashlib.sha512(data).digest()).decode("ascii")
    if integrity != dist["integrity"]:
        raise ValueError("package_integrity")
    (directory / "context-mode.tgz").write_bytes(data)
    mode = directory / "mode"
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        members = archive.getmembers()
        if sum(member.size for member in members) > 32 * 1024 * 1024:
            raise ValueError("package_expanded_size")
        for member in members:
            name = PurePosixPath(member.name)
            if name.is_absolute() or ".." in name.parts or not name.parts or name.parts[0] != "package":
                raise ValueError("package_path")
            if member.isdir():
                continue
            if not member.isfile() or "\\" in member.name:
                raise ValueError("package_member_type")
            target = mode.joinpath(*name.parts[1:])
            target.resolve().relative_to(mode.resolve())
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(archive.extractfile(member).read())
    metadata["mode"] = {"version": MODE_VERSION, "integrity": integrity, "tarball": dist["tarball"],
                        "sha256": hashlib.sha256(data).hexdigest(), "files": {
                            str(p.relative_to(mode)).replace("\\", "/"): hashlib.sha256(p.read_bytes()).hexdigest()
                            for p in sorted(mode.rglob("*")) if p.is_file()}}
    compiler = ROOT / "node_modules/typescript/bin/tsc"
    if not compiler.is_file():
        raise ValueError("existing_project_typescript_required")
    node = shutil.which("node")
    if not node:
        raise ValueError("existing_node_required")
    command = [node, str(compiler), "--ignoreConfig", "--noCheck", "--module", "nodenext", "--target", "es2022",
               "--rootDir", str(diet / "src"), "--outDir", str(diet / "dist"),
               *[str(p) for p in sorted((diet / "src").glob("*.ts"))]]
    temporary = directory / "compiler-temp"
    temporary.mkdir()
    env = dict(os.environ, TEMP=str(temporary), TMP=str(temporary), NODE_DISABLE_COMPILE_CACHE="1")
    compiled = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, timeout=30)
    (diet / "compile.json").write_text(json.dumps({"command": command, "exit": compiled.returncode, "typecheck": False,
                                                  "reason": "unchanged upstream filters transpiled; functional tests verify behavior"}, indent=2) + "\n", encoding="utf-8")
    if compiled.returncode:
        raise ValueError("diet_transpilation_failed")
    (directory / "packages.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"directory": str(directory), "diet_files": len(files),
                      "mode_files": len(metadata["mode"]["files"]), "integrity_verified": True}))


if __name__ == "__main__":
    prepare(sys.argv[1])

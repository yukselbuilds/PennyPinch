"""Acquire the pinned OSS evaluation binary and relevant primary sources, inside work only."""
import hashlib
import io
import json
from pathlib import Path
import sys
import urllib.request
import uuid
import zipfile

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
PLAN = json.loads((HERE / "plan.json").read_text(encoding="utf-8"))
DEST = ROOT / "work/proofs/rtk/toolchain" / PLAN["version"]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def fetch(url, limit=2 * 1024 * 1024):
    request = urllib.request.Request(url, headers={"User-Agent": "Headroom-H043-local-evaluation"})
    with urllib.request.urlopen(request, timeout=30) as response:
        data = response.read(limit + 1)
    assert len(data) <= limit, "download_size_limit"
    return data


def main():
    assert Path.cwd().resolve() == ROOT and ROOT == Path(r"D:\Projeler\Headroom"), "canonical_root_required"
    assert sys.argv[1:] == ["--run"], "explicit_prepare_required"
    if DEST.exists():
        receipt = json.loads((DEST / "receipt.json").read_text(encoding="utf-8"))
        assert receipt["plan_sha256"] == sha((HERE / "plan.json").read_bytes()), "plan_changed"
        assert all(sha((DEST / name).read_bytes()) == digest for name, digest in receipt["files"].items()), "toolchain_changed"
        print(json.dumps({"prepared": True, "reused": True, "path": DEST.relative_to(ROOT).as_posix()}))
        return
    api = "https://api.github.com/repos/rtk-ai/rtk/"
    tag = json.loads(fetch(api + "git/ref/tags/v" + PLAN["version"]))
    assert tag["object"] == {"sha": PLAN["commit"], "type": "commit", "url": api + "git/commits/" + PLAN["commit"]}, "tag_commit_changed"
    release = json.loads(fetch(api + "releases/tags/v" + PLAN["version"]))
    asset = next(row for row in release["assets"] if row["name"] == PLAN["asset"])
    assert asset["digest"] == "sha256:" + PLAN["asset_sha256"], "asset_digest_changed"
    checksum_asset = next(row for row in release["assets"] if row["name"] == "checksums.txt")
    checksums = fetch(checksum_asset["browser_download_url"])
    assert "sha256:" + sha(checksums) == checksum_asset["digest"], "checksum_file_digest"
    expected_line = PLAN["asset_sha256"] + "  " + PLAN["asset"]
    assert expected_line in checksums.decode("utf-8").splitlines(), "archive_checksum_missing"
    archive = fetch(asset["browser_download_url"], 8 * 1024 * 1024)
    assert sha(archive) == PLAN["asset_sha256"] and len(archive) == asset["size"], "archive_digest_mismatch"
    stage = DEST.parent / ("acquire-" + uuid.uuid4().hex)
    stage.mkdir(parents=True)
    (stage / PLAN["asset"]).write_bytes(archive)
    (stage / "checksums.txt").write_bytes(checksums)
    # Never extract arbitrary upstream paths or a standard/engine/proxy executable.
    with zipfile.ZipFile(io.BytesIO(archive)) as package:
        exe = [info for info in package.infolist() if Path(info.filename).name == "rtk.exe"]
        assert len(exe) == 1 and exe[0].file_size < 32 * 1024 * 1024, "unexpected_oss_archive"
        (stage / "rtk.exe").write_bytes(package.read(exe[0]))
        archive_members = [info.filename for info in package.infolist()]
    tree = json.loads(fetch(api + "git/trees/" + PLAN["commit"] + "?recursive=1"))
    assert not tree.get("truncated"), "source_tree_truncated"
    selected = {"Cargo.toml", "LICENSE", "README.md", "src/lib.rs", "src/core/user_dirs.rs",
                "src/core/config.rs", "src/core/tracking.rs", "src/core/tee.rs", "src/core/tee_file.rs",
                "src/core/retriever.rs", "src/core/runner.rs", "src/core/telemetry.rs",
                "src/core/telemetry_cmd.rs", "src/hooks/hook_cmd.rs", "src/hooks/decision.rs"}
    prefixes = ("src/cmds/git/", "src/cmds/rust/", "src/cmds/system/", "hooks/codex/",
                "hooks/claude/", "hooks/opencode/")
    for item in tree["tree"]:
        name = item["path"]
        if item["type"] != "blob" or not (name in selected or name.startswith(prefixes)):
            continue
        data = fetch("https://raw.githubusercontent.com/rtk-ai/rtk/" + PLAN["commit"] + "/" + name)
        target = stage / "source" / name
        assert target.resolve().is_relative_to(stage.resolve()), "source_path_escape"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    assert (stage / "source/LICENSE").read_text(encoding="utf-8").lstrip().startswith("Apache License"), "oss_license_required"
    files = {p.relative_to(stage).as_posix(): sha(p.read_bytes()) for p in sorted(stage.rglob("*")) if p.is_file()}
    receipt = {"plan_sha256": sha((HERE / "plan.json").read_bytes()), "commit": PLAN["commit"],
               "edition": PLAN["edition"], "release_published_at": release["published_at"],
               "archive_members": archive_members, "files": files, "signature": "not_provided_by_upstream",
               "verification": "Release API SHA-256 and published checksums over HTTPS; not a signature"}
    (stage / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    stage.rename(DEST)
    print(json.dumps({"prepared": True, "reused": False, "files": len(files), "path": DEST.relative_to(ROOT).as_posix()}))


if __name__ == "__main__":
    main()

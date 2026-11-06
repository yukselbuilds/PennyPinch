"""Prepare pinned public tokenizer data for the local engine; --check never downloads."""
import concurrent.futures
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[1]
assert ROOT == Path.cwd().resolve(), "Run from the project root"
assert sys.argv[1:] in ([], ["--check"]), "Only --check is supported"
manifest_path = ROOT / "runtime/manifest.json"
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
payload = (manifest_path.parent / manifest["engine"]["payload_root"]).resolve(strict=True)
assert payload.is_relative_to(ROOT), "Payload must be inside this project"
spec = importlib.util.spec_from_file_location("tokenizer_pins", ROOT / "runtime/desktop_controller.py")
controller = importlib.util.module_from_spec(spec)
spec.loader.exec_module(controller)
cache = payload / "tiktoken-cache"
if cache.exists():
    assert not cache.is_symlink() and cache.resolve().parent == payload
else:
    cache.mkdir()
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
sources = [Path(__file__), manifest_path, ROOT / "runtime/desktop_controller.py", payload / "Lib/site-packages/tiktoken_ext/openai_public.py"]
fingerprint = lambda: {str(p.relative_to(ROOT)): sha(p) for p in sources}
before = fingerprint()

def prepare(source):
    url, expected = source
    key = hashlib.sha1(url.encode("utf-8")).hexdigest()
    target = cache / key
    assert not target.is_symlink()
    if not target.exists():
        assert not sys.argv[1:], "Tokenizer data is missing; run without --check to prepare it"
        with urllib.request.urlopen(url, timeout=30) as response:
            data = response.read(10 * 1024 * 1024 + 1)
        assert len(data) <= 10 * 1024 * 1024 and hashlib.sha256(data).hexdigest() == expected, "Tokenizer checksum mismatch"
        temporary = cache / (key + "." + str(uuid.uuid4()) + ".tmp")
        with temporary.open("xb") as stream:
            stream.write(data)
        assert not target.exists(), "Tokenizer target changed during preparation"
        temporary.replace(target)
    assert target.is_file() and sha(target) == expected, "Existing tokenizer checksum mismatch"
    return {"file": key, "sha256": expected, "bytes": target.stat().st_size}

with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
    assets = list(pool.map(prepare, controller.TOKENIZER_SOURCES))
assert controller.verify_tokenizer_cache(payload) == cache
# Every built-in encoding must work with network connections prohibited.
bootstrap = (
    "import json,os,site,socket,sys;"
    f"sys.prefix=sys.exec_prefix={str(payload)!r};site.addsitedir({str(payload/'Lib/site-packages')!r});"
    f"os.environ['TIKTOKEN_CACHE_DIR']={str(cache)!r};"
    "socket.socket.connect=lambda *args:(_ for _ in ()).throw(AssertionError('offline_tokenizer_network_attempt'));"
    "import tiktoken;names=tiktoken.list_encoding_names();"
    "counts={name:len(tiktoken.get_encoding(name).encode('Headroom yerel tokenizer doğrulaması 123.')) for name in names};"
    "assert len(counts)==7 and all(counts.values());print(json.dumps({'offline':True,'encodings':names}))"
)
process = subprocess.run([str(payload/manifest["engine"]["python"]), "-I", "-c", bootstrap], cwd=ROOT, capture_output=True, text=True, timeout=30,
                         creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
assert process.returncode == 0, "Offline tokenizer smoke failed"
smoke = json.loads(process.stdout)
after = fingerprint()
assert before == after
proof_dir = ROOT / "work/proofs/tokenizers"
proof_dir.mkdir(parents=True, exist_ok=True)
proof = proof_dir / ("prepare-" + str(uuid.uuid4()) + ".json")
proof.write_text(json.dumps({"before": before, "after": after, "assets": assets, "smoke": smoke,
                             "python": sys.version, "check_only": bool(sys.argv[1:])}, indent=2) + "\n", encoding="utf-8")
print(json.dumps({"passed": True, "assets": len(assets), "bytes": sum(v["bytes"] for v in assets), "encodings": len(smoke["encodings"]), "offline_smoke": True, "proof": str(proof.relative_to(ROOT))}))

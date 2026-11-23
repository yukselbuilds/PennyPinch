#!/usr/bin/env bash
# Assemble native test resources inside this checkout; no artifacts are uploaded.
set -euo pipefail
umask 077
test "$(uname -m)" = arm64
test "$(pwd -P)" = "$GITHUB_WORKSPACE"
mkdir -p work
stage="$(mktemp -d "$GITHUB_WORKSPACE/work/m4.XXXXXX")"
log="$stage/prepare.log"
trap 'rm -f "$log"' EXIT

download() {
  curl --proto '=https' --proto-redir '=https' --location --fail --silent --show-error \
    --max-time 120 --output "$1" "$2"
  printf '%s  %s\n' "$3" "$1" | shasum -a 256 -c -
}
download "$stage/python.tgz" \
  'https://github.com/astral-sh/python-build-standalone/releases/download/20260504/cpython-3.13.13%2B20260504-aarch64-apple-darwin-install_only_stripped.tar.gz' \
  '001872dfdea7fdb5b985cbe03a5b15d1ba7d8a5df6056615d433423e2adb27c5'
download "$stage/uv.tgz" \
  'https://github.com/astral-sh/uv/releases/download/0.13.5/uv-aarch64-apple-darwin.tar.gz' \
  '743f85035a1996ecb350659e01cd9faea9709829be09b1b4b03c73eaf3c9cd73'
mkdir "$stage/payload" "$stage/tools"
tar --no-same-permissions -xzf "$stage/python.tgz" -C "$stage/payload"
tar -xzf "$stage/uv.tgz" -C "$stage/tools" --strip-components=1
uv="$stage/tools/uv"
python="$stage/payload/python/bin/python3.13"
test "$(lipo -archs "$python")" = arm64
test "$("$python" -I -B --version)" = 'Python 3.13.13'
test "$(shasum -a 256 runtime/engine-deps/uv.lock | cut -d' ' -f1)" = \
  'e1142cbbeaa139a6a23fcc3b1298af5a38f1f33b03d4833e744631c3015de3e7'
export UV_CACHE_DIR="$stage/uv-cache" UV_NO_MANAGED_PYTHON=1 UV_PYTHON_DOWNLOADS=never
"$uv" export --project runtime/engine-deps --python "$python" --locked --no-dev --no-emit-project \
  --format requirements-txt --output-file "$stage/requirements.txt" \
  > "$log" 2>&1 || { echo '::error::Frozen runtime requirements export failed.'; exit 1; }
"$uv" pip sync --strict --require-hashes --no-build --link-mode copy --python "$python" \
  --target "$stage/payload/site-packages" "$stage/requirements.txt" \
  > "$log" 2>&1 || { echo '::error::Pinned binary wheel install failed.'; exit 1; }
find -P "$stage/payload" -type d -exec chmod go-w {} +
find -P "$stage/payload" -type f -exec chmod go-w {} +

if ! "$python" -I -B -m unittest discover -s tests/runtime -p 'test_macos_*.py' -v > "$log" 2>&1; then
  echo '::error::Native macOS payload/bootstrap/staging tests failed.'
  awk '/^FAIL: / || /^ERROR: / || /^FAILED / { print substr($0, 1, 220) }' "$log"
  exit 1
fi
if grep -qE '^test_.*symlink.* skipped ' "$log"; then
  echo '::error::Native macOS symlink checks were skipped.'
  exit 1
fi
awk '/^Ran [0-9]+ tests/ || /^OK/ { print "Native Python " $0 }' "$log"
"$python" -I -B scripts/stage-macos-runtime.py --payload "$stage/payload" --output "$stage/runtime" \
  > "$stage/staging.json"
manifest_sha="$("$python" -I -B -c 'import json,sys; print(json.load(open(sys.argv[1]))["manifest_sha256"])' "$stage/staging.json")"
[[ "$manifest_sha" =~ ^[0-9a-f]{64}$ ]]
printf '%s\n' \
  "HEADROOM_MACOS_RUNTIME_MANIFEST_SHA256=$manifest_sha" \
  "HEADROOM_MACOS_RUNTIME_ROOT=$stage/runtime" \
  "HEADROOM_MACOS_PROOF_ROOT=$stage" >> "$GITHUB_ENV"
cat "$stage/staging.json"

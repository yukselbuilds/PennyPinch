#!/usr/bin/env bash
# Test the frozen runtime with an isolated provider on the standard Ubuntu runner.
set -euo pipefail
umask 077
test "$(uname -sm)" = 'Linux x86_64'
grep -qx 'VERSION_ID="24.04"' /etc/os-release
test "$(pwd -P)" = "$GITHUB_WORKSPACE"
test "$(git rev-parse HEAD)" = "$GITHUB_SHA"
git diff --quiet HEAD --

mkdir -p work
stage="$(mktemp -d "$GITHUB_WORKSPACE/work/lx.XXXXXX")"
log="$stage/probe.log"
trap 'rm -f -- "$log"' EXIT

download() {
  curl --proto '=https' --proto-redir '=https' --location --fail --silent --show-error \
    --max-time 120 --output "$1" "$2"
  printf '%s  %s\n' "$3" "$1" | sha256sum --check --status
}
download "$stage/python.tgz" \
  'https://github.com/astral-sh/python-build-standalone/releases/download/20260504/cpython-3.13.13%2B20260504-x86_64-unknown-linux-gnu-install_only_stripped.tar.gz' \
  '701c00bb8cc5b172b9c8a31eb6be57b6bc72cfe0addbff8e013db79a28981642'
download "$stage/uv.tgz" \
  'https://github.com/astral-sh/uv/releases/download/0.13.5/uv-x86_64-unknown-linux-gnu.tar.gz' \
  'f68d291c75783122dad58ab77def0e13f3233f358731db5d6d7e8715603b928f'
mkdir "$stage/payload" "$stage/tools"
tar --no-same-permissions -xzf "$stage/python.tgz" -C "$stage/payload"
tar --no-same-permissions -xzf "$stage/uv.tgz" -C "$stage/tools" --strip-components=1
python="$stage/payload/python/bin/python3.13"
uv="$stage/tools/uv"
test "$("$python" -I -B --version)" = 'Python 3.13.13'
test "$("$uv" --version | cut -d' ' -f1,2)" = 'uv 0.13.5'
printf '%s  %s\n' 'e1142cbbeaa139a6a23fcc3b1298af5a38f1f33b03d4833e744631c3015de3e7' \
  runtime/engine-deps/uv.lock | sha256sum --check --status

export UV_NO_CONFIG=1 UV_NO_PROGRESS=1 UV_NO_MANAGED_PYTHON=1 UV_PYTHON_DOWNLOADS=never
export UV_CACHE_DIR="$stage/uv-cache"
"$uv" export --project runtime/engine-deps --python "$python" --locked --no-dev --no-emit-project \
  --format requirements-txt --output-file "$stage/requirements.txt" > "$log" 2>&1 \
  || { echo '::error::Frozen runtime requirements export failed.'; exit 1; }
"$uv" pip sync --strict --require-hashes --no-build --link-mode copy --python "$python" \
  --target "$stage/payload/site-packages" "$stage/requirements.txt" > "$log" 2>&1 \
  || { echo '::error::Pinned binary wheel install failed.'; exit 1; }
# The pinned tiktoken wheel checks the vocabulary hash before caching the data.
mkdir "$stage/payload/tiktoken-cache"
TIKTOKEN_CACHE_DIR="$stage/payload/tiktoken-cache" "$python" -I -B -c '
import site, sys
site.addsitedir(sys.argv[1])
import tiktoken
assert tiktoken.get_encoding("o200k_base").encode("PennyPinch Linux fixture")
' "$stage/payload/site-packages" > "$log" 2>&1 \
  || { echo '::error::Pinned tokenizer preparation failed.'; exit 1; }
find -P "$stage/payload" \( -type d -o -type f \) -exec chmod go-w {} +
mv "$stage/payload" "$stage/relocated"
python="$stage/relocated/python/bin/python3.13"

fingerprint() {
  git rev-parse HEAD 'HEAD^{tree}'
  uname -smr
  grep '^VERSION_ID=' /etc/os-release
  printf '%s\n' "${ImageOS:-unknown}" "${ImageVersion:-unknown}"
  "$python" -I -B --version
  "$uv" --version
  sha256sum "$stage/python.tgz" "$stage/uv.tgz" "$stage/requirements.txt" runtime/engine-deps/uv.lock | cut -d' ' -f1
  "$python" -I -B scripts/macos-payload.py "$stage/relocated"
}
before="$(fingerprint)"
status=0
sudo -n unshare --net /bin/bash -euc '
  ip link set dev lo up
  exec runuser -u "$1" -- env -i PATH=/usr/bin:/bin "$2" -I -B "$3" "$4"
' bash "$(id -un)" "$python" "$GITHUB_WORKSPACE/scripts/probe-linux-engine.py" "$stage" \
  > "$log" 2>&1 || status=$?
after="$(fingerprint)"
test "$before" = "$after"
git diff --quiet HEAD --
digest="$(printf '%s\n' "$before" | sha256sum | cut -d' ' -f1)"
inventory="$(tail -n 1 <<< "$before")"
printf 'Inputs before and after: %s\nPayload inventory: %s\n' "$digest" "$inventory"
awk '/^Linux engine proof( failed)?: / { if (++count <= 2) print substr($0, 1, 240) }' "$log"
if [ "$status" -ne 0 ]; then
  echo '::error::The Linux engine probe failed.'
  exit "$status"
fi
printf '%s\n' '## Linux engine probe' '' "- Commit: $GITHUB_SHA" \
  "- Inputs before and after: $digest" "- Payload inventory: $inventory" \
  '- The relocated engine completed two synthetic Responses streams and stopped after each run.' \
  '- Linux verified the socket peer PID and UID and rejected a foreign peer.' \
  '- The network namespace exposed only loopback.' \
  '- This run checks the engine directly. Desktop management, client configuration, and packaging require separate checks.' \
  >> "$GITHUB_STEP_SUMMARY"

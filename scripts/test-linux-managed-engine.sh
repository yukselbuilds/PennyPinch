#!/usr/bin/env bash
# Run the native runtime manager tests with pinned resources and loopback only.
set -euo pipefail
umask 077
root="$HEADROOM_LINUX_ENGINE_PROOF_ROOT"
runtime="$HEADROOM_LINUX_RUNTIME_ROOT"
test "$runtime" = "$root/runtime"
test "$(sha256sum "$runtime/manifest.json" | cut -d' ' -f1)" = "$HEADROOM_LINUX_RUNTIME_MANIFEST_SHA256"
log="$root/manager.log"
trap 'rm -f -- "$log" "$root/manager-build.json"' EXIT

if ! cargo +1.92.0 test --locked --manifest-path src-tauri/Cargo.toml --features custom-protocol \
  --no-run --message-format=json > "$root/manager-build.json" 2> "$log"; then
  awk '/^error(\[[A-Z][0-9]+\])?:/ && ++count <= 8 { print substr($0, 1, 220) }' "$log"
  exit 1
fi
python="$runtime/payload/python/bin/python3.13"
test_binary="$("$python" -I -B -c '
import json, sys
with open(sys.argv[1]) as stream:
    rows = [json.loads(line) for line in stream]
binary, = [row["executable"] for row in rows if row["reason"] == "compiler-artifact"
           and row["target"]["name"] == "headroom-desktop" and row.get("executable") and row["profile"]["test"]]
print(binary)
' "$root/manager-build.json")"
test -x "$test_binary"
fingerprint() {
  git rev-parse HEAD 'HEAD^{tree}'
  sha256sum "$runtime/manifest.json" "$test_binary" src-tauri/Cargo.lock | cut -d' ' -f1
  "$python" -I -B scripts/macos-payload.py --case-sensitive "$runtime/payload"
}

before="$(fingerprint)"
status=0
sudo -n unshare --net /bin/bash -euc '
  ip link set dev lo up
  exec runuser -u "$1" -- env -i PATH=/usr/bin:/bin "$2" -I -B "$3" --managed "$4" "$5"
' bash "$(id -un)" "$python" "$GITHUB_WORKSPACE/scripts/probe-linux-engine.py" "$root" "$test_binary" \
  > "$log" 2>&1 || status=$?
test "$before" = "$(fingerprint)"
awk '/^test .* \.\.\. (ok|FAILED)$|^test result:|^headroom-macos-error=|^Linux engine proof/ { print substr($0, 1, 220) }
     /panicked at|^assertion|^  left:|^ right:/ && ++count <= 12 { print substr($0, 1, 220) }' "$log"
test "$status" -eq 0
grep -q '^test runtime::manager::macos_tests::managed_app_engine_lifecycle \.\.\. ok$' "$log"
grep -q '^test .*::managed_engine_routes_an_isolated_connection_and_undoes_all_client_configs \.\.\. ok$' "$log"
digest="$(printf '%s\n' "$before" | sha256sum | cut -d' ' -f1)"
printf '%s\n' '## Linux desktop runtime manager' '' "- Input fingerprint before and after: $digest" \
  '- Verified readiness, pause, resume, stop, restart, and conflict with a foreign listener.' \
  '- Verified one local Responses stream through the saved Codex endpoint and the authenticated Unix socket.' \
  '- Verified Apply and byte-exact Undo for isolated Codex, Claude Code, and OpenCode profiles.' >> "$GITHUB_STEP_SUMMARY"

#!/usr/bin/env bash
# Build a Debian package and check its installed lifecycle on an ephemeral runner.
set -euo pipefail
umask 077
trap 'printf "::error::Linux package check failed at line %s.\n" "$LINENO"' ERR

test "${GITHUB_ACTIONS:-}" = true
test "${RUNNER_ENVIRONMENT:-}" = github-hosted
test "$(uname -sm)" = 'Linux x86_64'
grep -qx 'VERSION_ID="24.04"' /etc/os-release
test "$(id -u)" -ne 0
test "$(pwd -P)" = "$GITHUB_WORKSPACE"
test "$(git rev-parse HEAD)" = "$GITHUB_SHA"
git diff --quiet HEAD --

runtime="$HEADROOM_LINUX_RUNTIME_ROOT"
test "$(realpath -e -- "$runtime")" = "$runtime"
case "$runtime" in
  "$GITHUB_WORKSPACE"/work/lx.*/runtime) ;;
  *) exit 2 ;;
esac
manifest_sha="$HEADROOM_LINUX_RUNTIME_MANIFEST_SHA256"
test "$(sha256sum "$runtime/manifest.json" | cut -d' ' -f1)" = "$manifest_sha"

build="$(mktemp -d "$GITHUB_WORKSPACE/work/lp.XXXXXXXX")"
test "$(findmnt --target "$build" --noheadings --output FSTYPE)" = ext4
log="$build/package.log"
config="$build/linux-package.json"
package_name=penny-pinch
installed_runtime=/usr/lib/PennyPinch/runtime
installed_app=/usr/bin/pennypinch
install_owned=0
cleanup() {
  if [[ "$install_owned" == 1 ]]; then
    sudo -n dpkg --purge "$package_name" > /dev/null 2>&1 || :
  fi
  rm -f -- "$log"
}
trap cleanup EXIT

if dpkg-query -W "$package_name" > /dev/null 2>&1; then
  echo '::error::PennyPinch was already registered on the test runner.'
  exit 1
fi
for path in "$installed_app" "${installed_runtime%/*}"; do
  test ! -e "$path" && test ! -L "$path" || exit 1
done

python=/usr/bin/python3
"$python" -I -B - "$runtime" > "$config" <<'PY'
import json
from pathlib import Path
import sys

root = Path.cwd()
print(json.dumps({
    "mainBinaryName": "pennypinch",
    "bundle": {
        "active": True,
        "targets": ["deb"],
        "category": "Utility",
        "shortDescription": "Local context compression and token tracking",
        "icon": ["icons/icon.png"],
        "linux": {"deb": {"files": {
            "/usr/lib/PennyPinch/runtime": sys.argv[1],
            "/usr/share/doc/penny-pinch/copyright": str(root / "LICENSE"),
            "/usr/share/doc/penny-pinch/NOTICE.md": str(root / "NOTICE.md"),
        }}},
    },
}, sort_keys=True))
PY

payload_inventory() {
  local root="$1"
  shift
  "$python" -I -B scripts/macos-payload.py --case-sensitive "$root/payload" "$@"
}
fingerprint() {
  git rev-parse HEAD 'HEAD^{tree}'
  uname -smr
  printf '%s\n' "${ImageOS:-unknown}" "${ImageVersion:-unknown}"
  cargo --version
  rustc --version
  node --version
  pnpm --version
  printf '%s\n' "$CARGO_PROFILE_RELEASE_LTO" "$CARGO_PROFILE_RELEASE_CODEGEN_UNITS"
  sha256sum src-tauri/Cargo.lock pnpm-lock.yaml "$runtime/manifest.json" "$config" | cut -d' ' -f1
  payload_inventory "$runtime"
}
export CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
before="$(fingerprint)"
source_inventory="$(payload_inventory "$runtime")"
if payload_inventory "$runtime" --root-owned > "$log" 2>&1; then
  echo '::error::The installed ownership check accepted a user-owned staging tree.'
  exit 1
fi
grep -Fq 'owner_match=False' "$log"

if ! (umask 022; pnpm tauri build --ci --no-sign --bundles deb --target x86_64-unknown-linux-gnu \
      --config "$config" -- --locked) > "$log" 2>&1; then
  echo '::error::The locked Tauri Debian build failed.'
  awk '/^error|[Ee]rror .* (bundle|config)|^failed to / { if (++count <= 10) print substr($0, 1, 240) }' "$log"
  exit 1
fi
shopt -s nullglob
packages=("$CARGO_TARGET_DIR"/x86_64-unknown-linux-gnu/release/bundle/deb/PennyPinch_*_amd64.deb)
test "${#packages[@]}" = 1
deb="${packages[0]}"
version="$("$python" -I -B -c 'import json; print(json.load(open("src-tauri/tauri.conf.json"))["version"])')"
test "$(dpkg-deb -f "$deb" Package)" = "$package_name"
test "$(dpkg-deb -f "$deb" Version)" = "$version"
test "$(dpkg-deb -f "$deb" Architecture)" = amd64
package_sha="$(sha256sum "$deb" | cut -d' ' -f1)"
package_bytes="$(stat -c '%s' "$deb")"
printf 'Linux Debian build passed. Package SHA-256: %s (%s bytes)\n' "$package_sha" "$package_bytes"

install_owned=1
if ! sudo -n dpkg --install "$deb" > "$log" 2>&1; then
  echo '::error::Debian package installation failed.'
  awk '/^dpkg:|dependency problems|depends on/ { if (++count <= 8) print substr($0, 1, 220) }' "$log"
  exit 1
fi
test "$(dpkg-query -W -f='${Status}' "$package_name")" = 'install ok installed'
test "$(stat -c '%u:%g %a' "$installed_app")" = '0:0 755'
desktop="$(dpkg-query -L "$package_name" | grep -E '^/usr/share/applications/[^/]+\.desktop$')"
test -f "$desktop"
grep -Eq '^Exec=(/usr/bin/)?pennypinch($| )' "$desktop"
cmp --silent LICENSE /usr/share/doc/penny-pinch/copyright
cmp --silent NOTICE.md /usr/share/doc/penny-pinch/NOTICE.md

installed_fingerprint() {
  test -z "$(find -P "$installed_runtime" \( ! -uid 0 -o ! -gid 0 \) -print -quit)" || return 1
  test -z "$(find -P "$installed_runtime" \( -type f -o -type d \) -perm /7022 -print -quit)" || return 1
  test "$(sha256sum "$installed_runtime/manifest.json" | cut -d' ' -f1)" = "$manifest_sha" || return 1
  test "$(payload_inventory "$installed_runtime" --root-owned)" = "$source_inventory" || return 1
  sha256sum "$installed_app" "$installed_runtime/manifest.json" "$installed_runtime"/*.py | cut -d' ' -f1
}
installed_before="$(installed_fingerprint)"
printf 'Linux Debian installation and runtime inventory passed.\n'

profile="$build/profile"
mkdir -m 700 "$profile"
mkdir -m 700 "$profile"/{home,data,config,cache,session,tmp}
printf 'keep\n' > "$profile/data/retained.txt"
status=0
sudo -n unshare --net /bin/bash -euc '
  ip link set dev lo up
  exec runuser -u "$1" -- env -i PATH=/usr/bin:/bin LANG=C.UTF-8 GDK_BACKEND=x11 \
    HOME="$2/home" XDG_DATA_HOME="$2/data" XDG_CONFIG_HOME="$2/config" \
    XDG_CACHE_HOME="$2/cache" XDG_RUNTIME_DIR="$2/session" TMPDIR="$2/tmp" \
    GTK_A11Y=always NO_AT_BRIDGE=0 \
    timeout 240s dbus-run-session -- xvfb-run -a -s "-screen 0 1280x900x24" \
    /usr/bin/python3 -I -B "$3" "$2"
' bash "$(id -un)" "$profile" "$GITHUB_WORKSPACE/scripts/probe-linux-package.py" > "$log" 2>&1 || status=$?
awk '/^headroom-linux-package[- :]/ { if (++count <= 14) print substr($0, 1, 240) }' "$log"
if [[ "$status" != 0 ]]; then
  echo '::error::Installed Linux desktop verification failed.'
  exit "$status"
fi
grep -Fxq 'headroom-linux-package-result=pass' "$log"
test "$installed_before" = "$(installed_fingerprint)"

sudo -n dpkg --purge "$package_name" > "$log" 2>&1
install_owned=0
if dpkg-query -W "$package_name" > /dev/null 2>&1; then
  echo '::error::The test package remains registered.'
  exit 1
fi
for path in "$installed_app" "${installed_runtime%/*}" "$desktop"; do
  test ! -e "$path" && test ! -L "$path" || exit 1
done
test "$(cat "$profile/data/retained.txt")" = keep
test -d "$profile/data/dev.headroom.desktop/runtime"
test "$(sha256sum "$deb" | cut -d' ' -f1)" = "$package_sha"
test "$before" = "$(fingerprint)"
git diff --quiet HEAD --

digest="$(printf '%s\n' "$before" | sha256sum | cut -d' ' -f1)"
printf 'Inputs before and after: %s\n%s\n' "$digest" "$before"
printf '::notice title=Ubuntu package receipt::{"version":"%s","package_bytes":%s,"package_sha256":"%s","manifest_sha256":"%s","input_sha256":"%s"}\n' \
  "$version" "$package_bytes" "$package_sha" "$manifest_sha" "$digest"
printf '%s\n' '## Ubuntu installed package' '' "- Commit: $GITHUB_SHA" \
  "- AMD64 Debian package SHA-256: $package_sha ($package_bytes bytes)" \
  "- Inputs before and after: $digest" "- Engine manifest SHA-256: $manifest_sha" \
  '- The native package installed. The installed runtime matched the staged file inventory.' \
  '- The installed window started, stopped, and restarted the engine. Confirmed exit cleaned up the process and listeners.' \
  '- Package purge removed the installed files and retained the separate user state.' \
  '- The native test used a private profile and loopback only.' >> "$GITHUB_STEP_SUMMARY"

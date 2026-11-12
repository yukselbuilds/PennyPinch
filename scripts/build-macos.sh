#!/bin/bash
# Create a project-local macOS QA bundle with the pinned runtime and private state.
set -euo pipefail
umask 077

test "$(uname -m)" = arm64
test "$#" = 1
test "$(pwd -P)" = "$GITHUB_WORKSPACE"
build_root="$1"
test "$(cd "$build_root" && pwd -P)" = "$build_root"
case "$build_root" in
  "$GITHUB_WORKSPACE"/work/macos-package.*) ;;
  *) exit 2 ;;
esac
test "$(stat -f '%u %Lp' "$build_root")" = "$(id -u) 700"
runtime_root="$HEADROOM_MACOS_RUNTIME_ROOT"
build_python="$runtime_root/payload/python/bin/python3.13"
build_config="$build_root/tauri.macos-qa.json"
build_log="$build_root/build.log"
trap 'rm -f "$build_log"' EXIT
test -x "$build_python"
test ! -e "$build_config"

"$build_python" -I -B - "$build_root" <<'PY'
import json
from pathlib import Path
import sys

root = Path(sys.argv[1])
state = root / "state"
state.mkdir(mode=0o700)
config = {
    "identifier": "dev.headroom.desktop.h048c5",
    "app": {"appDirectoriesOverride": str(state)},
    "bundle": {"active": True, "targets": ["app"], "icon": ["icons/icon.png"]},
}
with (root / "tauri.macos-qa.json").open("x", encoding="utf-8") as stream:
    json.dump(config, stream, indent=2)
    stream.write("\n")
PY

if ! pnpm tauri build --ci --bundles app --target aarch64-apple-darwin \
  --config "$build_config" --no-sign --no-binary-patching -- --locked \
  > "$build_log" 2>&1; then
  printf '%s\n' 'headroom-package-error=tauri_build'
  awk '/^error(\[[A-Z][0-9]+\])?: / { show = ++count <= 6; if (show) print substr($0, 1, 240) }
    show && /^ *--> / { print substr($0, 1, 180); show = 0 }
    /[Ee]rror.*[Cc]onfig/ { if (++config_count <= 3) print substr($0, 1, 240) }' "$build_log"
  exit 1
fi

source_app="$CARGO_TARGET_DIR/aarch64-apple-darwin/release/bundle/macos/Headroom.app"
bundle="$build_root/Headroom.app"
test -d "$source_app"
test ! -e "$bundle"
/usr/bin/ditto "$source_app" "$bundle"
resources="$bundle/Contents/Resources"
"$build_python" -I -B scripts/stage-macos-runtime.py \
  --payload "$runtime_root/payload" --output "$resources/runtime" > "$build_root/staging.json"
staged_sha="$("$build_python" -I -B -c \
  'import json,sys; print(json.load(open(sys.argv[1]))["manifest_sha256"])' "$build_root/staging.json")"
test "$staged_sha" = "$HEADROOM_MACOS_RUNTIME_MANIFEST_SHA256"

/usr/bin/codesign --force --sign - --timestamp=none "$bundle" > "$build_log" 2>&1
/usr/bin/codesign --verify --strict "$bundle" > "$build_log" 2>&1
test "$(lipo -archs "$bundle/Contents/MacOS/headroom-desktop")" = arm64
"$build_python" -I -B scripts/verify-macos-bundle.py --bundle "$bundle" \
  --runtime "$runtime_root" --expected-manifest "$HEADROOM_MACOS_RUNTIME_MANIFEST_SHA256" \
  > "$build_root/bundle-before.json"
printf '%s\n' 'headroom-package-build=pass' 'headroom-package-signature=ad_hoc_verified'
cat "$build_root/bundle-before.json"

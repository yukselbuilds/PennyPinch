#!/bin/bash
set -euo pipefail

test "${GITHUB_ACTIONS:-}" = true
test "${RUNNER_OS:-}" = macOS
test "$(uname -m)" = arm64
test "$#" = 2
picker_root="$1"
picker_executable="$2"
test "$(cd "$picker_root" && pwd -P)" = "$picker_root"
case "$picker_root" in
  "$GITHUB_WORKSPACE"/work/macos-picker.*) ;;
  *) exit 2 ;;
esac
test -x "$picker_executable"
test "$(stat -f '%u %Lp' "$picker_root")" = "$(id -u) 700"
picker_cli="$HEADROOM_MACOS_CLIENT_PROOF_ROOT/clients/bin/codex"
test -x "$picker_cli"

picker_app="$picker_root/HeadroomPickerProof.app"
picker_plist="$picker_app/Contents/Info.plist"
picker_stdout="$picker_root/picker.stdout"
picker_stderr="$picker_root/picker.stderr"
picker_driver="$picker_root/driver.stderr"
mkdir -p "$picker_app/Contents/MacOS"
install -m 700 "$picker_executable" "$picker_app/Contents/MacOS/macos-picker-proof"
/usr/bin/plutil -create xml1 "$picker_plist"
/usr/bin/plutil -insert CFBundlePackageType -string APPL "$picker_plist"
/usr/bin/plutil -insert CFBundleExecutable -string macos-picker-proof "$picker_plist"
/usr/bin/plutil -insert CFBundleIdentifier -string dev.headroom.picker-proof "$picker_plist"
touch "$picker_stdout" "$picker_stderr" "$picker_driver"

/usr/bin/open -n -W -a "$picker_app" --stdout "$picker_stdout" --stderr "$picker_stderr" \
  --env "GITHUB_ACTIONS=$GITHUB_ACTIONS" --env "RUNNER_OS=$RUNNER_OS" \
  --args "$picker_root" "$picker_cli" &
picker_launcher=$!
picker_status=0
for ((attempt = 0; attempt < 100; attempt++)); do
  if grep -qx 'headroom-picker-ready=file' "$picker_stdout"; then
    break
  fi
  sleep 0.1
done
if grep -qx 'headroom-picker-ready=file' "$picker_stdout"; then
  /usr/bin/osascript scripts/macos-picker-input.applescript "$picker_root" \
    > /dev/null 2> "$picker_driver" || picker_status=$?
else
  printf '%s\n' 'headroom-picker-input-driver=app_readiness_timeout' > "$picker_driver"
  picker_status=1
fi
wait "$picker_launcher" || picker_status=$?
awk '/^headroom-picker-/ { print substr($0, 1, 240) }
  /^thread .* panicked at / { detail = 2; print substr($0, 1, 240); next }
  detail > 0 { print substr($0, 1, 240); detail-- }' \
  "$picker_stdout" "$picker_stderr" "$picker_driver"
grep -qx 'headroom-picker-proof=pass' "$picker_stdout"
test "$picker_status" = 0

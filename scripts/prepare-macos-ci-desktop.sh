#!/usr/bin/env bash
set -euo pipefail
umask 077

root="$1"
workspace="$(pwd -P)"
[[ "$(uname -m)" == arm64 && "$workspace" == "$(git rev-parse --show-toplevel)" ]]
[[ "$root" == "$workspace"/work/macos-desktop.* && "$root" == "$(cd "$root" && pwd -P)" ]]
[[ "$(stat -f '%u %Lp' "$root")" == "$(id -u) 700" ]]

# Versioned archive from the official appcast; its signature and bundle inputs match the prior DMG proof.
image="$root/Codex.zip"
url='https://persistent.oaistatic.com/codex-app-prod/ChatGPT-darwin-arm64-26.1106.51428.zip'
expected_size=820584997
expected_hash=959adaad0a8b8cd0265ca1a53835af61827e2eaf659fd2bdad7ce39b712f257c
curl --proto '=https' --fail --silent --show-error --connect-timeout 15 --max-time 180 \
  --max-filesize "$expected_size" --output "$image" "$url"
[[ "$(stat -f '%z' "$image")" == "$expected_size" ]]
[[ "$(shasum -a 256 "$image" | cut -d' ' -f1)" == "$expected_hash" ]]

mkdir "$root/home"
mkdir "$root/home/Applications"
/usr/bin/ditto -x -k "$image" "$root/home/Applications"

bundle="$root/home/Applications/ChatGPT.app"
plist="$bundle/Contents/Info.plist"
launcher="$bundle/Contents/MacOS/ChatGPT"
backend="$bundle/Contents/Resources/codex-cli/CodexCLI.app/Contents/MacOS/codex"
identifier="$(/usr/bin/plutil -extract CFBundleIdentifier raw -o - "$plist")"
name="$(/usr/bin/plutil -extract CFBundleExecutable raw -o - "$plist")"
version="$(/usr/bin/plutil -extract CFBundleShortVersionString raw -o - "$plist")"
team="$(/usr/bin/codesign --display --verbose=4 "$bundle" 2>&1 | awk -F= '$1 == "TeamIdentifier" { print $2 }')"
arch="$(/usr/bin/lipo -archs "$launcher")"
backend_arch="$(/usr/bin/lipo -archs "$backend")"
[[ "$identifier" == com.openai.codex && "$name" == ChatGPT && "$version" == 26.1106.51428 ]]
[[ "$team" == 2DC432GLL2 && " $arch " == *' arm64 '* && " $backend_arch " == *' arm64 '* ]]
printf 'Desktop package: %s %s %s %s\n' "$identifier" "$name" "$version" "$team"
printf 'Desktop architectures: launcher=%s, backend=%s\n' "$arch" "$backend_arch"

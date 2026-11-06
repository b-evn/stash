#!/bin/sh
# Builds stash.app and the .dmg on macOS.
# Tauri signs ad hoc, and an ad-hoc signature only matches that exact build, so
# macOS would forget the Accessibility permission with every new build. Signing
# again with a requirement on the bundle identifier keeps it across builds.
# Usage: scripts/build-mac.sh [--target universal-apple-darwin]
set -eu
cd "$(dirname "$0")/../src-tauri"

target=
[ "${1:-}" = --target ] && target=$2
out=${CARGO_TARGET_DIR:-target}/${target:+$target/}release/bundle

# The Tauri CLI as `cargo tauri` (cargo install tauri-cli), else `tauri` (npm, as CI uses).
if command -v cargo-tauri >/dev/null 2>&1; then
  tauri() { cargo tauri "$@"; }
fi
tauri build --bundles app "$@"

app=$out/macos/stash.app
plist=$app/Contents/Info.plist
id=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$plist")
version=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$plist")
codesign --force --sign - --requirements "=designated => identifier \"$id\"" "$app"
codesign --verify --strict "$app"

case ${target:-$(uname -m)} in
  universal*) arch=universal ;;
  aarch64* | arm64) arch=aarch64 ;;
  *) arch=x64 ;;
esac
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
ditto "$app" "$stage/stash.app"
ln -s /Applications "$stage/Applications"
mkdir -p "$out/dmg"
dmg=$out/dmg/stash_${version}_$arch.dmg
# hdiutil now and then fails with "Resource busy" on CI runners, so it gets three tries.
for try in 1 2 3; do
  hdiutil create -volname stash -srcfolder "$stage" -format UDZO -ov "$dmg" && break
  [ "$try" = 3 ] && exit 1
  sync
  sleep 5
done
echo "$app"
echo "$dmg"

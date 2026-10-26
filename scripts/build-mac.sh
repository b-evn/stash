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

cargo tauri build --bundles app "$@"

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
hdiutil create -volname stash -srcfolder "$stage" -format UDZO -ov "$dmg"
echo "$app"
echo "$dmg"

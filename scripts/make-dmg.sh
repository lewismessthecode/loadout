#!/bin/sh
set -eu

bundle=src-tauri/target/universal-apple-darwin/release/bundle
version=$(node -p "require('./package.json').version")
out="$bundle/dmg/SkillsHub_${version}_universal.dmg"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT

cp -R "$bundle/macos/SkillsHub.app" "$stage/"
ln -s /Applications "$stage/Applications"
mkdir -p "$bundle/dmg"
rm -f "$out"
hdiutil create -volname SkillsHub -srcfolder "$stage" -fs HFS+ -format UDZO -ov "$out" >/dev/null
hdiutil verify "$out" >/dev/null
echo "$out"

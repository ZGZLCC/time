#!/bin/sh
set -eu

cd "$(dirname "$0")/.."
if [ "$(uname -s)" != Darwin ]; then
  echo "请在 macOS 上运行此脚本" >&2
  exit 1
fi

app="src-tauri/target/release/bundle/macos/Time.app"
plist="$app/Contents/Info.plist"
if [ ! -f "$plist" ]; then
  echo "请先运行 npm run tauri -- build" >&2
  exit 1
fi

bundle_id=$(/usr/libexec/PlistBuddy -c 'Print CFBundleIdentifier' "$plist")
if [ "$bundle_id" != 'com.time.tracker' ]; then
  echo "应用标识不匹配：$bundle_id" >&2
  exit 1
fi

version=$(/usr/libexec/PlistBuddy -c 'Print CFBundleShortVersionString' "$plist")
package="src-tauri/target/release/bundle/Time_${version}_macOS-upgrade.pkg"
productbuild --component "$app" /Applications "$package"
build_copy="src-tauri/target/release/bundle/macos/Time-build-${version}-$(date +%s)"
if [ -e "$build_copy" ]; then
  echo "构建副本路径已存在：$build_copy" >&2
  exit 1
fi
mv "$app" "$build_copy"
echo "已生成：$package"
echo "构建副本已移至 $build_copy，避免 Installer 将其误认作旧版应用。"

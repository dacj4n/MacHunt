#!/bin/zsh
set -euo pipefail

project_dir="${0:A:h:h}"
build_dir="$project_dir/.build"
app_dir="$build_dir/MacHunt.app"
configuration="${1:-release}"

if [[ "$configuration" != "debug" && "$configuration" != "release" ]]; then
    print -u2 "Usage: $0 [debug|release]"
    exit 2
fi

cd "$project_dir"
SWIFT_MODULECACHE_PATH="${SWIFT_MODULECACHE_PATH:-/private/tmp/machunt-swift-module-cache}" \
CLANG_MODULE_CACHE_PATH="${CLANG_MODULE_CACHE_PATH:-/private/tmp/machunt-clang-module-cache}" \
swift build --configuration "$configuration" --disable-sandbox

binary_dir="$(swift build --configuration "$configuration" --show-bin-path --disable-sandbox)"

mkdir -p "$app_dir/Contents/MacOS" "$app_dir/Contents/Resources" "$app_dir/Contents/Helpers"
cp "$binary_dir/MacHunt" "$app_dir/Contents/MacOS/MacHunt"
cp "$binary_dir/MacHuntCommand" "$app_dir/Contents/Helpers/machunt"
cp "$project_dir/Support/Info.plist" "$app_dir/Contents/Info.plist"
cp "$project_dir/Support/MacHunt.icns" "$app_dir/Contents/Resources/MacHunt.icns"
cp -R "$project_dir/Support/en.lproj" "$app_dir/Contents/Resources/"
cp -R "$project_dir/Support/zh-Hans.lproj" "$app_dir/Contents/Resources/"
xattr -cr "$app_dir"
codesign --force --deep --sign - "$app_dir"

print "$app_dir"

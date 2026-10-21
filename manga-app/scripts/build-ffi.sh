#!/bin/bash
# Builds the Rust engine for the iPhone and the simulator, generates the Swift bindings and packs
# both into the AkiManga Swift package: Packages/AkiManga/Akimanga.xcframework and
# Packages/AkiManga/Sources/AkiManga/akimanga.swift (both generated, not committed).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
package="$root/manga-app/Packages/AkiManga"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
profile="${PROFILE:-release}"
flag=""; [ "$profile" = release ] && flag="--release"

# The app is iOS 26 only; rustls's crypto library needs a deployment target newer than Rust's default (iOS 10).
export IPHONEOS_DEPLOYMENT_TARGET=26.0

cd "$root/desktop"
for target in aarch64-apple-ios aarch64-apple-ios-sim; do
  cargo build $flag -p akidev-manga-ffi --lib --target "$target"
done

# Bindings come from the library's own metadata, so they cannot drift from the Rust code.
cargo build -p akidev-manga-ffi --lib
cargo run -q -p akidev-manga-ffi --bin uniffi-bindgen -- generate \
  --library target/debug/libakimanga.dylib --language swift --out-dir "$work/gen"

mkdir -p "$work/headers"
cp "$work/gen/akimangaFFI.h" "$work/headers/"
cp "$work/gen/akimangaFFI.modulemap" "$work/headers/module.modulemap"

rm -rf "$package/Akimanga.xcframework"
xcodebuild -create-xcframework \
  -library "target/aarch64-apple-ios/$profile/libakimanga.a" -headers "$work/headers" \
  -library "target/aarch64-apple-ios-sim/$profile/libakimanga.a" -headers "$work/headers" \
  -output "$package/Akimanga.xcframework"
cp "$work/gen/akimanga.swift" "$package/Sources/AkiManga/akimanga.swift"
echo "built $package"

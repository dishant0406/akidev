#!/bin/bash
# Builds the Rust engine for Android (arm64 phones; the emulator on a Mac is arm64 too) and generates the Kotlin bindings,
# both into the :engine module (neither is committed): engine/src/main/jniLibs and engine/src/main/kotlin.
# Needs: cargo-ndk, the Android NDK (r28 or newer, so the library is 16 KB aligned as Google Play requires).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
engine="$root/manga-android/engine/src/main"
: "${ANDROID_HOME:=/opt/homebrew/share/android-commandlinetools}"
: "${ANDROID_NDK_HOME:=$(ls -d "$ANDROID_HOME"/ndk/* | sort -V | tail -1)}"
export ANDROID_HOME ANDROID_NDK_HOME
# `ios` is the size-optimised profile in desktop/Cargo.toml (it names the phone builds, both platforms).
profile="${PROFILE:-ios}"

cd "$root/desktop"
# Link flags that only shrink the library (about 0.8 MB): the relocations are stored packed (the Android linker reads them from API 23), and functions with identical code are merged
# unless their address is taken. Only for the phone build: the bindings below are built for this Mac.
RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,--pack-dyn-relocs=android -C link-arg=-Wl,--icf=safe" \
cargo ndk -t arm64-v8a -P 29 -o "$engine/jniLibs" build --profile "$profile" -p akidev-manga-ffi --lib

# Bindings come from the library's own metadata, so they cannot drift from the Rust code.
cargo build -q -p akidev-manga-ffi --lib
rm -rf "$engine/kotlin"
cargo run -q -p akidev-manga-ffi --bin uniffi-bindgen -- generate \
  --library target/debug/libakimanga.dylib --language kotlin --out-dir "$engine/kotlin" --no-format
echo "built $engine"

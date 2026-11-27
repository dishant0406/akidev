#!/bin/bash
# Builds the Rust engine for the TV (32-bit ARM: the TVs this app is for, such as the KTC Lumio Vision, run armeabi-v7a only)
# and generates the Kotlin bindings, both into the :engine module (neither is committed).
# Needs: cargo-ndk, the Android NDK, and `rustup target add armv7-linux-androideabi aarch64-linux-android`.
set -euo pipefail

root="$(cd "$(dirname "$0")/../../.." && pwd)"
engine="$root/apps/tv-android/engine/src/main"
: "${ANDROID_HOME:=/opt/homebrew/share/android-commandlinetools}"
: "${ANDROID_NDK_HOME:=$(ls -d "$ANDROID_HOME"/ndk/* | sort -V | tail -1)}"
export ANDROID_HOME ANDROID_NDK_HOME
profile="${PROFILE:-ios}"

cd "$root"
# 32-bit for the old TVs, 64-bit for the newer ones.
cargo ndk -t armeabi-v7a -t arm64-v8a -P 26 -o "$engine/jniLibs" build --profile "$profile" -p akidev-tv-ffi --lib

# Bindings come from the library's own metadata, so they cannot drift from the Rust code.
cargo build -q -p akidev-tv-ffi --lib
rm -rf "$engine/kotlin"
cargo run -q -p akidev-tv-ffi --bin uniffi-bindgen -- generate \
  --library target/debug/libakitv.dylib --language kotlin --out-dir "$engine/kotlin" --no-format
echo "built $engine"

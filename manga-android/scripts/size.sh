#!/bin/bash
# What a phone downloads from Google Play for the current bundle (an arm64, xhdpi, English phone), and the bundle itself: `./scripts/size.sh` after `./gradlew :app:bundleRelease`.
set -euo pipefail
cd "$(dirname "$0")/.."
bundle=app/build/outputs/bundle/release/app-release.aab
work="$(mktemp -d)"
apks="$work/out.apks"
spec="$work/spec.json"
echo '{"supportedAbis":["arm64-v8a"],"supportedLocales":["en-US"],"deviceFeatures":[],"glExtensions":[],"screenDensity":320,"sdkVersion":34}' > "$spec"
bundletool build-apks --bundle="$bundle" --output="$apks" --device-spec="$spec" --ks="$HOME/.android/debug.keystore" --ks-pass=pass:android --ks-key-alias=androiddebugkey --key-pass=pass:android > /dev/null
echo "download: $(bundletool get-size total --apks="$apks" --device-spec="$spec" | tail -1 | awk -F, '{print $2}') bytes"
echo "bundle:   $(stat -f %z "$bundle") bytes"

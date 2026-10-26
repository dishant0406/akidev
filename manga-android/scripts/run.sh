#!/bin/bash
# Builds the debug app, installs it on the phone or emulator that adb sees and opens it. `./scripts/run.sh clear` also wipes its data first.
set -euo pipefail
cd "$(dirname "$0")/.."
./gradlew :app:installDebug --console=plain -q
[ "${1:-}" = clear ] && adb shell pm clear live.akidev.manga >/dev/null
adb shell am force-stop live.akidev.manga
adb shell am start -n live.akidev.manga/.MainActivity >/dev/null

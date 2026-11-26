#!/bin/bash
# Times a cold start of the installed app on the phone or emulator adb sees: `./scripts/startup.sh [runs]` (default 5).
# TotalTime is until the first screen is drawn; run it on a release build to see what people get.
set -euo pipefail
runs="${1:-5}"
times=()
for _ in $(seq "$runs"); do
  adb shell am force-stop live.akidev.manga
  sleep 1
  times+=("$(adb shell am start -W -n live.akidev.manga/.MainActivity | tr -d '\r' | awk '/TotalTime/ {print $2}')")
done
echo "cold start, ms: ${times[*]}"
printf '%s\n' "${times[@]}" | sort -n | awk '{a[NR]=$1} END {print "median:", a[int((NR+1)/2)]}'

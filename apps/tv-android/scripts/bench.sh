#!/bin/bash
# Measures the app on a TV that adb can reach, with a RELEASE build installed (a debug build is much slower and says little).
# Cold start, how long the engine takes for each screen and when the first things reach the screen (the "perf" lines of the app's
# log), how smooth scrolling is (the system's frame times), and the time to the first video frame (needs a saved TorBox key).
# `memory` shows the memory after the start and after scrolling, and how many times the garbage collector ran (graphics memory is the
# one that grew: it holds the decoded pictures).
# Usage: TV=<ip>:5555 apps/tv-android/scripts/bench.sh [runs] [start|scroll|screens|memory|all]
set -uo pipefail
: "${TV:=$(adb devices | awk 'NR==2 {print $1}')}"
runs="${1:-3}"
only="${2:-all}"
want() { [ "$only" = all ] || [ "$only" = "$1" ]; }
adb="adb -s $TV"
app=live.akidev.tv
key() { for k in "$@"; do $adb shell input keyevent KEYCODE_$k; done; }
perf() { $adb logcat -d -s AkiTV:I | grep -o "perf .*" | grep -E "$1" | tail -1 | sed 's/^perf //'; }

want start && {
echo "== cold start (system time to the first frame, then the app's own marks), $runs runs"
for i in $(seq "$runs"); do
  $adb shell am force-stop $app; sleep 2; $adb logcat -c
  total=$($adb shell am start -W -n $app/.MainActivity | grep TotalTime | tr -dc 0-9)
  sleep 15
  echo "run $i: first frame ${total} ms | $(perf 'load home') | $(perf 'home rows on screen') | $(perf 'first backdrop') | $(perf 'first poster')"
done
}

want scroll && {
$adb shell am force-stop $app; sleep 2; $adb shell am start -n $app/.MainActivity > /dev/null; sleep 16
echo "== scrolling the home screen: 8 rows down, 12 posters along (frames from the system)"
$adb shell dumpsys gfxinfo $app reset > /dev/null
$adb shell 'for i in 1 2 3 4 5 6 7 8; do input keyevent 20; sleep 0.3; done; for i in 1 2 3 4 5 6 7 8 9 10 11 12; do input keyevent 22; sleep 0.3; done'
sleep 1
$adb shell dumpsys gfxinfo $app | grep -E "Total frames rendered|Janky frames:|50th percentile|90th percentile|95th percentile|99th percentile"
}

want screens && {
echo "== a title page, a search, and the first video frame"
$adb shell am force-stop $app; sleep 2; $adb shell am start -n $app/.MainActivity > /dev/null; sleep 14; $adb logcat -c
key DPAD_CENTER; sleep 7; echo "title page: $(perf 'load detail')"; key BACK; sleep 1
key DPAD_UP DPAD_UP DPAD_RIGHT DPAD_RIGHT DPAD_RIGHT DPAD_RIGHT DPAD_CENTER; sleep 2
$adb shell input text Frieren; key ENTER; sleep 8; echo "search: $(perf 'load search')"
key BACK DPAD_DOWN DPAD_LEFT DPAD_CENTER; sleep 7; $adb logcat -c; key DPAD_CENTER; sleep 25
echo "play: $(perf 'load stream') | $(perf 'player built') | $(perf 'player prepared') | $(perf 'player ready') | $(perf 'first frame')"
}

want memory && {
mem() { $adb shell dumpsys meminfo $app | grep -E "Java Heap:|Native Heap:|Graphics:|TOTAL PSS:" | awk '{printf "%s %s %s KB | ", $1, $2, $3}'; echo; }
$adb shell am force-stop $app; sleep 2; $adb logcat -c; $adb shell am start -n $app/.MainActivity > /dev/null; sleep 16
echo "== memory after the start"; mem
$adb logcat -c
$adb shell 'for i in 1 2 3 4 5 6 7 8; do input keyevent 20; sleep 0.3; done; for i in 1 2 3 4 5 6 7 8 9 10 11 12; do input keyevent 22; sleep 0.3; done'; sleep 1
echo "== memory after scrolling 8 rows and 12 posters"; mem
echo "garbage collections while scrolling: $($adb logcat -d | grep -c 'GC freed')"
}

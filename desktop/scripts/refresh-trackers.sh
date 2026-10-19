#!/usr/bin/env bash
#
# Replaces the public trackers in sources/sources.json with the first ones of ngosang/trackerslist
# (trackers_all.txt, which is ranked best first) and raises "version". Prints "changed" or
# "unchanged" on its last line and touches nothing when the list is the same.
#
# Websocket trackers are dropped (they are for WebTorrent in browsers, not for the app's
# client), and the list is cut at LIMIT, which must not exceed the cap `validate` enforces.
#
# Usage:
#   ./scripts/refresh-trackers.sh

set -euo pipefail
cd "$(dirname "$0")/.."

LIMIT="${LIMIT:-32}"
LIST="${LIST:-https://raw.githubusercontent.com/ngosang/trackerslist/master/trackers_all.txt}"
FILE=sources/sources.json

fetched="$(curl -fsS --retry 3 "$LIST")"
trackers="$(printf '%s\n' "$fetched" | tr -d '\r' | grep -E '^(udp|https?)://[^[:space:]]+$' | awk '!seen[$0]++' | head -n "$LIMIT")"

# A blank or truncated download must never wipe the list.
if [ "$(printf '%s\n' "$trackers" | grep -c .)" -lt 10 ]; then
  echo "only $(printf '%s\n' "$trackers" | grep -c .) usable trackers came back from $LIST; leaving $FILE alone" >&2
  exit 1
fi

new="$(printf '%s\n' "$trackers" | jq -R . | jq -s .)"
if [ "$(jq -c .trackers "$FILE")" = "$(jq -c . <<<"$new")" ]; then
  echo unchanged
  exit 0
fi

tmp="$(mktemp)"
jq --argjson t "$new" '.trackers = $t | .version += 1' "$FILE" > "$tmp"
mv "$tmp" "$FILE"
echo changed

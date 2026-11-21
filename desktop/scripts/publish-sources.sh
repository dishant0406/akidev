#!/usr/bin/env bash
#
# Publishes sources.json from the akidev-sources repo to the Cloudflare R2 bucket the app reads it from.
#
# It checks the document, refuses one that is not newer than what is live, signs it with the
# private key and uploads the document and its signature. Apps pick it up within six hours.
#
# Two feeds are published with one version number: the document as it is (schema 1, which every app
# reads) and the same document with "schema" 2, under v2/ (which only builds that know schema 2 read, and
# prefer). To make the schema 2 document differ, a value an old app would refuse, put it in
# sources-v2.json: that file is published as v2 when it exists, and its version must equal the other's.
#
# Requires: the cf CLI logged in to the Cloudflare account, and the signing key.
#
# Usage:
#   SOURCES_DIR=/path/to/akidev-sources ./scripts/publish-sources.sh
#   BUCKET=my-bucket KEY=/path/to/sources.key ./scripts/publish-sources.sh

set -euo pipefail
cd "$(dirname "$0")/.."

# A checkout of the akidev-sources repo. It is not guessed: the copy built into this repo is only a download.
SOURCES_DIR="${SOURCES_DIR:?Set SOURCES_DIR to a checkout of the akidev-sources repo}"
SOURCES_DIR="$(realpath "$SOURCES_DIR")"
BUCKET="${BUCKET:-akidev-sources}"
KEY="${KEY:-.secrets/sources.key}"
URL="${URL:-https://sources.akidev.live}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
tool=(cargo run -q -p akidev-core --bin akidev-sources --)

version() { grep -o '"version": *[0-9]*' | head -1 | grep -o '[0-9]*$'; }
new="$(version < "$SOURCES_DIR/sources.json")"
live="$(curl -fsS "$URL/sources.json" | version || echo 0)"
if [ "$new" -le "${live:-0}" ]; then
  echo "sources.json is version $new, and version ${live:-0} is live. Raise \"version\" first." >&2
  exit 1
fi

# An old app refuses a whole document for a value it does not know. Set OLD_REF to the oldest release still supported.
./scripts/check-old-apps.sh "$SOURCES_DIR/sources.json"

# The schema 2 document: its own file when there is one, otherwise the same document with the number changed.
if [ -f "$SOURCES_DIR/sources-v2.json" ]; then
  cp "$SOURCES_DIR/sources-v2.json" "$work/sources-v2.json"
else
  jq '.schema = 2' "$SOURCES_DIR/sources.json" > "$work/sources-v2.json"
fi
if [ "$(version < "$work/sources-v2.json")" != "$new" ]; then
  echo "sources-v2.json must have the same version as sources.json ($new)." >&2
  exit 1
fi

"${tool[@]}" sign "$KEY" "$SOURCES_DIR/sources.json" "$work/sources.sig"
"${tool[@]}" check "$SOURCES_DIR/sources.json" "$work/sources.sig"
"${tool[@]}" sign "$KEY" "$work/sources-v2.json" "$work/sources-v2.sig"
"${tool[@]}" check "$work/sources-v2.json" "$work/sources-v2.sig"

put() {
  cf r2 objects put "$1" --bucket-name "$BUCKET" --file "$2" --content-type "$3" > /dev/null
}
# The newer feed first, so a build that knows it never sees the older one ahead of it.
put v2/sources.json "$work/sources-v2.json" application/json
put v2/sources.sig "$work/sources-v2.sig" text/plain
put sources.json "$SOURCES_DIR/sources.json" application/json
put sources.sig "$work/sources.sig" text/plain
echo "Published version $new to $URL (and $URL/v2)"

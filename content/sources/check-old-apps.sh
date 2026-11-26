#!/usr/bin/env bash
#
# Checks a sources document the way an OLD release of the app would. An installed app ignores fields it
# does not know, but it refuses the whole document for a value it does not know (a new hole name, a new
# kind, another "schema"). A refused document also stops tracker updates and "min_version", so run this
# before publishing anything.
#
# It builds the checker from git tags and runs it: the oldest release we still support, and the newest one
# (a later validator can be stricter than the oldest).
#
# Usage:
#   ./content/sources/check-old-apps.sh                 checks content/sources/sources.json
#   ./content/sources/check-old-apps.sh /path/to/doc.json
#   OLD_REF=desktop-mac-v0.1.20 ./content/sources/check-old-apps.sh      the oldest to check (default v0.1.17)
#   OLD_REFS="desktop-mac-v0.1.17 desktop-mac-v0.1.25" ./content/sources/check-old-apps.sh   exactly these

set -euo pipefail
cd "$(dirname "$0")/../../desktop"

document="$(realpath "${1:-../content/sources/sources.json}")"
target="${OLD_TARGET:-${TMPDIR:-/tmp}/akidev-old-apps-target}"
newest="$(git tag --list 'desktop-mac-v*' | sort -V | tail -1)"
refs="${OLD_REFS:-${OLD_REF:-desktop-mac-v0.1.17} $newest}"

work="$(mktemp -d)"
trap 'git worktree remove --force "$work/old" 2>/dev/null || true; rm -rf "$work"' EXIT

for ref in $(printf '%s\n' $refs | awk '!seen[$0]++'); do
  git worktree add -q --detach "$work/old" "$ref"
  echo "Checking $document with the validator of $ref"
  (cd "$work/old/desktop" && CARGO_TARGET_DIR="$target" cargo run -q -p akidev-core --bin akidev-sources -- check "$document")
  git worktree remove --force "$work/old"
done

#!/bin/bash
# Copies manga-sources (the source of truth, in this repository) into a clone of its own GitHub repository,
# checks the index is what `index` would write, and commits the change there. It never pushes.
#
#   scripts/publish-manga-sources.sh ../manga-sources "Add the madara theme"
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
target="${1:?usage: publish-manga-sources.sh <clone of the manga-sources repository> <commit message>}"
message="${2:?a commit message}"

# The index must match the files (an app refuses a file whose hash differs from it).
(cd "$root/desktop" && cargo run -q -p akidev-manga --bin akidev-manga-cli -- index ../manga-sources)
if ! git -C "$root" diff --quiet -- manga-sources/index.json; then
  echo "index.json changed: commit it in this repository first, then run this again" >&2
  exit 1
fi

rsync -a --delete --exclude '.git' --exclude '.DS_Store' "$root/manga-sources/" "$target/"
cd "$target"
git add -A
if git diff --cached --quiet; then
  echo "nothing to publish"
else
  git commit -q -m "$message"
  git log --oneline -1
  echo "committed in $target; push it with: git -C $target push"
fi

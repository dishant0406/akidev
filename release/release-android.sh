#!/bin/bash
# Builds the Android app and sends it to Google Play through the fastlane lanes in fastlane/Fastfile.
#
# Usage:
#   release/release-android.sh "Fixed search"                    # internal, then alpha, then asks about production
#   release/release-android.sh "Fixed search" --prod             # builds and goes straight to production
#   release/release-android.sh "Fixed search" --version 1.5      # set 1.5 in both apps first
#
# versionCode is raised above every code on Play when needed. The version files are
# changed, committed and pushed for you. The Play key comes from fastlane/.env.
set -euo pipefail
cd "$(dirname "$0")/.."
export LC_ALL=en_US.UTF-8
source release/release-common.sh

notes=""
prod=""
version_arg=()
while [ $# -gt 0 ]; do
  case "$1" in
    --prod) prod=1; shift ;;
    --version) version_arg=("version:$2"); shift 2 ;;
    *) notes="$1"; shift ;;
  esac
done
[ -n "$notes" ] || { echo "Pass the release notes as an argument."; exit 1; }

require_clean_tree
check_content
echo "== 0/3 Set the version"
fastlane android bump ${version_arg[@]+"${version_arg[@]}"}
commit_version Android

if [ -n "$prod" ]; then
  echo "== Build and upload straight to production"
  fastlane android release "notes:$notes"
  echo "Sent to production. Check it with: fastlane android status"
  exit 0
fi

echo "== 1/3 Build and upload to internal"
fastlane android upload track:internal "notes:$notes"

echo
echo "== 2/3 Move the same build to alpha"
fastlane android promote from:internal to:alpha

echo
fastlane android status
read -r -p "== 3/3 Move this build from alpha to production? Type yes to continue: " answer
[ "$answer" = "yes" ] || { echo "Stopped. The build stays on alpha."; exit 0; }

fastlane android promote from:alpha to:production
echo "Sent to production. Check it with: fastlane android status"

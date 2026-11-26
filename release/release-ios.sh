#!/bin/bash
# Builds the iPhone app, uploads it to TestFlight, waits for Apple to process it, and sends it to the App Store.
# Everything goes through the fastlane lanes in fastlane/Fastfile.
#
# Usage:
#   release/release-ios.sh "Fixed search"                      # keep the current version
#   release/release-ios.sh "Fixed search" --version 1.5        # set 1.5 in both apps first
#   release/release-ios.sh "Fixed search" --phased             # 7-day phased release
#
# The version files are changed, committed and pushed for you when needed.
# ASC_ISSUER_ID must be exported in ~/.zshrc; the other values come from fastlane/.env.
set -euo pipefail
cd "$(dirname "$0")/.."
export LC_ALL=en_US.UTF-8
source release/release-common.sh

notes=""
version_arg=()
phased_arg=()
while [ $# -gt 0 ]; do
  case "$1" in
    --version) version_arg=("version:$2"); shift 2 ;;
    --phased) phased_arg=("rollout:phased"); shift ;;
    *) notes="$1"; shift ;;
  esac
done
[ -n "$notes" ] || { echo "Pass the release notes as an argument."; exit 1; }
: "${ASC_ISSUER_ID:?ASC_ISSUER_ID is not set. Run: source ~/.zshrc}"

require_clean_tree
echo "== 0/2 Set the version"
fastlane ios bump ${version_arg[@]+"${version_arg[@]}"}
commit_version iPhone

echo
echo "== 1/2 Build and upload to TestFlight"
fastlane ios upload wait:true

echo
echo "== Current state"
fastlane ios status

echo
read -r -p "== 2/2 Send this build to App Store review? Type yes to continue: " answer
[ "$answer" = "yes" ] || { echo "Stopped. The build stays on TestFlight."; exit 0; }

fastlane ios release "notes:$notes" ${phased_arg[@]+"${phased_arg[@]}"}
echo "Submitted. Watch the review with: fastlane ios status"

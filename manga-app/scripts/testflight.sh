#!/bin/bash
# Builds Akidev for the iPhone and uploads it to TestFlight, signing with an App Store Connect API key, so the Mac
# does not have to be signed in to the paid developer account (or to any account for this).
#
# One-time setup, on the paid account at appstoreconnect.apple.com:
#   1. Users and Access > Integrations > App Store Connect API > generate a key with the Admin role (it lets Xcode make
#      the distribution certificate and profile for you). Download the .p8 file once; note the Key ID and the Issuer ID.
#   2. Apps > + > New App: platform iOS, bundle id live.akidev.manga (register it under Certificates, Identifiers & Profiles
#      first if it is not offered), any name and SKU.
#   3. Your Team ID: developer.apple.com/account > Membership details.
#
# Then:
#   ASC_KEY_PATH=~/keys/AuthKey_ABC123.p8 ASC_KEY_ID=ABC123 ASC_ISSUER_ID=69a6de7e-... TEAM_ID=XXXXXXXXXX scripts/testflight.sh
#
# Add BUILD_ONLY=1 to stop after the archive (nothing is uploaded).
set -euo pipefail

: "${ASC_KEY_PATH:?path to the AuthKey_XXXX.p8 file}"
: "${ASC_KEY_ID:?the ID of the key}"
: "${ASC_ISSUER_ID:?the issuer ID shown on the Keys page}"
: "${TEAM_ID:?the developer team ID}"

cd "$(dirname "$0")/.."
work="build/testflight"
rm -rf "$work"
mkdir -p "$work"

# Every upload needs a build number higher than the last: the time does that.
build="$(date +%y%m%d%H%M)"

./scripts/build-ffi.sh
xcodegen generate -q

auth=(-authenticationKeyPath "$ASC_KEY_PATH" -authenticationKeyID "$ASC_KEY_ID" -authenticationKeyIssuerID "$ASC_ISSUER_ID")

xcodebuild archive \
  -project AkiManga.xcodeproj -scheme AkiMangaApp \
  -destination 'generic/platform=iOS' \
  -archivePath "$work/Akidev.xcarchive" \
  -allowProvisioningUpdates "${auth[@]}" \
  DEVELOPMENT_TEAM="$TEAM_ID" CODE_SIGN_STYLE=Automatic CODE_SIGNING_ALLOWED=YES \
  CURRENT_PROJECT_VERSION="$build"

[ -n "${BUILD_ONLY:-}" ] && { echo "archived: $work/Akidev.xcarchive (build $build)"; exit 0; }

cat > "$work/export.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>method</key><string>app-store-connect</string>
  <key>destination</key><string>upload</string>
  <key>teamID</key><string>$TEAM_ID</string>
  <key>signingStyle</key><string>automatic</string>
</dict></plist>
PLIST

xcodebuild -exportArchive \
  -archivePath "$work/Akidev.xcarchive" \
  -exportOptionsPlist "$work/export.plist" \
  -exportPath "$work/export" \
  -allowProvisioningUpdates "${auth[@]}"

echo "uploaded build $build: it appears in App Store Connect > TestFlight after a few minutes of processing"

#!/bin/bash
# Builds the Play Store phone screenshots: a headline on a flat colour with the app screen inside an Android phone frame.
# Usage: scripts/store-shots.sh   (reads store/screenshots, writes store/play-marketing)
set -e
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/store"
FONT="$ROOT/app/src/main/res/font/atkinson_bold.ttf"
TMP="$(mktemp -d)"
W=1080; H=1920
SW=800; SH=1600          # screen
BZ=24                    # bezel
BW=$((SW + 2*BZ))        # phone body width
BX=$(( (W - BW) / 2 )); BY=500
VISIBLE=$((H - BY))      # the phone runs off the bottom edge

frame() { # draws the phone body and the screen mask once
  magick -size ${BW}x$((SH + 2*BZ)) xc:none -fill '#2B2B2E' \
    -draw "roundrectangle 0,0 $((BW-1)),$((SH+2*BZ-1)) 112,112" \
    -fill '#050505' -draw "roundrectangle 5,5 $((BW-6)),$((SH+2*BZ-6)) 108,108" "$TMP/body.png"
  magick -size ${SW}x${SH} xc:black -fill white -draw "roundrectangle 0,0 $((SW-1)),$((SH-1)) 84,84" "$TMP/mask.png"
}

shot() { # file, line1, line2, mode(dark|yellow)
  local bg='#0D0D0E' fg='#F5F5F5' btn='#2B2B2E'
  if [ "$4" = yellow ]; then bg='#FFD21F'; fg='#141414'; btn='#141414'; fi
  magick "screenshots/$1" -resize ${SW}x${SH}\! "$TMP/mask.png" -alpha off -compose CopyOpacity -composite \
    -fill '#050505' -draw "circle $((SW/2)),46 $((SW/2)),60" "$TMP/screen.png"
  magick -size ${W}x${H} xc:"$bg" \
    -font "$FONT" -pointsize 100 -fill "$fg" -gravity north -annotate +0+150 "$2" -annotate +0+270 "$3" \
    -gravity northwest \
    -fill "$btn" -draw "roundrectangle $((BX+BW-2)),$((BY+330)) $((BX+BW+8)),$((BY+470)) 4,4" \
    -draw "roundrectangle $((BX+BW-2)),$((BY+520)) $((BX+BW+8)),$((BY+760)) 4,4" \
    "$TMP/body.png" -geometry +$BX+$BY -composite \
    "$TMP/screen.png" -geometry +$((BX+BZ))+$((BY+BZ)) -composite \
    -crop ${W}x${H}+0+0 +repage -strip PNG24:"play-marketing/aki-$1"
}

mkdir -p play-marketing
frame
shot 01-discover.png "Every series" "on one shelf" yellow
shot 02-series.png "Chapters ready" "when you are" dark
shot 03-genre-menu.png "Find your" "next read" yellow
shot 04-bookreader1.png "Books set" "beautifully" dark
shot 05-booksettings.png "Make the page" "yours" yellow
shot 06-dark-discover.png "Easy on the eyes" "day or night" dark
shot 07-reader-settings.png "Read it" "your way" yellow
shot 08-books.png "Your EPUBs," "one library" dark
rm -rf "$TMP"

#!/bin/sh
# Redraws the README's pictures (docs/*-dark.webp, docs/*-light.webp) from docs/readme-art.html:
# the real popup with its demo data, framed over the paintings in docs/art. The hero is animated and
# recorded frame by frame by scripts/readme-anim.mjs. Needs python3, cwebp (brew install webp), Node 22+,
# ffmpeg with libwebp and Google Chrome (set CHROME to use another Chromium). Run from
# anywhere: scripts/readme-art.sh, or name shots to redraw only those: scripts/readme-art.sh hero keys
set -e
ONLY=" $* "
cd "$(dirname "$0")/.."

CHROME=${CHROME:-"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"}
PORT=${PORT:-8765}
TMP=$(mktemp -d)

# Served over http so the scenes can reach into their popups (same origin), which file:// forbids.
python3 -m http.server "$PORT" >/dev/null 2>&1 &
SERVER=$!
trap 'kill $SERVER; rm -rf "$TMP"' EXIT
sleep 1

# WebP at quality 95 looks the same as Chrome's PNG at a sixth of the size; a shot without
# painting keeps every pixel (lossless) when that's no bigger.
webp() {
  cwebp -quiet -q 95 -m 6 -sharp_yuv -alpha_q 100 -exact "$1" -o "$TMP/lossy.webp"
  cwebp -quiet -lossless -z 9 -exact "$1" -o "$TMP/lossless.webp"
  if [ "$(wc -c <"$TMP/lossless.webp")" -le "$(wc -c <"$TMP/lossy.webp")" ]; then
    mv "$TMP/lossless.webp" "$2"
  else
    mv "$TMP/lossy.webp" "$2"
  fi
}

shot() {
  case "$ONLY" in "  " | *" $1 "*) ;; *) return 0 ;; esac
  for theme in dark light; do
    "$CHROME" --headless=new --hide-scrollbars --force-device-scale-factor=2 \
      --default-background-color=00000000 --window-size="$2,$3" --virtual-time-budget=9000 \
      --screenshot="$TMP/shot.png" "http://localhost:$PORT/docs/readme-art.html?shot=$1&theme=$theme" 2>/dev/null
    webp "$TMP/shot.png" "docs/$1-$theme.webp"
    echo "docs/$1-$theme.webp"
  done
}

# Animated: about 2 MB each, at 1.5x so the 175 frames stay light.
anim() {
  case "$ONLY" in "  " | *" $1 "*) ;; *) return 0 ;; esac
  for theme in dark light; do
    CHROME="$CHROME" BASE="http://localhost:$PORT" node scripts/readme-anim.mjs "$1" "$theme" "docs/$1-$theme.webp" 1.5
  done
}

anim hero
# The six section banners, see-through so they sit on GitHub's own page colour.
case "$ONLY" in "  " | *" title "*)
  for s in 1 2 3 4 5 6; do
    for theme in dark light; do
      "$CHROME" --headless=new --hide-scrollbars --force-device-scale-factor=2         --default-background-color=00000000 --window-size=1280,220 --virtual-time-budget=4000         --screenshot="$TMP/shot.png" "http://localhost:$PORT/docs/readme-art.html?shot=title&s=$s&theme=$theme" 2>/dev/null
      webp "$TMP/shot.png" "docs/title-$s-$theme.webp"
      echo "docs/title-$s-$theme.webp"
    done
  done ;;
esac
shot stack 1280 640
shot panels 1280 640
shot keys 1280 230
shot secrets 1280 460

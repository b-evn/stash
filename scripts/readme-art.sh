#!/bin/sh
# Redraws the README's pictures (docs/*-dark.png, docs/*-light.png) from docs/readme-art.html:
# the real popup with its demo data, framed over the paintings in docs/art. Needs python3 and
# Google Chrome (set CHROME to use another Chromium). Run from anywhere: scripts/readme-art.sh,
# or name shots to redraw only those: scripts/readme-art.sh hero keys
set -e
ONLY=" $* "
cd "$(dirname "$0")/.."

CHROME=${CHROME:-"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"}
PORT=${PORT:-8765}

# Served over http so the scenes can reach into their popups (same origin), which file:// forbids.
python3 -m http.server "$PORT" >/dev/null 2>&1 &
SERVER=$!
trap 'kill $SERVER' EXIT
sleep 1

shot() {
  case "$ONLY" in "  " | *" $1 "*) ;; *) return 0 ;; esac
  for theme in dark light; do
    "$CHROME" --headless=new --hide-scrollbars --force-device-scale-factor=2 \
      --default-background-color=00000000 --window-size="$2,$3" --virtual-time-budget=9000 \
      --screenshot="docs/$1-$theme.png" "http://localhost:$PORT/docs/readme-art.html?shot=$1&theme=$theme" 2>/dev/null
    echo "docs/$1-$theme.png"
  done
}

shot hero 1280 600
shot kinds 1280 380
shot stack 1280 640
shot panels 1280 640
shot keys 1280 230
shot secrets 1280 460

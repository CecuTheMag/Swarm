#!/bin/bash
# Renders the brand images (2x) from the HTML pages next to this script.
set -euo pipefail
cd "$(dirname "$0")"
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
shot() { "$CHROME" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=2 --window-size="$2,$3" --virtual-time-budget=3000 --screenshot="$1.png" "file://$PWD/$1.html" >/dev/null 2>&1; }
shot banner-light 1600 520
shot banner-dark 1600 520
shot social-preview 1280 640
rm -f banner-light.html banner-dark.html social-preview.html
ls -la *.png *.svg

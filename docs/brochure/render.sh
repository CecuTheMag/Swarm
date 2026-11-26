#!/bin/bash
# Renders outside.html and inside.html to 300 dpi PNGs and joins them into Рояк-диплянка.pdf.
# The glass needs blur and filters, so the PDF holds the rendered pages (print on A4, 2 sides, flip on the short edge).
set -euo pipefail
cd "$(dirname "$0")"
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
OUT="${OUT:-out}"; mkdir -p "$OUT"
for p in outside inside; do
  "$CHROME" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=3.125 \
    --window-size=1123,794 --virtual-time-budget=4000 --screenshot="$OUT/$p.png" "file://$PWD/$p.html" >/dev/null 2>&1
done
swift pdf.swift "$OUT/outside.png" "$OUT/inside.png" "$OUT/Рояк-диплянка.pdf"
echo "$OUT/Рояк-диплянка.pdf"

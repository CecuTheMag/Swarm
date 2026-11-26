#!/usr/bin/env bash
# Builds the documentation for the jury: docs/documentation/rojak.html with the authors'
# details and the test logins filled in, printed to a PDF outside the repository (the
# repository is public; the committed documentation keeps its yellow placeholders).
#
#   just documentation
#
# Reads, from OUT_DIR (default ~/rojak-jury):
#   people.env     AUTHOR_NAME, AUTHOR_SHORT, AUTHOR_PHONE, AUTHOR_EMAIL, AUTHOR_SCHOOL,
#                  AUTHOR_CLASS, SUPERVISOR_NAME, SUPERVISOR_PHONE, SUPERVISOR_EMAIL,
#                  SUPERVISOR_POSITION (shell assignments; an empty value stays a placeholder)
#   accounts.env   written by scripts/test-accounts.sh
# Writes OUT_DIR/Рояк-с-профили.pdf and says what is still a placeholder.
set -euo pipefail

OUT_DIR="${OUT_DIR:-$HOME/rojak-jury}"
DOC_DIR="$(cd "$(dirname "$0")/../docs/documentation" && pwd)"
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
[ -x "$CHROME" ] || { echo "Chrome not found at $CHROME (set CHROME)" >&2; exit 1; }
command -v python3 >/dev/null || { echo "python3 is needed" >&2; exit 1; }
mkdir -p "$OUT_DIR/doc"
chmod 700 "$OUT_DIR"
rm -rf "$OUT_DIR/doc"
mkdir -p "$OUT_DIR/doc"
cp -R "$DOC_DIR/img" "$OUT_DIR/doc/img"

OUT_DIR="$OUT_DIR" SRC="$DOC_DIR/rojak.html" DST="$OUT_DIR/doc/rojak.html" python3 - <<'PY'
import os, re

def load(path):
    values = {}
    if os.path.exists(path):
        for line in open(path, encoding="utf-8"):
            line = line.rstrip("\n")
            if "=" in line and not line.lstrip().startswith("#"):
                k, v = line.split("=", 1)
                values[k.strip()] = v.strip().strip("'")
    return values

out = os.environ["OUT_DIR"]
people = load(os.path.join(out, "people.env"))
creds = load(os.path.join(out, "accounts.env"))
html = open(os.environ["SRC"], encoding="utf-8").read()
left = []

def person(m):
    key, shown = m.group(1), m.group(2)
    value = people.get(key, "")
    if value:
        return value
    left.append(shown)
    return m.group(0)

html = re.sub(r'<span class="fill" data-k="([A-Z_]+)">(\[[^\]]*\])</span>', person, html)

if creds:
    pair = re.compile(r'<td><span class="fill">\[имейл\]</span></td><td><span class="fill">\[парола\]</span></td>')
    order = ["ADMIN", "OWNER", "ADMIN_NET", "VIEWER"]
    assert len(pair.findall(html)) == len(order), "the table's placeholders changed"
    for key in order:
        html = pair.sub(f'<td><code>{creds[key + "_EMAIL"]}</code></td><td><code>{creds[key + "_PASSWORD"]}</code></td>', html, count=1)
    link = "<code>/join/КОД</code>"
    assert link in html, "the volunteer row changed"
    host = creds["SWARM_URL"].split("://", 1)[-1]
    html = html.replace(link, f'<code>{host}/join/{creds["JOIN_CODE"]}</code>')
else:
    left.append("test logins (run scripts/test-accounts.sh)")

open(os.environ["DST"], "w", encoding="utf-8").write(html)
open(os.path.join(out, "doc", ".left"), "w", encoding="utf-8").write("\n".join(left))
PY

"$CHROME" --headless=new --disable-gpu --no-pdf-header-footer \
  --print-to-pdf="$OUT_DIR/Рояк-с-профили.pdf" "file://$OUT_DIR/doc/rojak.html" >/dev/null 2>&1
pages="$(python3 -c "
import re,sys
d=open(sys.argv[1],'rb').read()
print(len(re.findall(rb'/Type\s*/Page[^s]',d)))" "$OUT_DIR/Рояк-с-профили.pdf")"
echo "Documentation: $OUT_DIR/Рояк-с-профили.pdf ($pages pages)"
if [ -s "$OUT_DIR/doc/.left" ]; then
  echo "Still placeholders: $(paste -sd, "$OUT_DIR/doc/.left")"
fi

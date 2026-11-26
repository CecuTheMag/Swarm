#!/usr/bin/env bash
# Creates the jury's test accounts, one per role, through the coordinator's own API, and
# prints them once. Run it against the server the jury will use:
#
#   just test-accounts                      # https://swarmrelay.duckdns.org
#   SWARM_URL=http://localhost:7700 just test-accounts
#
# It needs a platform admin to sign in as. Make a separate one for the jury first
# (`just admin-create EMAIL user@host` prints its password once); that account is also the
# jury's "platform admin" login, so do not use your own.
#
#   ADMIN_EMAIL / ADMIN_PASSWORD   the platform admin (asked for when not set)
#   SWARM_URL                      the server (default https://swarmrelay.duckdns.org)
#   NETWORK_NAME                   the test network (default „Защита“)
#   OUT_DIR                        where the logins and the filled documentation go
#                                  (default ~/rojak-jury, outside the repository)
#
# Roles created: network owner (creates the network), network admin and viewer (invited
# by the owner). Run again and nothing is created twice: logins already in the file are
# checked and kept. The volunteer needs no account: the network's join link is printed.
# The passwords are written to OUT_DIR/accounts.env (mode 600) and never into the
# repository, which is public; the filled documentation goes there too.
set -euo pipefail

command -v curl >/dev/null || { echo "curl is needed" >&2; exit 1; }
command -v jq >/dev/null || { echo "jq is needed (brew install jq)" >&2; exit 1; }

BASE="${SWARM_URL:-https://swarmrelay.duckdns.org}"
BASE="${BASE%/}"
NETWORK_NAME="${NETWORK_NAME:-Защита}"
OUT_DIR="${OUT_DIR:-$HOME/rojak-jury}"
FILE="$OUT_DIR/accounts.env"
DOMAIN="rojak-test.example"

mkdir -p "$OUT_DIR"
chmod 700 "$OUT_DIR"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

if [ -z "${ADMIN_EMAIL:-}" ]; then read -r -p "Platform admin email: " ADMIN_EMAIL; fi
if [ -z "${ADMIN_PASSWORD:-}" ]; then read -r -s -p "Platform admin password: " ADMIN_PASSWORD; echo; fi

# api JAR METHOD PATH [JSON] -> body on stdout; fails (with the server's words) on a
# status of 400 or more. The Origin header is what the coordinator's same-origin check
# wants on anything that changes data.
api() {
  local jar="$1" method="$2" path="$3" body="${4:-}" out status
  out="$TMP/response"
  if [ -n "$body" ]; then
    status="$(curl -sS -o "$out" -w '%{http_code}' -X "$method" "$BASE$path" \
      -b "$jar" -c "$jar" -H "Origin: $BASE" -H 'Content-Type: application/json' -d "$body")"
  else
    status="$(curl -sS -o "$out" -w '%{http_code}' -X "$method" "$BASE$path" \
      -b "$jar" -c "$jar" -H "Origin: $BASE")"
  fi
  if [ "$status" -ge 400 ]; then
    echo "$method $path -> $status: $(cat "$out")" >&2
    return 1
  fi
  cat "$out"
}

login() { # JAR EMAIL PASSWORD -> 0 when it signs in
  api "$1" POST /api/auth/login "$(jq -nc --arg e "$2" --arg p "$3" '{email:$e,password:$p}')" >/dev/null 2>&1
}

new_password() { LC_ALL=C tr -dc 'abcdefghjkmnpqrstuvwxyzABCDEFGHJKMNPQRSTUVWXYZ23456789' </dev/urandom | head -c 16 || true; }

# Earlier logins, so a second run changes nothing.
OWNER_EMAIL="owner@$DOMAIN" ADMIN_NET_EMAIL="admin@$DOMAIN" VIEWER_EMAIL="viewer@$DOMAIN"
OWNER_PASSWORD="" ADMIN_NET_PASSWORD="" VIEWER_PASSWORD="" JOIN_CODE="" NETWORK_ID=""
if [ -f "$FILE" ]; then
  keep_email="$ADMIN_EMAIL" keep_password="$ADMIN_PASSWORD"
  # shellcheck disable=SC1090
  . "$FILE"
  ADMIN_EMAIL="$keep_email" ADMIN_PASSWORD="$keep_password"
fi

echo "Signing in as the platform admin on $BASE ..."
login "$TMP/admin.jar" "$ADMIN_EMAIL" "$ADMIN_PASSWORD" || { echo "Sign-in failed." >&2; exit 1; }
[ "$(api "$TMP/admin.jar" GET /api/me | jq -r '.is_admin')" = "true" ] \
  || { echo "$ADMIN_EMAIL is not a platform admin." >&2; exit 1; }

# join_with_invite JAR TOKEN EMAIL NAME PASSWORD
accept() {
  api "$1" POST "/api/invites/$2" \
    "$(jq -nc --arg e "$3" --arg n "$4" --arg p "$5" '{email:$e,name:$n,password:$p}')" >/dev/null
}

# 1. The network owner: an account made by the platform admin's invitation.
if [ -n "$OWNER_PASSWORD" ] && login "$TMP/owner.jar" "$OWNER_EMAIL" "$OWNER_PASSWORD"; then
  echo "Owner exists: $OWNER_EMAIL"
else
  OWNER_PASSWORD="$(new_password)"
  token="$(api "$TMP/admin.jar" POST /api/admin/invites "$(jq -nc --arg e "$OWNER_EMAIL" '{email:$e}')" | jq -r .token)"
  accept "$TMP/owner.jar" "$token" "$OWNER_EMAIL" "Тестов собственик" "$OWNER_PASSWORD"
  echo "Owner created: $OWNER_EMAIL"
fi

# 2. The network, owned by that account.
NETWORK_ID="$(api "$TMP/owner.jar" GET /api/networks | jq -r --arg n "$NETWORK_NAME" '[.[] | select(.name == $n and .role == "owner")][0].id // empty')"
if [ -z "$NETWORK_ID" ]; then
  NETWORK_ID="$(api "$TMP/owner.jar" POST /api/networks "$(jq -nc --arg n "$NETWORK_NAME" '{name:$n}')" | jq -r .id)"
  echo "Network created: $NETWORK_NAME"
fi
JOIN_CODE="$(api "$TMP/owner.jar" GET "/api/networks/$NETWORK_ID" | jq -r .join_code)"

# 3. A network admin and a viewer, invited by the owner with those roles.
member() { # VARPREFIX EMAIL NAME ROLE
  local pw_var="$1_PASSWORD" email="$2" name="$3" role="$4" pw token jar="$TMP/$1.jar"
  pw="${!pw_var}"
  if [ -n "$pw" ] && login "$jar" "$email" "$pw"; then
    echo "$role exists: $email"
    return
  fi
  pw="$(new_password)"
  token="$(api "$TMP/owner.jar" POST "/api/networks/$NETWORK_ID/invites" \
    "$(jq -nc --arg e "$email" --arg r "$role" '{email:$e,role:$r}')" | jq -r .token)"
  accept "$jar" "$token" "$email" "$name" "$pw"
  printf -v "$pw_var" '%s' "$pw"
  echo "$role created: $email"
}
member ADMIN_NET "$ADMIN_NET_EMAIL" "Тестов администратор" admin
member VIEWER "$VIEWER_EMAIL" "Тестов наблюдател" viewer

# 4. Each login is checked once more from nothing, as the jury would use it.
for pair in "$ADMIN_EMAIL:$ADMIN_PASSWORD" "$OWNER_EMAIL:$OWNER_PASSWORD" \
  "$ADMIN_NET_EMAIL:$ADMIN_NET_PASSWORD" "$VIEWER_EMAIL:$VIEWER_PASSWORD"; do
  login "$TMP/check.jar" "${pair%%:*}" "${pair#*:}" || { echo "Check failed for ${pair%%:*}" >&2; exit 1; }
  rm -f "$TMP/check.jar"
done
roles="$(api "$TMP/viewer.jar" GET /api/networks | jq -r '[.[] | select(.id == "'"$NETWORK_ID"'") | .role][0]')"
[ "$roles" = "viewer" ] || { echo "The viewer's role is $roles, not viewer." >&2; exit 1; }

JOIN_LINK="$BASE/join/$JOIN_CODE"
umask 077
cat >"$FILE" <<EOF
# Test logins for the jury. Keep this file out of the repository.
SWARM_URL='$BASE'
ADMIN_EMAIL='$ADMIN_EMAIL'
ADMIN_PASSWORD='$ADMIN_PASSWORD'
OWNER_EMAIL='$OWNER_EMAIL'
OWNER_PASSWORD='$OWNER_PASSWORD'
ADMIN_NET_EMAIL='$ADMIN_NET_EMAIL'
ADMIN_NET_PASSWORD='$ADMIN_NET_PASSWORD'
VIEWER_EMAIL='$VIEWER_EMAIL'
VIEWER_PASSWORD='$VIEWER_PASSWORD'
NETWORK_ID='$NETWORK_ID'
JOIN_CODE='$JOIN_CODE'
EOF

# 5. The documentation with the four logins filled in, built outside the repository.
DOC_DIR="$(cd "$(dirname "$0")/../docs/documentation" && pwd)"
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
if [ -x "$CHROME" ] && command -v python3 >/dev/null; then
  rm -rf "$OUT_DIR/doc"
  mkdir -p "$OUT_DIR/doc"
  cp -R "$DOC_DIR/img" "$OUT_DIR/doc/img"
  FILE="$FILE" SRC="$DOC_DIR/rojak.html" DST="$OUT_DIR/doc/rojak.html" JOIN_LINK="$JOIN_LINK" python3 - <<'PY'
import os, re
creds = {}
for line in open(os.environ["FILE"], encoding="utf-8"):
    if "=" in line and not line.startswith("#"):
        k, v = line.rstrip("\n").split("=", 1)
        creds[k] = v.strip("'")
html = open(os.environ["SRC"], encoding="utf-8").read()
pair = re.compile(r'<td><span class="fill">\[имейл\]</span></td><td><span class="fill">\[парола\]</span></td>')
order = ["ADMIN", "OWNER", "ADMIN_NET", "VIEWER"]
assert len(pair.findall(html)) == len(order), "the table's placeholders changed"
for key in order:
    html = pair.sub(f'<td><code>{creds[key + "_EMAIL"]}</code></td><td><code>{creds[key + "_PASSWORD"]}</code></td>', html, count=1)
link = "<code>/join/КОД</code>"
assert link in html, "the volunteer row changed"
html = html.replace(link, f'<code>{os.environ["JOIN_LINK"]}</code>')
open(os.environ["DST"], "w", encoding="utf-8").write(html)
PY
  "$CHROME" --headless=new --disable-gpu --no-pdf-header-footer \
    --print-to-pdf="$OUT_DIR/Рояк-с-профили.pdf" "file://$OUT_DIR/doc/rojak.html" >/dev/null 2>&1 || true
fi

cat <<EOF

Test accounts on $BASE
  platform admin   $ADMIN_EMAIL / $ADMIN_PASSWORD
  network owner    $OWNER_EMAIL / $OWNER_PASSWORD
  network admin    $ADMIN_NET_EMAIL / $ADMIN_NET_PASSWORD
  network viewer   $VIEWER_EMAIL / $VIEWER_PASSWORD
  volunteer        no account: $JOIN_LINK

Saved in $FILE
EOF
[ -f "$OUT_DIR/Рояк-с-профили.pdf" ] && echo "Documentation with these logins: $OUT_DIR/Рояк-с-профили.pdf"

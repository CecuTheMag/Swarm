#!/usr/bin/env bash
# New passwords for the jury's four test accounts (after a leak, or before a round):
# resets each on the server (`coordinator password-reset`, which also ends its sessions),
# writes the new ones into OUT_DIR/accounts.env, checks every login on the public
# address and rebuilds the jury's documentation there. Nothing goes into the repository.
#
#   SWARM_SERVER=user@host just rotate-jury
#
# Needs the accounts.env that scripts/test-accounts.sh wrote (OUT_DIR, default ~/rojak-jury).
set -euo pipefail
SERVER="${SWARM_SERVER:?set SWARM_SERVER=user@host}"
OUT_DIR="${OUT_DIR:-$HOME/rojak-jury}"
FILE="$OUT_DIR/accounts.env"
[ -f "$FILE" ] || { echo "no $FILE: run scripts/test-accounts.sh first" >&2; exit 1; }
# shellcheck disable=SC1090
. "$FILE"
BASE="${SWARM_URL%/}"

reset() { # EMAIL -> new password on stdout
  ssh "$SERVER" "cd ~/swarm && docker compose --env-file ~/swarm.env -f deploy/compose.server.yaml \
    exec -T coordinator coordinator password-reset '$1'" | sed -n 's/.*the password: //p'
}

for role in ADMIN OWNER ADMIN_NET VIEWER; do
  email_var="${role}_EMAIL"
  email="${!email_var}"
  password="$(reset "$email")"
  [ -n "$password" ] || { echo "reset failed for $email" >&2; exit 1; }
  # Replace the line in place; the file stays mode 600.
  python3 - "$FILE" "${role}_PASSWORD" "$password" <<'PY'
import sys
path, key, value = sys.argv[1:]
lines = open(path, encoding="utf-8").read().splitlines()
lines = [f"{key}='{value}'" if l.startswith(key + "=") else l for l in lines]
open(path, "w", encoding="utf-8").write("\n".join(lines) + "\n")
PY
  status="$(curl -sS -o /dev/null -w '%{http_code}' -H "Origin: $BASE" -H 'Content-Type: application/json' \
    -d "$(printf '{"email":"%s","password":"%s"}' "$email" "$password")" "$BASE/api/auth/login")"
  [ "$status" = 200 ] || { echo "the new password of $email does not sign in ($status)" >&2; exit 1; }
  echo "rotated: $email"
done

OUT_DIR="$OUT_DIR" bash "$(dirname "$0")/build-documentation.sh"
echo "New logins in $FILE; give the jury the new documentation if they have the old one."

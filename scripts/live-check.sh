#!/usr/bin/env bash
# Checks the public site the way the jury will use it: the pages answer, the API and its
# description answer, and every test account signs in. Run by .github/workflows/live.yml
# every night and by hand before a demo:
#
#   bash scripts/live-check.sh                                  # https://swarmrelay.duckdns.org
#   SWARM_URL=http://localhost:7701 bash scripts/live-check.sh
#
# JURY_ACCOUNTS (optional) holds one "email password" pair per line; without it the
# sign-ins are skipped. The passwords never appear in the output.
set -uo pipefail
BASE="${SWARM_URL:-https://swarmrelay.duckdns.org}"
BASE="${BASE%/}"
failed=0

check() { # NAME EXPECTED_STATUS PATH [GREP]
  local name="$1" want="$2" path="$3" grep="${4:-}" body status
  body="$(mktemp)"
  status="$(curl -sS -o "$body" -w '%{http_code}' --max-time 20 "$BASE$path" || echo 000)"
  if [ "$status" != "$want" ]; then
    echo "FAIL $name: $path answered $status, not $want"
    failed=1
  elif [ -n "$grep" ] && ! grep -q "$grep" "$body"; then
    echo "FAIL $name: $path does not contain \"$grep\""
    failed=1
  else
    echo "ok   $name"
  fi
  rm -f "$body"
}

check "coordinator and database" 200 /healthz
check "home page" 200 / "Рояк"
check "worker page" 200 /worker
check "user guide" 200 /guide "Ръководство"
check "API reference" 200 /docs/api
check "OpenAPI description" 200 /api/openapi.json '"openapi"'
check "feature flags" 200 /api/flags "ai_jobs"
check "totals" 200 /api/stats "units_decided"

if [ -n "${JURY_ACCOUNTS:-}" ]; then
  while read -r email password; do
    [ -z "$email" ] && continue
    jar="$(mktemp)"
    status="$(curl -sS -o /dev/null -w '%{http_code}' --max-time 20 -c "$jar" \
      -H "Origin: $BASE" -H 'Content-Type: application/json' \
      -d "$(printf '{"email":"%s","password":"%s"}' "$email" "$password")" \
      "$BASE/api/auth/login" || echo 000)"
    me="$(curl -sS -o /dev/null -w '%{http_code}' --max-time 20 -b "$jar" "$BASE/api/me" || echo 000)"
    if [ "$status" = 200 ] && [ "$me" = 200 ]; then
      echo "ok   sign-in $email"
    else
      echo "FAIL sign-in $email: login $status, /api/me $me"
      failed=1
    fi
    curl -sS -o /dev/null --max-time 20 -b "$jar" -H "Origin: $BASE" -X POST "$BASE/api/auth/logout" || true
    rm -f "$jar"
  done <<<"$JURY_ACCOUNTS"
else
  echo "skip sign-ins: JURY_ACCOUNTS is not set"
fi

exit "$failed"

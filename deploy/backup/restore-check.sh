#!/bin/sh
# Proves the latest dump restores: loads it into a scratch database next to the live one
# and compares the row counts of the main tables. Run on the server with
# `just backup-check` (it runs this in the backup container); the live database is only
# read.
set -eu
DUMP="${1:-/backups/latest.dump}"
[ -f "$DUMP" ] || { echo "no dump at $DUMP" >&2; exit 1; }
SCRATCH=swarm_restore_check
dropdb --if-exists "$SCRATCH"
createdb "$SCRATCH"
trap 'dropdb --if-exists "$SCRATCH"' EXIT
pg_restore --no-owner --dbname="$SCRATCH" "$DUMP"

status=0
for table in accounts networks devices jobs units schools; do
  live=$(psql -Atc "SELECT count(*) FROM $table")
  restored=$(psql -d "$SCRATCH" -Atc "SELECT count(*) FROM $table")
  echo "$table: live $live, restored $restored"
  # Rows come and go after the dump, but a table with data must not restore empty.
  if [ "$live" -gt 0 ] && [ "$restored" -eq 0 ]; then status=1; fi
done
[ "$status" -eq 0 ] && echo "restore check: OK ($DUMP)" || echo "restore check: FAILED" >&2
exit "$status"

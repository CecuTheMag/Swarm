#!/bin/sh
# Nightly dumps of the database (run by the `backup` service in compose.server.yaml).
# One custom-format dump a day in /backups, written to a temporary name first so a dump
# cut off half-way never looks finished; dumps older than BACKUP_KEEP_DAYS go.
set -eu
KEEP="${BACKUP_KEEP_DAYS:-14}"
HOUR="${BACKUP_HOUR:-3}"

dump() {
  name="/backups/swarm-$(date -u +%Y-%m-%dT%H%M).dump"
  if pg_dump --format=custom --compress=6 --file="$name.part"; then
    mv "$name.part" "$name"
    ln -sf "$(basename "$name")" /backups/latest.dump
    echo "backup: $name ($(du -h "$name" | cut -f1))"
  else
    rm -f "$name.part"
    echo "backup: pg_dump failed" >&2
  fi
  find /backups -name 'swarm-*.dump' -mtime +"$KEEP" -delete
}

# One at start, so a fresh deploy has a dump at once; then every night at HOUR (UTC).
dump
while true; do
  now=$(date -u +%s)
  next=$(date -u -d "@$(( (now / 86400) * 86400 + HOUR * 3600 ))" +%s)
  [ "$next" -le "$now" ] && next=$(( next + 86400 ))
  sleep $(( next - now ))
  dump
done

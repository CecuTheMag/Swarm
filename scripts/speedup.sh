#!/usr/bin/env bash
# The speedup curve on one machine: the same job computed by 1, 2, 4, … "virtual
# devices", each a sim-worker process pinned to a CPU core of its own, so the devices are
# identical and do not share a core. The coordinator and PostgreSQL are pinned to the
# other cores. Every run starts from an empty database with one result per unit (no
# second check, no honeypots), so it measures how the work spreads, not verification.
#
#   bash scripts/speedup.sh pi            # π, 96 units of 2^28 samples
#   bash scripts/speedup.sh pt            # path tracer, 1280×768, 128² tiles, 64 spp × 2 passes
#
# DEVICE_CPUS lists the cores for the devices (default: the 8 efficiency cores of an
# i5-14600KF, 12–19), SERVER_CPUS the rest (default 0–11), COUNTS the device counts.
# Needs `docker compose up -d` (PostgreSQL on 7732) and `cargo build --release -p
# coordinator -p sim-worker`. Prints one CSV line per run.
set -euo pipefail
cd "$(dirname "$0")/.."

WORKLOAD="${1:-pi}"
read -r -a DEVICE_CPUS <<<"${DEVICE_CPUS:-12 13 14 15 16 17 18 19}"
SERVER_CPUS="${SERVER_CPUS:-0-11}"
read -r -a COUNTS <<<"${COUNTS:-1 2 4 6 8}"
PORT="${PORT:-7710}"
DB_NAME=swarm_speedup
DB="postgres://swarm:swarm@localhost:7732/$DB_NAME"
PG="$(docker compose ps -q postgres)"
[ -n "$PG" ] || { echo "start PostgreSQL first: docker compose up -d" >&2; exit 1; }

case "$WORKLOAD" in
  pi) JOB='{"template":"pi","params":{"units":96,"samples_per_unit":268435456,"seed":2026}}' ;;
  pt) JOB='{"template":"pt","params":{"width":1280,"height":768,"tile":128,"spp":64,"passes":2,"seed":7}}' ;;
  *) echo "workload is pi or pt" >&2; exit 1 ;;
esac

# PostgreSQL shares the server's cores, not the devices'.
docker update --cpuset-cpus "$SERVER_CPUS" "$PG" >/dev/null
trap 'docker update --cpuset-cpus "0-$(( $(nproc) - 1 ))" "$PG" >/dev/null; kill "${COORD:-0}" 2>/dev/null || true' EXIT

echo "workload,devices,units,seconds,units_per_second"
for n in "${COUNTS[@]}"; do
  [ "$n" -le "${#DEVICE_CPUS[@]}" ] || { echo "only ${#DEVICE_CPUS[@]} device cores" >&2; exit 1; }
  docker exec "$PG" dropdb -U swarm --if-exists --force "$DB_NAME" >/dev/null 2>&1
  DATABASE_URL="$DB" COORDINATOR_ADDR="127.0.0.1:$PORT" SWARM_OPEN_PUBLIC=1 SWARM_REDUNDANCY=1 \
    RUST_LOG=warn taskset -c "$SERVER_CPUS" target/release/coordinator >/dev/null 2>&1 &
  COORD=$!
  until curl -sf "http://127.0.0.1:$PORT/healthz" >/dev/null; do sleep 0.2; done

  job="$(curl -sf -X POST "http://127.0.0.1:$PORT/api/jobs" -H 'content-type: application/json' -d "$JOB")"
  id="$(jq -r .id <<<"$job")"
  pids=()
  for ((i = 0; i < n; i++)); do
    taskset -c "${DEVICE_CPUS[$i]}" target/release/sim-worker \
      --url "ws://127.0.0.1:$PORT/ws/worker" --exit-when-idle --in-flight 2 >/dev/null 2>&1 &
    pids+=($!)
  done
  wait "${pids[@]}"

  done_job="$(curl -sf "http://127.0.0.1:$PORT/api/jobs/$id")"
  [ "$(jq -r .status <<<"$done_job")" = done ] || { echo "job did not finish: $done_job" >&2; exit 1; }
  jq -r --arg w "$WORKLOAD" --arg n "$n" \
    '[$w, $n, .units_total, ((.finished_at - .created_at) / 1000), (.units_total / ((.finished_at - .created_at) / 1000))]
     | "\(.[0]),\(.[1]),\(.[2]),\(.[3]),\(.[4] * 100 | round / 100)"' <<<"$done_job"
  kill "$COORD"; wait "$COORD" 2>/dev/null || true
done

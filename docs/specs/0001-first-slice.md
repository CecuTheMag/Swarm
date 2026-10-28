# 0001 — First end-to-end slice

Status: implemented

## Goal

The thinnest path through the whole system: a browser opens the worker page, receives
work units over a WebSocket, computes them with a WGSL compute shader, returns results,
and the coordinator aggregates them while a dashboard shows progress live.

Everything later (lease expiry, verification, path tracer, auth) builds on the pieces
defined here, so the protocol and data model are designed for them now even if this
slice only uses part of them.

## Scope

In:
- `protocol` crate: worker and dashboard message types, TypeScript generated with `ts-rs`.
- `kernels` crate: deterministic Monte Carlo π reference implementation.
- Coordinator: `POST /api/jobs`, `GET /api/jobs`, `GET /api/jobs/{id}`, `GET /healthz`,
  worker WebSocket `/ws/worker`, dashboard WebSocket `/ws/dashboard`, PostgreSQL with
  migrations (the database is created on first start).
- Web: `/worker` (kernel in a Web Worker), `/dashboard`, `/selftest` (GPU correctness
  check against shared vectors plus a speed measurement).
- `sim-worker`: speaks the real protocol, computes with the `kernels` crate.

Out (later specs):
- Lease expiry by time (a reaper), retry limits → 0002.
- Redundancy, quorum, honeypots, reputation → 0004.
- WASM fallback (→ 0005), benchmark-based sizing, throttling, background-tab behaviour.
- Path tracer, Ising, ML templates. Auth and roles.

The π template is a **pipeline smoke test**, not a showcase. It exists because its
answer is trivially checkable. The first real template (path tracer) comes in 0003.

## Determinism

Verification by exact comparison only works if every device produces bit-identical
output. This template is therefore integer-only:

- **Counter-based RNG.** Sample `i` of unit `u` gets two random `u32`s from
  `lowbias32(pcg2d(i, base))` with `base = pcg_hash(seed ^ pcg_hash(u))` — a pure
  function of its coordinates, so the result does not depend on GPU thread scheduling.
  The generator was chosen by measurement (docs/measurements.md): the first candidate
  was biased by ~8 standard errors.
- **Integer geometry.** Each sample picks a cell of a 16384² grid and tests its centre
  in doubled coordinates: `X = 2x + 1`, inside if `X² + Y² < 32768²`. Max value fits in
  `u32` (WGSL has no `u64`); the grid's own bias is ~1·10⁻⁶.
- **Order-independent reduction.** Per-workgroup counts are summed with `atomicAdd` on
  `u32`. Integer addition is associative, so the total is identical on every device.
- **Chunked dispatch.** The kernel processes a sample range `[start, end)`; the browser
  splits a unit into dispatches of 2²² samples so slow GPUs stay within driver
  watchdog limits. Chunking does not change the result.

The same algorithm is implemented in Rust (`kernels::pi`), WGSL (`templates/pi/pi.wgsl`)
and, independently, in Python (used once to generate `templates/pi/vectors.json`).
Rust unit tests and the browser self-test both check against those vectors.

## Protocol

JSON over WebSocket, internally tagged (`{"type": "...", ...}`). Defined in
`crates/protocol`, exported to `web/src/lib/protocol/` by `ts-rs`. Timestamps are Unix
milliseconds.

Worker → coordinator:

| Message | Fields | Meaning |
|---|---|---|
| `hello` | `protocol_version`, `client_version`, `device { backend, vendor, architecture }` | Must be the first message |
| `request_work` | — | Ready for one unit |
| `heartbeat` | `lease_ids` | Still alive; extends the listed leases |
| `result` | `lease_id`, `output`, `elapsed_ms` | Finished a unit |
| `failed` | `lease_id`, `reason` | Could not compute the unit |

Coordinator → worker:

| Message | Fields | Meaning |
|---|---|---|
| `welcome` | `worker_id`, `heartbeat_interval_ms` | Session accepted |
| `assign` | `lease_id`, `unit_id`, `template { id, version, sha256 }`, `params`, `lease_expires_at` | Compute this |
| `no_work` | `retry_after_ms` | Queue empty, ask again later |
| `ack` | `lease_id`, `accepted` | Result received; `accepted` = counted |
| `error` | `message` | Protocol violation; the connection is closed |

The worker refuses to run a unit whose `sha256` differs from the hash of the kernel it
has bundled.

Coordinator → dashboard (`/ws/dashboard`): `snapshot` on connect (and again if the
dashboard falls behind), then `worker_joined`, `worker_left`,
`unit_completed { job_id, unit_id, worker_id, units_done, units_total }`,
`job_updated { job }` (created, started, finished).

Rules enforced by the coordinator: `hello` first and only once; matching
`protocol_version`; at most one active lease per worker; messages ≤ 64 KiB; a
connection silent for 30 s is closed (workers heartbeat every 10 s).

## Data model (PostgreSQL)

```
jobs     (id, template, template_version, params, status, units_total, units_done,
          result, created_at, finished_at)
units    (id, job_id, seq, params, status, output, created_at)
           status: pending | leased | done; output = the accepted result
leases   (id, unit_id, worker_id, issued_at, expires_at, completed_at, released_at,
          output, elapsed_ms)
workers  (id, backend, vendor, architecture, client_version, connected_at,
          last_seen_at, disconnected_at)
```

- **Claiming** a unit is one transaction using `SELECT … FOR UPDATE SKIP LOCKED`, so
  concurrent claimers (even across coordinator instances) never get the same unit.
  Order: oldest job first, then unit sequence.
- **Completing** increments `jobs.units_done` under the job row lock; exactly one
  completion observes `units_done = units_total` and aggregates the job in the same
  transaction.
- **Releasing**: when a worker disconnects, times out, or reports `failed`, its active
  leases are released and their units return to `pending`. A late result for a
  released lease is ignored.
- `leases` is separate from `units` because redundancy (0004) assigns one unit to
  several workers; verification will choose which lease output becomes `units.output`.

## Acceptance criteria — results

1. ✅ `cargo nextest run`: π reference matches the shared vectors; aggregation equals a
   single-machine run; concurrent claims never share a unit; concurrent completions
   finish the job exactly once; foreign/repeated/invalid results are rejected;
   departed workers' units are re-queued.
2. ✅ Browser worker computes a job on the GPU; result equals the reference exactly
   (Playwright, headless Chromium with SwiftShader WebGPU).
3. ✅ Dashboard shows workers, progress and the final estimate live.
4. ✅ Several workers (simulated and browser) complete one job together exactly.
5. ✅ The WGSL kernel matches the Rust reference bit for bit in the browser self-test.

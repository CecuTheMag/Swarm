# 0001 — First end-to-end slice

Status: draft

## Goal

The thinnest path through the whole system: a browser opens the worker page, receives
work units over a WebSocket, computes them with a WGSL compute shader, returns results,
and the coordinator aggregates them while a dashboard shows progress live.

Everything later (leases with expiry, verification, path tracer, auth) builds on the
pieces defined here, so the protocol and data model are designed for them now even if
this slice only uses part of it.

## Scope

In:
- `protocol` crate: worker and dashboard message types, TypeScript generated with `ts-rs`.
- `kernels` crate: deterministic Monte Carlo π reference implementation.
- Coordinator: `POST /api/jobs`, `GET /api/jobs/{id}`, worker WebSocket `/ws/worker`,
  dashboard WebSocket `/ws/dashboard`, PostgreSQL persistence with migrations.
- Web: `/worker` page (WebGPU kernel in a Web Worker), `/dashboard` page.
- `sim-worker`: completes units using the `kernels` crate (no GPU).

Out (later specs):
- Lease expiry, heartbeats-driven extension, retries → 0002.
- Redundancy, quorum, honeypots, reputation → 0004.
- WASM fallback, benchmark, adaptive sizing, throttling.
- Path tracer, Ising, ML templates. Auth and roles.

The π template is a **pipeline smoke test**, not a showcase. It exists because its
answer is trivially checkable. The first real template (path tracer) comes in 0003.

## Determinism (the core design decision)

Verification by exact comparison only works if every device produces bit-identical
output. This template is therefore integer-only:

- **Counter-based RNG.** Sample `i` of unit `u` draws its random numbers from
  `pcg_hash(seed ^ hash(u) ^ i, k)`, a stateless hash of its own coordinates. No RNG
  state is shared, so the result does not depend on how the GPU schedules threads.
- **Integer geometry.** `x, y` are 15-bit integers; inside if `x² + y² < 32768²`.
  Max value `2·32767² < 2³¹` fits in `u32` (WGSL has no `u64`).
- **Order-independent reduction.** Per-workgroup counts are summed with `atomicAdd` on
  `u32`. Integer addition is associative, so the total is identical on every device.

The same algorithm is implemented in Rust (`kernels::pi`) and WGSL
(`templates/pi/pi.wgsl`). A test asserts they agree for fixed inputs.

## Protocol

JSON over WebSocket, internally tagged (`{"type": "...", ...}`). Defined in
`crates/protocol`, exported to `web/src/lib/protocol/` by `ts-rs`.

Worker → coordinator:

| Message | Fields | Meaning |
|---|---|---|
| `hello` | `client_version`, `device { backend: "webgpu" \| "wasm" \| "native", vendor, architecture }` | First message after connect |
| `request_work` | — | Ready for one unit |
| `heartbeat` | `lease_ids` | Still alive; still working on these |
| `result` | `lease_id`, `output` (template-specific JSON), `elapsed_ms` | Finished a unit |
| `failed` | `lease_id`, `reason` | Could not compute the unit |

Coordinator → worker:

| Message | Fields | Meaning |
|---|---|---|
| `welcome` | `worker_id`, `heartbeat_interval_ms` | Session accepted |
| `assign` | `lease_id`, `unit_id`, `template { id, version, sha256 }`, `params`, `lease_expires_at` | Compute this |
| `no_work` | `retry_after_ms` | Queue empty, ask again later |
| `ack` | `lease_id`, `accepted` | Result received |

Coordinator → dashboard (`/ws/dashboard`): `snapshot` on connect (workers, jobs), then
`worker_joined`, `worker_left`, `unit_completed { job_id, unit_id, done, total }`,
`job_completed { job_id, result }`.

A worker requests one unit at a time in this slice (prefetch comes with adaptive sizing).

## Data model (PostgreSQL)

```
jobs     (id uuid pk, template text, template_version int, params jsonb,
          status text,            -- queued | running | done | failed
          result jsonb, created_at, finished_at)
units    (id uuid pk, job_id fk, index int, params jsonb,
          status text,            -- pending | leased | done
          created_at)
leases   (id uuid pk, unit_id fk, worker_id uuid,
          issued_at, expires_at, completed_at, output jsonb)
workers  (id uuid pk, backend text, vendor text, architecture text,
          connected_at, last_seen_at)
```

Claiming a unit is one transaction using `SELECT … FOR UPDATE SKIP LOCKED`, so many
coordinator instances can share the queue without double-assigning. `leases` is a
separate table (not columns on `units`) because redundancy later assigns one unit to
several workers.

## REST

- `POST /api/jobs` `{ "template": "pi", "params": { "units": 64, "samples_per_unit": 16777216, "seed": 42 } }` → `201 { "id": … }`
- `GET /api/jobs/{id}` → job with progress and, when done, `{ "inside": u64, "total": u64, "pi": f64 }`

## Acceptance criteria

1. `cargo nextest run` passes, including: π reference is deterministic; aggregation of
   unit results equals a single-machine run; two concurrent claims never get the same unit.
2. With `docker compose up -d`, the coordinator and `pnpm dev` running, opening
   `/worker` in a WebGPU browser and submitting a job completes it; the result equals
   `kernels::pi` for the same parameters **exactly**.
3. `/dashboard` shows connected workers, per-job progress, and the final estimate,
   updating live without reload.
4. Two browsers plus one `sim-worker` complete one job together; the result is still exact.
5. A Playwright test (headless Chromium + SwiftShader WebGPU) runs the WGSL kernel and
   matches the Rust reference for fixed inputs.

## Open questions

- Template signing: this slice only carries `sha256` of the shader; real signatures in a later spec.
- Dashboard transport: WebSocket chosen for symmetry with workers; SSE would also work.

# Automated tests

They run on every change (`just check`, `just e2e`, `just e2e-webkit`; CI runs the same).

## Rust (`cargo test --workspace`)

About 110 tests across the crates, many against a real PostgreSQL (`sqlx::test`).

| Area | What they prove |
|---|---|
| Kernels (`crates/kernels`) | π and the path tracer match their vectors; the random numbers pass their checks; the integer neural network is deterministic, learns well above chance, never overflows `i32` |
| Templates (`crates/coordinator/src/templates`) | Jobs split as specified, bad parameters are refused, outputs are validated, agreement and aggregation are right; the shared test vectors equal the reference (`UPDATE_VECTORS=1` rewrites them) |
| Verification (spec 0004) | Two honest devices agree, a cheater is caught by disagreement or a known-answer check, banned, and its results redone, also in finished jobs |
| Accounts and networks (spec 0010) | Passwords, sessions, invites, roles on every route, isolation (a device never gets another network's unit), pause, working hours, removal |
| End to end (`tests/e2e.rs`) | Simulated devices run real jobs through a real coordinator: π exact, a rendered picture, a neural network sweep with a cheater among the devices (banned; every score equals the reference; the downloaded model has the accuracy the devices reported) |

## Browser unit tests (`pnpm test`, Vitest)

| File | What |
|---|---|
| `cpu/pi.spec.ts`, `cpu/pt.spec.ts` | The WebAssembly kernels match the shared vectors |
| `console/digit.spec.ts` | A drawn digit is redrawn like the dataset's; the page reads all 1 024 validation digits exactly as the devices scored the network |
| `worker/session.spec.ts` | The worker session against a scripted coordinator: lost connections at the worst moment, back-off, pause and resume, power levels, the owner's controls, one network per CPU thread |
| `worker/pacing.spec.ts`, `dashboard/state.spec.ts` | Power pacing; the live view's state |

## Browser tests (`just e2e`, Playwright)

Each runs in Chromium with WebGPU (software GPU), Chromium without WebGPU (the CPU
path), Firefox, and WebKit (Safari's engine) with `just e2e-webkit`.

| File | What |
|---|---|
| `selftest.e2e.ts` | The self-test page is bit-exact against the Rust reference |
| `worker.e2e.ts` | A browser worker computes π exactly |
| `render.e2e.ts` | A browser worker renders; the render page shows it |
| `reconnect.e2e.ts` | Losing the connection mid-unit, then carrying on |
| `safety.e2e.ts` | Power levels and pausing when hidden (spec 0007) |
| `wake-lock.e2e.ts` | The screen stays on while computing (not in WebKit on Linux) |
| `chaos.e2e.ts` | The resilience tools: stop devices, cut the network, add cheaters (spec 0008) |
| `console.e2e.ts` | Sign in, create a network, a device joins by link, jobs, pause and approval (spec 0010) |
| `sweep.e2e.ts` | A neural network sweep from the form to the leaderboard, the model download, and a digit drawn on the page read by the network |
| `look.e2e.ts` | The glass surfaces really blur in the production build |

Latest full run: see the bottom of `docs/measurements.md` (Browser engines).

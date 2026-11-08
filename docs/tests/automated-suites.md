# Automated tests

They run on every change (`just check`, `just e2e`, `just e2e-webkit`; CI runs the same).

## Rust (`cargo test --workspace`)

About 260 tests across the crates, many against a real PostgreSQL (`sqlx::test`). `cargo
test` runs four at a time per test binary (`.cargo/config.toml`): sqlx shares 20
connections among a binary's database tests. `cargo nextest run` runs each in its own
process.

| Area | What they prove |
|---|---|
| Kernels (`crates/kernels`) | π and the path tracer match their vectors; the random numbers pass their checks; the integer neural network is deterministic, learns well above chance, never overflows `i32` |
| Templates (`crates/coordinator/src/templates`) | Jobs split as specified, bad parameters are refused, outputs are validated, agreement and aggregation are right; the shared test vectors equal the reference (`UPDATE_VECTORS=1` rewrites them) |
| Verification (spec 0004) | Two honest devices agree, a cheater is caught by disagreement or a known-answer check, banned, and its results redone, also in finished jobs |
| Accounts and networks (spec 0010) | Passwords, sessions, invites, roles on every route, isolation (a device never gets another network's unit), pause, working hours, removal |
| Programs checker (`crates/programs`, spec 0012) | Every upload rule refuses what breaks it with a message saying what to change (imports, memory without a maximum or above 256 MiB, threads, relaxed SIMD, 64-bit memory, tail calls, mistyped exports, size); the interpreter runs a program, stops an endless loop, reports traps and out-of-bounds pointers |
| Programs (`tests/programs.rs`) | Upload rules and roles, a program job on simulated devices of its network at quorum 1, 2 and 3 with outputs equal to the reference, a cheating member banned, workers run exactly the named module, an endless program fails its job, review for the public network, devices that refuse community programs (also when lent) never get them, deleting a network stops its public jobs |
| Timetables (`crates/timetable`, specs 0013, 0015) | Every cost rule counted on hand-checked timetables; malformed schools refused with a reason; the incremental cost equals the cost from scratch after any moves, with pinned hours moved too (property test); no search moves a pinned hour, whatever the pins (property test); a search is a pure function of its inputs and never returns worse than its start; example schools are valid |
| Timetable jobs (`templates/tt.rs`, `tests/timetables.rs`) | Units refer to the school by hash; forged timetables (wrong cost, length or slot) are refused and their device banned; a lazy device is outvoted; a device that moves a pinned hour cannot win; simulated devices solve a job and the result equals the reference; the vectors match the solver (one with pins) |
| Schools (`tests/schools.rs`, spec 0014) | A school saved and read back is the same; a save from an older version is refused and changes nothing; members read, admins change, other networks see nothing; only searchable schools without namesakes are kept; the 50-school limit holds when creates race |
| Edited timetables (`tests/timetable_edits.rs`, spec 0015) | An edit saved and read back is the same, with the coordinator's own cost; a first edit twice, or a save from an older or dropped version, is refused and keeps nothing; timetables of the wrong length, slots outside the week, bad base64 and pinned hours out of range, twice or pinned in the school already are refused; viewers read, admins change, other networks see nothing; only timetable jobs have one |
| Hardening (`tests/networks.rs`, spec 0016) | Without its token the public network refuses strangers on every route and still serves its admins; it opens only on request, never once a token is set; a sign-in waits for a free password check and is told to try again, which never counts as a wrong password, and one given up halfway keeps its check's slot until the check ends; every coordinator response, errors included, carries its security headers |
| Live events (`state.rs`) | A cheater outvoted after it left is still announced as caught, once; a connected one once; how many departed workers are remembered is bounded |
| End to end (`tests/e2e.rs`) | Simulated devices run real jobs through a real coordinator: π exact, a rendered picture, a neural network sweep with a cheater among the devices (banned; every score equals the reference; the downloaded model has the accuracy the devices reported) |

## Browser unit tests (`pnpm test`, Vitest)

| File | What |
|---|---|
| `cpu/pi.spec.ts`, `cpu/pt.spec.ts` | The WebAssembly kernels match the shared vectors |
| `console/digit.spec.ts` | A drawn digit is redrawn like the dataset's; the page reads all 1 024 validation digits exactly as the devices scored the network |
| `worker/session.spec.ts` | The worker session against a scripted coordinator: lost connections at the worst moment, back-off, pause and resume, power levels, the owner's controls, one network per CPU thread |
| `worker/pacing.spec.ts`, `dashboard/state.spec.ts` | Power pacing; the live view's state |
| `program/abi.spec.ts`, `program/runner.spec.ts` | A program runs on a fresh instance per unit; a module whose hash differs or that has imports is refused; a unit past the time limit, or stopped, ends its thread; units run side by side |
| `cpu/tt.spec.ts`, `console/timetable.spec.ts` | The WebAssembly solver gives the shared vectors exactly and scores timetables as the Rust tests do; a timetable drawn per class and teacher, and written as CSV; hard conflicts named; an hour moved or traded by hand; hand pins added to a school for the next search |
| `school/csv.spec.ts`, `school/import.spec.ts` | CSV as spreadsheets save it, read back exactly whatever the fields hold; a plan imported with every bad row named by line, pinned hours too; every example school's plan exported and read back |
| `school/checks.spec.ts`, `school/edit.spec.ts` | Each impossibility check fires for a school built to fail it and never for the examples, pins included; edits keep lessons pointing at the right entries, and a refused edit changes nothing; pins kept within the lesson's hours and the week |
| `bytes.spec.ts`, `console/programs.spec.ts` | Base64, hex and SHA-256 as the coordinator computes them; job inputs from a file or a count |

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
| `headers.e2e.ts` | Pages and the API carry their security headers; the home, sign-in, worker, self-test and projector pages run under their Content-Security-Policy without a violation (spec 0016) |
| `timetables.e2e.ts` | A school's devices search its timetable; every search's cost equals an independent recomputation; the console shows it per class and teacher and downloads it as CSV (spec 0013) |
| `schools.e2e.ts` | A deputy director imports a plan (bad rows named), marks a teacher away, finds a class with too many hours, saves, reloads, is stopped from overwriting another admin's save, searches on the school's device (the teacher's free day is kept free) and prints every class on its own page (spec 0014) |
| `timetable-edit.e2e.ts` | A deputy director pins a lesson with the plan and another in the editor (a slot taken is refused), searches, moves an hour onto a busy teacher and sees the clash named, undoes it, moves it where it breaks nothing, saves, reloads to the same cost the coordinator computed, prints it, gives the edit back and takes it again, and searches again: the hour put by hand is pinned in the new timetable (spec 0015) |
| `programs.e2e.ts` | A network uploads its own program, a device computes it, the outputs download exactly; an endless program leaves the page usable and stops when asked; review opens the public network; a contributor's choice about community programs is kept (spec 0012) |

Latest full run: see the bottom of `docs/measurements.md` (Browser engines).

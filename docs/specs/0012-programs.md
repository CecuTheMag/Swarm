# 0012 — Programs: a network's own code

Status: implemented (a run on a real phone is still to be recorded)

## Goal

A network runs **its own computation**, not only Рояк's templates: a teacher's class
experiment on the class's devices, a student's simulation on a lab, a researcher's
parameter sweep. A program that passes review may run on every volunteer's device. The
promise to contributors stays: **nothing runs on a device without review or its owner's
choice to join that network**, and nothing that runs can reach the network, the device's
files or anything but its own memory.

| Code | Runs on | Review |
|---|---|---|
| Рояк's templates (π, path tracer, neural sweep) | any network's devices | shipped with the site |
| A network's program | that network's devices (and devices it lends, see below) | none: joining a network is the device owner's choice |
| An approved program on the public network | every volunteer who allows community programs | the platform admin approves the exact module |

## Programs

A program is a WebAssembly module with four exports and **no imports**:

| Export | Type | Meaning |
|---|---|---|
| `memory` | memory | the module's own linear memory, with a declared maximum |
| `swarm_input` | `(len: i32) -> i32` | where the host writes the `len` input bytes |
| `swarm_run` | `() -> i32` | computes; the output's length, or a negative error code |
| `swarm_output` | `() -> i32` | where the output bytes are |

The coordinator checks every module at upload (`crates/programs`): at most 4 MiB; valid
WebAssembly 2.0 (fixed-width SIMD included; threads, relaxed SIMD, 64-bit memory,
exceptions, GC and tail calls refused); no imports; one memory, exported, with a maximum
of at most 256 MiB; the four exports with exactly these types. Each refusal says what to
change. `sdk/rust` provides a one-line macro; `docs/programs.md` is the guide.

Programs belong to a network: its admins upload them, its viewers see them. A network
keeps at most 50; the same module uploaded twice is one program.

## Jobs

- `template: "program"`, `params`: `{program, count, args}` (unit `i` receives `i` as 4
  little-endian bytes, then `args`) or `{program, inputs}` (one base64 input per unit).
  Inputs and outputs ≤ 256 KiB, ≤ 20 000 units and 16 MiB of inputs per job. The job's
  params show `{program, name, units}`; inputs stay with the units.
- Workers receive `TemplateRef { id: "program", version: 1, sha256 }` (the module's
  hash), download `GET /api/modules/{sha256}.wasm`, check the hash and the absence of
  imports themselves, and run each unit in a fresh instance in a thread of its own,
  stopped after 120 s or when the contributor stops.
- Outputs agree byte for byte (WebAssembly without imports is deterministic, apart from
  NaN bit patterns). `GET /api/networks/{id}/jobs/{job}/outputs` lists them in unit
  order for the network's viewers. **The coordinator never runs uploaded code.**
- **Quorum per job**: in a network other than the public one, the job may ask for 1, 2
  or 3 agreeing results per unit (`quorum`); 1 when the owner trusts every device, 2 or 3
  to catch a member returning made-up results. Public jobs keep the coordinator's setting.
- Honeypots come only from Рояк's templates (spec 0004), so a program's unit is never
  sent to anyone outside its network as a check.

## Review and the public network

- An admin of the network asks for review (`POST …/programs/{p}/review`). The platform
  admin lists pending programs and approves or rejects the exact module, with a note
  (`/api/admin/programs`). A changed module is a new program.
- An approved program's job may run on the public network (`public: true`). It is
  created in the public network but stays its network's: listed with its jobs, its
  outputs readable by its viewers. A network has at most 2 such jobs at once.
- Contributors choose: the worker page has "also run reviewed community programs"
  (default on). Off, a device gets no program units from the public network, also when
  its network lends it. Its own network's programs run regardless: joining is consent.

## Acceptance criteria

1. Modules breaking any rule are refused with a message saying what to change (unit test
   per rule; API test).
2. A program job completes on simulated devices of its network, with outputs equal to
   running the module locally, at quorum 1, 2 and 3; devices of other networks never get
   its units (end to end).
3. A cheating member is caught and banned; its results never reach the outputs.
4. A browser joins a network and computes a program job in every engine; a program that
   never returns leaves the page usable and stops when the contributor stops.
5. Unapproved programs cannot reach the public network; approved ones run there, but not
   on devices that turned community programs off.
6. Roles: viewers see programs and outputs, only admins upload, ask for review and start
   jobs; only the platform admin decides reviews; outsiders get 404.

## Verified by

`cargo nextest run` (checker, scheduling, API, end to end with simulated workers),
`pnpm test:unit` (runner and its time limit), Playwright in every engine, and a run on a
real phone recorded in `docs/measurements.md`.

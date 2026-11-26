# Roadmap

What is left between today's grid and the full vision ([vision.md](design/vision.md)), in build order. Each
item gets its own spec in `docs/specs/` before work starts; this file only orders them.

Status on 2026-11-08: specs 0001–0008 and 0010–0016 implemented (π and path-tracer
templates, leases with expiry, quorum verification with honeypots and reputation, CPU
fallback, load tests, contributor safety, the chaos panel, networks that clients own
and operate from the console, neural network sweeps, a network's own programs with
review for the public network, school timetables with a deputy director's data entry,
printing, pinned lessons and editing by hand, and the hardening of spec 0016) and the
server public at `https://swarmrelay.duckdns.org`.

## Next up, in order (November 2026)

Registration closes on 25 November. Items marked **(owner)** need the owner in person:
accounts, devices, the school.

1. ~~**Reliable CI**~~ — done 6 November: browsers cached and a stalled package mirror
   fails over; failures keep traces; pause and hold waits sized to a unit; `cargo test`
   runs four tests at a time per binary; sign-in optimised in debug builds. Two real bugs
   found on the way (a cheater outvoted after leaving was never shown as caught; example
   schools had namesakes). Left: one Firefox run in four full rounds kept „computing“
   after a pause with only short units queued; not reproduced in six Firefox-only rounds.
2. **Timetables for a real school** (specs 0013, 0014):
   - ~~data entry for a deputy director~~ — done (spec 0014): the plan imported from a
     spreadsheet, the week, teachers, classes and rooms, hours away on a grid, checks
     before a search, versioned saves;
   - ~~printing~~ — done: a page per class or teacher, all classes or all teachers;
   - shifts (a second shift's week) and a class's own room;
   - rounds: later searches continue from the best timetables found so far;
   - ~~pinning lessons by hand and editing the result~~ — done (spec 0015): pins in the
     plan and the editor, a found timetable moved hour by hour with each move's cost
     shown first, kept per job, printed, and searched again around what was pinned;
   - **(owner)** a real school's data, from the deputy director, and their feedback;
     measurements on it and on a public benchmark (XHSTT) in `docs/measurements.md`.
3. **Real devices (owner)** — a phone on mobile data, an iPhone (Safari's WebGPU), wake
   lock and losing signal on a real phone, a program and a timetable job on a phone, the
   laptop fallback (`docs/competition/demo-day.md` Plan B); results in `docs/measurements.md`.
   The MacBook's part (Safari, Metal, an iPhone through Web Inspector, the presenting
   laptop) is planned in `docs/tests/macbook.md`.
4. ~~**Registration material**~~ — done 21 November: the annotation in
   [registration.md](competition/registration.md), around what Рояк is now, with measured numbers
   only; the school agreed to the lab run (4 November); the lab's idle time is the
   author's estimate from its timetable (about 75 %), still to be logged on the PCs.
5. **Defense run sheet** — written 21 November (`docs/competition/demo-day.md`, „The 20 minutes“);
   the deck and the full script were added 23 November (`docs/competition/presentation/`: `Рояк.pptx`
   with Morph transitions, `Рояк.pdf` as a fallback, `сценарий.md` timed to the regulation's
   20 minutes and mapped to its scoring protocol);
   still to rehearse and time: the committee joins by QR, a
   timetable for a real school is searched live on their phones, a cheater is caught,
   a device is switched off mid-job; `docs/competition/demo-day.md` updated and timed.
6. ~~**README for the jury**~~ — done 21 November, in Bulgarian; the original design
   moved to [vision.md](design/vision.md) without the catalog metadata.
6a. **AI on the swarm** — every capable device runs a whole small model in the browser
   (a language model, speech recognition), and the swarm spreads many requests over
   them: a lesson transcribed, 2 000 essays summarised ([spec 0017](specs/0017-ai-on-the-swarm.md)); verification of
   float outputs (fixed decoding, tolerance on scores, spot checks on trusted devices)
   and model caching on the school's network are the hard parts.
   Phase 1 done 21 November: SmolLM2-360M answers a file of prompts on the network's
   devices with WebGPU, models pinned by hash and cached, the near-tie rule for GPUs that
   round differently, the console's form and answers; a known answer checked whenever a
   device loads a model, so a GPU that computes it wrongly takes none of its units (it
   found two runtime defaults that break the Adreno 642L). Next: a model that answers
   Bulgarian, then speech (Whisper, phase 2).
7. **More example programs** — a Monte Carlo experiment a physics teacher would run in
   class and a parameter sweep, in `examples/programs`, and a C header for programs not
   written in Rust.

## 1. Defense readiness (before registration, ≤ 25 November 2026)

The school defense has the committee join from their phones. That has to work the
first time.

1. ~~**Load test**~~ — done (spec 0006): the coordinator takes ~1 000 results/s; the
   demo moved from Tailscale Funnel (2 MB/s) to a real domain on the home line
   (20 MB/s). No VPS needed.
2. ~~**Lock-down**~~ — done: starting and stopping jobs and the chaos panel need an
   account with a role in the network or an API key (`SWARM_SUBMIT_TOKEN` for the
   public network); joining as a worker stays open.
3. ~~**Contributor safety**~~ — done (spec 0007): power levels (75 % by default),
   pause on a hidden page (unless `?background`) and below 30 % battery, a clear stop.
4. ~~**QR join**~~ — done: each network's QR code and link open its join page, and
   `/join` takes the code typed on PCs without a camera.
5. ~~**Networks**~~ — done (spec 0010): accounts by invitation, networks with roles,
   device approval, pause, power cap and working hours, live stats on joined devices.
6. **Bulgarian UI** — Bulgarian is done everywhere; English is still to add.
7. ~~**Chaos panel**~~ — done (spec 0008): in a network's live view, switch off or cut off a
   device, add a cheater or colluders; counters for what was caught and redone.
8. **Real devices** — a phone on mobile data, an iPhone, wake lock and losing signal on
   a real phone, the laptop fallback (docs/competition/demo-day.md Plan B); results go into
   docs/measurements.md.
9. ~~**Registration material**~~ — done: [registration.md](competition/registration.md).

## 2. Full version (December 2026 – February 2027)

Scheduling:
- Device benchmark on join; unit size targeted at a fixed expected runtime and at
  the device's link (slow uploads get bigger units; spec 0009 phase 2).
- Straggler mitigation: duplicate slow units when a job is nearly done.
- Weighted fair queuing across submitters.

Trust:
- Signed templates: Ed25519 manifests verified in the browser (today: hash pinning).
- Verification audit trail per unit, and "check it yourself": a judge's device
  recomputes a random verified unit.

Templates:
- 2D Ising model temperature sweep (phase-transition plot).
- ~~Hyperparameter sweep~~ — done (spec 0011): an integer-only MLP trained bit-exact on
  every device, with a live leaderboard. Next: MNIST and Fashion-MNIST, a convolutional
  layer, successive halving (spec 0011 phase 4).
- N-body / cellular-automata parameter sweep.
- ~~School timetables~~ — done (spec 0013): a school's devices search its weekly
  timetable in parallel, every timetable re-scored by the coordinator. Next: data entry
  for a deputy director (tables, spreadsheet import), printing per class and teacher,
  rounds that continue from the best timetables, and measurements on a real school's
  data and on public benchmarks (XHSTT).
- ~~Own programs~~ — done (spec 0012): a network runs its own WebAssembly on its devices;
  reviewed programs run on the public network with each device's consent.

Product:
- Sign-in with a school's Google or Microsoft account (OIDC), next to email and password.
- Submitter portal on the REST API with OpenAPI; quotas, reviewer approval for large jobs.
- Kiosk mode for lab PCs: the join page opens at boot and survives restarts.
- Lab node (spec 0009, planned): one PC in the lab takes the lab's traffic over the
  LAN, compares copies and sums passes locally, and sends ~16× fewer bytes upstream;
  each PC tests the lab path and falls back to the server. Only if the school's
  network checks say it pays off.
- Reproducible job bundles: params, seeds and template hash replayed bit-exactly by
  `sim-worker`.

Operations and evidence:
- Prometheus metrics and Grafana dashboards; Unleash flags for template rollout.
- k6 / headless scaling runs to 1,000 simulated workers.
- Energy estimate per job from measured device power.
- School lab run: speed-up and efficiency curve; verification-overhead graph.

## 3. Stretch (March 2027 onwards)

- WebRTC peer-to-peer data distribution.
- Federated learning template.
- Multi-school run.

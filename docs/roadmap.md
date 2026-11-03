# Roadmap

What is left between today's grid and the full README vision, in build order. Each
item gets its own spec in `docs/specs/` before work starts; this file only orders them.

Status on 2026-11-03: specs 0001–0008 and 0010 implemented (π and path-tracer
templates, leases with expiry, quorum verification with honeypots and reputation, CPU
fallback, load tests, contributor safety, the chaos panel, networks that clients own
and operate from the console) and the server public at `https://swarmrelay.duckdns.org`.

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
   a real phone, the laptop fallback (docs/demo-day.md Plan B); results go into
   docs/measurements.md.
9. **Registration material** — annotation (README section 21), measured numbers for it,
   the school's written permission and the lab's idle time (a week of measuring).

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
- Hyperparameter sweep: a tiny CNN trained per unit in WGSL, with a leaderboard.
- N-body / cellular-automata parameter sweep.

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

# Roadmap

What is left between today's grid and the full README vision, in build order. Each
item gets its own spec in `docs/specs/` before work starts; this file only orders them.

Status on 2026-10-30: specs 0001–0005 implemented (π and path-tracer templates, leases
with expiry, quorum verification with honeypots and reputation, CPU fallback, an
always-on server with public HTTPS).

## 1. Defense readiness (before registration, ≤ mid-December 2026)

The school defense has the committee join from their phones. That has to work the
first time.

1. **Load test** — many workers at once, locally and through the public address.
   Finds where the coordinator and the relay stop scaling; decides whether the demo
   needs a VPS and a real domain instead of Tailscale Funnel.
2. **Lock-down** — job submission and admin actions need a token; joining as a
   worker stays open.
3. **Contributor safety** — duty-cycle cap, pause on hidden tab, low battery and
   user activity, a clear stop button.
4. **QR join** — one scan from the dashboard to a phone-first worker page.
5. **Bulgarian UI** — Bulgarian by default, English available.
6. **Chaos panel** — admin-only dashboard controls that inject cheaters and kill or
   cut off workers, with live "caught / banned / result still correct" counters.

## 2. Full version (December 2026 – February 2027)

Scheduling:
- Device benchmark on join; unit size targeted at a fixed expected runtime.
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
- OIDC sign-in and roles (contributor, school admin, submitter, reviewer, admin).
- Submitter portal on a REST API with OpenAPI; quotas, reviewer approval for large jobs.
- School admin console and lab/kiosk mode with time windows and caps.
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

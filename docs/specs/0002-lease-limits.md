# 0002 — Lease limits

Status: implemented

## Problem

Spec 0001 returned a unit to the queue only when its worker disconnected. Three cases
stayed open:

1. A worker that stays connected and keeps sending heartbeats but never finishes (a
   GPU hung inside a tab) holds its unit forever.
2. Leases of a coordinator that crashed or restarted are never released: nobody is left
   to notice their workers leaving.
3. A unit that breaks every worker that tries it is handed out forever.

## Design

- **Expiry.** A lease expires `lease_ttl` (60 s) after it was issued or last renewed by a
  heartbeat.
- **Maximum lifetime.** Heartbeats renew a lease to `min(now + lease_ttl, issued_at +
  max_lease_duration)`; `max_lease_duration` is 10 minutes, far above any unit's
  expected run time (spec: 10–30 s).
- **Reaper.** Every `reap_interval` (5 s) each coordinator releases expired leases
  (`FOR UPDATE SKIP LOCKED`, so several coordinators can reap at once). Late results for
  a released lease are stale and not counted.
- **Release reasons.** Every released lease records why: `disconnected`, `failed`
  (the worker reported an error), `invalid` (the output failed validation) or `expired`.
- **Releases.** A unit counts how often its leases ended without a result. At
  `max_releases` (8) the unit fails and so does its job, with an error naming the unit
  and the last reason. Only releases count, not leases: with redundant execution (0004)
  a healthy unit is leased several times. The job's remaining units stop being
  handed out, and results still in flight become stale.

## Tests

- Database: heartbeats renew only their own active leases and never past the maximum
  lifetime; expired leases are reaped with reason `expired` and their late results
  ignored; a unit released too often fails its job, stops further claims and makes
  in-flight results stale.
- End to end, with the reaper running: a worker that keeps sending heartbeats but never
  answers loses its unit to a healthy worker, which completes the job; the hung
  worker's late result is rejected.

# 0018 — Accepting results that wait for a second device

Status: implemented

## Problem

A job in a network asks for two agreeing results per unit (spec 0004). A client that
has proved itself (20 verified results, trust 0.95) is given nine units in ten to compute
alone; every tenth is a spot check that needs a second client, because a device cannot
check itself. With one device online a job therefore climbs to about 90 % and stops (the
live organisation's image jobs stood at 486/540 and 473/540). An untrusted device is given
no unit alone, so a job stops earlier.

## What the owner sees and can do

- The job says why: „Всяка част се проверява от 2 устройства, а на линия има 1“, with
  the advice to bring another device online.
- Image, π and sweep jobs can be started with „Проверка“ set to one result.
- **New:** the owner (role admin or above) can press „Приеми без втора проверка“ (two taps)
  on a stalled job. `POST /api/networks/{id}/jobs/{job}/accept-unchecked` decides every
  unit of the job that holds a result that nobody has checked, with that result (the
  earliest one from a client that is not banned). The units are decided as a trusted
  client's solo units are: the lease's verdict is `trusted`, which counts for nobody's
  reputation, so accepting does not make a device look verified. Units without a result are
  left alone, so a job that has not run yet is not finished by it. The job finishes
  through the normal path: image, progress and dashboards update as for any decided unit.
- Nothing is accepted automatically. The public network has no such control: its
  verification is the point of the demonstration.

## Verified by

- `tests/db.rs`: a lone device answers every unit and nothing moves; accepting decides
  them, finishes the job once, earns no reputation and finds nothing left to accept;
  units without a result and the results of banned clients are left out.
- `tests/networks.rs`: over HTTP and a real device, a viewer is refused (403), the owner
  gets the finished job.
- `e2e/quorum.e2e.ts`: in the browser, one device with a check of two shows the notice, the
  same job with one result finishes, and the two taps on the stalled job finish it.

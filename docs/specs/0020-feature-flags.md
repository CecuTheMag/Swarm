# 0020 — Feature flags

Status: implemented

## Problem

Some features carry risk at the wrong moment: language-model jobs download hundreds of
megabytes to every capable device, the chaos panel lets an admin switch devices off, and
programs reviewed for the public network run on volunteers' devices. Turning one of them
off meant a code change and a deploy. A platform needs to switch a feature off in a
second (a model misbehaves, a demo goes wrong) and on again just as fast.

## What there is

- A `feature_flags` table (migration 0012): key, on or off, what it switches (in
  Bulgarian, for the console), when and by whom it was switched last. Three flags, all on
  at first:
  - `ai_jobs`: „Текст с ИИ“ jobs (spec 0017);
  - `chaos_panel`: the resilience test, i.e. switching devices off, cutting them off,
    adding cheaters (spec 0008);
  - `public_programs`: networks' approved programs on the public network (spec 0012).
- **The server enforces them.** A request for a feature that is off is refused with
  `409` and a reason ("language-model jobs are switched off") before anything else is
  looked at, whoever sends it, the console or a script with an API key. A flag without
  a row counts as on: flags switch features off and must not take one away by accident.
- `GET /api/flags`: every flag and whether it is on, for any page. The console hides
  what is off: the „Текст с ИИ“ kind of job, the tab „Устойчивост“ and the chaos panel of
  the live view, the „На общата мрежа“ option of a program job.
- The platform admin switches them in „Администрация“ → „Функции“
  (`GET /api/admin/flags`, `PATCH /api/admin/flags/{key}`); the change holds at once,
  without a deploy, and is logged.

## Verified by

- `tests/networks.rs` `feature_flags_switch_features_off_on_the_server`: everyone reads
  the three flags; a visitor (401) and a network admin (403) cannot switch them; the
  platform admin can, and the change is recorded with their name; with each flag off, an
  AI job, a program on the public network and adding cheaters are refused with 409, for
  the network's own admin too; an unknown flag is 404; switched on again, the same AI job
  request reaches the template's own checks (400).
- The OpenAPI test (spec 0019) covers the three new routes.
- By hand: switching `ai_jobs` off in „Функции“ removes „Текст с ИИ“ from a network's
  jobs page; switching it on brings it back.

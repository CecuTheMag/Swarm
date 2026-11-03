# 0010 — Networks that clients own and operate

Status: implemented

## Goal

Рояк is a product, not one shared grid. A client (a school, a lab, a researcher) has an
account, creates their own **мрежа** (network) of devices, adds devices with a link or a
QR code and operates them: runs jobs, pauses and resumes devices, sets rules and working
hours, invites colleagues. Devices that join see their network live. The public volunteer
grid stays, as one network among the others.

Decisions: sign-in by email and password; accounts by invitation only; a network may lend
its idle devices to the public network, off by default.

## Model

Migration `0005_networks.sql`.

| Table | Holds |
|---|---|
| `accounts` | email (unique, case-insensitive), name, argon2id hash, `is_admin` (platform admin) |
| `sessions` | SHA-256 of the session token, account, expiry (30 days) |
| `invites` | SHA-256 of the token, optional email, optional network and role, expiry (14 days), single use |
| `networks` | name, join code, join mode (open or approval), max power (25/50/75/100 %), working hours, paused, lend idle |
| `memberships` | network, account, role: owner, admin or viewer |
| `devices` | one per (network, browser): name, status (active, pending, removed), paused, parts done, last seen |
| `api_keys` | network, SHA-256 of the key (`swk_…`), label, last used |

Jobs, units and workers carry `network_id`; the migration puts existing rows in the public
network, whose id is fixed (`00000000-0000-0000-0000-000000000001`) and whose join code is
`public`. Reputation and bans stay on `clients`, across networks.

## Accounts

- The platform admin is created on the server: `just admin-create EMAIL user@host` runs
  `coordinator admin-create EMAIL` in the container and prints a random password once,
  there only. Running it again resets the password. Development and tests use
  `SWARM_BOOTSTRAP_ADMIN=email:password` instead.
- The admin creates invitation links (`/app/admin`); an owner or admin of a network
  invites people into it with a role no higher than their own (`Членове`). The link opens
  `/invite/<token>`: a new person picks a name and a password (at least 10 characters);
  someone already signed in just joins. A link works once.
- The session is an HttpOnly, SameSite=Lax cookie (Secure over HTTPS) holding a random
  256-bit token; the database keeps only its hash. Requests that change something must
  come from the same origin (the `Origin` host equals `Host`), on top of SameSite.
- Failed sign-ins are limited to 8 per email per 15 minutes; an unknown email costs as
  much time as a wrong password.

## Roles

| Role | Can |
|---|---|
| viewer | see the network, its devices, jobs and live view |
| admin | and run and stop jobs, pause, rename, approve and remove devices, change the rules, create API keys, use the resilience tools, invite admins and viewers |
| owner | and change members' roles, remove members, delete the network |

A network always keeps an owner. Someone who is not a member gets 404, not 403, so
networks cannot be discovered. API keys act as admin of their own network only;
`SWARM_SUBMIT_TOKEN` is the public network's key, so scripts, load tests and the old
`/api/jobs` routes keep working.

## Devices and control

`Hello` carries an optional `join_code` (none means the public network). The coordinator
resolves the network (unknown code: closed with `unknown network`), creates or updates the
device row and, in approval mode, keeps a new device pending until an admin approves it.

After `Welcome` and whenever something changes, the coordinator sends
`Control { control, max_power }`, where `control` is the first of these that applies:

| State | When |
|---|---|
| `removed` | an admin removed the device: the connection closes and the page stops for good |
| `pending_approval` | approval mode, not yet approved |
| `network_paused` | the whole network is paused |
| `paused_by_owner` | this device is paused |
| `outside_hours { resumes_at }` | the network has working hours and now is outside them |
| `running` | otherwise |

A device that is not running stays connected and gets no work; the worker page says why in
plain words. Its power choice is capped at `max_power`. The device's owner can always stop
it.

Working hours: a timezone and up to 14 windows of days (1 = Monday) and `from`–`to`
times. A window whose end is before its start runs past midnight (15:00–08:00); 00:00–00:00
is the whole day. „След училище“ fills weekday 15:00–08:00 and the whole weekend.

## Scheduling and isolation

A device gets units only from its own network's jobs. When its network has nothing to do
and lends idle devices, it takes units from the public network. Honeypots (spec 0004) come
only from the device's own network or the public one, never from another client's.
Results are verified as before.

## Live data

- `/ws/networks/{id}/live` (members): the same events as the old dashboard, for this
  network only — devices, jobs, progress, bans. The console's overview, jobs and live view
  draw from it.
- `/ws/join/{code}/summary` (anyone with the join code): once a second, the network's
  name, devices online by kind, parts per second and the current job's progress. The
  device's own share is counted on the page. No device details leak.
- `/ws/dashboard` stays as the public network's stream (the home page's live numbers).

## Web

| Page | For |
|---|---|
| `/login`, `/invite/[token]` | signing in, accepting an invitation |
| `/app` | my networks, create one |
| `/app/n/[id]` | **Обзор**: live map, devices online, current job, pause the network, add devices (link, QR code, code) |
| `…/devices` | the table: name, kind, state, parts, last seen; pause, resume, rename, remove; approvals; several at once |
| `…/jobs` | a new job in plain terms (image quality or π), advanced settings folded; the list with progress, stop, result |
| `…/rules` | name, who can join, max power, working hours, lending, new join link, delete |
| `…/members` | members and roles, invitation links |
| `…/api` | API keys |
| `…/resilience` | switch off or cut off a device, add cheaters (spec 0008) |
| `…/live` | full-screen dark live view for a screen or projector |
| `/app/admin` | platform admin: invitations, accounts |
| `/join/[code]` | the worker page bound to that network, with its live card |
| `/worker` | the worker page for the public network |

`/dashboard` redirects to `/app`; `/render/[id]` shows a public job's live view.

## Tests

- Rust (`tests/networks.rs`): accounts and invitations (single use, roles no higher than
  the inviter's), roles on every route (a viewer cannot pause, an outsider gets 404),
  isolation (a device never gets another network's unit; lending only when on and idle),
  honeypots never cross networks, owner controls reach a connected device (pause, hours,
  approval, removal), per-network live streams with a real session cookie. Unit tests for
  working hours (past midnight, next opening) and control order.
- Unit (Vitest): the worker session obeys `Control` and stops for good when removed.
- Browser (`e2e/console.e2e.ts`): sign in, create a network, read its QR code, a second
  browser joins by the link and sees the live card, pause and resume reach it, a π job runs
  on it, approval mode holds a newcomer until approved, a viewer invited by link cannot
  change anything, a removed device stops, the network is deleted.

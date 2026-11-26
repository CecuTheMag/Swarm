# 0008 — Chaos panel

Status: implemented

## Goal

The defense claims that the grid survives devices that vanish and devices that lie.
The organiser proves it live, from the projector page, without a terminal: switch a
device off, cut one off, add a cheater or two colluding ones, and the room watches the
image come out right anyway.

## Coordinator

Every action needs the submit password (`SWARM_SUBMIT_TOKEN`, as for starting jobs).

| Endpoint | Effect |
|---|---|
| `POST /api/chaos/workers/{id}/kill` | The device is switched off: its connection closes at once and its units go back to the queue (`worker_left` with `units_returned`) |
| `POST /api/chaos/workers/{id}/cut` | The device loses its signal: the coordinator stops reading from it and answering it. The session ends only when the connection has been silent for the idle timeout (30 s), as for a phone in a tunnel; the browser meanwhile notices that no reply comes (20 s) and reconnects as a new device |
| `POST /api/chaos/cheaters` `{count, collude}` | 1–3 simulated devices that make up every result join through the coordinator's own WebSocket, like any device. `collude: true` makes them return the same wrong answer, so they agree with each other |

Both disruptions answer 404 for a device not connected to this coordinator. Each
session has a one-shot channel the endpoint sends on.

Cheaters run in the coordinator process (`sim-worker`'s library), take 1 s per unit
so the room sees them join before they are caught, and leave after 5 minutes if they
never are (no job running). At most 6 run at once. They need the coordinator's own
address, derived from `COORDINATOR_ADDR`; without it the endpoint answers 400.

Catching them is the real verification of spec 0004: disagreements with honest
results, and honeypots, which colluders fail. Colluders may get a unit or two accepted
before that; the ban redoes them, also in a job that has already finished, which then
runs again until they are decided anew (spec 0004, extended for this panel).

## Projector

The render page shows the panel in its side column, under the event feed, only in a
browser that has the submit password stored (typed once on the dashboard). „Изключи
устройство“ and „Прекъсни връзката“ pick a random connected device, browsers first.
„Добави измамник“ adds one random cheater, „Двама в сговор“ two colluders.

Everyone sees the outcome: the feed and the honeycomb as before, and once a cheater
is caught a counter under the image — cheaters caught, wrong results rejected, units
computed again — with the panel adding devices that left and units returned.

The run sheet in `docs/competition/demo-day.md` uses the panel instead of `just cheater` (which
stays, for a terminal).

## Tests

- Rust: switching off needs the password, closes the connection and is announced;
  unknown devices give 404; a cut-off device gets no answers and loses its unit only
  after the idle timeout, after which another device finishes the job; panel cheaters
  (colluders) are both caught and the result is exact; counts outside 1–3 and a
  coordinator without its own address are refused. A ban reopens a finished job and
  withdraws the honeypot failures its poisoned known answer caused.
- Unit (Vitest): the totals of departures, units returned, bans, wrong results and
  units redone.
- Browser (Playwright): without the password the render page has no panel; with it,
  switching off a browser worker makes it reconnect, and a cheater added from the
  panel is caught on the projector.

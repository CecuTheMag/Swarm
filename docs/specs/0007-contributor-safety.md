# 0007 — Contributor safety

Status: implemented

## Goal

A device that joins the grid stays the owner's device: it never runs flat out unless
the owner asks, it stops when the owner looks away or the battery runs low, and the
owner can see why it is not computing. This is the original design's promise ([vision.md](../vision.md)) ("throttling, never
full GPU, pauses on battery < 30 %, visibility-aware, clear stop button") and the answer
to the jury's "will it cook my phone?".

## Power limit (duty cycle)

The worker page offers four power levels: 25, 50, 75 and 100 %. At level `p`, after a
unit that took `t` ms the device rests `t·(100 − p)/p` ms before starting the next, so it
computes `p` % of the time. Default 75 %. The choice is remembered in the browser.

Resting does not idle the link: units are still fetched ahead, sized to the slower pace
(the effective unit time is `t·100/p`).

## Pauses

The device pauses when:

| Reason | Condition | Resumes when |
|---|---|---|
| hidden | the page is hidden (another tab or app, screen off) | the page is visible again |
| battery | the battery is below 30 % and not charging (Battery Status API: Chrome and Edge have it; Safari and Firefox do not, so there the pause never fires) | it charges, or is above 30 % again |

Pausing finishes the unit being computed and sends its result, then closes the
connection: units fetched in advance go back to the queue at once instead of waiting
for their leases to expire, and the dashboard shows the device leaving. Resuming
reconnects. A device that is never resumed costs the grid nothing.

The page says why it is paused. "Спри" still stops everything.

`/worker?background` keeps computing while the page is hidden (the battery still
pauses it). It is for a device whose owner set it up on purpose: the presenting laptop's
worker tab behind the projector page, and later lab PCs, where the school admin console
will turn it on.

## Tests

- Unit (Vitest): rest time for each level; the effective unit time used for fetching
  ahead; a pause during a unit sends that unit's result and then closes the connection
  without reconnecting; a resume reconnects; a pause while idle closes at once; battery
  and visibility map to the right pause reason.
- Browser (Playwright): the power level survives a reload; hiding the page pauses and
  showing it resumes; with `?background` a hidden page keeps computing.
- On a real phone (Nothing Phone (1), over adb): power levels change the measured duty
  cycle; switching apps pauses and returning resumes.

# 0016 — Hardening: a closed public network, bounded sign-ins, security headers

Status: implemented

## Problem

A security pass on 2026-11-05 (probes against a local coordinator, then a read of the
code) found three weak spots. None was exploited, and the server's configuration hid the
first one, but each was one mistake away from mattering:

1. **The public network failed open.** A coordinator started without `SWARM_SUBMIT_TOKEN`
   made anyone who did not sign in an admin of the public network: start and cancel jobs,
   use the chaos panel, list, pause and remove its devices, create API keys. The server's
   compose file refuses to start without the token, so production was closed, but any
   other coordinator (a laptop on a lab network, a new server) was wide open.
2. **Sign-ins ran without limit.** Each one checks a password with Argon2id: 19 MiB and
   about 50 ms of a core. The per-email limit (8 wrong passwords in 15 minutes) never
   triggers for made-up emails, which still run a check against the dummy hash, so a flood
   of sign-ins could run thousands of checks at once and use up the server's memory.
3. **No security headers.** Neither the site, nor the coordinator, nor either Caddy set a
   Content-Security-Policy, HSTS, or anything against framing. Another site could load the
   console invisibly in a frame and trick an owner into clicking „Премахни“ on a device.

## A closed public network

- `network_access` opens the public network to strangers only when the coordinator runs
  with `SWARM_OPEN_PUBLIC=1` and no submit token. Without either, the public network takes
  jobs only from its admins, signed in, as a network created by a client does.
- A submit token always wins: with it, `SWARM_OPEN_PUBLIC` changes nothing.
- On start the coordinator logs which it is: a warning when the public network is open
  ("never on a public address"), a note when it is closed without a token.
- `.env.example` sets `SWARM_OPEN_PUBLIC=1` for a development machine, where `just job`
  submits without a password. Note: `just share-public` puts that machine on the internet,
  open mode included.

## Bounded sign-ins

- At most `SIGN_IN_CHECKS` (4) password checks run at once, so they take at most 76 MiB
  together. A sign-in waits for a free check up to `sign_in_wait` (10 s), then gets 429,
  which the console shows as „Твърде много опити. Опитайте след малко.“
- Waiting never counts as a wrong password: a flood cannot lock a real teacher's email.
- A check keeps its slot until it is done, even when its sign-in is given up: a client
  that hangs up drops its request, and if that freed the slot, clients hanging up at once
  would run unlimited checks after all.
- 30 teachers signing in at once wait about 0.4 s at most for their turn.

## Security headers

| Where | Headers |
|---|---|
| Site (SvelteKit `csp`, `hooks.server.ts`) | `Content-Security-Policy` (below), `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: same-origin` |
| Coordinator (every response, errors included) | `Content-Security-Policy: default-src 'none'; frame-ancestors 'none'`, `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer` |
| Swarm's Caddy (`deploy/Caddyfile`) | `Strict-Transport-Security: max-age=31536000` on everything; the other three on responses that lack them, such as static files |

The site's policy: everything from its own origin (`default-src 'self'`); scripts its
own, inline ones only with the response's nonce, and WebAssembly compiled from its own
modules (`'wasm-unsafe-eval'`); styles may be inline, as Svelte sets style attributes;
images also from `data:` and `blob:` (the live image); WebSockets `ws:` and `wss:`,
because older Safari does not count a same-origin WebSocket as `'self'`; no plugins
(`object-src 'none'`), no `<base>` elsewhere, forms only to the site, and no framing
(`frame-ancestors 'none'`). Web Workers load from the site's own files, which carry no
policy, so programs and kernels compile there as before.

`Referrer-Policy: same-origin` keeps join and invite links, whose codes are secrets, from
reaching other sites in a `Referer`.

HSTS is honoured only where a browser reached the site over HTTPS (Kant's Caddy,
Tailscale Funnel); on the LAN port, plain HTTP, browsers ignore it.

## Tests

- `networks.rs`: `the_public_network_stays_closed_without_its_token` (strangers get 401 on
  jobs, devices, keys, members and the chaos panel; the platform admin still submits;
  open only with `open_public`, never once a token is set),
  `sign_ins_wait_for_a_free_password_check` (with every check taken, ten sign-ins get
  429, then the right password still works),
  `an_abandoned_sign_in_keeps_its_slot_until_its_check_ends`,
  `responses_carry_security_headers`.
- Each fails with its fix undone.
- `web/e2e/headers.e2e.ts`: the home, sign-in, worker, self-test and projector pages carry
  the headers, run their inline script, and raise no CSP violation in Chromium (with and
  without WebGPU), Firefox and WebKit. Every other browser test runs under the policy too.

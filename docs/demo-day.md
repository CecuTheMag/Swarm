# Demo day

How to run Рояк in front of judges, and what to do when something fails. Browsers allow
WebGPU (and the hashing workers use to check kernels) only on HTTPS pages or on
`localhost`, so every plan needs an HTTPS address that the audience's phones can reach.

## Plan A: the server (normal)

The always-on server is public at **`https://swarmrelay.duckdns.org`** (`just deploy`).
Devices connect straight to the home line, with a Let's Encrypt certificate, so a phone
needs nothing but the QR code.

How the address works: the DuckDNS name points to the home connection, whose router
forwards ports 80 and 443 to the server. On the server those ports belong to the Caddy
of another project (Kant), which also serves `swarmrelay.duckdns.org`: it terminates
TLS, renews the certificate by itself and passes everything, WebSockets included, to
Swarm's own proxy on port 7780 ([`deploy/front-proxy.caddy`](../deploy/front-proxy.caddy)
is the block it holds). Swarm needs no port of its own beyond 7780 and no secret in the
repository. If the home IP changes, DuckDNS must be updated (the router or a cron job
does it with the DuckDNS token, which stays outside the repository).

The Tailscale Funnel address `https://swarm.tail629f50.ts.net` (`just public`) stays as
a second door. It is about ten times slower (≈2 MB/s against ≈20 MB/s, [measurements](measurements.md#public-address-relay-sweep-spec-0006))
and dropped a third of 30 connections, so use it only if the DuckDNS name fails.

The day before:

- `curl https://swarmrelay.duckdns.org/healthz` answers 200.
- On the presenting laptop, open `/dashboard`, enter the submit password once (it is in
  `~/swarm.env` on the server: `ssh <server> cat swarm.env`); the browser remembers it,
  and the render page then shows the chaos panel (spec 0008).
- Start a small render job and open its render page on the projector. Scan the QR code
  with a phone on **mobile data** (Wi-Fi off), tap „Включи се“, and watch the device
  count go up. Try „Изключи устройство“ and „Добави измамник“ once.

At the defense (README section 18, minutes 3–10), with the render page of a fresh job
on the projector (`/dashboard` → „Пусни“). Stop any job still running first („Спри“ on
the dashboard): jobs run in order, and a forgotten one would hold up the new one.

| Say | Do | The room sees |
|---|---|---|
| "Please join." | Point at the QR code | Each phone becomes a cell of the honeycomb as it joins (amber: graphics card, blue: processor); the feed says "Включиха се N устройства" |
| "Every tile you see came from one of your devices." | — | The image fills in and sharpens; cells flash as their devices finish work |
| "Together you are N times faster than the fastest phone here." | — | ×speed-up, light paths per second, total paths computed |
| "Now one of your phones loses its power." | „Изключи устройство“ in the chaos panel | A cell disappears; the feed: its units go back to the queue and are computed elsewhere; the image keeps filling |
| "Anyone can join, so anyone can cheat. Here is a cheater." | „Добави измамник“ | A violet cell appears and within seconds turns red with ✕; a red banner over the image: caught, blocked, its work checked again; a red counter under the image |
| "Cheaters can also agree with each other." (if time) | „Двама в сговор“ | Two cells, both caught by known-answer units in about a minute; anything they got accepted is computed again |

„Прекъсни връзката“ shows a device that loses its signal: the coordinator notices only
when it goes silent (30 s), so it is slower to watch; keep it for questions. Closing
the worker tab on the presenting laptop still works as the physical version of
„Изключи устройство“ (open it as `/worker?background` before the defense: a plain worker
tab pauses while hidden behind the projector page), and `just cheater` (SSH to the
server) as a terminal version of „Добави измамник“.

Add colluders while the job still has two minutes or more to run. If they get units
accepted and the job finishes before they are caught, the job reopens and computes
those units again (spec 0004): correct, but the room sees "готово" turn back into a
running job. Rehearsed on the server on 2026-11-03 with 4 honest simulated devices: a
single cheater was blocked 3.4 s after it was added; two colluders got 49 of 360 tiles
accepted by agreeing with each other, were blocked after about 70 s, and those tiles
were computed again within 11 s. With a room of phones they meet each other's units
less often.

Rehearsed on the server on 2026-10-29: the cheater was blocked 3.1 s after joining, and
the whole segment fits one 1920×1080 or 1600×900 screen. Optional: `just fleet 8` adds
the server's processor as 8 more devices, `just fleet 0` removes them.

Defaults (1280×720, 64² tiles × 128 samples, 8 passes) take a phone ~0.1 s per unit and
a lab PC's processor ~0.4 s. The coordinator is not a concern: it handled 1 000
simultaneous devices at ~1 000 results/s. The home line is the ceiling: each result is
~62 KB, so a phone at full speed sends ~0.6 MB/s and the line's 22 MB/s download takes
about 35 such phones. For a bigger room, set „Проби на проход“ to 256: half the bytes
per light path.

## Plan B: the server or its internet is down

Any laptop with Docker and internet access (school Wi-Fi, or a phone's hotspot) runs the
same stack:

```
just stack                 # build and start everything on port 7780 (first build ~10 min)
just share-public 7780     # public HTTPS through this laptop's Tailscale Funnel
```

The render page's QR code points to whatever address it is opened on, so open it through
the Funnel address the second command prints. Funnel must be allowed for the laptop in
the tailnet's settings once beforehand. Build the stack on the laptop the day before:
the build needs internet access too. `just stack-down` stops it; `just unshare` ends
sharing. Funnel carries about 2 MB/s, a handful of phones at full speed: plenty for
the demo's story, not for a record speed-up. The chaos panel works the same.

## Plan C: no internet at all

Phones cannot join: plain `http://<laptop-ip>` is not secure, so their browsers refuse
WebGPU and the kernel checks. What still works is the laptop itself: `just stack`, then
`http://localhost:7780/worker` in several browser windows and simulated workers
(`just sim 4 ws://localhost:7780/ws/worker`), with
the render page on the projector. Keep a screen recording of a Plan A run as the last
resort.

# Demo day

How to run Рояк in front of judges, and what to do when something fails. Browsers allow
WebGPU (and the hashing workers use to check kernels) only on HTTPS pages or on
`localhost`, so every plan needs an HTTPS address that the audience's phones can reach.

## Plan A: the server (normal)

The always-on server publishes `https://swarm.tail629f50.ts.net` through Tailscale Funnel
(`just deploy`, `just public`).

The day before:

- `curl https://swarm.tail629f50.ts.net/healthz` answers 200.
- On the presenting laptop, open `/dashboard`, enter the submit password once (it is in
  `~/swarm.env` on the server: `ssh <server> cat swarm.env`); the browser remembers it.
- Start a small render job and open its render page on the projector. Scan the QR code
  with a phone on **mobile data** (Wi-Fi off), tap „Включи се“, and watch the device
  count go up.

At the defense (README section 18, minutes 3–10), with the render page of a fresh job
on the projector (`/dashboard` → „Пусни“). Stop any job still running first („Спри“ on
the dashboard): jobs run in order, and a forgotten one would hold up the new one.

| Say | Do | The room sees |
|---|---|---|
| "Please join." | Point at the QR code | Each phone becomes a cell of the honeycomb as it joins (amber: graphics card, blue: processor); the feed says "Включиха се N устройства" |
| "Every tile you see came from one of your devices." | — | The image fills in and sharpens; cells flash as their devices finish work |
| "Together you are N times faster than the fastest phone here." | — | ×speed-up, light paths per second, total paths computed |
| "Now I pull the plug on my laptop." | Close the worker tab on the presenting laptop (open `/worker?background` before the defense: a plain worker tab pauses while hidden behind the projector page) | Its cell disappears; the feed: its units go back to the queue and are computed elsewhere; the image keeps filling |
| "Anyone can join, so anyone can cheat. Here is a cheater." | `just cheater` on a laptop with SSH to the server | A new cell appears and within seconds turns red with ✕; a red banner over the image: caught, blocked, its work checked again |

Rehearsed on the server on 2026-10-29: the cheater was blocked 3.1 s after joining, and
the whole segment fits one 1920×1080 or 1600×900 screen. Optional: `just fleet 8` adds
the server's processor as 8 more devices, `just fleet 0` removes them.

Defaults (1280×720, 64² tiles × 128 samples, 8 passes) take a phone ~0.1 s per unit and
a lab PC's processor ~0.4 s. The coordinator is not the limit: the server handled 1 000
simultaneous devices at ~1 000 results/s. **Funnel is**: it uploads ~1.5 MB/s in total,
about 25 tile results per second, which two or three phones already fill at these
defaults ([measurements](measurements.md#public-relay-tailscale-funnel)). Through Funnel
the image still fills in for any number of devices, but the speed-up stays near ×2–3. A
direct HTTPS path to the server (a VPS or a forwarded port with a domain) removes the
limit.

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
sharing.

## Plan C: no internet at all

Phones cannot join: plain `http://<laptop-ip>` is not secure, so their browsers refuse
WebGPU and the kernel checks. What still works is the laptop itself: `just stack`, then
`http://localhost:7780/worker` in several browser windows and simulated workers
(`just sim 4 ws://localhost:7780/ws/worker`), with
the render page on the projector. Keep a screen recording of a Plan A run as the last
resort.

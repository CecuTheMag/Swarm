# Device test session

How to put real devices (lab PCs, phones) on the grid and record what they do in
[measurements.md](measurements.md).

## Always-on server

The deployed stack (`just deploy`) is public at `https://swarmrelay.duckdns.org`
(docs/demo-day.md, Plan A), and at `https://swarm.tail629f50.ts.net` through Tailscale
Funnel as a slower second address (`just public`; the first run prints a login link).
Devices open `/selftest` and `/worker` there: nothing to install, no dev server needed.
`just fleet N` adds N simulated workers on the server.

## Why HTTPS

Browsers only expose WebGPU in a secure context: `https://` or `localhost`. A device
opening `http://192.168.x.x:7701` gets no `navigator.gpu` and the worker fails. The
dev server is therefore served over HTTPS at this machine's Tailscale name.

| Device | How it reaches the dev server |
|---|---|
| Phone or PC with Tailscale | `just share` → `https://<machine>.<tailnet>.ts.net` (tailnet only) |
| Device without Tailscale (school PC) | `just share-public` (Tailscale Funnel, public URL) |

`share-public` puts the app on the internet with no auth: anyone with the URL can join
as a worker or submit jobs. Keep it on only for the session, then `just unshare`.
Installing Tailscale on a school PC needs the school's permission; the public URL
needs nothing installed on the device.

## Before

1. `just services`, `just coordinator`, `just web` (each in its own terminal).
2. `just share` or `just share-public`; note the URL it prints.
3. `just job 64 16777216 1`: queue a job so workers start immediately.

## On each device

1. Chrome or Edge, up to date. Note OS, browser version, and the GPU
   (`chrome://gpu` on desktop).
2. Open `<url>/selftest`. Note whether it passes and the speed it reports.
3. Open `<url>/worker` and leave it running until the job finishes.
4. Watch `<url>/dashboard` on another screen.

If `/selftest` says WebGPU is unavailable, check `chrome://gpu` for "WebGPU: Hardware
accelerated" and record the device anyway: unsupported devices are a result too.

## School network (for spec 0009)

Ten minutes on one lab PC, during the same visit. They decide whether a lab node is
worth building; record the answers in [measurements.md](measurements.md) ("School
network").

1. **Speed.** Open `https://speed.cloudflare.com` on a lab PC and note download and
   upload. A lab of 30 PCs on the CPU sends about 4.6 MB/s (37 Mbit/s) of render
   results; if the upload is well above that, no lab node is needed.
2. **Private names.** In a command prompt: `nslookup 10.0.0.1.nip.io`. The answer
   `10.0.0.1` means the school's DNS lets a public name point to a private address (the
   lab node needs that); no answer or an error means it blocks it.
3. **PCs reach each other.** Plug the laptop into the lab network (the same socket or
   switch as the PCs, not the school Wi-Fi), run `just stack` on it, and open
   `http://<laptop's address>:7780/healthz` on a lab PC. A page that loads means lab PCs
   can reach a machine in the lab; a timeout means they are isolated.

## After

1. `just bench-report`: per-device unit count, average unit time and Msamples/s.
2. Copy the rows into [measurements.md](measurements.md) (WebGPU support and
   Benchmarks tables) with the date and browser version.
3. `just unshare`.

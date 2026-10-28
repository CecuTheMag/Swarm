# Device test session

How to put real devices (lab PCs, phones) on the grid and record what they do in
[measurements.md](measurements.md).

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

## After

1. `just bench-report`: per-device unit count, average unit time and Msamples/s.
2. Copy the rows into [measurements.md](measurements.md) (WebGPU support and
   Benchmarks tables) with the date and browser version.
3. `just unshare`.

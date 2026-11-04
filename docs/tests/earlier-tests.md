# Tests before the lab

The details and every number are in [`docs/measurements.md`](../measurements.md); this
page is the list.

| Date | Test | Result | Section of measurements.md |
|---|---|---|---|
| 2026-10-27 | WebGPU on the dev PC (headless Chromium) | Only a software adapter (SwiftShader) headless | WebGPU support by device |
| 2026-10-28 | Nothing Phone (1), Chrome 156 | WebGPU on the Adreno 642L; self-test bit-exact | WebGPU support by device |
| 2026-10-28 | ThinkPad T540p (2014), Edge 156 | No WebGPU adapter; joins on the CPU | WebGPU support by device |
| 2026-10-29 | Browser engines: Chromium (WebGPU and CPU), Firefox, WebKit | All browser tests pass, bit-exact on the CPU | Browser engines |
| 2026-10-29 | Load: 100 and 1 000 simulated devices | 1 000 devices, 930–980 results/s, exact π; 4 bottlenecks found and fixed | Scale (load tests) |
| 2026-10-29 | Load: 1 000 devices rendering 1280×720 with the render page open | Done in 43 s | Scale (load tests) |
| 2026-10-29 | Cheating devices (spec 0004) | Caught and banned; their results redone | Result verification |
| 2026-10-30 | Power levels on the phone (25–100 %) | Share of time computing matched the level within 2 points | Contributor safety on a real phone |
| 2026-10-30 | Pause when the page is hidden | Prefetched unit returned at once; rejoined in 4 s | Contributor safety on a real phone |
| 2026-11-02 | Public relay (Tailscale Funnel) and the home line | Both serve devices from outside | Public relay; Public address |
| 2026-11-03 | Phone on 5G through the public address | Joined, rendered 166 tiles alone | A phone on mobile data |
| 2026-11-03 | Phone on 5G joins with a typed code (spec 0010) | Joined on its GPU; the network card showed it live | A phone on mobile data |
| 2026-11-03 | Rehearsal: 30 simulated devices in one client network (spec 0010) | Pausing stopped all 30 within a second; the job finished after resuming; none of its jobs reached the public network | — |
| 2026-11-03 | Integer training against f32 (spec 0011) | Integer networks within half a point of f32 | Neural network sweep: integer training against f32 |

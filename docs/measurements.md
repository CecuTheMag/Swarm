# Measurements

Every number used in the competition documents comes from this log.

## WebGPU support by device

| Date       | Device                       | GPU                                                 | OS              | Browser                                       | WebGPU                                                                           | Notes                                                                                                                                                                                                                                                   |
| ---------- | ---------------------------- | --------------------------------------------------- | --------------- | --------------------------------------------- | -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-10-27 | Dev PC                       | NVIDIA RTX 4060                                     | Arch Linux      | Chromium 154 (headless)                       | Only with `--enable-unsafe-webgpu --enable-features=Vulkan`; SwiftShader adapter | Headless does not use the real GPU                                                                                                                                                                                                                      |
| 2026-10-28 | Nothing Phone (1)            | Adreno 642L (Snapdragon 778G+)                      | Android 15      | Chrome 156                                    | Yes                                                                              | Adapter `qualcomm` / `adreno-6xx`; self-test bit-exact on all vectors                                                                                                                                                                                   |
| 2026-10-28 | Lenovo ThinkPad T540p (2014) | Intel HD Graphics 4600, driver 20.19.15.5126 (2020) | Windows 10 22H2 | Edge 156                                      | No                                                                               | `edge://gpu` reports WebGPU enabled, but `requestAdapter()` returns null for default, high-performance, low-power, compatibility and fallback adapters. Joins through the CPU fallback (spec 0005)                                                      |
| 2026-11-21 | MacBook Pro, Apple M1 Max    | Apple M1 Max, 24-core GPU (Metal)                   | macOS 26.3.1    | Chrome 158                                    | Yes                                                                              | Adapter `apple` / `metal-3`, `maxBufferSize` 4 GiB; self-test bit-exact on all vectors, the path tracer and the neural network included (3 runs)                                                                                                        |
| 2026-11-21 | MacBook Pro, Apple M1 Max    | Apple M1 Max, 24-core GPU (Metal)                   | macOS 26.3.1    | Edge 157                                      | Yes                                                                              | As Chrome: adapter `apple` / `metal-3`; bit-exact on all vectors (3 runs)                                                                                                                                                                               |
| 2026-11-21 | MacBook Pro, Apple M1 Max    | Apple M1 Max, 24-core GPU (Metal)                   | macOS 26.3.1    | WebKit 27.2 (Playwright, in a window)         | Yes                                                                              | Safari's engine and WGSL compiler; adapter `apple` / `apple`, `maxBufferSize` 2 GiB; bit-exact on all vectors (3 runs)                                                                                                                                  |
| 2026-11-21 | MacBook Pro, Apple M1 Max    | Apple M1 Max, 24-core GPU (Metal)                   | macOS 26.3.1    | Firefox 157 (Playwright's build, in a window) | Yes, slowly                                                                      | Firefox's WGSL compiler (naga); the adapter reports no vendor; bit-exact on all vectors, but π at 38 M samples/s, about 150× slower than the other engines. Playwright's patched build, not the released Firefox 160, so not a number for Firefox users |

## Browser engines (automated tests)

Every browser test (self-test against the Rust reference, a worker computing exact π, a
worker rendering, the dashboard, losing the connection mid-unit, keeping the screen on)
runs in each engine. Headless Firefox and WebKit on Linux expose no WebGPU, so there they
test the CPU fallback, which is what a browser without WebGPU gets.

| Date | Engine | Version | Backend | Result |
|---|---|---|---|---|
| 2026-10-29 | Chromium (headless, SwiftShader) | Playwright 1.64 | WebGPU | All pass; π exact, path tracer within tolerance |
| 2026-10-29 | Chromium (headless, no WebGPU) | Playwright 1.64 | CPU (WebAssembly) | All pass, bit-exact |
| 2026-10-29 | Firefox (headless) | Playwright 1.64 | CPU (WebAssembly) | All pass, bit-exact; screen wake lock works |
| 2026-10-29 | WebKit, Safari's engine (headless, Ubuntu container) | Playwright 1.64 | CPU (WebAssembly) | All pass, bit-exact; wake lock not tested (WebKit on Linux has none) |
| 2026-11-11 | Chromium 156 (headless, macOS 26.3.1, M1 Max) | Playwright 1.64.1 | WebGPU, SwiftShader adapter (`google` / `swiftshader`): the Vulkan flag picks it on macOS too | All pass |
| 2026-11-11 | Chromium 156 (headless, no WebGPU, macOS) | Playwright 1.64.1 | CPU (WebAssembly) | All pass, bit-exact |
| 2026-11-11 | Firefox 157 (headless, macOS) | Playwright 1.64.1 | CPU (WebAssembly): no adapter in Playwright's headless Firefox | All pass, bit-exact |
| 2026-11-11 | WebKit 27.2, Safari's engine (headless, natively on macOS) | Playwright 1.64.1 | **WebGPU on the real GPU** (adapter `apple` / `apple`, Metal on the M1 Max) | All pass; self-test bit-exact on the GPU; screen wake lock works. The first automated run on real GPU hardware |

Not yet tested on real devices: Safari on an iPhone (iOS 26 ships WebGPU, so it would
take the GPU path) and the released Firefox with WebGPU (Windows, macOS). Safari's engine on Apple's
GPU passes in WebKit on macOS above, which is the same WebGPU stack the iPhone uses.

**Three shader compilers, one GPU (2026-11-21).** On the M1 Max the same WGSL goes to
Metal through Tint (Chrome, Edge), WebKit's compiler and naga (Playwright's Firefox).
All three give the reference's results bit for bit: π, the neural network's accuracy,
loss curve and SHA-256 of the final weights for all six vectors, and the path tracer's
five tiles exactly, not only within the GPU tolerance. Spec 0011's sentence about Apple
silicon holds. The Rust workspace's 263 tests also pass natively on `aarch64-apple-darwin`,
so the shared vectors are checked on a second processor architecture.

## School lab, 2026-11-04

13 PCs (Intel i3 of 2011–2012, 4 GB, Intel HD Graphics, Windows 10), all on the CPU
path; no WebGPU adapter on any. Each page in `docs/tests/` has the details.

| Test | Result |
|---|---|
| Self-test, Edge and Chrome 156 | Bit-exact on all 13; 224–369 million π samples/s and 1.5–2.2 million light paths/s per PC |
| π, public network, 400 × 2²⁸ samples | 43 s, 0 disagreements |
| π, the lab's own network, 600 × 2²⁸ samples | 60 s |
| Picture 1920×1080, 1 024 samples per pixel | About 70 s, 2 304 units, 0 disagreements |
| Neural network sweep, 672 trainings | 154 s on one thread per PC; 91 s on three, with an identical leaderboard |
| MNIST sweep, 240 trainings of 2 500 steps | 112 s; best 93.7 % of 2 000 unseen handwritten digits |

## School lab idle time

| Date | Lab | PCs | Observed idle hours | Method |
|---|---|---|---|---|
| 2026-11-21 | The lab of the 2026-11-04 run | 13 | About 75 % of the week | Estimate by the author from the lab's timetable and opening hours; not logged by the PCs themselves |

Use it in the competition documents as an estimate („около 75 % по разписанието на
кабинета“), not as a measured number. Logging it on the PCs (the share of each hour a
worker page could have computed) is still to be done.

## School network (for spec 0009)

| Date | Lab | Download / upload | Private names resolve | PCs reach a laptop in the lab | Decision |
|---|---|---|---|---|---|

## Benchmarks

| Date | Device | Template | Units/s | GFLOPS | Notes |
|---|---|---|---|---|---|
| 2026-10-28 | Nothing Phone (1), Chrome 156 | π, 2²⁸ samples/unit | 2.98 | — | 800 M samples/s sustained: 388 units, 335 ms average, no slowdown over ~5 min of work; on charger, GPU 36 °C. Self-test: 901 M samples/s |
| 2026-10-28 | Nothing Phone (1), Chrome 156 | π, 2²⁴ samples/unit | 40 | — | 25 ms per unit: units this small are dominated by per-unit overhead (680 M samples/s) |
| 2026-10-28 | Nothing Phone (1), Chrome 156 | π, 2²⁸ samples/unit, 10.6 min | 2.83 | — | Sustained-load run: 1800 units (4.83·10¹¹ samples) submitted from the T540p's dashboard, computed by the phone alone. Unit time 332 ms average, 351 ms p95, flat from first to last minute: no thermal throttling. GPU 36–37 °C, skin 37–38 °C, battery 32 °C, on charger. 761 M samples/s wall-clock including coordinator round trips |

| 2026-10-28 | ThinkPad T540p (i5-4300M, 2C/4T), Edge 156, CPU fallback (3 threads) | π, 2²⁷ samples/unit | 4.05 | — | 544 M samples/s: 128 units, 247 ms average, 255 ms p95, 36 s for the job. Self-test: 512 M samples/s. About two thirds of the phone's GPU, from a laptop with no usable WebGPU |
| 2026-10-29 | Nothing Phone (1), Chrome 156, through the public address | path tracer, 128² tiles × 64 spp | — | — | 9.66 M paths/s while computing (109 ms per unit), about 7 server CPU workers (1.41 M paths/s each). Self-test: 5.96 M paths/s, bit-exact on all vectors. After the render: GPU 43 °C, battery 35 °C, on charger |
| 2026-10-29 | magserver (Xeon E5-1620, 4C/8T, 2012), 8 native sim-workers | path tracer, 1280×720, 32² tiles, 16 spp × 16 passes | 310 | — | 14 720 units (2.36·10⁸ paths, 256 spp) in 47.5 s. 1.42 M paths/s per worker while computing, 5.0 M paths/s overall: units of ~11 ms are dominated by per-unit overhead on a server that also runs the coordinator and Postgres. Larger tiles or more samples per unit are needed for efficiency |
| 2026-10-29 | Dev laptop, headless Chromium | path tracer self-test, 64² tile × 16 spp | — | — | SwiftShader (software WebGPU): 2.98 M paths/s, bit-exact with the reference. CPU fallback (WebAssembly): 8.19 M paths/s, bit-exact |
| 2026-10-29 | Dev laptop, headless Chromium, through the public address (Tailscale Funnel) | path tracer, 160×90, 32² tiles, 8 spp × 2 passes | 1.5 | — | 30 units in 20.3 s although each computes in milliseconds: ~0.67 s round trip per unit through Funnel's relay. Units for remote devices must carry seconds of work |
| 2026-11-21 | MacBook Pro M1 Max, Chrome 158, WebGPU (Metal) | π, 2²⁸ samples/unit, 128 units | 14.4 | — | 66 ms average, 75 ms p95: 4.07 G samples/s while computing, 3.86 G/s for the 8.9 s job. 5× the phone's 335 ms per unit |
| 2026-11-21 | MacBook Pro M1 Max, Edge 157, WebGPU (Metal) | π, 2²⁸ samples/unit, 128 units | 14.4 | — | 64 ms average, 73 ms p95: 4.19 G samples/s while computing; same engine as Chrome, same speed |
| 2026-11-21 | MacBook Pro M1 Max, WebKit 27.2 (Safari's engine), WebGPU (Metal) | π, 2²⁸ samples/unit, 128 units | 24.6 | — | 37 ms average, 39 ms p95: 7.26 G samples/s while computing, 6.61 G/s for the 5.2 s job. 1.8× Chrome on the same GPU: WebKit's compiler produces the faster kernel |
| 2026-11-21 | MacBook Pro M1 Max, Chrome 158, WebGPU (Metal) | π, 2²⁸ samples/unit, 10 min | 15.4 | — | Sustained-load run on mains power, Low Power Mode off: 9 236 units (2.48·10¹² samples), 63 ms average, 77 ms p95; 62 ms in the first minute, 63 ms in the last: no throttling. 4.13 G samples/s wall-clock including coordinator round trips |
| 2026-11-21 | MacBook Pro M1 Max, WebKit 27.2, WebGPU (Metal) | π, 2²⁸ samples/unit, 10 min | 27.6 | — | Sustained-load run as above: 16 571 units (4.45·10¹² samples), 35.6 ms average, 38 ms p95, the same in every minute: no throttling. 7.41 G samples/s wall-clock, 98 % of the time computing |
| 2026-11-21 | MacBook Pro M1 Max, Chrome 158, CPU fallback (9 threads) | π, 2²⁸ samples/unit, 128 units | 12.3 | — | 74 ms average, 83 ms p95: 3.63 G samples/s. π is light integer work, so eight performance cores come close to the GPU in Chrome |
| 2026-11-21 | MacBook Pro M1 Max, WebKit 27.2, CPU fallback | π, 2²⁸ samples/unit, 128 units | 11.5 | — | 77 ms average, 78 ms p95: 3.49 G samples/s |
| 2026-11-21 | MacBook Pro M1 Max, Chrome 158, WebGPU (Metal) | path tracer, 1280×768, 128² tiles × 64 spp × 2 passes | 40.0 | — | 22 ms average, 26 ms p95: 47.7 M paths/s while computing, 41.9 M/s for the 3.0 s job. 4.9× the phone (9.66 M/s) |
| 2026-11-21 | MacBook Pro M1 Max, Edge 157, WebGPU (Metal) | path tracer, as above | 41.4 | — | 21 ms average, 26 ms p95: 49.9 M paths/s while computing |
| 2026-11-21 | MacBook Pro M1 Max, WebKit 27.2, WebGPU (Metal) | path tracer, as above | 63.2 | — | 15 ms average, 17 ms p95: 69.9 M paths/s while computing, 66.2 M/s for the 1.9 s job |
| 2026-11-21 | MacBook Pro M1 Max, Chrome 158, CPU fallback (9 threads) | path tracer, as above | 15.0 | — | 63 ms average, 68 ms p95: 16.6 M paths/s; the GPU is 2.9× faster here, where the work is floating point |
| 2026-11-21 | MacBook Pro M1 Max, WebKit 27.2, CPU fallback | path tracer, as above | 13.8 | — | 69 ms average, 74 ms p95: 15.2 M paths/s |
| 2026-11-21 | MacBook Pro M1 Max, 10 native sim-workers | path tracer, 1280×720, 32² tiles, 16 spp × 16 passes | 646 | — | The magserver job above: 14 720 units in 22.8 s (47.5 s on magserver), 5.8 ms per unit, 2.82 M paths/s per worker while computing, 10.3 M paths/s overall; coordinator (release build) and PostgreSQL on the same Mac |

Throughput through the public address (2026-10-29): one browser worker on the dev laptop
(CPU fallback, 15 threads), connecting through Tailscale Funnel, path-tracer units of
128² tiles × 64 spp (57–61 ms of compute, a 262 KiB result). Fetching one unit at a time:
0.73 units/s, the device idle ~95 % of the time: every unit cost a round trip of about
1.3 s through the relay. With units fetched in advance (as many as the round trip and
unit time call for, up to 8): 1.62 units/s, 2.2× faster, now limited by bandwidth:
uploads through the relay ran at ~300 KB/s against 17.5 MB/s on the LAN. Both ends were
behind the same home connection here, so results crossed its slow upload; a phone
elsewhere only uses the server's download. Default render jobs now use 64² tiles × 128
spp: 4× fewer bytes per light path.

Grid correctness (2026-10-28): a 128-unit π job (2²⁷ samples/unit, seed 2026) computed by
the phone (52 units) and four CPU sim-workers (76 units) gave exactly the same count as a
CPU-only run: 13 492 969 218 inside of 17 179 869 184. A 64-unit job (seed 7001) whose
phone tab was reloaded mid-unit also matched its clean rerun exactly. The same seed-2026
job computed by the T540p alone through the CPU fallback matched too.

The first CPU-fallback build lost counts when several threads finished chunks at once
(`inside += await …` reads `inside` before awaiting). The coordinator accepted those
wrong results: a 128-unit job came out at 10 442 581 916 instead of 13 492 969 218.
Shared test vectors now include a unit of five 2²² chunks, and the browser tests fail
on that build. It is also a concrete case for result verification (spec 0004): a
faulty client, not only a malicious one, can return plausible wrong counts.

## Result verification (spec 0004)

Method: a coordinator with production settings (2 agreeing results per unit, honeypots
5 % / 20 % for untrusted clients, 10 % spot checks for trusted ones) on a fresh database.
A 20-unit job is verified first so honeypots have known answers. Then the same 400-unit
π job (2·10⁶ samples per unit, seed 2026) runs three times with 6 honest simulated
workers: alone, with 2 workers that always make up random results, and with 2 colluders
that always return the same wrong result. Overhead = results computed per unit.

| Date | Scenario | Result | Results per unit | Units decided alone by trusted clients | Cheaters banned | Honest clients marked wrong |
|---|---|---|---|---|---|---|
| 2026-10-29 | Honest only | 628 338 914 | 1.20–1.21 | 318–322 of 400 | — | 0 |
| 2026-10-29 | + 2 random cheaters | 628 338 914 (exact) | 1.24–1.26 | 313–317 | 2 of 2 | 0 |
| 2026-10-29 | + 2 colluders | 628 338 914 (exact) | 1.20–1.25 | 319–324 | 2 of 2 | 0 |

Real devices with verification on (2026-10-29): the phone (Adreno 642L, Chrome 156, through
the public HTTPS address) and 8 native workers on the server rendered 1280×720 at 512 spp
(128² tiles × 64 spp, 8 passes: 480 units) in 62.5 s. No disputes and no disagreements.
Of the 23 units computed by both the phone's GPU and a server CPU, 18 were bit-identical
and 5 differed slightly yet agreed within the tolerance: on full tiles a real GPU
occasionally takes another branch, although it matched every self-test vector exactly.
Exact comparison alone would have rejected the phone. The phone became trusted during
the job and computed 54 units alone; 58 of 58 decided honeypots were answered correctly.

Two rounds each. No wrong result was accepted in any run: every scenario produced the
honest count. About 150 honeypots per round caught the cheaters (6–7 wrong answers).
Without adaptive replication the overhead would be at least 2.0; trusted clients halve it.

Found by these runs and fixed: a deadlock between a ban (which reverses verdicts on
other clients' leases) and a concurrent completion dropped a worker's connection
(transactions are now retried on `40P01`/`40001`); and reversing verdicts read the new,
not the old, value from `RETURNING`, so honest clients kept marks earned against
colluders' answers.

Chaos panel on the server (2026-11-03, spec 0008): a 640×360 render (360 units) with 4
honest simulated devices (300 ms per unit) and cheaters added through the panel's API.

| Cheaters | Caught after | Units they got accepted | Redone | Result |
|---|---|---|---|---|
| 1 random | 3.4 s (3 disagreements) | 0 | — | Done |
| 2 colluders | ~70 s (first: 2 failed honeypots; second: disagreements) | 49 of 360, the job had already finished | 49, in 11 s; the job reopened and finished again | Done, all 49 recomputed by honest devices |

Before this run's fix a ban only redid units of jobs still running, so the 49 tiles
would have stayed in the finished image.

## Coordinator capacity (load test, spec 0006)

Setup: dev PC (20 threads), release coordinator and PostgreSQL 18 (Docker) on the same
machine, redundancy 2 with honeypots on, π units of 2¹⁶ samples (≈ free to compute), so
the coordinator is the only bottleneck. `sim-worker --in-flight 4`, connections ramped
over 2–5 s. Round trip = result sent → ack received.

| Date | Workers | Run | Results/s | Round trip p50 / p95 / p99 | Failed |
|---|---|---|---|---|---|
| 2026-10-30 | 10 | 20 s | 508 | 37 / 92 / 123 ms | 0 |
| 2026-10-30 | 100 | 20 s | 564 | 355 / 516 / 613 ms | 0 |
| 2026-10-30 | 300 | 15 s | 718 | 741 / 1249 / 1378 ms | 0 |
| 2026-10-30 | 1000 | 30 s | 841 | 2147 / 3209 / 3905 ms | 0 |

Limit: every completion updates its job's row, so results for one job queue on that row
lock. PostgreSQL wait events during the run are almost all `Lock:tuple` and
`Lock:transactionid`; the coordinator used ~33 % of one core and each database backend
~14 %. Real units take seconds (a phone renders about one 64-pixel tile per second), so
one job serves several hundred devices before this lock binds. These runs predate
the changes under [Scale](#scale-load-tests) below, which shorten the time each
completion holds that lock.

## Public relay (Tailscale Funnel)

| Date | Path | Measured | Result |
|---|---|---|---|
| 2026-10-30 | Home line (Cloudflare speed test) | 20 MB up ×2, 50 MB down | 4.5 MB/s up, 21.9 MB/s down |
| 2026-10-30 | Through Funnel (3 ingress addresses × 3) | 2.0 MB PNG download | median 534 KB/s (216–1384), first byte 0.30–0.90 s |
| 2026-10-30 | Direct tailnet path, same file | 2.0 MB PNG download | 18.2 MB/s, first byte 0.03 s |
| 2026-10-29 | Phone through Funnel (commit b12fa25) | path-tracer results upload | ~300 KB/s, 1.62 units/s |
| 2026-10-30 | Through Funnel, one stream (3 ingress addresses) | 1.87 MB request body upload | 0.72, 0.91, 1.43 MB/s |
| 2026-10-30 | Through Funnel, 4 / 12 parallel streams | 1.87 MB each | 1.36 / 1.74 MB/s in total |
| 2026-10-30 | Direct tailnet path, same body | 1.87 MB upload | 21.4 MB/s |

A 64-pixel tile result is 65.5 KB (64·64·3 floats in base64), so Funnel's ~1.5 MB/s of
upload carries about 25 results per second for the whole grid. With the demo defaults a
phone finishes a tile in ~0.1 s (~650 KB/s of results each), and every tile is computed
twice until devices are trusted: through Funnel the room computes at the speed of two or
three phones, however many join. The relay, not the coordinator or the home line,
limits the demo.

## Public address: relay sweep (spec 0006)

Method (2026-11-03): `sim-worker` in a container on the server, `--in-flight 4`, 2 s
ramp, 30 s per run, path-tracer units of 64² tiles × 2 spp (a 62 KB result, almost no
computing), production verification. Through the new address it connects to the
home line's public IP (`--connect-to`), so traffic leaves the server, turns at the
router and comes back in through the front Caddy (TLS), Swarm's proxy and the
coordinator. Through Funnel it crosses the internet to Tailscale's relay and back.

| Workers | `swarmrelay.duckdns.org`: results/s | MB/s | round trip p50 / p95 | Funnel: results/s | MB/s | round trip p50 / p95 | Funnel connections failed |
|---|---|---|---|---|---|---|---|
| 1 | 50 | 3.1 | 25 / 52 ms | 11 | 0.7 | 171 / 338 ms | 0 |
| 5 | 292 | 18.0 | 25 / 44 ms | 27 | 1.7 | 354 / 938 ms | 0 |
| 10 | 311 | 19.2 | 52 / 121 ms | 28 | 1.8 | 695 / 1801 ms | 0 |
| 30 | 319 | 19.8 | 134 / 326 ms | 30 | 2.0 | 1110 / 3215 ms | 10 of 30 |

The direct address carries about ten times what Funnel does, with no failed
connections; Funnel also refused a third of 30 simultaneous devices. The plateau at
~20 MB/s matches the direct LAN path measured before (17.5–18.2 MB/s): the TLS front
adds no limit of its own. For devices outside, the home line is the ceiling: 21.7–22.1
MB/s down (results arriving), 4.8 MB/s up (units and images leaving), measured the
same day. With 62 KB per result that is ~350 results/s, about 35 phones rendering
default units at full speed.

The sweep from the dev laptop, which was on a phone hotspot tunnelled home, measured
that link instead (86 KB/s up) and is not counted.

## Scale (load tests)

Method: a second coordinator on the server (Xeon E5-1620, 4C/8T, 2012; it also runs
other services) with its own database and production settings (2 agreeing results per
unit, honeypots, spot checks). Simulated workers run on the dev laptop and connect over
the LAN; each waits a set time per unit, as a device would, instead of computing for it
(`sim-worker --delay-ms`). New clients, so no worker is trusted yet: about 2.3 results
per unit (two copies plus 20 % honeypots for new clients).

| Date | Workers | Unit time | Job | Results/s (steady) | Request → unit (p50 / p95 / p99) | Failed workers | Result |
|---|---|---|---|---|---|---|---|
| 2026-10-29 | 100 | 1 s | π, 2 000 units | 92 (of ~100 ideal) | 6 / 9 / 12 ms | 0 | Exact |
| 2026-10-29 | 1 000 | 1 s | π, 10 000 units | 930–980 (of ~1 000 ideal) | 19 / 209 / 356 ms | 0 | Exact |
| 2026-10-29 | 1 000 | none (saturation) | π, 10 000 units | 1 000–1 100 | 462 / 553 / 703 ms | 0 | Exact |
| 2026-10-29 | 1 000 | 1 s | path tracer 1280×720, 7 200 units, render page open | 600–850 | 200 / 469 / 653 ms | 0 | Done in 43 s |

All 1 000 workers connected within 5 s (and at once without a ramp) and none failed.
At saturation the server runs out of processor (PostgreSQL ~5 cores, coordinator ~1);
path-tracer results are larger, and in that run other services on the server took two
cores. For comparison, a school lab of 30 PCs needs about 75 results/s and 30 phones on
the GPU about 200.

Found by these runs and fixed:

- The projector page fell 15 s and more behind with 1 000 devices (it showed 8 % while
  the job was at 96 %): the coordinator sent one event per decided unit, and the browser
  copied the whole state for each. Decided units now go out in batches every 250 ms;
  the page then showed every job finished within a second of the real end.
- Picking a known-answer unit (honeypot) scanned every unit of a finished job and found
  none in about a third of the tries: `random()` in the join condition was drawn anew
  for every row. It is now drawn once and uses the index.
- Every decided unit updates its job's row, and each commit waited for the disk while
  holding that row, so a job's results went one at a time. Results are now committed
  without waiting for the disk (a database crash may lose the last fraction of a second
  of results, which are then computed again; job creation still waits). Short queries
  no longer pay for JIT compilation either. p95 latency with 1 000 workers went from
  246 ms to 173 ms.
- Each image request held the lock that every accepted unit needs while it read the
  database and encoded the PNG. Units now update their own tile, and one cached PNG is
  shared by all viewers.

Also found (outside the load tests):

- A worker that lost its connection while computing, and got new units on the new
  connection before the old one finished, stopped computing: it held the new units
  (keeping them alive with heartbeats) for up to the 10-minute lease limit. Found with
  a unit test that drops the connection at exactly that moment.
- WebSocket pings reset the coordinator's idle timer, so a connection kept open by a
  proxy, with the worker gone, held its units indefinitely.

## A phone on mobile data through the public address

2026-11-03: Nothing Phone (1), Chrome, on 5G (A1), opened `https://swarmrelay.duckdns.org/worker`
(Let's Encrypt certificate, nothing installed) and tapped „Включи се“ on the redesigned page.
It joined, kept the screen on and rendered a 640×360 job at 75 % power: 166 of 240 tiles in
about two minutes, alone. The rest were spot checks that wait for a second device, so the job
was stopped there. Battery 94 %, 35 °C, on a charger.

2026-11-03, after spec 0010: the same phone on 5G opened `/join`, typed the code `public`
and joined on its graphics card (Adreno 6xx); the network card showed it live. The glass
header showed the page under it sharply: the production CSS had lost
`backdrop-filter` (only the `-webkit-` twin survived the minifier, which Chromium
ignores). Fixed the same day; `e2e/look.e2e.ts` guards it.

## Contributor safety on a real phone (spec 0007)

Nothing Phone (1), Chrome 156, on USB power, worker page through `adb reverse` to the dev
server; π units of 2²⁷ samples (~165 ms on the Adreno 642L). 20 s per level.

| Date | Power | Units/s | Unit time | Share of time computing |
|---|---|---|---|---|
| 2026-10-30 | 100 % | 6.10 | 161 ms | 98 % |
| 2026-10-30 | 75 % | 4.70 | 168 ms | 79 % |
| 2026-10-30 | 50 % | 3.00 | 167 ms | 50 % |
| 2026-10-30 | 25 % | 1.45 | 173 ms | 25 % |

Pause: pressing Home hid the page; the coordinator logged the worker leaving and
returned its one prefetched unit at once, and no unit was computed in the next 10 s.
Back in Chrome, the page rejoined within 4 s and computed 5.8 units/s again. The battery
pause is covered by unit tests only (the phone's battery state cannot be lowered without
overriding system settings).

**Known limit: the battery pause works only in Chromium browsers** (2026-11-21, M1 Max).
`navigator.getBattery` exists in Chrome 158 and Edge 157 (both report the Mac's charge),
but not in WebKit 27.2, Safari's engine, nor in Firefox 157: neither engine offers the
Battery Status API to web pages. In Safari (on a Mac or an iPhone) and in Firefox the
pause below 30 % therefore never fires; the power level, the pause on a hidden page and
„Спри“ still apply. The Screen Wake Lock API is present in all four.

## Random number generator validation (π template)

Method: 16 units × 2²² samples per run (67.1M samples), independent seeds per
run, error of the π estimate measured against the exact value. `z` is the mean
error divided by its standard error; |z| > 3 means a real bias. A good generator
also matches the binomial standard deviation √(π(4−π)/N) = 2.00·10⁻⁴.

| Date | Generator | Grid point | Runs | Mean error | z | SD | Verdict |
|---|---|---|---|---|---|---|---|
| 2026-10-27 | `pcg_hash(base+2i)`, `pcg_hash(base+2i+1)` | cell corner | 40 | +2.71·10⁻⁴ (vs lattice) | +7.8 | 2.21·10⁻⁴ | Biased; rejected |
| 2026-10-27 | `pcg2d(i, base)` | cell centre | 400 | −2.8·10⁻⁶ | −0.3 | 1.67·10⁻⁴ | Unbiased but samples correlated (SD 17% below binomial) |
| 2026-10-27 | `pcg2d(i, base)` + `lowbias32` | cell centre | 400 | +4.2·10⁻⁶ | +0.4 | 2.12·10⁻⁴ | **Chosen**: unbiased, binomial variance |

Grid bias: testing cell corners of a 32768² grid overestimates π by 1.21·10⁻⁴;
testing cell centres of a 16384² grid reduces it to 1.1·10⁻⁶.

## Neural network sweep: integer training against f32 (spec 0011)

2026-11-03, `cargo run --release -p kernels --example nn_sweep`, one core of the
development laptop. The same networks, the same initial weights and the same batches,
trained once with the integer arithmetic every device uses and once in `f32`; accuracy
on the 1 024 held-out `glyphs` images (chance: 10 %).

| hidden | layers | lr | batch | steps | integer | f32 | ms (integer) |
|---|---|---|---|---|---|---|---|
| 32 | 1 | 0.05 | 32 | 600 | 67.4 % | 67.6 % | 199 |
| 32 | 1 | 0.2 | 32 | 600 | 71.9 % | 71.6 % | 199 |
| 64 | 1 | 0.1 | 32 | 600 | 73.4 % | 73.8 % | 429 |
| 128 | 1 | 0.1 | 32 | 600 | 75.1 % | 75.0 % | 1022 |
| 128 | 1 | 0.3 | 32 | 600 | 80.9 % | 80.7 % | 1038 |
| 64 | 2 | 0.1 | 32 | 600 | 76.4 % | 75.9 % | 638 |
| 128 | 2 | 0.1 | 32 | 1000 | 82.7 % | 83.0 % | 3201 |
| 128 | 1 | 0.02 | 32 | 600 | 66.2 % | 66.1 % | 1022 |
| 16 | 1 | 0.1 | 16 | 600 | 56.2 % | 56.2 % | 54 |
| 256 | 1 | 0.1 | 64 | 600 | 75.3 % | 75.6 % | 4010 |

Integers cost nothing measurable: every pair is within half a point. The settings matter
(56 % to 83 %), which is what makes a sweep worth running. A first version of `glyphs`
was too easy (99–100 % for every setting); stronger distortion, noise and a stray stroke
on half the images give the spread above.

Bit-exactness, checked the same day on the shared vectors (`templates/nn/vectors.json`):
the Rust reference, its WebAssembly build in Chromium and Firefox, and the WGSL kernel on
Chromium's WebGPU (SwiftShader) give the same accuracy, loss curve and SHA-256 of the
final weights for every vector. A local sweep of 48 trainings on 12 simulated devices
finished in seconds; the best setting reached 82.8 % (1 × 128, learning rate 0.4).

The same sweep natively on one core of the M1 Max (2026-11-21, `aarch64-apple-darwin`):
every integer accuracy is identical to the table above, while the `f32` column differs on
two rows by 0.1 point (128 × 1 at lr 0.1: 75.1 %; 256 × 1: 75.7 %). Floating point
differs between x86-64 and ARM; the integer training does not. Times: 52 ms to 3 455 ms
(the table's 54–4 010 ms). On the M1 Max's GPU the shared vectors are bit-exact in Chrome,
Edge, WebKit and Firefox ([above](#browser-engines-automated-tests)).

## School timetables (spec 0013)

Example high schools from `timetable::sample` (grades 8–12, 31 hours a week per class,
5 days × 7 periods, subject teachers with full loads, labs and gyms in proportion, one
teacher in eight away one day a week). Dev PC (Intel i5-14600KF), one thread, release
build; `cargo run --release -p timetable --example searches -- CLASSES ITERATIONS SEARCHES [L]`.

Speed (2 000 000 iterations, one search): 0.38 s for 20 classes (620 lesson hours), 0.40 s
for 40 (1 240), 0.41 s for 60 (1 860): an iteration costs the same whatever the size, as
costs are updated incrementally. The WebAssembly build that browsers run (Node 24, same
machine) takes 0.45–0.48 s for the 40-class school and gives exactly the native costs
(60, 35, 50 for searches 0–2).

MacBook Pro M1 Max, one thread, natively on ARM (2026-11-21): 0.46 s for 20 classes,
0.49 s for 40, 0.48 s for 60, with exactly the same costs as the dev PC (10, 60, 150 for
one search; 60, 35, 50 for searches 0–2 of the 40-class school).

History length `L` of late acceptance, 40 classes, six searches each (cost; every
timetable valid, so costs are soft only):

| L | 2 000 000 iterations | 10 000 000 iterations |
|---|---|---|
| 1 | 70 70 60 55 45 65 | 0 30 10 20 10 5 |
| 10 | 60 60 45 65 60 35 | — |
| 20 | 75 35 80 35 55 70 | 5 5 5 0 5 20 |
| 50 | 85 50 50 75 55 60 | 0 0 5 5 5 0 |
| 100 | 55 60 65 80 75 55 | 5 5 5 10 5 5 |
| 200 | 70 105 105 85 100 80 | 5 10 10 0 10 0 |
| 500 | — | 35 25 30 40 40 30 |

At the budgets a unit has, lengths from 10 to 100 do best and longer ones have no time
to settle (the first portfolio, 500–5 000, gave costs of 400–1 000 at 10 M iterations).
Units therefore cycle through L = 10, 20, 50, 100.

Best cost against the number of searches (2 000 000 iterations each, the portfolio):

| Classes | 1 | 2 | 4 | 8 | 16 | 32 | Spread of the 32 |
|---|---|---|---|---|---|---|---|
| 20 | 10 | 5 | 0 | 0 | 0 | 0 | 0–25 |
| 40 | 60 | 35 | 35 | 35 | 35 | 35 | 35–80 |
| 60 | 150 | 150 | 150 | 125 | 125 | 125 | 125–240 |

Every search found a valid timetable (no clashes, no unavailable slots). More searches
give a better best timetable, with diminishing returns; the spread is what a single
computer running one search would risk. These example schools are easier than a real
one (no class or room preferences beyond the gym and labs): the numbers for a real
school's data are still to be measured.

## Language models on the swarm (spec 0017)

2026-11-21, MacBook Pro M1 Max (macOS 26.3.1, mains power), SmolLM2-360M-Instruct
(`q4f16`, 273 MB of files) through transformers.js 4.3.2 on WebGPU, one browser alone on
the local grid, 50 English prompts, system prompt „Answer in one or two sentences.“, at
most 64 new tokens, greedy decoding.

| Browser | First unit (model loaded from the server) | First unit, model from the cache | Other units | Tokens/s | 50 prompts |
|---|---|---|---|---|---|
| Chrome 158 | 2 029 ms | 2 008 ms | 426 ms | 67.4 | 23.2 s |
| Edge 157 | 2 516 ms | 2 034 ms | 429 ms | 67.0 | 23.9 s |
| WebKit 27.2 (Safari's engine) | 1 913 ms | 1 842 ms | 449 ms | 65.0 | 24.1 s |

Tokens per second count every generated token over the units' compute time, the prompt
included; answers averaged 28 tokens. Loading the model costs about 1.6 s on top of a
unit, whether its files come over the network (here from the same machine) or from the
browser's checked copy: hashing 273 MB and preparing the GPU dominate. Over the home
line (4.8 MB/s up, [above](#public-address-relay-sweep-spec-0006)) the download alone is
about a minute per device, once.

**Do different GPUs choose the same tokens?** The same 50 prompts in the three browsers:

- Chrome and Edge (the same shader compiler, Tint): identical answers for 50 of 50, margins
  included.
- Chrome or Edge against WebKit (another compiler on the same GPU): identical for 34 of 50.
  Every other answer parted at a token both browsers chose by at most 0.31 (logit of the
  chosen token minus the runner-up's), at token 5 to 29, never at the first.
- Margins of Chrome's 1 416 tokens: median 1.97; 74 below 0.1, 253 below 0.5. 16 of 150
  first tokens (all three browsers) were chosen by less than 0.5.

With exact agreement, an honest iPhone and an honest Android phone would have disagreed on
a third of the prompts, and each disagreement counts against a device's trust. Phase 1
therefore takes the near-tie rule from phase 3: two answers agree when they are the same
up to a token both devices chose by less than 0.5, and not at the first token (matching
the first token takes running the model). Under it all 150 pairs above agree.

**„Спри“** during a 512-token answer: the page stopped 24 ms after the click; the
coordinator released the unit as the device left and it went back to the queue with no
result.

**Language.** SmolLM2-360M answers English well and does not answer Bulgarian: for
„Какво е фотосинтеза?“ and „Назови три реки в България.“ it repeated the question. A
model for Bulgarian prompts is still to be chosen and measured.

**Caching in WebKit.** In Chrome and Firefox a new page reads the model from Cache
Storage with no request. In WebKit as Playwright opens it (a private session) a new
page's worker found the cache empty and downloaded the files again; a Safari profile on
a real device is still to be checked.

**On a phone (2026-11-21).** Nothing Phone (1) (Adreno 642L, Android 15, Chrome 157, on
a charger), on the Mac's dev server through `adb reverse`; the same 50 prompts. The page
reports WebGPU with `shader-f16` and a 1 GiB largest buffer, so it qualifies for
SmolLM2. Results:

- **Wrong answers.** Every answer parted from the Mac's at the first token, 14 of 14, and
  the text is not language („together_mond_des+oles-268same-splitn_18…“). The near-tie
  rule agreed with none of them, so against a Mac these answers are refused. Two phones
  with the same GPU could agree with each other on such answers: a model must not run on
  a device where it computes wrongly (to fix before phones take language-model units).
- **Speed.** 15.0 s per answer after a first unit of 33.8 s (download over USB, hash,
  set-up): 4.25 tokens/s, 16× slower than the M1 Max.
- **GPU lost.** After 14 answers (3.5 minutes) the page reported „GPU device lost: A
  valid external Instance reference no longer exists“ and stopped; its two units in hand
  went back to the queue. Battery 40 °C, CPU 57 °C, thermal status normal.

The `q4f16` build keeps activations in 16-bit floats; whether the `q4` build (32-bit
activations, 388 MB) computes correctly on the Adreno is the next check.

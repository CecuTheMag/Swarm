# Measurements

Every number used in the competition documents comes from this log.

## WebGPU support by device

| Date | Device | GPU | OS | Browser | WebGPU | Notes |
|---|---|---|---|---|---|---|
| 2026-10-27 | Dev PC | NVIDIA RTX 4060 | Arch Linux | Chromium 154 (headless) | Only with `--enable-unsafe-webgpu --enable-features=Vulkan`; SwiftShader adapter | Headless does not use the real GPU |
| 2026-10-28 | Nothing Phone (1) | Adreno 642L (Snapdragon 778G+) | Android 15 | Chrome 156 | Yes | Adapter `qualcomm` / `adreno-6xx`; self-test bit-exact on all vectors |
| 2026-10-28 | Lenovo ThinkPad T540p (2014) | Intel HD Graphics 4600, driver 20.19.15.5126 (2020) | Windows 10 22H2 | Edge 156 | No | `edge://gpu` reports WebGPU enabled, but `requestAdapter()` returns null for default, high-performance, low-power, compatibility and fallback adapters. Joins through the CPU fallback (spec 0005) |

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

Not yet tested on real devices: Safari on an iPhone (iOS 26 ships WebGPU, so it would
take the GPU path) and Firefox with WebGPU (Windows, macOS).

## School lab idle time

| Date | Lab | PCs | Observed idle hours | Method |
|---|---|---|---|---|

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

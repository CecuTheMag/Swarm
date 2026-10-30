# Measurements

Every number used in the competition documents comes from this log.

## WebGPU support by device

| Date | Device | GPU | OS | Browser | WebGPU | Notes |
|---|---|---|---|---|---|---|
| 2026-10-27 | Dev PC | NVIDIA RTX 4060 | Arch Linux | Chromium 154 (headless) | Only with `--enable-unsafe-webgpu --enable-features=Vulkan`; SwiftShader adapter | Headless does not use the real GPU |
| 2026-10-28 | Nothing Phone (1) | Adreno 642L (Snapdragon 778G+) | Android 15 | Chrome 156 | Yes | Adapter `qualcomm` / `adreno-6xx`; self-test bit-exact on all vectors |
| 2026-10-28 | Lenovo ThinkPad T540p (2014) | Intel HD Graphics 4600, driver 20.19.15.5126 (2020) | Windows 10 22H2 | Edge 156 | No | `edge://gpu` reports WebGPU enabled, but `requestAdapter()` returns null for default, high-performance, low-power, compatibility and fallback adapters. Joins through the CPU fallback (spec 0005) |

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
one job serves several hundred devices before this lock binds.

## Public relay (Tailscale Funnel)

| Date | Path | Measured | Result |
|---|---|---|---|
| 2026-10-30 | Home line (Cloudflare speed test) | 20 MB up ×2, 50 MB down | 4.5 MB/s up, 21.9 MB/s down |
| 2026-10-30 | Through Funnel (3 ingress addresses × 3) | 2.0 MB PNG download | median 534 KB/s (216–1384), first byte 0.30–0.90 s |
| 2026-10-30 | Direct tailnet path, same file | 2.0 MB PNG download | 18.2 MB/s, first byte 0.03 s |
| 2026-10-29 | Phone through Funnel (commit b12fa25) | path-tracer results upload | ~300 KB/s, 1.62 units/s |

A 64-pixel tile result is 65.5 KB (64·64·3 floats in base64). Funnel carries roughly
5–8 such results per second for the whole grid; 30 phones at one tile per second each
need ~2 MB/s. The relay, not the coordinator or the home line, limits the demo.

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

# Measurements

Every number used in the competition documents comes from this log.

## WebGPU support by device

| Date | Device | GPU | OS | Browser | WebGPU | Notes |
|---|---|---|---|---|---|---|
| 2026-10-27 | Dev PC | NVIDIA RTX 4060 | Arch Linux | Chromium 154 (headless) | Only with `--enable-unsafe-webgpu --enable-features=Vulkan`; SwiftShader adapter | Headless does not use the real GPU |
| 2026-10-28 | Nothing Phone (1) | Adreno 642L (Snapdragon 778G+) | Android 15 | Chrome 156 | Yes | Adapter `qualcomm` / `adreno-6xx`; self-test bit-exact on all vectors |
| 2026-10-28 | Lenovo ThinkPad T540p (2014) | Intel HD Graphics 4600, driver 20.19.15.5126 (2020) | Windows 10 22H2 | Edge 156 | No | `edge://gpu` reports WebGPU enabled, but `requestAdapter()` returns null for default, high-performance, low-power, compatibility and fallback adapters. Typical of older lab PCs: needs a CPU fallback |

## School lab idle time

| Date | Lab | PCs | Observed idle hours | Method |
|---|---|---|---|---|

## Benchmarks

| Date | Device | Template | Units/s | GFLOPS | Notes |
|---|---|---|---|---|---|
| 2026-10-28 | Nothing Phone (1), Chrome 156 | π, 2²⁸ samples/unit | 2.98 | — | 800 M samples/s sustained: 388 units, 335 ms average, no slowdown over ~5 min of work; on charger, GPU 36 °C. Self-test: 901 M samples/s |
| 2026-10-28 | Nothing Phone (1), Chrome 156 | π, 2²⁴ samples/unit | 40 | — | 25 ms per unit: units this small are dominated by per-unit overhead (680 M samples/s) |
| 2026-10-28 | Nothing Phone (1), Chrome 156 | π, 2²⁸ samples/unit, 10.6 min | 2.83 | — | Sustained-load run: 1800 units (4.83·10¹¹ samples) submitted from the T540p's dashboard, computed by the phone alone. Unit time 332 ms average, 351 ms p95, flat from first to last minute: no thermal throttling. GPU 36–37 °C, skin 37–38 °C, battery 32 °C, on charger. 761 M samples/s wall-clock including coordinator round trips |

Grid correctness (2026-10-28): a 128-unit π job (2²⁷ samples/unit, seed 2026) computed by
the phone (52 units) and four CPU sim-workers (76 units) gave exactly the same count as a
CPU-only run: 13 492 969 218 inside of 17 179 869 184. A 64-unit job (seed 7001) whose
phone tab was reloaded mid-unit also matched its clean rerun exactly.

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

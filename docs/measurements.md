# Measurements

Every number used in the competition documents comes from this log.

## WebGPU support by device

| Date | Device | GPU | OS | Browser | WebGPU | Notes |
|---|---|---|---|---|---|---|
| 2026-10-27 | Dev PC | NVIDIA RTX 4060 | Arch Linux | Chromium 154 (headless) | Only with `--enable-unsafe-webgpu --enable-features=Vulkan`; SwiftShader adapter | Headless does not use the real GPU |

## School lab idle time

| Date | Lab | PCs | Observed idle hours | Method |
|---|---|---|---|---|

## Benchmarks

| Date | Device | Template | Units/s | GFLOPS | Notes |
|---|---|---|---|---|---|

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

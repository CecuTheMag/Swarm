# 0003 — Path tracer

Status: implemented

## Goal

The showpiece template. The grid renders a 3D scene; a projector page shows the image
filling in tile by tile and then sharpening as every device adds samples. It must stay
correct on every backend (WebGPU, CPU WebAssembly, native) and give verification
(0004) something to check.

## Scene

A Cornell box, defined identically in Rust (`kernels::pt`) and WGSL
(`templates/pt/pt.wgsl`):

- Box `x ∈ [−1, 1]`, `y ∈ [0, 2]`, `z ∈ [−1, 3.5]`, open towards the camera (`z = 3.5`),
  so wide frames see the side walls rather than the outside.
  Left wall red, right wall green, floor, ceiling and back wall white (diffuse).
- Light: the ceiling square `|x|, |z| < 0.3`, emission `(10, 8.5, 6)`.
- Mirror sphere at `(−0.42, 0.4, −0.3)`, radius 0.4. Glass sphere (IOR 1.5, Schlick
  Fresnel) at `(0.45, 0.33, 0.35)`, radius 0.33.
- Pinhole camera at `(0, 0.9, 3.6)` looking down −z, vertical `tan(fov/2) = 0.33`.

Unidirectional path tracing, at most 6 bounces, black background. Diffuse bounces use
`normal + random unit vector` (rejection-sampled), so the kernel needs only
`+ − × ÷ √`.

## Determinism

- Random numbers depend only on `(seed, pixel, sample)`: a pixel's sample is the same
  whichever tile, pass or device computes it.
- Native Rust and WebAssembly use IEEE `f32` with correctly rounded `√` and no
  contraction, so **CPU results are bit-identical**. WGSL may fuse or approximate, so GPU
  results are compared with a tolerance.

## Work units

Job parameters: `width`, `height` (16–2048), `tile` (8–128), `spp` (samples per pixel per
pass, 1–1024), `passes` (1–256), `seed`. A unit is one tile × one pass; units are ordered
pass-major, so the whole image appears first and then refines. At most 20 000 units.

A unit's output is `{ "rgb": base64 }`: little-endian `f32` sums of radiance over the
unit's samples, `w·h·3` values in row-major RGB order. The coordinator rejects outputs of
the wrong length, non-finite or negative values, or values above `spp × 16` (no path can
carry more than the light's emission).

## Live image

The coordinator keeps an accumulator per job (sum and sample count per pixel), fed by
accepted units and rebuilt from the database when needed.
`GET /api/jobs/{id}/image.png` returns the current image (RGBA; pixels without samples
are transparent). The job result holds the size and samples per pixel. `/render/{id}` shows
the image live, refreshed on dashboard events, with devices and throughput.

## Tests

- Rust: determinism; splitting a tile into rows or passes gives the same pixels; a pixel
  looking straight at the light returns exactly `spp × emission`; the image mean is
  stable across seeds.
- Shared vectors (`templates/pt/vectors.json`): small tiles with reference sums from the
  Rust reference. The CPU (WebAssembly) must match exactly, WebGPU within tolerance.
- Coordinator: split, validation and aggregation; the PNG endpoint.
- Browser: self-test and a worker rendering a small job, on the WebGPU and CPU projects.

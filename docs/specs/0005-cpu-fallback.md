# 0005 — CPU fallback

Status: implemented

## Goal

Devices without usable WebGPU still contribute. The first school-style laptop tested
(ThinkPad T540p, Intel HD Graphics 4600, Edge 156) gets no WebGPU adapter at all, and
older lab PCs are likely to be the same. Without a fallback the grid cannot use the
machines the project is about.

## Design

- **Same code as the reference.** `crates/kernels-wasm` exports the Rust reference
  kernels (`kernels::pi`) to WebAssembly. The CPU path therefore runs exactly the code
  every result is checked against; nothing is ported by hand.
- **One crate, one export per kernel.** `pi_count_inside_range(seed, unit, start, end)`
  counts a sample range. The module has no imports; `u32` values cross as `i32` with
  the same bits (JavaScript reads results with `>>> 0`). The crate allows `unsafe_code`
  only for `#[unsafe(no_mangle)]`, which the rest of the workspace forbids.
- **Threads.** A unit is split into chunks of 2²² samples. All but one hardware thread
  each run a Web Worker with its own instance and take the next chunk until none are
  left. Counts of disjoint ranges add up exactly, so the split never changes the result.
- **Selection.** The compute worker uses WebGPU when `requestAdapter()` succeeds and the
  CPU otherwise. `?backend=wasm` on `/worker` and `/selftest` forces the CPU, for
  comparisons and tests. Workers report `backend: "wasm"`, `vendor: "cpu"`,
  `architecture: "<n> threads"`.
- **Template hash.** The pinned hash stays that of the WGSL source: it identifies the
  template version. The CPU path runs the same version, built from the reference that the
  shared vectors check against the WGSL kernel.
- **Build.** A Vite plugin runs `cargo build -p kernels-wasm --target wasm32-unknown-unknown
  --release` when the dev server, a build or the unit tests start, so the site never ships
  a stale module. CI's web and e2e jobs install the wasm target.

## Tests

- Rust: range counts add up to the unit count.
- Unit (Vitest): the built `.wasm` matches every shared vector, split ranges add up, and
  `u32` values above 2³¹ pass unchanged.
- Browser (Playwright): every test runs twice, with WebGPU (`webgpu` project) and without
  it (`cpu` project, where workers fall back). The self-test also runs with
  `?backend=wasm`. The worker test computes a unit of five chunks, which the GPU splits
  into dispatches and the CPU across threads.
- `templates/pi/vectors.json` gained that unit (seed 2026, unit 0, 20 983 865 samples →
  16 481 195), computed by an independent NumPy implementation and cross-checked with the
  Rust reference.

## Results

See [measurements](../measurements.md): the T540p computes 544 M samples/s on 3 threads,
about two thirds of a Snapdragon 778G+ phone GPU, and its results match GPU and native
runs exactly.

# 0011 — Neural network sweep

Status: implemented (phases 1–3); phase 4 planned

## Goal

A teacher, a student or a researcher asks a question every machine-learning course
asks: *which settings train this network best?* Рояк answers it with the room's
devices. Each device trains one configuration of a small neural network (layer size,
learning rate, batch size, training length) and reports how well it did; the console
shows a leaderboard that fills in live, the learning curves, and the best model, which
can be downloaded.

It is the useful job the README promises (section 17: "a toy that computes π" kills
the project), and it keeps Рояк's defining property: **every result is checked**.

## The hard part, and the answer: training that is bit-exact on every device

Verification (spec 0004) compares answers from different devices. Floating-point
training does not give the same answer twice: GPUs may fuse multiply-adds, flush
subnormals, approximate `exp`, and sum in any order, and over hundreds of training
steps the differences grow. The path tracer copes with a tolerance; for training a
tolerance would let a cheater nudge a configuration up the leaderboard.

So training here uses **integer fixed-point arithmetic only**, as the π template does:

- every value is an `i32` with a fixed scale (Q8 for activations, Q24 for the master
  weights, Q15 for probabilities); products are shifted back with rounding;
- integer addition is exact and associative, so a GPU may sum in any order, across any
  number of threads, and get the same bits;
- the only non-linear function, `exp` in the softmax, is a 256-entry table of
  `2^(f/256)` and shifts, identical in Rust and WGSL;
- random numbers (initial weights, batch order) are the counter-based hashes of
  `kernels::rng`, a pure function of (seed, step, index);
- values are clamped so no step overflows `i32` (WGSL has no 64-bit integers);
  the Rust reference checks this in tests.

The result: a configuration trained on an Adreno phone, an NVIDIA card, Apple silicon
or a lab PC's processor ends with **the same weights, bit for bit**. Two honest devices
always agree exactly; a cheater has to guess a 256-bit digest of the final weights.
It is also the strongest demonstration of the system: a judge's phone can retrain any
row of the leaderboard and get the identical digest.

Rounding to integers costs nothing measurable: the same networks trained in `f32` end
within half a point of the integer ones (docs/measurements.md). Master weights in Q24
keep even small updates from vanishing, so no stochastic rounding is needed.

## Model

A multilayer perceptron: input → 1 or 2 hidden layers (16–256 units, ReLU) → 10
classes (softmax, cross-entropy), trained with mini-batch SGD. Every unit is capped at
2.5 · 10⁹ multiply-adds (`kernels::nn::MAX_WORK`, about 5 s on one laptop core), so no
device gets a part that runs for minutes. Small enough to train in seconds on a phone, big enough that the settings
matter. A convolutional layer is phase 4.

## Data

The dataset is part of the template: its hash is pinned with the kernel, so every
device trains on exactly the same bytes, and nothing personal ever enters it.

| Dataset | Content | Size | Phase |
|---|---|---|---|
| `glyphs` | Procedural handwritten-style digits 0–9 (strokes with random shift, scale, slant, thickness, noise and a stray stroke on half of them), 14×14, 4 096 train + 1 024 validation, generated from a fixed seed by the reference kernels | 0 bytes: every device generates it | 1 |
| `mnist` | MNIST digits (CC BY-SA 3.0), downsampled to 14×14, 10 000 train + 2 000 validation | ~2.4 MB, cached | 4 |
| `fashion` | Fashion-MNIST (MIT), same format | ~2.4 MB, cached | 4 |

Pixels are `u8`; a fixed validation split is never trained on.

## A job and its units

A job is a **sweep**: lists of values for each setting, and the coordinator trains every
combination (a grid), each with one or more seeds.

```json
{ "dataset": "glyphs", "steps": 600,
  "hidden": [32, 64, 128], "layers": [1, 2],
  "learning_rate": [0.05, 0.1, 0.2], "batch": [16, 32], "seeds": 2 }
```

One unit = one configuration × one seed, trained from scratch for `steps` steps (at
most 2 000 units per job). A unit returns:

| Field | Meaning |
|---|---|
| `correct` | validation images classified correctly (integer) |
| `curve` | 16 points of mean training loss (Q16 integers), for the learning curve |
| `digest` | SHA-256 of the final weights |

Agreement is equality of all three (spec 0004: two devices, honeypots from finished
jobs). Validation rejects counts above the validation size, wrong curve lengths and
malformed digests.

The job result is the leaderboard: every configuration with its mean accuracy over
seeds, its spread, and its curve, sorted best first. While it runs,
`GET /api/networks/{id}/jobs/{job}/leaderboard` ranks the units decided so far. The
best model's weights are not uploaded by devices: the coordinator retrains the first run
of that configuration with the reference kernels (deterministic, so identical) when
someone downloads it (`GET /api/networks/{id}/jobs/{job}/model.json`: the settings,
its accuracy, the digest the devices agreed on, and every layer's weights as real
numbers).

## Browser

- WGSL kernels (`templates/nn/nn.wgsl`): pick the batch, forward pass, softmax and
  loss, backward pass, update, end of step, evaluation; one entry point each, parameters
  per dispatch through a dynamic uniform offset. Twenty steps go in one submission with
  no readback until the end, so phones stay efficient and stopping stays responsive. A
  rejected command buffer fails the unit instead of returning untrained weights.
- The dataset and the initial weights come from the WebAssembly build of the reference
  on every device (the GPU only trains), so every device starts from the same bytes.
- CPU fallback: the Rust reference in WebAssembly, on one thread of the pool (a unit is
  a few seconds at most).
- Simulated devices (`sim-worker`) train units with the reference kernels and can cheat
  (a made-up score and digest), for load tests and the resilience tools.
- `templates/nn/vectors.json`: digests for fixed configurations; the self-test page and
  the browser tests check the WGSL kernel against them bit for bit.

## Console

**Задачи** → „Невронна мрежа“: a preset in plain words — „Бързо сравнение“ (6
configurations), „Нормално“ (24 × 2 runs), „Задълбочено“ (100 × 2) — or the lists under
„Подробни настройки“; the form shows how many configurations and parts that makes. The
job's page („Класация“) updates every two seconds while it runs: the best so far in
words („1 скрит слой със 128 неврона · скорост на учене 0.4 · партида 32“) with its
accuracy, a heat map of accuracy over learning rate × layer size (one hue, light to
dark; the best of the other settings in each cell), the learning curves of the top five
(each configuration keeps its colour while it stays in the top five; hover shows every
value), and the full leaderboard. When done: „Изтегли модела“.

## Phases

1. ~~**Reference and template**~~ (done): integer trainer and `glyphs`
   data in `crates/kernels`, protocol types, the coordinator template (split, validate,
   agree, aggregate), unit tests that it learns (well above chance) and is
   deterministic.
2. ~~**Browser kernels**~~ (done): WGSL port, WebAssembly fallback, test vectors, self-test,
   browser tests in all four engines.
3. ~~**Console**~~ (done): job form, live leaderboard, heat map, curves, model download.
4. **More**: MNIST and Fashion-MNIST packed datasets, a convolutional layer,
   successive halving (train many configurations briefly, continue the best).

## Tests

- Reference: training is deterministic (same digest twice, across thread counts);
  accuracy on `glyphs` well above chance for a sensible configuration and near chance
  for a learning rate of zero; no `i32` overflow over the parameter ranges (checked
  arithmetic in tests); the softmax table sums correctly.
- Template: split builds the grid in order; bad parameters are refused; validation and
  agreement as above; the leaderboard ranks and averages correctly.
- Coordinator: the shared vectors equal the reference (`UPDATE_VECTORS=1` rewrites
  them); end to end, simulated devices train a sweep with a cheater among them, the
  cheater is banned and every score equals the reference; the live leaderboard equals
  the result and the downloaded model has the accuracy the devices reported.
- Browser: the self-test checks the vectors bit for bit on the GPU and CPU paths;
  `e2e/sweep.e2e.ts` signs in, creates a network, joins a browser, runs a sweep through
  the form, waits for the leaderboard and checks the downloaded model.

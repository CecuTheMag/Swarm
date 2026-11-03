# 0011 — Neural network sweep

Status: in progress (phase 1)

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

Rounding to integers costs a little accuracy compared with `f32`; stochastic rounding
of weight updates (with the deterministic hash) keeps small updates from vanishing.
Phase 1 measures how close it comes to a floating-point run of the same network.

## Model

A multilayer perceptron: input → 1 or 2 hidden layers (16–256 units, ReLU) → 10
classes (softmax, cross-entropy), trained with mini-batch SGD (with momentum as an
option). Small enough to train in seconds on a phone, big enough that the settings
matter. A convolutional layer is phase 4.

## Data

The dataset is part of the template: its hash is pinned with the kernel, so every
device trains on exactly the same bytes, and nothing personal ever enters it.

| Dataset | Content | Size | Phase |
|---|---|---|---|
| `glyphs` | Procedural handwritten-style digits 0–9 (strokes with random shift, scale, slant, thickness and noise), 14×14, generated from a seed by the reference kernels | 0 bytes: every device generates it | 1 |
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
seeds, its spread, and its curve, sorted best first. The best model's weights are not
uploaded by devices: the coordinator retrains that one configuration with the reference
kernels (deterministic, so identical) when someone downloads it
(`GET /api/networks/{id}/jobs/{job}/model.json`).

## Browser

- WGSL kernels (`templates/nn/nn.wgsl`): forward pass, softmax and loss, backward pass,
  update, one entry point each; a whole training run is encoded in a few command
  buffers with no readback until the end, so phones stay efficient.
- CPU fallback: the Rust reference compiled to WebAssembly, as for π and the path tracer.
- `templates/nn/vectors.json`: digests for fixed configurations; the self-test page and
  the browser tests check the WGSL kernel against them bit for bit.

## Console

**Задачи** → „Невронна мрежа“: pick the data, then a preset in plain words — „Бързо
сравнение“ (12 configurations), „Задълбочено търсене“ (72) — or choose the lists under
„Подробни настройки“. While it runs: the leaderboard filling in, a heat map of accuracy
over learning rate × layer size, the learning curves of the top five, and "the best so
far" with its settings. When done: „Изтегли модела“.

## Phases

1. **Reference and template** (this spec's first commit): integer trainer and `glyphs`
   data in `crates/kernels`, protocol types, the coordinator template (split, validate,
   agree, aggregate), unit tests that it learns (well above chance) and is
   deterministic.
2. **Browser kernels**: WGSL port, WebAssembly fallback, test vectors, self-test,
   browser tests in all four engines.
3. **Console**: job form, live leaderboard, heat map, curves, model download.
4. **More**: MNIST and Fashion-MNIST packed datasets, a convolutional layer,
   successive halving (train many configurations briefly, continue the best).

## Tests

- Reference: training is deterministic (same digest twice, across thread counts);
  accuracy on `glyphs` well above chance for a sensible configuration and near chance
  for a learning rate of zero; no `i32` overflow over the parameter ranges (checked
  arithmetic in tests); the softmax table sums correctly.
- Template: split builds the grid in order; bad parameters are refused; validation and
  agreement as above; the leaderboard ranks and averages correctly.
- Browser (phase 2): the WGSL digest equals the Rust digest for every vector, on GPU and
  CPU paths.

# Lab: neural network sweeps

2026-11-04, school lab, the lab's own network ([lab-method.md](lab-method.md)). The
sweeps of spec 0011: small networks learning the `glyphs` digits, every setting
trained on a device and checked on another.

## The sweeps

All on the 13 PCs' processors (WebAssembly), 600 training steps each.

| Sweep | Settings × runs | Trainings | Time | Best | Worst | Devices |
|---|---|---|---|---|---|---|
| „Нормално“: 32–128 neurons, 1–2 layers, 4 learning rates, batch 32 | 24 × 2 | 48 | 17 s | 82.1 % (1 × 128, rate 0.4) | 61.2 % | 12 |
| „Задълбочено“: 16–256 neurons, 1–2 layers, 5 rates, batch 16 and 32 | 100 × 2 | 200 | 66 s | 81.9 % (2 × 128, rate 0.2) | 27.3 % | 12 |
| Largest: 8 sizes 16–256, 1–2 layers, 7 rates 0.02–0.4, batch 16 and 32 | 224 × 3 | 672 | 154 s | 84.0 % (1 × 256, rate 0.4, batch 32) | 34.3 % | 13 |
| The largest again, every CPU thread (below) | 224 × 3 | 672 | 91 s | 84.0 %, the same | | 13 |

One PC sat out the first two (its window counted as hidden; see
[2026-11-04-lab-own-network.md](2026-11-04-lab-own-network.md)).

Verification, all sweeps together: every training was either compared with another
device's (agreed: 340) or done by a device with a good record (trusted: 1 422), and
13 known-answer checks passed. No disagreement and no lost unit. Agreement here is
exact: the same score, the same 16-point learning curve and the same SHA-256 of the
final weights.

## One network per CPU thread

The first runs showed each PC training one network at a time on one thread, leaving
three of its four threads idle. The worker now trains one network per thread of its
pool (all threads but one). The same largest sweep, rerun with the new version and the
same seed:

| | One thread | One per thread (3) |
|---|---|---|
| Time | 153.8 s | 90.7 s (1.7× faster) |
| Time per training | 2.3 s | 3.6 s (three at once on 2 cores) |
| Leaderboard | | **identical** in all 224 rows |

The i3s have 2 cores with 2 threads each, so 1.9× was the most to expect; the last
seconds of a sweep, with few trainings left, use fewer threads.

The identical leaderboard is the point of integer training (spec 0011): 1 344
trainings, on different PCs, in a different order, two thread counts, gave the same
bits.

## Real handwriting (MNIST)

Added the same day (spec 0011, phase 4): 10 000 handwritten digits to learn from and
2 000 by other writers to score on, 14 × 14, a 2.4 MB file every device downloads once.
The console's „Задълбочено“ preset for it, on the 13 PCs, every CPU thread:

| | |
|---|---|
| Settings × runs | 120 × 2 (6 layer sizes 32–256, 1–2 layers, 5 learning rates 0.05–0.45, batch 8 and 16), 2 500 steps each |
| Time | 112 s |
| Best | **93.7 %** of the 2 000 unseen digits (1 layer of 256 neurons, rate 0.45, batch 16); range of its two runs 93.5–93.8 % |
| Worst | 86.5 % (2 × 256, rate 0.05) |
| Verdicts | 52 agreed, 214 trusted, 2 known-answer checks passed; no disagreement, no lost unit |
| Time per training | 11 s on average; the slowest 81 s |

The slowest trainings were the first on each PC: each of a PC's three threads
downloaded the 2.4 MB file at the same moment, 39 downloads through the home server's
upload. A device now downloads it once for all its threads (not yet measured in the
lab; later jobs read it from the browser's cache in any case).

For comparison, the same kind of network trained in floating point on all 60 000
MNIST images at 28 × 28 reaches about 97–98 %; 93.7 % from a sixth of the images at a
quarter of the pixels, in integers, is in line.

## Found

- The sweep's page could show a finished sweep one row short: it read the live
  leaderboard and the job at the same moment, and once the job said „done“ it stopped
  reading. A finished sweep now shows the result stored with the job. Found by the
  browser tests after the faster training made units finish together.
- „Изпробвайте мрежата“ (draw a digit, the best network reads it) was added the same
  day; the browser test draws a „1“ and gets an answer.

## Is it useful beyond a demonstration?

Not yet for real machine learning, and these numbers should not be presented as if:

- `glyphs` is a generated dataset; 84 % on it says little about real tasks. On real
  handwriting (MNIST, above) the room's best network reads 93.7 % of digits it never
  saw; full-size MNIST networks reach 97–98 %.
- The networks are small (up to ~120 000 weights) and train for seconds.
- The 13 PCs of 2012 together, with every thread, do about as much as one or two modern
  laptops.

What is real: training that gives the same bits on every device, so strangers'
results can be checked exactly; and the sweep itself, which is how real
hyperparameter searches work. Real datasets (MNIST, a school's own data), larger
models and spending compute on the promising settings (successive halving) are what
would make it useful (spec 0011, phase 4).

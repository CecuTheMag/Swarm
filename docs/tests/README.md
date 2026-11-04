# Tests

Every test Рояк has been through, one page each: what was tested, how, on what, and
what came out. Numbers here are the ones measured; `docs/measurements.md` keeps the
running log they also go into.

## Automated (run on every change)

| Page | What |
|---|---|
| [automated-suites.md](automated-suites.md) | Rust tests, browser unit tests (Vitest) and browser tests (Playwright) in four engines |

## School lab, 2026-11-04

13 Windows 10 PCs (Intel i3 of 2011–2012, 4 GB, Intel HD Graphics), the live server at
home, the school's own network.

| Page | Result |
|---|---|
| [lab-method.md](lab-method.md) | How the PCs were driven from one laptop: hidden browsers, visible windows |
| [2026-11-04-lab-selftest.md](2026-11-04-lab-selftest.md) | Self-test bit-exact on all 13 PCs, in Edge and in Chrome; speed per PC |
| [2026-11-04-lab-public-pi.md](2026-11-04-lab-public-pi.md) | 13 PCs on the public network: 107 billion samples of π in 43 s, no disagreement |
| [2026-11-04-lab-own-network.md](2026-11-04-lab-own-network.md) | A client's own network: account, join code, visible windows on every screen, π in 60 s |
| [2026-11-04-lab-render.md](2026-11-04-lab-render.md) | A 1920×1080 picture, 2.1 billion light paths, in about 70 s |
| [2026-11-04-lab-nn-sweeps.md](2026-11-04-lab-nn-sweeps.md) | Neural network sweeps up to 672 trainings, all cores; real handwriting (MNIST): 93.7 % from 240 trainings in 112 s |

## Before the lab

| Page | Result |
|---|---|
| [earlier-tests.md](earlier-tests.md) | Phones, the old laptop, load tests up to 1 000 devices, the 30-device rehearsal |

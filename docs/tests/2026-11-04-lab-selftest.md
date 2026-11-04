# Lab: self-test on every PC

2026-11-04, school lab, 13 PCs ([lab-method.md](lab-method.md)).

**What:** `/selftest` on the live site, which checks the π and path-tracer kernels
against the Rust reference's test vectors bit for bit and measures the device's speed.
Run in a hidden browser on every PC at once, once in Edge and once in Chrome.

**Result:** passed on all 13 PCs in Edge and on all 12 with Chrome. Every PC took the
CPU path (WebAssembly, 3 threads: all but one); none got a WebGPU adapter. A full run
took 2–8 s.

| PC | Processor | Edge | π, million samples/s | Light paths, million/s | Chrome 156: π | Chrome: paths |
|---|---|---|---|---|---|---|
| USER006-01 | i3 | 156.0.4314.58 | 309 | 1.93 | — | — |
| USER006-02 | i3-3220 | 156.0.4314.50 | 341 | 1.72 | 336 | 2.05 |
| USER006-03 | i3-3240 | 156.0.4314.58 | 314 | 2.05 | 353 | 1.77 |
| USER006-04 | i3-3220 | 156.0.4314.58 | 308 | 1.82 | no Chrome | |
| USER006-05 | i3-2120 | 154.0.4258.37 | 248 | 1.93 | 291 | 1.72 |
| USER006-06 | i3-2120 | 154.0.4258.37 | 224 | 1.68 | 286 | 1.99 |
| USER006-07 | i3-3220 | 156.0.4314.58 | 336 | 1.68 | 278 | 1.46 |
| USER006-08 | i3 | 156.0.4314.58 | 255 | 1.56 | 321 | 1.72 |
| USER006-10 | i3-3240 | 147.0.3912.98 | 329 | 1.68 | 267 | 1.82 |
| USER006-11 | i3-3240 | 155.0.4283.62 | 365 | 1.68 | 305 | 1.87 |
| USER006-12 | i3-3240 | 153.0.4234.48 | 369 | 1.68 | 359 | 2.18 |
| USER006-13 | i3-3240 | 147.0.3912.60 | 327 | 1.87 | 315 | 1.77 |
| USER006-14 | i3-3240 | 156.0.4314.50 | 353 | 1.77 | 291 | 1.82 |

On USER006-01 the staff account's Chrome path did not exist, so only Edge was run.
Each run was one measurement while 12 other PCs ran theirs; differences of ±15 % between
Edge and Chrome on one PC are run-to-run noise, not the browser.

**Seen in the console:** one 401 (the page asks who is signed in; nobody was) and one 404
per load. Harmless, but the 404 is worth finding.

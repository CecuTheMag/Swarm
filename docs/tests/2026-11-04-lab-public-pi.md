# Lab: 13 PCs compute π on the public network

2026-11-04, school lab ([lab-method.md](lab-method.md)).

**What:** every PC opened `/join/public` in a hidden Edge and pressed „Включи се“; 30 s
later the server submitted a π job to the public network: 400 units of 2²⁸ samples.

**Result:**

| | |
|---|---|
| Samples | 107 374 182 400 |
| Time from submission to done | 43 s |
| π | 3.14159923709929 |
| Devices | 13, all on the CPU (WebAssembly) |
| Verdicts | 206 agreed (two devices, same answer), 297 trusted (one device with a good record), 232 known-answer checks passed |
| Disagreements, failed checks, lost units | 0, 0, 0 |
| Time per unit | 0.7 s on average |

Each PC was accepted 49–65 results in the run. A unit of 2²⁸ samples took about 0.7 s,
so a PC did about 380 million samples a second, as its self-test said.

After the job, idle devices kept getting known-answer checks from finished jobs
(spec 0004): the counters on the worker pages kept rising while they said „waiting“.
That is by design, but it means an idle lab still uses some processor.

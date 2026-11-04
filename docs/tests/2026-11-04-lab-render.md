# Lab: a 1920×1080 picture

2026-11-04, school lab, the lab's own network ([lab-method.md](lab-method.md)).

**What:** a path-tracer job (spec 0003) on the 13 PCs: 1920×1080, tiles of 120×120,
16 passes of 64 samples per pixel (1 024 per pixel in all). The console's live view
showed the picture refine pass by pass on the laptop.

**Result:**

| | |
|---|---|
| Units | 2 304 (144 tiles × 16 passes) |
| Light paths | 2 123 366 400 |
| Time | about 70 s: about 30 million paths a second for the room |
| Devices | 13 |
| Verdicts | 450 agreed, 2 079 trusted, 4 known-answer checks passed |
| Disagreements, lost units | 0, 0 |

Two devices' tiles agree within the path tracer's tolerance (spec 0004), not bit for bit;
every pair that was compared agreed.

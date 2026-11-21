# 0017 — AI on the swarm

Status: planned (phase 1 starts 21 November 2026; target for the defense: phases 1–2)

## Goal

A school asks a question it cannot afford to ask a cloud: *transcribe every recorded
lesson of the term*, *summarise 2 000 essays*, *label 50 000 photos of the herbarium*.
Рояк answers it with the devices the school already has. Every capable device runs a
whole small model in its browser, on its graphics card, and the swarm spreads the
requests over them. Nothing leaves the school's devices and the server: no cloud API,
no account, no cost per request.

What this is **not**: one large model split across devices. Splitting a model's layers
over Wi-Fi sends every token's activations between phones; the link, not the GPUs,
would set the speed. Рояк runs many requests in parallel, which is what a volunteer grid
is good at.

## Runtime

- **transformers.js** (`@huggingface/transformers`, Apache-2.0, compatible with GPL-3)
  on ONNX Runtime Web: WebGPU on devices that have it, WebAssembly otherwise. One
  runtime covers language models and speech recognition (Whisper).
- It runs in a Web Worker of its own, like every kernel, so the page stays responsive
  and „Спри“ terminates it.
- The device reports what it can carry when it joins (backend, and the adapter's
  `maxBufferSize` and `maxStorageBufferBindingSize`); the coordinator gives AI units
  only to devices that can hold the job's model. Lab PCs without WebGPU do not get
  language-model units by default (too slow on a 2011 i3; measured in phase 1).

## Models are known code too

A model is data, but it decides what a device computes, so it is pinned like a kernel:

- The site ships a **model catalog**: for each model, its files with their size and
  SHA-256, its licence, and the minimum device it needs. A job names a catalog model;
  it cannot point at a URL.
- The browser downloads each file, checks its SHA-256 before use, and keeps it in
  Cache Storage: a device downloads a model once.
- Files are served from Swarm's own origin (`/models/…`), so the CSP keeps
  `connect-src 'self'`. Optionally an operator adds one mirror origin for model files
  (a pinned revision on a CDN); the hash check makes the mirror unable to change what
  runs.
- Only models whose licence allows redistribution: in phase 1 a small instruct model
  (SmolLM2-360M-Instruct or Qwen2.5-0.5B-Instruct, Apache-2.0, quantised); in phase 2
  Whisper (MIT).

**The catch is the download.** The home line sends 4.8 MB/s
([measurements](../measurements.md#public-address-relay-sweep-spec-0006)). A model of
a few hundred MB to 30 phones is gigabytes: tens of minutes. Phase 1 measures it; the
defense either uses a small model (Whisper tiny or base, tens of MB), loads the room's
devices before the committee arrives, or adds the CDN mirror. The lab node (spec 0009)
later serves a lab's PCs over the LAN.

## Jobs and units

- **Text** (phase 1): inputs are a file of prompts (one per line or JSONL), a system
  prompt, and `max_new_tokens`; one unit = one prompt. Output: the generated text, its
  token ids, and for each token the margin between the chosen token's logit and the
  runner-up's.
- **Speech** (phase 2): an audio file is cut by the console into 30 s chunks with 2 s of
  overlap; one unit = one chunk; the result is the transcript with the overlaps merged.
- Decoding is **greedy** (no sampling, no temperature) so two honest devices compute the
  same thing.
- Results are downloadable as a CSV or text file, and the console shows them as they
  arrive.

## Verification

Floating point differs between GPUs, so this is the hardest part, and it is measured
before it is trusted.

- **Phase 1:** the usual quorum (spec 0004). Two devices answer each unit; `agree` is
  exact equality of token ids. A disagreement goes to a third device. Honeypots are
  prompts whose greedy answer is known from earlier units. Measured: how often honest
  devices on different GPUs disagree, and at what margin the first different token was
  chosen.
- **Phase 3 — the near-tie rule:** two outputs agree if they are equal up to the first
  different token and both devices report a margin below ε there (an honest near-tie).
  ε comes from the phase-1 measurement. A divergence at a confident token is a real
  disagreement.
- **Phase 3 — a cheap check:** a verifier device does not generate again; it runs the
  model once over prompt + claimed answer (one parallel pass instead of one pass per
  token) and checks that every claimed token was the top choice, within ε. Faking text
  that passes means running the model anyway. Trusted devices then answer alone, with
  spot checks, as in spec 0004.

## Phases

1. **Text on the swarm**: the runtime in a worker, the model catalog with hash checks
   and caching, the `ai` template with exact-match quorum, the console's job form and
   results, a test model in CI. Measurements: model load time, tokens per second on the
   phone, the M1 Max and a lab PC, and the disagreement rate between GPUs.
2. **Speech**: Whisper, a lesson transcribed live by the room (the defense moment).
3. **Verification that scales**: the near-tie rule and the one-pass check.
4. **Bigger models on bigger devices**: tiers by device memory, a 1–3B model on PCs with
   a real graphics card; images (classification, embeddings for search).

## Acceptance (phase 1)

- A network starts a text job from a file of prompts in the console; its devices with
  WebGPU compute it; the results download as a CSV in prompt order.
- A model file whose SHA-256 differs from the catalog is refused, and the unit fails
  on that device without a result.
- A second load of the same model reads it from the cache (no network request for it).
- A device that cannot hold the model is never given its units.
- „Спри“ stops a generation mid-token and the unit returns to the queue.
- A simulated cheater returning made-up text is outvoted and banned (Rust test).
- CI: the browser test uses a tiny test model (a few MB, random weights) in Chromium
  with WebGPU and the CPU fallback; Rust tests cover split, validation, agree and
  aggregate.

## Verified by

Rust tests for the template; Vitest for the catalog checks and result merging;
Playwright with the tiny model; measurements on real devices in
`docs/measurements.md` before any number reaches a competition document.

# 0017 — AI on the swarm

Status: phase 1 built 21 November 2026, with the near-tie rule of phase 3 (see
„Phase 1 as built“); target for the defense: phases 1–2

## Goal

A school asks a question it cannot afford to ask a cloud: _transcribe every recorded
lesson of the term_, _summarise 2 000 essays_, _label 50 000 photos of the herbarium_.
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

## Computers only

Language models run on computers, not on phones or tablets. A handheld would spend minutes
downloading 275 MB over mobile data and then heat up, and a room of phones joining at once
would send the server gigabytes. A phone or tablet (`lib/device-name.ts::isHandheld`: the
browser says it is mobile, an Android, iPhone or iPad user agent, or a "Mac" with a touch
screen) leaves the size of its GPU's buffers out of what it tells the coordinator, so the
coordinator never gives it a model's units; it still computes images, π, neural sweeps,
timetables and programs. Checked 24 November 2026 with an iPhone 15 (WebKit) and a Pixel 7
(Brave) emulated beside a Mac: both rendered an image, and in an AI job every unit went to
the Mac.

## Models are known code too

A model is data, but it decides what a device computes, so it is pinned like a kernel:

- The site ships a **model catalog**: for each model, its files with their size and
  SHA-256, its licence, and the minimum device it needs. A job names a catalog model;
  it cannot point at a URL.
- The browser downloads each file, checks its SHA-256 before use, and keeps it in
  Cache Storage: a device downloads a model once.
- Files are served from Swarm's own origin (`/api/models/…`), so the CSP keeps
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

## Phase 1 as built (21 November 2026)

- **Template `ai`.** The catalog (`templates/ai/catalog.json`) is the template's pinned
  source: its SHA-256 goes with every unit, and each file it lists carries its size and
  SHA-256. `just models` downloads the files from their pinned Hugging Face revision and
  checks them; `just deploy-models` copies them to the server, whose coordinator serves
  `/api/models/{model}/{file}` for catalog files only.
- **Models.** SmolLM2-360M-Instruct (`q4f16`, 273 MB). The catalog's test model,
  `hf-internal-testing/tiny-random-LlamaForCausalLM` (10 MB, random weights, no licence
  stated), is accepted and served only where `SWARM_TEST_MODELS=1`: the browser tests and
  development, never a server.
- **Which devices.** A device reports its GPU's largest buffer (`max_buffer_size`); a job
  of a model with `min_gpu_buffer` (256 MiB for SmolLM2) goes only to devices that report
  at least that much. Devices without WebGPU report none, so lab PCs and the CPU fallback
  never get SmolLM2 units; the test model runs anywhere.
- **In the browser.** transformers.js and ONNX Runtime Web load in the compute worker with
  the first such unit. Every file goes through a fetch that loads only catalog files from
  this site, checks the SHA-256 before use, and keeps checked copies in Cache Storage. The
  runtime's WebAssembly is served by the site too (no CDN), loaded as a module, not a blob.
  A logits processor records each token's margin; a stopping criterion ends the
  generation after the current token when „Спри“ is pressed.
- **Agreement: the near-tie rule now.** Measured on the M1 Max, Chrome and WebKit gave the
  same 64-token answer for only 34 of 50 prompts; every other answer parted at a token both
  chose by at most 0.31. Exact agreement would have counted a third of honest answers
  against their devices. Two answers agree when they are identical, or identical up to a
  token both devices chose by less than 0.5 (`NEAR_TIE`), and not at the first token: an
  honest device was that unsure of its first token for 16 of 150 answers, so a made-up
  answer would pass that often, while every honest parting came at token 5 or later.
  Faking the first token takes running the model.
- **A known answer at every load.** Each catalog model carries a check: a prompt, the
  tokens a device known to compute correctly gives (Chrome on the M1 Max), and their
  margins. A device that loads the model answers it first; an answer that parts from the
  known one other than at a near-tie (the same rule as two devices' answers) means the
  model computes wrongly there. The device then sends `failed` with `unfit`: the unit goes
  back to the queue without being charged a try, the coordinator records the job in the
  worker's `unfit_jobs` and gives that connection none of its units again, and the worker
  page says so. Other templates keep running on it. SmolLM2's check is „List the first five
  planets of the solar system, one per line.“: 20 tokens, every margin at least 0.8.
- **Runtime settings for every GPU.** ONNX Runtime gets a GPU device made by the runner,
  without the `subgroups` feature, and graph optimisation `basic`. On the Adreno 642L the
  subgroup kernels hang the GPU, and the fused kernels of the higher levels (layer norm
  fused with the following 4-bit matrix product, used when decoding one token at a time)
  compute wrong tokens. Every device runs the same kernels this way; on the M1 Max it costs
  about 8 % (69 → 62 tokens/s).
- **No honeypots of language models yet.** A known answer computed on another kind of GPU
  can part at a near-tie, and two wrong answers to known units ban a device. Devices
  answering language-model units still get honeypots of the other templates.
- **Simulated devices** answer with a stand-in (`sim_worker::stand_in_answer`): no model,
  a pure function of the unit, so the Rust tests of order, scheduling and cheaters need no
  weights.
- **Console.** „Текст с ИИ“: a model, the answer length, verification, instructions, and
  the prompts typed or from a file (lines or JSON Lines with `prompt`). The job's page
  shows each answer as it is decided and downloads them as CSV or text.

**Found.** SmolLM2-360M does not answer Bulgarian (it repeats the question). A model for
Bulgarian prompts is still to be chosen. On a Nothing Phone (1) (Adreno 642L) the first
runs gave wrong answers from the first token and lost the GPU; the cause was two
runtime defaults (above), not the `q4f16` build: with the runner's settings the phone
answers like the Mac. Measurements: [measurements.md](../measurements.md#language-models-on-the-swarm-spec-0017).

## Acceptance (phase 1)

- A network starts a text job from a file of prompts in the console; its devices with
  WebGPU compute it; the results download as a CSV in prompt order.
- A model file whose SHA-256 differs from the catalog is refused, and the unit fails
  on that device without a result.
- A second load of the same model reads it from the cache (no network request for it).
- A device that cannot hold the model is never given its units.
- A device that computes the model wrongly (its answer to the model's check is wrong)
  takes none of the job's units, and its refusals do not fail the job (Rust test; on the
  phone by hand, with the runtime's defaults put back).
- „Спри“ stops a generation mid-token and the unit returns to the queue.
- A simulated cheater returning made-up text is outvoted and banned (Rust test).
- CI: the browser test uses a tiny test model (a few MB, random weights) in Chromium
  with WebGPU and the CPU fallback; Rust tests cover split, validation, agree and
  aggregate.

All met on 21 November 2026 (Rust: `templates::ai`, `models`, `tests/ai.rs`; browser:
`e2e/ai.e2e.ts` in Chromium with and without WebGPU, Firefox and WebKit; Vitest:
`lib/ai/files.spec.ts`), with two notes: „Спри“ mid-token was checked by hand with
SmolLM2 (the test model answers too fast to interrupt), and in WebKit as Playwright opens
it a new page re-downloads the model (a private session; see measurements).

## Verified by

Rust tests for the template; Vitest for the catalog checks and result merging;
Playwright with the tiny model; measurements on real devices in
`docs/measurements.md` before any number reaches a competition document.

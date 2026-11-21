# The MacBook

A plan for what the new MacBook does for Рояк between now (10 November 2026) and
registration (25 November), and after it. It turns open items of
[roadmap.md](roadmap.md) (3. Real devices, 4. Registration material, 5. Defense run
sheet) into concrete steps that this machine can do and the ThinkPad could not.

## What the machine is

MacBook Pro, Apple M1 Max: 10 CPU cores (8 performance, 2 efficiency), 24-core GPU,
Metal 4, 32 GB, macOS 26.3. HDMI and Ethernet over USB-C, a battery that lasts a school
day.

## Why it matters

Every WebGPU number in [measurements.md](measurements.md) so far comes from one Adreno
phone or from SwiftShader, a GPU emulated on the processor. The MacBook adds:

1. **A real desktop GPU through Metal.** The first one in the log.
2. **Safari 26 with WebGPU.** It runs the same engine as the iPhone. The README's support
   list (section 23) names Safari 26 on macOS and iOS, and Firefox 145+ on Apple-silicon
   Macs, and none of them has been tested on a real device ("Not yet tested on real
   devices" under [Browser engines](measurements.md#browser-engines-automated-tests)).
3. **Three independent shader compilers on one GPU.** Safari (WebKit's WGSL compiler),
   Chrome and Edge (Tint) and Firefox (naga) each translate the same WGSL to Metal in their
   own way. Spec 0011 says a configuration trained on "Apple silicon" ends with the same
   weights bit for bit, but no one has checked that yet. This machine can.
4. **Safari Web Inspector for an iPhone.** An iPhone on USB shows its console and errors
   on the Mac, so an iPhone failure comes with a message.
5. **Measured power.** `powermetrics` reports CPU and GPU watts every second. That gives
   the README's electricity question (section on objections) a measured answer, and starts
   the roadmap's "energy estimate per job from measured device power".
6. **A presenting laptop.** It is fast enough to build and run the whole stack (Plan B
   and C in [demo-day.md](demo-day.md)) and to drive the projector all day on battery.

## State on 10 November

| Ready | Missing |
|---|---|
| Homebrew, git, `gh` | `just`, `cargo-nextest` |
| Rust 1.95 with `wasm32-unknown-unknown` (matches `rust-toolchain.toml`) | Docker on the PATH: OrbStack is installed but has not been opened yet |
| Node 26, pnpm 12 | `tailscale` on the PATH: the app is installed but not signed in |
| 1Password, an SSH key that logs in to magserver | Chrome, Firefox, Edge (only Safari is installed) |
| The repository at `~/Documents/GitHub/Swarm` | `web/node_modules`, Playwright's browsers, `.env` |

## Phase 0: a working dev machine (10 November, about an hour)

1. Tools:
   ```
   brew install just cargo-nextest zbar   # zbar: the console test decodes the QR code
   brew install --cask google-chrome firefox microsoft-edge
   ```
2. Open **OrbStack** once: it puts `docker` and `docker compose` on the PATH. Give it
   8 GB of memory in its settings (the stack's first build is the heaviest step).
3. Open **Tailscale**, sign in to the same tailnet as magserver and the phone, and turn
   on its command-line tool in the settings. Then `tailscale status` lists magserver.
4. Project setup:
   ```
   cp .env.example .env
   just services
   cd web && pnpm install && pnpm exec playwright install chromium firefox webkit
   ```
5. Add `export SWARM_SERVER=magcecu@<magserver>` to `~/.zshrc`. Use the LAN address
   at home and the tailnet name everywhere else, so `just deploy`, `just fleet` and
   `just cheater` work without typing it.
6. In Safari: Settings → Advanced → "Show features for web developers". This turns on
   the Develop menu, which Phase 3 needs.

Done when `just check` passes, `just coordinator` + `just web` serve the app at
`localhost:7701`, and `just sim 4` empties a `just job`.

## Phase 1: what breaks on macOS (10–11 November)

The tooling was written on Linux. Known and likely problems:

| Problem | Evidence | Fix |
|---|---|---|
| `just deploy` fails | macOS tar is bsdtar 3.5.3: `tar: Option --ignore-failed-read is not supported` (checked 10 November) | Drop the flag and pass only files that exist (`git ls-files` with `--cached` also lists deleted files), or use `gtar` when it is installed |
| `just deploy` would copy macOS metadata | bsdtar adds `._*` AppleDouble files for extended attributes | `COPYFILE_DISABLE=1` before `tar` in the recipe |
| `just e2e-webkit` assumes Linux | It starts Playwright's Ubuntu container because WebKit needs Ubuntu's libraries there | On macOS, Playwright runs WebKit natively: `pnpm exec playwright test --project webkit`. Make the recipe pick that on Darwin |
| The `webgpu` test project uses Linux flags | `--enable-features=Vulkan` exists to reach SwiftShader | On macOS, headless Chromium may get the real Metal adapter instead. Record which adapter the self-test reports. If it is the real GPU, it is the first automated test on real hardware |
| Example programs are checked for x86-64 Linux in CI | `ci.yml` runs `cargo clippy/test --target x86_64-unknown-linux-gnu` | Locally use `--target aarch64-apple-darwin`; `just programs` needs no change |

Done when a deploy from the Mac restarts the server cleanly, `just e2e` and the WebKit
project pass natively, and [Browser engines](measurements.md#browser-engines-automated-tests)
has rows for macOS (Chromium, Firefox, WebKit, natively on Apple silicon).

## Phase 2: the Mac as a measured device (12–13 November)

Everything here goes into [measurements.md](measurements.md) with the date, the browser
version and the macOS version. Run on mains power, with Low Power Mode off, and with
`caffeinate -i` in a terminal so the Mac does not sleep.

### 2.1 WebGPU support and bit-exactness

For each of Safari, Chrome, Edge and Firefox, open `/selftest`:

- the adapter it reports (vendor, architecture), and whether the worker page says
  „видеокарта“ or „процесор“;
- bit-exact on every shared vector: π, the path tracer (within tolerance), the neural
  network (`templates/nn/vectors.json`: accuracy, loss curve and SHA-256 of the final
  weights) and the timetable search.

That gives four new rows in [WebGPU support by device](measurements.md#webgpu-support-by-device).
The neural-network rows decide whether spec 0011's sentence about Apple silicon stays.
A mismatch in one browser is a bug found before the jury finds it. Keep the failing
vector and the browser version.

Also run `cargo nextest run --workspace` natively. It is the first time the Rust
references are built for ARM, so the shared vectors are checked on a second
architecture.

### 2.2 Benchmarks

Use the same settings as the phone and ThinkPad rows, so they compare directly:

| Run | Settings (as earlier rows) | Each in |
|---|---|---|
| π | 2²⁸ samples per unit, 128 units | Safari, Chrome, Firefox (GPU) and the CPU fallback in Chrome |
| Path tracer | 128² tiles × 64 spp | the same |
| Sustained load | π 2²⁸, 10 minutes, as the phone's 1800-unit run | Safari and Chrome: unit time first minute against last, which shows throttling |
| Native | `just sim 10`, path tracer 1280×720, 32² tiles, 16 spp × 16 passes | the same job as the magserver row |
| Timetables | `cargo run --release -p timetable --example searches -- 40 2000000 1` | native, and the WebAssembly build in Safari |
| Neural sweep | `cargo run --release -p kernels --example nn_sweep` | native, for the ms column |

`just bench-report` gives units, average unit time and Msamples/s per device from the
local database.

### 2.3 Energy

During each GPU run above:

```
sudo powermetrics --samplers cpu_power,gpu_power -i 1000 -o ~/power-<run>.txt
```

Note the idle baseline first, with the same browser open and the worker stopped. Report
joules per 10⁹ π samples and per million light paths, GPU against CPU fallback. The
lab PCs and the phone cannot be measured this way. Say that, and do not estimate their
numbers as if they were measured.

### 2.4 Contributor safety in Safari (spec 0007)

- **Hidden page:** switch tabs, minimise the window, move to another Space, close the
  lid. Each should pause; `?background` should keep computing.
- **Wake lock:** Safari has had the Screen Wake Lock API since 16.4. Check that the screen
  stays on while computing.
- **Power levels:** 25 / 50 / 75 / 100 %, 20 s each, as in the phone's table.
- **Battery:** Safari has no `navigator.getBattery`. `web/src/lib/worker/conditions.svelte.ts`
  calls it optionally, so the pause below 30 % battery never fires in Safari, on the Mac
  or on an iPhone. Record it as a known limit in measurements.md and in the README's
  contributor-safety text, so a juror with an iPhone does not catch it first.
- **Losing the connection:** turn Wi-Fi off in the middle of a unit for 10 s and then for
  60 s. The page should rejoin and the unit should be computed again elsewhere.
- **Low Power Mode:** turn it on while computing and note how much throughput drops.

## Phase 3: the iPhone, through the Mac

Roadmap item 3 asks for an iPhone with Safari's WebGPU. With an iPhone at hand (your own,
borrowed, or the first juror's at a rehearsal):

1. On the iPhone: Settings → Apps → Safari → Advanced → Web Inspector on. Connect it to
   the Mac by USB. Safari on the Mac → Develop → the iPhone → the worker page: its console,
   network and errors live.
2. On mobile data (Wi-Fi off), open `https://swarmrelay.duckdns.org/join`, join the public
   network and run the `/selftest` vectors. That gives a WebGPU support row and a
   benchmark row, like the Nothing Phone's.
3. Wake lock; pause on Home; Airplane mode for 10 s and 60 s; a program (spec 0012) and
   a timetable job (spec 0013) on the phone.
4. Temperature: the iPhone reports none. Note "warm/hot to the touch" and whether iOS
   dims the screen (thermal throttling).
5. Two different iPhone models before the defense, if possible: an older one without
   WebGPU should fall back to the CPU, and that needs checking too.

**Without an iPhone:** Xcode's iOS Simulator runs iOS Safari on the Mac's GPU. Xcode is a
large download, so install it only for this. It checks the join page's layout, the
flows and the error messages on a phone screen. It is not a device, so its numbers do
not go into the benchmark tables.

## Phase 4: the presenting laptop (14 November, then before every rehearsal)

1. **Plan B on this Mac:** `just stack`. The base images (`rust:1.95-slim`,
   `node:24-slim`, `debian:trixie-slim`) are multi-arch, so it builds natively on arm64.
   The first build takes about 10 minutes. Then create the admin on the stack's own
   database (the command in demo-day.md, Plan B) and a network „Защита“.
2. **Funnel:** allow Funnel for this Mac in the tailnet's settings once. `just share-public
   7780`, then scan the QR code with the phone on mobile data and check that it joins and
   computes. Note the address the command prints in demo-day.md.
3. **Plan C, offline:** Wi-Fi off, `just stack`, three browser windows on
   `localhost:7780/worker?background`, and `just sim 8 ws://localhost:7780/ws/worker`, with the
   live view and the chaos panel on the screen. Time it, so you know it works without
   any network.
4. **The last resort:** record a full Plan A run (⌘⇧5, the whole screen, at the
   projector's resolution) once the run sheet is final. Keep it on the Mac and on a USB
   stick.
5. **Projector:** HDMI from the Mac, mirrored at 1920×1080 and at 1600×900. demo-day.md
   says the segment fits both, so check it on this screen. Before the defense: a Focus
   mode with no notifications, the charger in, `caffeinate -d` in a terminal, and Safari or
   Chrome in full screen.
6. Rebuild the stack after every big merge (`just stack`), so Plan B is never a version
   behind.

## Phase 5: in the school lab (the week of 16 November, owner)

The Mac answers the one School network question that needs a laptop
([lab-session.md](lab-session.md), "PCs reach each other"; the column "PCs reach a laptop
in the lab" in measurements.md is still empty):

1. A USB-C Ethernet adapter, plugged into the lab's socket or switch, not the school
   Wi-Fi.
2. `just stack`, then `http://<the Mac's address>:7780/healthz` on a lab PC. Allow the
   incoming connection when the macOS firewall asks.
3. If the page loads, lab PCs can reach a machine in the lab, which spec 0009's lab node
   needs. A timeout means the PCs are isolated, and that decides spec 0009 too.
4. While there, also measure the speed and check private names (the same list), and
   start the week of idle-time measuring (roadmap item 4).

## Phase 6: a second node and a remote client (any time)

- **Load from outside the home line.** On school Wi-Fi or a phone hotspot,
  `just sim 10 wss://swarmrelay.duckdns.org/ws/worker` (sim-worker speaks TLS)
  joins from a real remote network. Use a network of your own, not the public one, so
  the public grid's numbers stay clean. It repeats the relay sweep of spec 0006 from a
  second place.
- **Timetables at scale:** the real school's data from the deputy director (roadmap
  item 2) and the public XHSTT benchmarks, on the M1 Max's 10 cores. Searches run in
  parallel, so the native number for "N devices" can be checked against the grid.
- **Printing in a second engine:** print a class and a teacher timetable from Safari
  (File → Print → PDF). Printing was built and checked in Chromium. WebKit lays out pages
  differently, and the deputy director may well use a Mac or an iPad.

## Phase 7: registration material and the defense (21–24 November)

- **Annotation (roadmap item 4):** the numbers from Phase 2 (a real GPU in four browsers,
  bit-exact; energy per job) and Phase 3 (an iPhone), next to the phone and the lab.
- **Screenshots:** take them on the Retina screen for the annotation and the jury's
  README (roadmap item 6): the console, the live view with the honeycomb, a caught
  cheater, a printed timetable.
- **Run sheet (roadmap item 5):** rehearse the 20 minutes on the Mac with a timer and
  the phone as the first "juror": QR, a live timetable search, a cheater caught, a device
  switched off. Update demo-day.md with the times.

## Keeping it safe

- FileVault on; Time Machine (or another backup) for the home folder.
- `~/swarm.env` (the stack's submit password) and the SSH key stay off the repository
  (`.gitignore` already covers `.env*`, `*.key`, `.DS_Store`). Keep a copy of both in
  1Password.
- Keep the ThinkPad as it is: it is the "old laptop without usable WebGPU" in the log
  (Edge, CPU fallback).

## Schedule

| Date | What | Phase |
|---|---|---|
| Tue 10 Nov | Tools, OrbStack, Tailscale, `just check` | 0 |
| Wed 11 Nov | macOS fixes in the justfile; native e2e; deploy from the Mac | 1 |
| Thu 12 – Fri 13 Nov | Self-tests, bit-exactness, benchmarks, energy, Safari safety | 2 |
| Sat 14 Nov | Plan B and C rehearsed on the Mac; first recording | 4 |
| Sun 15 Nov | iPhone through Web Inspector (or the Simulator) | 3 |
| Mon 16 – Fri 20 Nov | Lab: PCs reach the Mac; network checks; idle-time week starts | 5 |
| any evening | Remote load from school Wi-Fi; Safari printing; XHSTT | 6 |
| Sat 21 – Mon 23 Nov | Annotation, screenshots, README for the jury | 7 |
| Tue 24 Nov | Stack rebuilt, final recording, run sheet timed | 4, 7 |
| Wed 25 Nov | Registration closes | |

## Done when

- [ ] `just check`, `just e2e` and the WebKit tests pass natively on the Mac.
- [ ] `just deploy` works from macOS.
- [ ] measurements.md has the M1 Max in Safari, Chrome, Edge and Firefox: support,
      bit-exactness (π, path tracer, neural network, timetable) and benchmarks.
      21 November: Chrome, Edge, WebKit 27.2 and Playwright's Firefox measured; the
      timetable natively. Still open: the released Safari 26 (needs "Allow remote
      automation" for `safaridriver`) and the released Firefox 160 (needs `geckodriver`).
- [x] Spec 0011's Apple-silicon claim is measured, or corrected (holds: bit-exact in
      Tint, WebKit's compiler and naga, 21 November).
- [ ] Energy per job, measured with `powermetrics`, in measurements.md and the README.
- [x] Safari's missing battery API is written down as a known limit (Firefox has none
      either).
- [ ] At least one iPhone row, from a real iPhone on mobile data.
- [ ] Plan B and Plan C rehearsed on the Mac. A recording of Plan A on the Mac and a USB
      stick.
- [ ] The lab's "PCs reach a laptop" column filled in.

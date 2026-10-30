# 21 · Рояк (Swarm): A Volunteer Supercomputer Built from Idle School Browsers with WebGPU

| | |
|---|---|
| **Category** | Разпределени приложения (Distributed applications) |
| **Group** | XI–XII grade |
| **Team** | 2 students (distributed backend + GPU/web compute) |
| **Difficulty** | ★★★★★ |
| **Evaluation score** | 85.0 / 100, rank #1 of 40 (see [EVALUATION.md](../../EVALUATION.md)) |
| **Budget** | ~10–30 €/month (coordinator VPS) |
| **Winning angle** | At the defense, the judges scan a QR code. Their phones and laptops join the grid **through the browser** and start computing a real task. A live counter shows the room's combined GFLOPS growing, and the final result renders on the projector. |

---

## TL;DR

Bulgarian schools have **thousands of computers idle most of the day and all night**, and every student has a phone with a capable GPU. Meanwhile Bulgarian students, teachers and researchers lack compute for **ML experiments, simulations and rendering**. Рояк turns any device with a **modern browser** into a compute node: open a page (or keep a tab open in the computer lab) and it runs **sandboxed WebGPU/WebAssembly tasks**. A **coordinator** splits jobs into work units, schedules them on heterogeneous, unreliable devices, **verifies results** (redundancy + spot checks, because anyone can join and cheat), aggregates them, and exposes a **REST API** where researchers and teachers submit jobs from **approved task templates**: Monte Carlo simulations, path-traced rendering, hyperparameter sweeps for small neural networks, scientific parameter sweeps.

---

## Why this dominates

- **Pure power projection:** distributed scheduling, fault tolerance, Byzantine (cheating) workers, GPU compute shaders, and a live room-scale demo. Judges in the Distributed category will recognize real distributed-systems problems.
- **Zero install:** the browser is the runtime. "Your school is already a supercomputer; it just doesn't know it."
- **Unique:** volunteer computing exists (BOINC), browser grids existed as research, but a **WebGPU** school-scale grid with verification and an education angle is new, and nobody else at НОИТ will have it.

## 1. The problem

- Students doing ML or science projects (including other НОИТ participants!) hit compute limits: free Colab quotas, no GPU at school, expensive cloud.
- **Researchers** at Bulgarian universities and institutes compete for limited HPC time (Bulgaria has EuroHPC machine Discoverer, but access requires proposals and is not for school projects).
- School computer labs (thousands of PCs nationally) are **idle > 70% of the time** (evenings, weekends, holidays); verify by measuring at your school.
- Since 2023–2025, **WebGPU** has shipped in Chrome/Edge, and later Safari and Firefox (verify current status), giving browsers real GPU compute access, safely sandboxed.

## 2. Who uses it

| Role | Need |
|---|---|
| Contributor (any device) | One click to donate compute; see own contribution; zero risk to the device |
| School node admin (IT teacher) | Enrol lab PCs, set schedules (e.g. 16:00–07:00), monitor |
| Job submitter (student/teacher/researcher) | Submit jobs from templates, track progress, download results |
| Reviewer | Approve new job templates (security), approve large jobs |
| Admin | Coordinator, quotas, abuse handling, feature flags |

## 3. Existing solutions & the gap

| Solution | Limitation |
|---|---|
| BOINC, Folding@home | Native install, admin rights (impossible on school PCs), fixed science projects |
| Cloud GPUs / Colab | Cost or quotas |
| Old browser computing research (JS grids, pre-WebGPU) | CPU-only, abandoned, no verification, no education flow |
| National HPC (Discoverer) | Not for school-level projects; proposal process |

**The gap:** a **no-install, GPU-capable, verified** volunteer grid that schools can join in minutes and that serves **educational and research workloads** from approved templates.

## 4. Goal and tasks

**Goal:** turn idle school and personal devices into a safe, verifiable distributed computer available to students, teachers and researchers.

**Tasks:**
1. Coordinator: job API, work-unit generation, scheduling, leases, retries, result aggregation.
2. Worker runtime in the browser: WebGPU compute (WGSL), WebAssembly fallback, Web Workers, throttling (battery, temperature, user activity).
3. **Result verification** against cheaters/faulty devices: redundancy, quorum, spot-checking with known answers, reputation.
4. Task templates: Monte Carlo (e.g. π, option pricing, physics), **path tracer** (renders a 3D scene tile by tile), **neural network hyperparameter sweep** (small CNN training in WebGPU), **N-body / cellular automata** parameter sweeps.
5. Dashboards: live grid map, contributions, job progress; school admin console.
6. Benchmarks: speed-up vs number of nodes, efficiency, verification overhead.

## 5. Features

### MVP
- Open the page → device benchmark → receives work units → computes in WebGPU → submits results.
- Two templates: Monte Carlo simulation and tile-based path tracing.
- Coordinator with leases (timeouts) and retries; redundancy factor 2 with comparison.
- Live dashboard: active nodes, total GFLOPS, job progress, rendered image filling in tile by tile.

### Full version
- **Adaptive scheduling:** work-unit size by device speed; straggler mitigation (speculative re-execution of slow units near the end).
- **Verification engine:** quorum for deterministic tasks; tolerance-based comparison for floating point; **honeypot units** with known answers; worker reputation score; blacklisting.
- **School mode:** kiosk URL for labs, time windows, CPU/GPU caps, pause when a user moves the mouse.
- **Job API** (REST + OpenAPI): submit from templates with parameters, quotas per user, results as files.
- ML template: distributed hyperparameter search for small models (each unit trains one configuration for N steps in WebGPU), with leaderboard of configurations.
- Feature flags (new templates rolled out gradually), CI/CD, containerized coordinator, observability.
- Roles: contributor, school admin, submitter, reviewer, admin.

### Stretch
- Peer-to-peer data distribution (WebRTC) to reduce coordinator bandwidth.
- Federated learning template (training on data that stays on the device).

## 6. Architecture

```
  Browsers (lab PCs, laptops, phones)
  ┌─────────────────────────────────────────────────────────────────────┐
  │ Worker page: benchmark · scheduler client · Web Worker pool          │
  │ WebGPU (WGSL compute shaders) / WASM fallback                         │
  │ Throttle: battery API, visibility, user activity, temperature proxy   │
  └───────────────┬───────────────────────────────▲─────────────────────┘
                  │ WebSocket (lease request,     │ work units (params, seeds,
                  │ heartbeats, results)          │ template id + hash)
                  ▼                               │
  ┌──────────────── Coordinator cluster (stateless API + workers) ──────────────────┐
  │ Job API (REST/OpenAPI) · Template registry (signed, versioned)                  │
  │ Work-unit generator · Scheduler (leases, priorities, quotas)                    │
  │ Verification service (quorum, tolerance, honeypots, reputation)                 │
  │ Aggregator (per template: reduce, assemble image tiles, rank configs)           │
  └──────────────┬──────────────────────────────────────────────────────────────────┘
                 ▼
  PostgreSQL (jobs, units, results, reputation) · Redis (lease queue, presence) ·
  object storage (inputs/outputs) · Prometheus/Grafana · feature flags (Unleash)
                 ▼
  Web: live grid dashboard · submitter portal · school admin console · reviewer console
```

**Security principle:** workers **only run code from signed, reviewed templates** shipped with the site. Jobs provide **parameters and data**, never arbitrary code. That prevents the grid from becoming a botnet.

## 7. Tech stack (GPL-3-compatible)

| Layer | Choice | License |
|---|---|---|
| Worker | TypeScript, WebGPU + WGSL, Web Workers, WASM (Rust → wasm-bindgen) | Standards / MIT-Apache ✅ |
| Coordinator | Go (or Rust/Node) services | BSD / MIT-Apache ✅ |
| Transport | WebSocket; REST (OpenAPI) for jobs | Standards ✅ |
| DB / queue | PostgreSQL, Redis | ✅ |
| Storage | MinIO (separate service) or filesystem | AGPL (separate) ✅ |
| Frontend | SvelteKit, ECharts | MIT / Apache ✅ |
| Ops | Docker, Kubernetes (k3s), GitHub Actions, Prometheus, Grafana, Unleash | ✅ |
| Testing | Vitest, Playwright (with WebGPU-enabled Chromium), Go test, k6 | MIT/BSD ✅ |

## 8. Data

| Data | Source | Notes |
|---|---|---|
| Benchmarks | Own measurements on school lab PCs, phones, laptops | Device classes anonymized (GPU vendor/class only) |
| Path tracer scenes | **Own 3D scenes** (Blender) or CC0 models (Poly Haven) | Asset licence table |
| ML template data | Small open datasets (MNIST: CC BY-SA 3.0; Fashion-MNIST: MIT) | Licence noted |
| Simulations | Generated | |

## 9. Core algorithms

### 9.1 Scheduling with leases
- Each work unit is **leased** to a worker for a deadline proportional to the unit size / device speed.
- Heartbeats extend leases; missed deadline → unit returns to the queue.
- Priority queue per job with fair sharing across submitters (weighted fair queuing).
- **Straggler mitigation:** when a job is > 90% done, duplicate the remaining slow units on fast devices; first verified result wins.

### 9.2 Verification (Byzantine workers)
- **Deterministic templates** (integer/fixed-seed): redundancy r=2, results must match exactly; mismatch → third execution → majority.
- **Floating-point templates:** GPUs differ in floating-point details. Compare within tolerances; for Monte Carlo, check statistical consistency (e.g. a chi-square test against other units).
- **Honeypots:** ~2–5% of units have precomputed answers; a wrong answer → reputation drop.
- **Reputation:** Beta distribution of correct/incorrect; trusted workers get r=1 with random spot checks (adaptive replication, as in BOINC research), saving compute.
- Measure: verification overhead vs. probability that a cheater's result is accepted. A clear graph.

### 9.3 Device-aware work sizing
- On join: a short WGSL benchmark (matrix multiply + memory bandwidth) → device score.
- Unit size chosen so expected runtime ≈ 10–30 s (good balance between overhead and loss on disconnect).

### 9.4 Templates (examples)
- **Path tracer:** each unit = image tile × N samples; aggregator averages samples per pixel (progressive rendering shown live).
- **Monte Carlo physics:** e.g. 2D Ising model at different temperatures; aggregator plots the phase transition.
- **Hyperparameter sweep:** each unit trains a tiny CNN for k steps with WGSL kernels (matmul, conv, softmax), returns validation accuracy.

## 10. Security, privacy & ethics

- **No arbitrary code execution:** templates are signed, reviewed and versioned; workers verify template hashes; jobs carry only parameters/data within schemas.
- **Contributor safety:** throttling (never full GPU), pauses on battery < 30%, visibility-aware, opt-in only, clear "stop" button. No data collected beyond anonymous device class and contribution counters.
- **School consent:** school admin enables lab mode; respects the school's IT policy; power consumption estimate shown (ethics of energy use: run only when PCs are on anyway, or include the cost honestly).
- **Abuse:** quotas, reviewer approval for large jobs, rate limits; prevent crypto-mining by design (no arbitrary code; mining templates refused).
- Standard web security: auth for submitters (OIDC), CSRF/XSS protections, CSP that forbids loading external scripts.

## 11. Testing & validation

| What | How | Target |
|---|---|---|
| Correctness | Each template: compare grid output vs single-machine reference | Exact or within tolerance |
| Scheduler | Unit + property tests (every unit completes exactly once in results, no lost units under random disconnects) | In CI |
| Fault injection | Simulated workers (headless) that disconnect, are slow or **cheat** | Job still correct; cheater detection rate reported |
| Scalability | 1 → 10 → 100 → 1,000 simulated workers (k6/headless) | Coordinator throughput, p95 latency |
| Real speed-up | Lab of 20–30 PCs: speed-up and efficiency vs one PC | Speed-up curve |
| WebGPU kernels | Unit tests in Playwright (Chromium with WebGPU) | In CI |
| Security | CSP tests, template signature tests, OWASP ZAP | No high findings |

## 12. How it scores (protocol 6е)

| Block | Pts | How |
|---|---|---|
| General | 20 | Original, technically bold, useful for education and research |
| Design | 25 | Clean distributed architecture: stateless API, leases, verification service, signed templates |
| Implementation | 35 | Fault tolerance, Byzantine verification, GPU kernels, load + fault tests, CI/CD, feature flags, observability |
| Defense | 20 | **Live room grid**; speed-up and verification graphs; school lab results |

## 13. Category traps to avoid

- ❌ "Runs any code anyone uploads" = a security disaster and an easy jury kill. ✅ Signed templates + parameters only.
- ❌ A demo that only works with your 2 laptops. ✅ A real school lab run + simulated 1,000 workers.
- ❌ Ignoring floating-point differences between GPUs. ✅ Tolerance-based and statistical verification, explained.

## 14. Timeline (2026–27, assumed)

| When | Milestone |
|---|---|
| **Oct 2026** | WebGPU Monte Carlo kernel in a browser; coordinator skeleton with leases |
| **Nov 2026** | Path tracer template; live dashboard; redundancy verification |
| **≤ mid-Dec 2026** | **Register idea**; school defense: the committee joins the grid from their phones |
| **Dec 2026–Jan 2027** | Reputation + honeypots; adaptive sizing; school lab mode; first lab run (holidays!) |
| **Feb 2027** | Job API + submitter portal; ML template; tests, load tests, docs |
| **≤ ~1 Mar 2027** | Upload; grid online with test accounts |
| **Early Mar 2027** | Regional defense |
| **Mar–Apr 2027** | Multi-school run; P2P data distribution (stretch); scaling paper-style report |
| **May 2027** | National round |

## 15. Team split

| Student A: Coordinator & distributed systems | Student B: Worker runtime & GPU |
|---|---|
| Job API, scheduler, leases, fair sharing | WebGPU kernels (WGSL), WASM fallback |
| Verification service, reputation, honeypots | Benchmarking, throttling, device-aware sizing |
| Load/fault tests, CI/CD, observability | Templates: path tracer, Monte Carlo, CNN sweep |
| Security, quotas, feature flags | Dashboards, school admin console, docs |

## 16. Budget

| Item | Approx. |
|---|---|
| VPS for coordinator (2–4 vCPU) | 10–20 €/month |
| Domain + TLS | 10 €/year |
| Hardware | Your school's computer lab (with the IT teacher's permission) |

## 17. Risks & mitigations

| Risk | Mitigation |
|---|---|
| WebGPU unsupported on old lab PCs/browsers | WASM fallback; measure; ask for Chrome updates |
| Floating-point differences across GPUs | Tolerances, statistical checks, deterministic integer templates where possible |
| School IT policy blocks it | Run in a lab you control; document the safety model for the director |
| "So what?" usefulness question | Real users: another НОИТ team's ML sweep, a physics teacher's simulation |
| **What kills this project** | A toy that computes π. **Show a useful job (rendering/ML sweep) and the speed-up curve.** |

## 18. 20-minute defense plan

| Min | Content |
|---|---|
| 0–2 | "There are N idle computers in Bulgarian schools right now." (Your measured estimate.) |
| 2–3 | The QR code: "Please join." |
| 3–10 | **Live:** judges' devices appear on the grid map → a path-traced image fills in tile by tile → live counters (light paths per second, ×speed-up over the fastest single device) → you kill your own laptop mid-job: the job continues; a "cheater" node is injected and gets caught (run sheet: `docs/demo-day.md`) |
| 10–14 | Architecture: leases, verification, signed templates; speed-up curve from the school lab; scaling test to 1,000 simulated nodes |
| 14–15 | Security and ethics: why it can never become a botnet or a miner |
| 15–20 | Questions |

**Plan B:** if the hall Wi-Fi is bad, a pre-joined lab at your school (remote) keeps computing and the dashboard shows it.

## 19. Expected jury questions

- *How do you stop malicious workers?* → Redundancy, honeypots, reputation, statistical checks; detection numbers.
- *How do you stop malicious jobs?* → No arbitrary code; signed templates; reviewer approval.
- *What's the real speed-up?* → Curve; overheads; where it stops scaling (coordinator bandwidth, stragglers).
- *Isn't it a waste of electricity?* → Uses machines already on; energy estimate; idle-only schedules.
- *Why not BOINC?* → No install, works on locked-down school PCs and phones, WebGPU, education templates.

## 20. Promo materials

- Landing page with a live global counter: "Рояк изчислява в момента…"
- A school "compute night" event: all labs join a rendering job
- Stickers for lab PCs: "Този компютър е част от Рояк."
- Short promo video of the image rendering tile by tile from 50 devices

## 21. Registration annotation draft (BG)

> **Име на проекта:** Рояк – разпределен доброволен суперкомпютър от неизползваните училищни и лични устройства чрез браузъра и WebGPU
>
> **Направление:** Разпределени приложения
>
> **Анализ на потребностите:** Учениците, учителите и изследователите често нямат достъп до изчислителна мощност за машинно обучение, симулации и изобразяване, а облачните услуги са скъпи. В същото време хиляди училищни компютри и лични устройства стоят неизползвани голяма част от денонощието. Съществуващите системи за доброволни изчисления изискват инсталиране на софтуер с администраторски права, което в училищата е невъзможно.
>
> **Цел:** Да се създаде разпределена система, в която всяко устройство с модерен браузър се включва с едно отваряне на страница и изпълнява безопасно изчислителни задачи чрез графичния процесор (WebGPU), а координатор разпределя, проверява и обединява резултатите.
>
> **Задачи:** (1) координатор с програмен интерфейс за задачи, разпределяне на работни единици, срокове и повторения; (2) изчислителна среда в браузъра с WebGPU и WebAssembly и ограничаване на натоварването; (3) проверка на резултатите срещу грешни или злонамерени участници чрез дублиране, контролни задачи и репутация; (4) шаблони за задачи – симулации по метода Монте Карло, изобразяване чрез проследяване на лъчи, търсене на параметри за невронни мрежи; (5) табла за наблюдение и училищен режим; (6) измерване на ускорението и мащабируемостта.
>
> **Потребители:** ученици, учители, изследователи, училищни администратори, доброволци.
>
> **Основни очаквани характеристики:** без инсталиране; изпълнение само на подписани и проверени шаблони; устойчивост при прекъсване на устройства; откриване на измамни резултати; програмен интерфейс по REST; автоматични тестове, непрекъсната интеграция, превключватели на функционалности, контейнери; роли за доброволец, училищен администратор, заявител, рецензент и администратор.
>
> **Съществуващи решения:** системи като BOINC, изискващи инсталиране; облачни услуги срещу заплащане; национални суперкомпютри, недостъпни за ученически проекти. Предложеното решение не изисква инсталиране и използва графичните процесори чрез браузъра.
>
> **План за реализация:** октомври–ноември – изчислително ядро и координатор; декември–януари – проверка на резултатите, училищен режим, първо изпитване в компютърен кабинет; февруари – програмен интерфейс, шаблон за машинно обучение, тестове и документация; март–април – изпитване в няколко училища.

## 22. Compliance checklist

- [ ] GPL-3.0; coordinator, worker runtime, templates, dashboards
- [ ] Asset licence table for 3D scenes and datasets
- [ ] Grid online; test accounts: contributor, school admin, submitter, reviewer, admin
- [ ] Written permission from the school for lab use
- [ ] Git access; Appendix 1 docs; Appendix 4 declaration

## 23. Verify before registering

- Current WebGPU support in Chrome, Edge, Firefox and Safari (desktop and mobile).
- The actual idle time of your school's computer lab (measure for a week).
- Your school IT policy for running browser workloads.
- BOINC research on adaptive replication to cite.

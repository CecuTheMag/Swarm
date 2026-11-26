# Device test session

How to put real devices (lab PCs, phones) on the grid and record what they do in
[measurements.md](../measurements.md).

## Always-on server

The deployed stack (`just deploy`) is public at `https://swarmrelay.duckdns.org`
(docs/competition/demo-day.md, Plan A), and at `https://swarm.tail629f50.ts.net` through Tailscale
Funnel as a slower second address (`just public`; the first run prints a login link).
Devices open a network's join link (or `/worker` for the public network) there:
nothing to install, no dev server needed.
`just fleet N` adds N simulated workers on the server.

## Why HTTPS

Browsers only expose WebGPU in a secure context: `https://` or `localhost`. A device
opening `http://192.168.x.x:7701` gets no `navigator.gpu` and the worker fails. The
dev server is therefore served over HTTPS at this machine's Tailscale name.

| Device | How it reaches the dev server |
|---|---|
| Phone or PC with Tailscale | `just share` → `https://<machine>.<tailnet>.ts.net` (tailnet only) |
| Device without Tailscale (school PC) | `just share-public` (Tailscale Funnel, public URL) |

`share-public` puts the app on the internet with no auth: anyone with the URL can join
as a worker or submit jobs. Keep it on only for the session, then `just unshare`.
Installing Tailscale on a school PC needs the school's permission; the public URL
needs nothing installed on the device.

## Before (at home, the day before)

1. Deploy (`just deploy <server>`) and check `https://swarmrelay.duckdns.org/healthz`.
2. Sign in at `/login` (the platform admin: `just admin-create EMAIL <server>` prints
   the password once, on the server's terminal).
3. **Нова мрежа** → name it after the lab („Кабинет 12“). In **Правила**: leave „Всеки с
   връзката“ for a supervised session (or „След одобрение“ to let in only the lab's PCs),
   max power 100 % (lab PCs on mains), no working hours for the session itself.
4. Note the network's code from **Обзор** (10 letters and digits) and write it on paper:
   lab PCs have no camera, so they type `swarmrelay.duckdns.org/join` and the code.
5. In **Задачи**, start a small π job once and check that it finishes on your own
   browser (open the join link in another window).

## In the lab

1. On the teacher's PC (or your laptop on the projector): the console, **Обзор** of the
   network. Every PC that joins appears there within a second.
2. On each lab PC, in Chrome or Edge: `swarmrelay.duckdns.org/join` → type the code →
   „Продължи“ → „Включи се“. The page shows the network's card: its name, devices online,
   speed and this PC's parts. Leave the tab in front: a hidden tab pauses, unless the
   address ends in `?background` (`…/join/<code>?background`).
3. Note per PC: OS, browser version, the GPU (`chrome://gpu`: "WebGPU: Hardware
   accelerated" or not) and whether the worker page says „видеокарта“ or „процесор“.
   Unsupported devices are a result too; they compute on the processor.
4. **Задачи** → „Изображение“, quality „Нормално“ → „Пусни“ → „На живо“: the image fills in
   from the lab. Note the time it takes and the speed-up the live view shows.
5. Try the controls once, and watch the PCs' pages react within a second: **Устройства** →
   „Пауза“ on one PC (its page says the owner paused it), „Пусни“ again; pause the whole
   network from **Обзор**; in **Правила** set max power to 50 % and save.
6. If time allows, a π job with many units (**Подробни настройки**: 256 units) for the
   benchmark table, and **Устойчивост** → „Изключи устройство“ / „Добави измамник“.

Optional: `/selftest` on one PC of each kind, for the speed it reports.

## School network (for spec 0009)

Ten minutes on one lab PC, during the same visit. They decide whether a lab node is
worth building; record the answers in [measurements.md](../measurements.md) ("School
network").

1. **Speed.** Open `https://speed.cloudflare.com` on a lab PC and note download and
   upload. A lab of 30 PCs on the CPU sends about 4.6 MB/s (37 Mbit/s) of render
   results; if the upload is well above that, no lab node is needed.
2. **Private names.** In a command prompt: `nslookup 10.0.0.1.nip.io`. The answer
   `10.0.0.1` means the school's DNS lets a public name point to a private address (the
   lab node needs that); no answer or an error means it blocks it.
3. **PCs reach each other.** Plug the laptop into the lab network (the same socket or
   switch as the PCs, not the school Wi-Fi), run `just stack` on it, and open
   `http://<laptop's address>:7780/healthz` on a lab PC. A page that loads means lab PCs
   can reach a machine in the lab; a timeout means they are isolated.

## After

1. On the server: `ssh <server> 'cd ~/swarm && docker compose -f deploy/compose.server.yaml exec -T postgres psql -U swarm swarm' < deploy/sql/benchmarks.sql`
   gives per-device unit count, average unit time and Msamples/s (`just bench-report`
   does the same for the local dev database).
2. Copy the rows into [measurements.md](../measurements.md) (WebGPU support and
   Benchmarks tables) with the date and browser version.
3. Leave the network: its PCs rejoin by themselves the next time someone opens the link.
   For a scheduled lab, set working hours (**Правила** → „След училище“).

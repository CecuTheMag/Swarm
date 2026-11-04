# How the lab PCs were tested

2026-11-04. One laptop on the lab's cable network (192.168.6.0/24) drove all PCs; the
coordinator ran on the always-on server at home, reached by the PCs through the public
address `https://swarmrelay.duckdns.org`, exactly as a school would use it.

## The PCs

| | |
|---|---|
| Count | 14 on the list, 13 switched on (USER006-01 … USER006-14) |
| Processor | Intel Core i3-2120, i3-3220 or i3-3240: 2 cores, 4 threads, 2011–2012 |
| Memory | 4 GB |
| Graphics | Intel HD Graphics (no WebGPU in any browser: every PC computes on the CPU) |
| System | Windows 10 Education 22H2 (build 19045) |
| Browsers | Chrome 156 on 12 PCs, Edge 147–156 on all 13 |
| Network | Each PC reached the site in 0.3–0.5 s (`curl https://…/healthz`) |

The registry's Chrome version entries said 92–131 on several PCs; the browsers
themselves reported 156. Ask the browser, not the registry.

## Access

The school's IT set up SSH (OpenSSH for Windows) with a key on every PC, under a
staff account; the students stayed signed in at each screen. Nothing was installed and
no account was changed.

Two PCs differed: one had a PATH without `System32` (commands with full paths work),
one refused PowerShell to the staff account (plain `cmd` works), and one had no Chrome
and Edge only under `EdgeCore`.

## Hidden tests (the students' screens untouched)

For each PC, over SSH: start Edge or Chrome headless with a separate, temporary profile
and `--remote-debugging-port`, forward that port to the laptop (`ssh -L`), and drive the
page with the DevTools protocol from a small Node script: open the page, press the
button, read the page's test markers (`data-testid`, `data-status`) every few seconds.
Afterwards every test browser was stopped and its profile deleted, and checked.

## Visible windows (to watch the work)

A browser started over SSH runs in the SSH session, which has no screen. To show it on
the PC's screen, a one-off scheduled task runs Edge as the signed-in user, interactive
(`New-ScheduledTaskPrincipal -LogonType Interactive`; `schtasks /it` with a batch file
where PowerShell was refused), maximized, with its own profile and a debugging port;
the task is deleted at once. The laptop then pressed „Включи се“ through the debugging
port.

A window covered by another one counts as hidden to the browser, and the worker page
pauses while hidden (spec 0007): one PC paused until its page was opened with
`?background`.

## The console

The network's console ran on the laptop in a separate browser window. The lab's test
account was created on the server (`coordinator admin-create`), its password written
only to a file there; the server signed in and the laptop's window got that session's
cookie, so no password was typed or sent anywhere else.

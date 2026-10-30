# 0006 — Load test

Status: implemented; WebSocket sweep through the relay waits for a job on the server

## Goal

Know, before the defense, how many devices the grid can take at once and what limits
it: the coordinator (CPU, database), or the public relay (Tailscale Funnel). The answer
decides whether the demo needs a VPS and a real domain.

## Tool

`sim-worker` becomes the load generator. It already speaks the real protocol and
computes correct results, so verification stays on and every accepted unit is real.

New options:

| Option | Meaning |
|---|---|
| `--in-flight N` | Units each worker keeps requested at once, as the browser does (1–8) |
| `--duration S` | Disconnect every worker after S seconds |
| `--ramp S` | Spread worker connections evenly over S seconds |
| `--connect-to IP` | Open the TCP connection to IP instead of resolving the URL's host (TLS still uses the host name). On a tailnet machine this forces traffic through the public relay instead of the direct tailnet path |
| `--report` | Print a JSON summary when done |

`wss://` URLs are supported (rustls, WebPKI roots).

The summary reports: workers connected and failed, units completed and accepted, wall
time, accepted units per second, bytes sent and received, and the result round trip
(result sent → ack received) at p50/p95/p99 and max.

Computation runs on the blocking thread pool so a thousand workers do not stall the
async runtime.

## Payloads

The result size of a path-tracer tile does not depend on samples per pixel (it is
`w·h·3` floats), so low-spp tiles test bandwidth without spending CPU on rendering.
Tiny π units (`samples_per_unit` = 2¹⁶) test the coordinator's message and database
throughput.

## Experiments

1. Coordinator capacity, local: π, 10 → 100 → 1,000 workers.
2. Public relay: 64-pixel path-tracer tiles, 1 → 30 workers through Funnel.

Results go into `docs/measurements.md`.

## Acceptance criteria

1. `sim-worker` options above work against a local coordinator and through the public
   address; existing tests still pass.
2. A unit test covers the percentile summary.
3. Both experiments measured and recorded, with the limiting factor named.

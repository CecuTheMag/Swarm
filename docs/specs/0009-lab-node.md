# 0009 — Lab node

Status: planned (after registration; with the school lab mode, December 2026 – January
2027). This is an outline: details are settled when work starts.

## Goal

A school lab's PCs exchange the heavy traffic over the lab's own network, at switch
speed, and only a small fraction goes out to the server. The school's internet upload
and the server's home line stop being the limit, so a bigger lab can join. Each PC
finds out by itself whether the lab path works and is faster, and falls back to the
normal server if not.

## Does it pay off? (decide first)

Only rendered tiles are heavy (~62 KB each; a π result is ~50 bytes). A lab of 30 PCs on
the CPU sends about 75 tiles a second, ~4.6 MB/s. Whether that is a problem depends on
the school, so three checks come first (`docs/tests/lab-session.md`, "School network"):

| Check | If it fails |
|---|---|
| Speed of a lab PC to `swarmrelay.duckdns.org` | If the upload easily carries the lab, the lab node is not needed |
| A public name that points to a private address resolves (`10.0.0.1.nip.io`) | The school's DNS blocks it: the lab node cannot get its HTTPS name; stay on the server |
| Lab PCs reach a laptop on the lab network | PCs are isolated: no lab path |

Small labs (10–15 PCs) do not need it either way.

## Design

**The node.** One lab PC or the teacher's PC runs a small Swarm node: the coordinator
binary in a "lab" mode, or the same Docker image. The lab's browsers connect to it.

- Upstream it is one large client of the main coordinator: it leases units in batches
  and returns results through the normal protocol, with a node token issued by the
  organiser.
- **Redundancy stays local.** Both copies of a unit come back over the LAN; the node
  compares them (the same tolerance check as the coordinator) and sends one result.
- **Passes are summed locally.** Path-tracer passes of a tile add up; the node sends one
  sum per tile instead of one per pass. With 8 passes and 2 copies, about 16× less
  upstream traffic.
- π and other small results pass through unchanged.

**Trust.** The main coordinator treats the node as one client with a reputation (spec
0004). Honeypots go through the node, which cannot tell them apart from real work: a
node that forwards wrong sums is caught and banned like any cheater. A cheating lab PC is
caught by the node's own comparison; if the node hides it, the honeypots catch the node.
The node never holds the submit password.

**HTTPS on the LAN.** Browsers allow WebGPU only on HTTPS pages.

- A DuckDNS name of its own for the lab (`lab-<school>.duckdns.org`) points to the
  node's private address. (A sub-name of `swarmrelay` would get the parent's address.)
- The node's Caddy gets a Let's Encrypt certificate through the DNS challenge
  (caddy-dns/duckdns writes a TXT record), since nothing outside reaches a lab PC.
- The DuckDNS token lives only on the node, in its env file, never in the repository.

**Choosing the path.**

1. A lab PC opens `swarmrelay.duckdns.org/worker` as usual.
2. Nodes register their public (egress) address with the main coordinator. When a
   browser connects from the same public address, the coordinator tells it about the
   node.
3. The page probes the node and the server: health check (1.5 s limit), round trip,
   and a ~256 KB upload.
4. If the node answers and is faster, the page moves to the node's own address (not
   only its WebSocket). A page from a private address talking to a private address
   avoids Chrome's prompt for public sites reaching the local network.
5. Anything failing (DNS blocked, isolated PCs, node off) keeps the PC on the server;
   the lab path is tried again on the next reconnect.
6. Phones on mobile data never see a node: their public address differs.

**Dashboard.** The node and its PCs show on the projector as one group ("Кабинет 12:
30 устройства"), so a full lab does not crowd out the room's phones.

## Not covered

- No internet at all: lab PCs cannot look up the node's name. That would need the
  school's own DNS (its IT staff); Plan C in `docs/competition/demo-day.md` stays as it is.
- Several nodes in one lab, and node-to-node traffic.

## Phases

1. **Measure** the three checks at the school (10 minutes, with the lab idle-time visit).
2. **Units sized to each link** (roadmap, Scheduling): the coordinator gives devices
   with slow uploads bigger units, so they send fewer bytes per second. Helps phones on
   mobile data too, and the node later.
3. **Node, pass-through:** lab mode, HTTPS by DNS challenge, registration and path
   choice in the page. No local verification yet: proves the LAN path works.
4. **Node, local work:** local comparison of copies, summed passes, node reputation and
   honeypots through the node.
5. **School lab run** with and without the node: upstream bytes, speed-up, and how often
   PCs fell back to the server.

## Tests (when built)

- Rust: the node's comparison and pass sums equal the coordinator's result for the same
  units; a node forwarding wrong sums is banned through honeypots; a node that
  disappears returns its leases.
- Browser: the page picks the node when it is reachable and faster, stays on the server
  when the node's name does not resolve or its health check times out, and moves back
  when the node goes away.
- At the school: measured upstream traffic per tile with and without the node.

## Open questions

- Is the node a mode of the coordinator binary, or a separate small binary sharing its
  crates?
- How does the organiser issue and revoke node tokens (dashboard, or a `just` recipe)?
- Do lab PCs behind the same public address but on different networks (Wi-Fi, other
  labs) probe a node they cannot reach, and how fast do they give up?

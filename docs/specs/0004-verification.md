# 0004 — Result verification

Status: implemented

## Problem

Anyone can open the worker page, so any result may be wrong: a buggy or overheating
device, a modified client, or someone returning made-up numbers to look productive. The
first CPU-fallback build showed it is not hypothetical: it returned plausible wrong
counts, and the coordinator accepted them (measurements, 2026-10-28).

## Identity

A worker's connection is short-lived; its reputation must not be. The hello message
carries an optional `client_id`, a random id the browser keeps in local storage (the
simulated worker makes one per worker). The coordinator keeps a row per client with its
counts of correct and incorrect results. Ids are self-chosen, so a cheater can always
start over with a fresh one: a new id is simply untrusted, which costs it nothing but
gains it nothing either.

## Redundancy and quorum

- A unit is complete when `redundancy` results (default 2) **agree**, each from a
  different client. Redundant copies run in parallel: a unit stays claimable until enough
  leases are out.
- Agreement is per template: π outputs must be equal; path-tracer tiles must be equal or,
  for GPUs, within the self-test's tolerance (90 % of values within 0.1 %, total within
  5 %).
- When results disagree, the unit asks for one more result, up to `redundancy + 3`. The
  largest agreeing group of at least `redundancy` results wins: its first result becomes
  the unit's output, its members count as correct, the others as incorrect. If no group
  forms, the unit fails, and with it the job ("results never agreed").
- Results that arrive after the unit was decided are still compared with its output and
  count for reputation.

## Known answers (honeypots)

With probability `honeypot_rate` (5 %; 20 % for clients with fewer than 20 verified
results), a work request is answered with a copy of a unit that was already verified in
a finished job. Nothing distinguishes it from real work. The answer is compared with the
known output and counts for reputation only. Honeypots catch what redundancy cannot:
colluding clients that return the same wrong answer.

## Reputation and adaptive replication

- A client's trust is the mean of its Beta posterior, `(correct + 1) / (correct +
  incorrect + 2)`. A client is **trusted** with at least 20 correct results and trust of
  at least 0.95.
- A trusted client may compute a fresh unit alone (`solo`), except for a random
  `spot_check_rate` (10 %) of its units, which are still replicated. This roughly halves
  the cost of verification once most devices are known (as in BOINC's adaptive
  replication).
- A client is **banned** after 2 wrong answers to honeypots (colluders can agree with
  each other, but not with a known answer), or with at least 3 incorrect results and
  trust below 0.5. It cannot connect, and its undecided results are ignored.
- Malformed outputs count as incorrect too.
- A ban **redoes** what the client decided in jobs still running: every unit it helped
  decide that no longer has `redundancy` agreeing results from clients in good standing
  goes back to the queue, the other clients' verdicts on it are reversed, and their
  results count again in the new decision. This bounds what colluders get accepted.

## Interaction with lease limits (0002)

Releasing a lease never touches a decided unit. Honeypot leases count for nothing but
reputation: their release does not count against the unit.

## Concurrency

Completions, releases and bans touch overlapping rows (a ban reverses verdicts on other
clients' leases), so PostgreSQL may abort one of two transactions to break a deadlock.
Every transaction a worker session or the reaper runs is retried on `40P01`/`40001` with
random back-off; an aborted transaction was rolled back completely, so this is safe.

## Configuration

`SWARM_REDUNDANCY` sets `redundancy` (production: 2). Development with a single device
sets it to 1, which turns verification off.

## Tests

- Template agreement: exact and tolerant comparison, including disagreeing outputs.
- Database: two clients must agree; one client never verifies its own work; a
  disagreement asks for a tiebreaker and the majority wins; endless disagreement fails the
  job; honeypots are verified against the known answer; trusted clients work solo except
  for spot checks; repeat offenders are banned.
- End to end: honest workers complete jobs exactly while cheating workers run alongside
  them; cheaters are caught and banned; a colluding pair returning identical wrong
  answers is caught by honeypots.
- Measurements ([measurements](../measurements.md)): no wrong result accepted with
  random cheaters or colluders; overhead 1.20–1.26 results per unit with adaptive
  replication.

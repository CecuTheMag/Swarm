# Writing programs for Swarm

A program is your own computation, run on the devices of your network, or on every
volunteer's device once it passes review (spec 0012). It is a WebAssembly module that
takes bytes and returns bytes: one unit of work per input.

## What a program can and cannot do

A program has **no imports**. It cannot open a connection, read a file, look at the
clock or draw random numbers: it can only compute on its input, in its own memory. That
is what makes it safe to run on a stranger's phone, and it is checked from the module
itself when you upload it.

It also makes a program deterministic: the same input gives the same output on every
device. Swarm relies on that to catch wrong results, by comparing what different devices
computed. If you need pseudo-random numbers, derive them from the input (the unit index
is a good seed).

Limits: module ≤ 4 MiB, memory ≤ 256 MiB (declared), input and output ≤ 256 KiB per unit,
up to 20 000 units and 16 MiB of inputs per job, 120 s per unit on a device. Aim for units
of a few seconds: shorter ones spend more time travelling than computing, longer ones
lose more work when a phone leaves.

## In Rust

Make a library crate:

```toml
# Cargo.toml
[lib]
crate-type = ["cdylib"]

[dependencies]
swarm-program = { path = "…/sdk/rust" }
```

```toml
# .cargo/config.toml: declare how much memory the program may use
[target.wasm32-unknown-unknown]
rustflags = ["-C", "link-arg=--max-memory=67108864"]   # 64 MiB
```

```rust
// src/lib.rs
fn run(input: &[u8]) -> Vec<u8> {
    // … compute …
    output
}

swarm_program::program!(run);
```

Build with `cargo build --release --target wasm32-unknown-unknown`. A worked example,
with tests, is `examples/programs/primes`: unit `i` counts the primes in the `i`-th block
of numbers.

## In another language

Export a memory with a declared maximum and three functions:

| Export | Type | Meaning |
|---|---|---|
| `memory` | memory | the module's linear memory |
| `swarm_input` | `(len: i32) -> i32` | returns where Swarm writes the `len` input bytes |
| `swarm_run` | `() -> i32` | computes; returns the output's length, or a negative error code |
| `swarm_output` | `() -> i32` | returns where the output bytes are |

Swarm creates a fresh instance for every unit, calls `swarm_input`, writes the input
there, calls `swarm_run`, then reads the output at `swarm_output`. In C with clang:
`--target=wasm32 -nostdlib -Wl,--no-entry -Wl,--export=swarm_input
-Wl,--export=swarm_run -Wl,--export=swarm_output -Wl,--max-memory=67108864`.

Allowed features are those of WebAssembly 2.0 (including fixed-width SIMD). Threads,
relaxed SIMD, 64-bit memory, exceptions and garbage collection are refused.

## Check it before uploading

```bash
just programs
```

```bash
cargo run -p programs --features cli -- check my_program.wasm
```

```bash
cargo run --release -p programs --features cli -- run my_program.wasm --index 0 --args-hex 40420f0000000000
```

`check` applies exactly the rules of the upload; `run` computes one unit in an
interpreter and prints the output as hex.

## Run it

1. In the console, open your network's **Програми** tab and upload the module (admins
   only). Give a link to its source if you will ask for review.
2. Devices join the network as for any job: its link, QR code or typed code.
3. In **Задачи**, choose **Своя програма**: either a **count** of units, where unit `i`
   receives `i` as 4 little-endian bytes followed by the shared arguments, or a **file**
   of inputs (one per line as text, or a JSON array of base64 strings).
4. Choose how many devices must agree on each unit: 1 if you trust every device in the
   network, 2 or 3 to catch made-up results.
5. Download the outputs when the job is done: a JSON file with one base64 string per
   unit, in unit order.

The same steps through the API, with a session or an API key of the network
(`Authorization: Bearer swk_…`): `POST /api/networks/{id}/programs`
(`{name, description, source_url?, module}`), `POST /api/networks/{id}/jobs`
(`{template: "program", params: {program, count, args} | {program, inputs}, quorum}`),
`GET /api/networks/{id}/jobs/{job}/outputs`.

## Run it on everyone's devices

Ask for review from the **Програми** tab. The platform admin reads the source and
approves or rejects the exact module (`/app/admin`); a changed module is a new program
and needs its own review. An approved program's jobs can run on the public network
(**На общата мрежа** when starting the job), where every unit is computed by devices
that must agree; the job stays your network's, and so do its outputs. Volunteers choose
whether their devices run reviewed community programs at all, and a network that lends
its idle devices lends them only for the programs each device allows.

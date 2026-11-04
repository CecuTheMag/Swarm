//! Checks a program before uploading it, and runs it on one input as a device would.
//!
//! ```text
//! swarm-program check primes.wasm
//! swarm-program run primes.wasm --index 3 --args-hex 40420f0000000000
//! ```

use clap::{Parser, Subcommand};
use std::{path::PathBuf, time::Instant};

#[derive(Parser)]
#[command(about = "Check and run Swarm programs (spec 0008)")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Checks that a module is a program Swarm accepts.
    Check { module: PathBuf },
    /// Runs one unit and prints its output as hex.
    Run {
        module: PathBuf,
        /// The unit's input as hex.
        #[arg(long, conflicts_with_all = ["index", "args_hex"])]
        input_hex: Option<String>,
        /// The unit's index, for jobs given as a count: the input is then the index as
        /// 4 little-endian bytes followed by the shared arguments.
        #[arg(long)]
        index: Option<u32>,
        /// Shared arguments as hex, for jobs given as a count.
        #[arg(long)]
        args_hex: Option<String>,
        /// Work budget, roughly in instructions.
        #[arg(long, default_value_t = 50_000_000_000)]
        fuel: u64,
    },
}

fn main() -> anyhow::Result<()> {
    match Args::parse().command {
        Command::Check { module } => {
            let checked = programs::check(&std::fs::read(&module)?)?;
            println!(
                "{}: accepted; memory up to {} MiB",
                module.display(),
                checked.memory_max_pages / 16
            );
        }
        Command::Run {
            module,
            input_hex,
            index,
            args_hex,
            fuel,
        } => {
            let input = match input_hex {
                Some(hex) => hex::decode(hex)?,
                None => {
                    let mut input = index.unwrap_or(0).to_le_bytes().to_vec();
                    input.extend(hex::decode(args_hex.unwrap_or_default())?);
                    input
                }
            };
            let started = Instant::now();
            let output = programs::run(&std::fs::read(&module)?, &input, fuel)?;
            eprintln!(
                "{} bytes in {:.1} ms",
                output.len(),
                started.elapsed().as_secs_f64() * 1e3
            );
            println!("{}", hex::encode(output));
        }
    }
    Ok(())
}

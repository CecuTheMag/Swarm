//! Runs one or more simulated workers against a coordinator.

use clap::Parser;
use sim_worker::{Cheat, Options, Stats, run};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(about = "Headless simulated Рояк workers")]
struct Args {
    /// Coordinator worker endpoint.
    #[arg(long, default_value = "ws://localhost:7700/ws/worker")]
    url: String,
    /// Number of concurrent simulated workers.
    #[arg(short, long, default_value_t = 1)]
    workers: u32,
    /// Each worker disconnects after this many units.
    #[arg(long)]
    max_units: Option<u32>,
    /// Exit when the queue is empty instead of polling forever.
    #[arg(long)]
    exit_when_idle: bool,
    /// Fault injection: share of results each worker makes up (0..=1).
    #[arg(long, default_value_t = 0.0)]
    cheat_rate: f64,
    /// Made-up results differ (`random`) or all cheaters return the same (`constant`).
    #[arg(long, default_value = "random")]
    cheat_mode: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let args = Args::parse();

    let cheat = match (args.cheat_rate, args.cheat_mode.as_str()) {
        (rate, _) if rate <= 0.0 => Cheat::Honest,
        (rate, "random") => Cheat::Random { rate },
        (rate, "constant") => Cheat::Constant { rate },
        (_, other) => anyhow::bail!("unknown cheat mode {other:?}"),
    };
    let opts = Options {
        url: args.url,
        max_units: args.max_units,
        exit_when_idle: args.exit_when_idle,
        client_id: None,
        cheat,
    };
    let handles: Vec<_> = (0..args.workers)
        .map(|_| tokio::spawn(run(opts.clone())))
        .collect();

    let mut total = Stats::default();
    for h in handles {
        match h.await? {
            Ok(s) => {
                total.completed += s.completed;
                total.accepted += s.accepted;
            }
            Err(e) => tracing::error!("worker failed: {e:#}"),
        }
    }
    tracing::info!(
        completed = total.completed,
        accepted = total.accepted,
        "done"
    );
    Ok(())
}

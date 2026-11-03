//! Runs one or more simulated workers against a coordinator.

use clap::Parser;
use serde_json::json;
use sim_worker::{Cheat, Options, Stats, percentile, run};
use std::{
    net::{IpAddr, SocketAddr},
    time::{Duration, Instant},
};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(about = "Headless simulated Рояк workers")]
struct Args {
    /// Coordinator worker endpoint (ws:// or wss://).
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
    /// Units each worker keeps requested at once, as the browser does.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=8))]
    in_flight: u32,
    /// Disconnect every worker after this many seconds.
    #[arg(long)]
    duration: Option<f64>,
    /// Spread worker connections evenly over this many seconds.
    #[arg(long, default_value_t = 0.0)]
    ramp: f64,
    /// Connect to this IP instead of resolving the URL's host (TLS still uses the host
    /// name). On a tailnet machine this forces traffic through the public relay.
    #[arg(long)]
    connect_to: Option<IpAddr>,
    /// Extra milliseconds each unit takes, as if on a slower device.
    #[arg(long, default_value_t = 0)]
    delay_ms: u64,
    /// Print a JSON summary when done.
    #[arg(long)]
    report: bool,
    /// Join this network (the code from its join link) instead of the public one.
    #[arg(long)]
    join: Option<String>,
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
    let connect_to = match args.connect_to {
        None => None,
        Some(ip) => {
            let port = if args.url.starts_with("wss://") {
                443
            } else {
                80
            };
            let port = args
                .url
                .split('/')
                .nth(2)
                .and_then(|authority| authority.rsplit_once(':'))
                .and_then(|(_, p)| p.parse().ok())
                .unwrap_or(port);
            Some(SocketAddr::new(ip, port))
        }
    };
    let started = Instant::now();
    let opts = Options {
        url: args.url,
        max_units: args.max_units,
        exit_when_idle: args.exit_when_idle,
        client_id: None,
        join_code: args.join,
        cheat,
        in_flight: args.in_flight,
        connect_to,
        deadline: args.duration.map(|s| started + Duration::from_secs_f64(s)),
        delay: Duration::from_millis(args.delay_ms),
    };
    let handles: Vec<_> = (0..args.workers)
        .map(|i| {
            let opts = opts.clone();
            let delay = Duration::from_secs_f64(args.ramp * f64::from(i) / f64::from(args.workers));
            tokio::spawn(async move {
                tokio::time::sleep(delay).await;
                run(opts).await
            })
        })
        .collect();

    let mut total = Stats::default();
    let mut failed = 0u32;
    for h in handles {
        match h.await? {
            Ok(s) => total.merge(s),
            Err(e) => {
                failed += 1;
                tracing::error!("worker failed: {e:#}");
            }
        }
    }
    let wall = started.elapsed().as_secs_f64();
    tracing::info!(
        completed = total.completed,
        accepted = total.accepted,
        "done"
    );

    if args.report {
        let mut rtt = total.round_trips_ms;
        rtt.sort_unstable();
        let summary = json!({
            "workers": args.workers,
            "failed": failed,
            "in_flight": args.in_flight,
            "completed": total.completed,
            "accepted": total.accepted,
            "wall_s": round(wall, 1),
            "accepted_per_s": round(f64::from(total.accepted) / wall, 2),
            "mb_sent": round(total.bytes_sent as f64 / 1e6, 2),
            "mb_received": round(total.bytes_received as f64 / 1e6, 2),
            "mb_sent_per_s": round(total.bytes_sent as f64 / 1e6 / wall, 3),
            "round_trip_ms": {
                "p50": percentile(&rtt, 0.5),
                "p95": percentile(&rtt, 0.95),
                "p99": percentile(&rtt, 0.99),
                "max": rtt.last().copied().unwrap_or(0),
            },
        });
        println!("{summary}");
    }
    Ok(())
}

fn round(x: f64, digits: i32) -> f64 {
    let f = 10f64.powi(digits);
    (x * f).round() / f
}

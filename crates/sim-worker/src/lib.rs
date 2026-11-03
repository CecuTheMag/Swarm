//! Headless simulated worker: speaks the real worker protocol and computes
//! units with the CPU reference kernels. Used for tests, load generation and
//! fault injection.

use anyhow::{Context, bail};
use futures_util::{SinkExt, StreamExt};
use protocol::{
    Backend, CoordinatorMessage, DeviceControl, DeviceInfo, PROTOCOL_VERSION, WorkerMessage,
    templates::{pi, pt},
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    future::pending,
    net::SocketAddr,
    time::{Duration, Instant},
};
use tokio::{net::TcpStream, time::sleep_until};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, client_async_tls, connect_async, tungstenite::Message,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Options {
    /// Coordinator worker endpoint, e.g. `ws://localhost:7700/ws/worker`.
    pub url: String,
    /// Disconnect after this many completed units.
    pub max_units: Option<u32>,
    /// Disconnect once the queue is empty instead of polling.
    pub exit_when_idle: bool,
    /// The client id to report; `None` makes a new one per run.
    pub client_id: Option<Uuid>,
    /// Join this network (its join code) instead of the public one (spec 0010).
    pub join_code: Option<String>,
    /// Fault injection: return wrong results.
    pub cheat: Cheat,
    /// Units kept requested at once; the browser keeps 2–8 in flight.
    pub in_flight: u32,
    /// Open the TCP connection here instead of resolving the URL's host. TLS still
    /// uses the host name.
    pub connect_to: Option<SocketAddr>,
    /// Disconnect at this instant.
    pub deadline: Option<Instant>,
    /// Extra time each unit takes, as if on a slower device. Load tests use it to act as
    /// many devices without needing their processors.
    pub delay: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            url: "ws://localhost:7700/ws/worker".into(),
            max_units: None,
            exit_when_idle: false,
            client_id: None,
            cheat: Cheat::Honest,
            join_code: None,
            in_flight: 1,
            connect_to: None,
            deadline: None,
            delay: Duration::ZERO,
        }
    }
}

/// How a simulated worker cheats (spec 0004 fault injection).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Cheat {
    #[default]
    Honest,
    /// With probability `rate`, a made-up result (different every time).
    Random { rate: f64 },
    /// With probability `rate`, the same wrong result every cheater returns, so
    /// colluding cheaters agree with each other.
    Constant { rate: f64 },
}

impl Cheat {
    /// The result to return in place of the real one, if cheating this time.
    pub fn falsify(self, template_id: &str, params: &Value) -> Option<Value> {
        let constant = match self {
            Self::Honest => return None,
            Self::Random { rate } | Self::Constant { rate } if fastrand::f64() >= rate => {
                return None;
            }
            Self::Random { .. } => false,
            Self::Constant { .. } => true,
        };
        match template_id {
            pi::ID => {
                let samples = params.get("samples")?.as_u64()? as u32;
                let inside = if constant {
                    samples / 2
                } else {
                    fastrand::u32(0..=samples)
                };
                Some(json!({ "inside": inside }))
            }
            pt::ID => {
                let p: pt::UnitParams = serde_json::from_value(params.clone()).ok()?;
                let n = p.w as usize * p.h as usize * 3;
                let rgb: Vec<f32> = (0..n)
                    .map(|_| {
                        if constant {
                            0.25 * p.spp as f32
                        } else {
                            fastrand::f32() * p.spp as f32
                        }
                    })
                    .collect();
                Some(json!({ "rgb": pt::encode_rgb(&rgb) }))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Stats {
    pub completed: u32,
    pub accepted: u32,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    /// Result sent → ack received, per completed unit, in milliseconds.
    pub round_trips_ms: Vec<u32>,
}

impl Stats {
    pub fn merge(&mut self, other: Stats) {
        self.completed += other.completed;
        self.accepted += other.accepted;
        self.bytes_sent += other.bytes_sent;
        self.bytes_received += other.bytes_received;
        self.round_trips_ms.extend(other.round_trips_ms);
    }
}

/// Nearest-rank percentile of `sorted` (ascending), `q` in 0..=1; 0 when empty.
pub fn percentile(sorted: &[u32], q: f64) -> u32 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (q * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

/// Computes one unit with the reference kernel.
pub fn compute(template_id: &str, version: u32, params: &Value) -> anyhow::Result<Value> {
    match (template_id, version) {
        (pi::ID, pi::VERSION) => {
            let p: pi::UnitParams = serde_json::from_value(params.clone())?;
            let inside = kernels::pi::count_inside(kernels::pi::UnitParams {
                seed: p.seed,
                unit_index: p.unit_index,
                samples: p.samples,
            });
            Ok(serde_json::to_value(pi::UnitOutput { inside })?)
        }
        (pt::ID, pt::VERSION) => {
            let p: pt::UnitParams = serde_json::from_value(params.clone())?;
            let mut rgb = vec![0.0; p.w as usize * p.h as usize * 3];
            kernels::pt::render_tile(
                &kernels::pt::TileParams {
                    width: p.width,
                    height: p.height,
                    x: p.x,
                    y: p.y,
                    w: p.w,
                    h: p.h,
                    spp: p.spp,
                    pass: p.pass,
                    seed: p.seed,
                },
                &mut rgb,
            );
            Ok(serde_json::to_value(pt::UnitOutput {
                rgb: pt::encode_rgb(&rgb),
            })?)
        }
        _ => bail!("unsupported template {template_id} v{version}"),
    }
}

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Empty answers in a row before an `exit_when_idle` worker leaves. One is not enough:
/// a unit another worker is claiming at that moment is skipped (`SKIP LOCKED`), so the
/// queue can look empty while a unit still needs a result from this worker.
const IDLE_ANSWERS: u32 = 3;
const IDLE_RETRY: Duration = Duration::from_millis(50);

async fn connect(opts: &Options) -> anyhow::Result<Socket> {
    // Both rustls providers end up in the workspace; pick one. Errors when already set.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let ws = match opts.connect_to {
        None => connect_async(&opts.url).await?.0,
        Some(addr) => {
            let tcp = TcpStream::connect(addr).await?;
            client_async_tls(opts.url.as_str(), tcp).await?.0
        }
    };
    Ok(ws)
}

enum Event {
    Stop,
    Retry,
    Beat,
    Frame(Option<Result<Message, tokio_tungstenite::tungstenite::Error>>),
}

/// Sleeps until `at`, or forever when there is no such instant.
async fn sleep_until_opt(at: Option<tokio::time::Instant>) {
    match at {
        Some(at) => sleep_until(at).await,
        None => pending().await,
    }
}

pub async fn run(opts: Options) -> anyhow::Result<Stats> {
    let mut ws = connect(&opts)
        .await
        .with_context(|| format!("connecting to {}", opts.url))?;

    let mut stats = Stats::default();
    let send = async |ws: &mut Socket, stats: &mut Stats, msg: WorkerMessage| {
        let text = serde_json::to_string(&msg).unwrap();
        stats.bytes_sent += text.len() as u64;
        ws.send(Message::text(text)).await
    };
    send(
        &mut ws,
        &mut stats,
        WorkerMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
            client_version: concat!("sim-worker-", env!("CARGO_PKG_VERSION")).into(),
            device: DeviceInfo {
                backend: Backend::Native,
                vendor: "cpu".into(),
                architecture: std::env::consts::ARCH.into(),
            },
            client_id: Some(opts.client_id.unwrap_or_else(Uuid::new_v4)),
            join_code: opts.join_code.clone(),
        },
    )
    .await?;

    // Replies the coordinator still owes: an assign or no_work per request, an ack per
    // result. Draining stops new requests; the worker leaves once nothing is owed.
    let mut owed = 0u32;
    let mut draining = false;
    for _ in 0..opts.in_flight.max(1) {
        send(&mut ws, &mut stats, WorkerMessage::RequestWork).await?;
        owed += 1;
    }

    let deadline = opts.deadline.map(tokio::time::Instant::from_std);
    let mut retry_at = None;
    let mut retries = 0u32;
    let mut heartbeat = None::<tokio::time::Interval>;
    let mut held: HashMap<Uuid, Instant> = HashMap::new();
    let mut idle_answers = 0u32;

    loop {
        let beat = async {
            match heartbeat.as_mut() {
                Some(h) => h.tick().await,
                None => pending().await,
            }
        };
        let event = tokio::select! {
            _ = sleep_until_opt(deadline) => Event::Stop,
            _ = sleep_until_opt(retry_at) => Event::Retry,
            _ = beat => Event::Beat,
            frame = ws.next() => Event::Frame(frame),
        };
        let frame = match event {
            Event::Stop => break,
            Event::Retry => {
                retry_at = None;
                for _ in 0..std::mem::take(&mut retries) {
                    send(&mut ws, &mut stats, WorkerMessage::RequestWork).await?;
                    owed += 1;
                }
                continue;
            }
            Event::Beat => {
                let lease_ids = held.keys().copied().collect();
                send(&mut ws, &mut stats, WorkerMessage::Heartbeat { lease_ids }).await?;
                continue;
            }
            Event::Frame(frame) => frame,
        };
        let text = match frame {
            None => break,
            Some(frame) => match frame? {
                Message::Text(t) => t,
                Message::Close(_) => break,
                _ => continue,
            },
        };
        stats.bytes_received += text.len() as u64;
        match serde_json::from_str::<CoordinatorMessage>(&text)? {
            CoordinatorMessage::Welcome {
                heartbeat_interval_ms,
                ..
            } => {
                let period = Duration::from_millis(heartbeat_interval_ms.into());
                let mut h = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
                h.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                heartbeat = Some(h);
            }
            CoordinatorMessage::Assign {
                lease_id,
                template,
                params,
                ..
            } => {
                owed -= 1;
                idle_answers = 0;
                let started = Instant::now();
                let cheat = opts.cheat;
                let computed = tokio::task::spawn_blocking(move || {
                    match cheat.falsify(&template.id, &params) {
                        Some(fake) => Ok(fake),
                        None => compute(&template.id, template.version, &params),
                    }
                })
                .await?;
                let msg = match computed {
                    Ok(output) => WorkerMessage::Result {
                        lease_id,
                        output,
                        elapsed_ms: started.elapsed().as_millis() as u32,
                    },
                    Err(e) => WorkerMessage::Failed {
                        lease_id,
                        reason: e.to_string(),
                    },
                };
                if !opts.delay.is_zero() {
                    tokio::time::sleep(opts.delay).await;
                }
                held.insert(lease_id, Instant::now());
                send(&mut ws, &mut stats, msg).await?;
                owed += 1;
            }
            CoordinatorMessage::Ack {
                lease_id, accepted, ..
            } => {
                owed -= 1;
                if let Some(sent) = held.remove(&lease_id) {
                    stats.round_trips_ms.push(sent.elapsed().as_millis() as u32);
                }
                stats.completed += 1;
                stats.accepted += u32::from(accepted);
                if opts.max_units.is_some_and(|max| stats.completed >= max) {
                    draining = true;
                }
                if !draining {
                    send(&mut ws, &mut stats, WorkerMessage::RequestWork).await?;
                    owed += 1;
                }
            }
            CoordinatorMessage::NoWork { retry_after_ms } => {
                owed -= 1;
                idle_answers += 1;
                let mut wait = Duration::from_millis(retry_after_ms.into());
                if opts.exit_when_idle {
                    draining |= idle_answers >= IDLE_ANSWERS;
                    wait = wait.min(IDLE_RETRY);
                }
                if !draining {
                    retries += 1;
                    retry_at.get_or_insert_with(|| tokio::time::Instant::now() + wait);
                }
            }
            // Paused, outside working hours or awaiting approval: work requests get
            // `no_work` meanwhile, which already paces the worker.
            CoordinatorMessage::Control { control, .. } => {
                if control == DeviceControl::Removed {
                    break;
                }
            }
            CoordinatorMessage::Error { message } => bail!("coordinator error: {message}"),
        }
        if draining && owed == 0 {
            break;
        }
    }
    let _ = ws.close(None).await;
    Ok(stats)
}

/// Convenience for building a path-tracer job body.
pub fn pt_job(width: u32, height: u32, tile: u32, spp: u32, passes: u32, seed: u32) -> Value {
    json!({ "template": pt::ID, "params": {
        "width": width, "height": height, "tile": tile, "spp": spp, "passes": passes, "seed": seed
    } })
}

/// Convenience for building a pi job body.
pub fn pi_job(units: u32, samples_per_unit: u32, seed: u32) -> Value {
    json!({ "template": pi::ID, "params": { "units": units, "samples_per_unit": samples_per_unit, "seed": seed } })
}

#[cfg(test)]
mod tests {
    use super::percentile;

    #[test]
    fn percentiles_use_nearest_rank() {
        let v: Vec<u32> = (1..=100).collect();
        assert_eq!(percentile(&v, 0.5), 50);
        assert_eq!(percentile(&v, 0.95), 95);
        assert_eq!(percentile(&v, 0.99), 99);
        assert_eq!(percentile(&v, 1.0), 100);
        assert_eq!(percentile(&v, 0.0), 1);
        assert_eq!(percentile(&[7], 0.5), 7);
        assert_eq!(percentile(&[], 0.5), 0);
        assert_eq!(percentile(&[1, 2, 3], 0.5), 2);
    }
}

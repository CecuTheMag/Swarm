//! Headless simulated worker: speaks the real worker protocol and computes
//! units with the CPU reference kernels. Used for tests, load generation and
//! fault injection.

pub mod network;

use anyhow::{Context, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::{SinkExt, StreamExt};
use protocol::{
    Backend, CoordinatorMessage, DeviceControl, DeviceInfo, PROTOCOL_VERSION, TemplateRef,
    WorkerMessage,
    templates::{ai, nn, pi, program, pt, tt},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    future::pending,
    net::SocketAddr,
    sync::Arc,
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
    /// Whether to run reviewed community programs on the public network (spec 0012).
    pub community: bool,
    /// Work budget of one program unit, roughly in instructions: the browser's time
    /// limit in the interpreter's terms.
    pub fuel: u64,
    /// The GPU buffer to report, as a browser with WebGPU does, so units of language models
    /// that need one come to this worker (spec 0017). None reports no GPU.
    pub gpu_buffer: Option<u64>,
    /// Fault injection: refuse language-model units as a device whose answer to the
    /// model's known question was wrong (spec 0017).
    pub ai_unfit: bool,
}

/// About two minutes of work for the interpreter.
pub const DEFAULT_FUEL: u64 = 20_000_000_000;

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
            community: true,
            fuel: DEFAULT_FUEL,
            gpu_buffer: None,
            ai_unfit: false,
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
            // A timetable no school has, with the best cost there is: refused on arrival.
            tt::ID => Some(json!({ "slots": "", "cost": 0 })),
            program::ID => {
                let output: Vec<u8> = if constant {
                    b"swarm".to_vec()
                } else {
                    (0..16).map(|_| fastrand::u8(..)).collect()
                };
                Some(json!({ "output": STANDARD.encode(output) }))
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
            // Plausible text a liar made up instead of running the model.
            ai::ID => {
                let tokens: Vec<u32> = if constant {
                    vec![42; 3]
                } else {
                    (0..3).map(|_| fastrand::u32(..1000)).collect()
                };
                Some(json!({ "text": "made up", "tokens": tokens, "margins": [1.0, 1.0, 1.0] }))
            }
            nn::ID => {
                // A plausible score and a made-up network: a liar cannot know the
                // digest without training.
                let digest = if constant {
                    "c".repeat(64)
                } else {
                    format!("{:064x}", fastrand::u128(..))
                };
                let correct = if constant {
                    900
                } else {
                    fastrand::u32(0..=1024)
                };
                Some(json!({ "correct": correct, "curve": vec![20_000u32; 16], "digest": digest }))
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
    compute_template(template_id, version, params)
}

/// Computes one program unit (spec 0012) with the interpreter.
pub fn compute_program(module: &[u8], params: &Value, fuel: u64) -> anyhow::Result<Value> {
    let p: program::UnitParams = serde_json::from_value(params.clone())?;
    let input = STANDARD.decode(p.input)?;
    let output = programs::run(module, &input, fuel)?;
    Ok(serde_json::to_value(program::UnitOutput {
        output: STANDARD.encode(output),
    })?)
}

/// Files units refer to by their hash (program modules, schools), fetched from the
/// coordinator the first time a unit needs one and checked against the hash.
struct Downloads {
    http: reqwest::Client,
    base: reqwest::Url,
    cache: HashMap<String, Arc<Vec<u8>>>,
}

impl Downloads {
    fn new(opts: &Options) -> anyhow::Result<Self> {
        let mut base = reqwest::Url::parse(&opts.url)?;
        let scheme = match base.scheme() {
            "ws" => "http",
            "wss" => "https",
            other => bail!("unsupported scheme {other}"),
        };
        base.set_scheme(scheme)
            .map_err(|()| anyhow::anyhow!("cannot use {scheme} for {}", opts.url))?;
        base.set_path("/api/");
        let mut http = reqwest::Client::builder();
        if let (Some(addr), Some(host)) = (opts.connect_to, base.host_str()) {
            http = http.resolve(host, addr);
        }
        Ok(Self {
            http: http.build()?,
            base,
            cache: HashMap::new(),
        })
    }

    /// `/api/{dir}/{sha256}{extension}`, checked against its hash as the browser does.
    async fn get(
        &mut self,
        dir: &str,
        sha256: &str,
        extension: &str,
    ) -> anyhow::Result<Arc<Vec<u8>>> {
        if let Some(bytes) = self.cache.get(sha256) {
            return Ok(bytes.clone());
        }
        let url = self.base.join(&format!("{dir}/{sha256}{extension}"))?;
        let bytes = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        if hex::encode(Sha256::digest(&bytes)) != sha256 {
            bail!("{dir}/{sha256} hash mismatch; refusing to run");
        }
        let bytes = Arc::new(bytes.to_vec());
        self.cache.insert(sha256.to_owned(), bytes.clone());
        Ok(bytes)
    }

    /// What a unit needs besides its params: a program's module, or a school.
    async fn input(
        &mut self,
        template: &TemplateRef,
        params: &Value,
    ) -> Option<anyhow::Result<Arc<Vec<u8>>>> {
        match template.id.as_str() {
            program::ID => Some(self.get("modules", &template.sha256, ".wasm").await),
            tt::ID => {
                let school = params
                    .get("school")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                Some(self.get("blobs", school, ".json").await)
            }
            _ => None,
        }
    }
}

/// Computes one timetable unit (spec 0013): a search over the school, from its seed.
pub fn compute_timetable(school: &[u8], params: &Value) -> anyhow::Result<Value> {
    let p: tt::UnitParams = serde_json::from_value(params.clone())?;
    let school: tt::School = serde_json::from_slice(school)?;
    let problem = timetable::Problem::new(&school).map_err(anyhow::Error::msg)?;
    let slots = timetable::solve(
        &problem,
        timetable::Search::unit(p.seed, p.index, p.iterations),
    );
    let cost = timetable::cost(&problem, &slots).map_err(anyhow::Error::msg)?;
    let bytes: Vec<u8> = slots.iter().flat_map(|s| s.to_le_bytes()).collect();
    Ok(serde_json::to_value(tt::UnitOutput {
        slots: STANDARD.encode(bytes),
        cost: cost.total,
    })?)
}

fn compute_template(template_id: &str, version: u32, params: &Value) -> anyhow::Result<Value> {
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
        (nn::ID, nn::VERSION) => {
            let p: nn::UnitParams = serde_json::from_value(params.clone())?;
            let (output, _) = network::train_unit(&p).map_err(anyhow::Error::msg)?;
            Ok(serde_json::to_value(output)?)
        }
        (ai::ID, ai::VERSION) => {
            let p: ai::UnitParams = serde_json::from_value(params.clone())?;
            Ok(serde_json::to_value(stand_in_answer(&p))?)
        }
        _ => bail!("unsupported template {template_id} v{version}"),
    }
}

/// What a simulated device answers for a language-model unit. It runs no model: the
/// answer is a pure function of the unit, so honest simulated devices agree with each other
/// as devices running the same model greedily do, and tests of verification, ordering and
/// scheduling need no weights. Browsers compute real answers (spec 0017).
pub fn stand_in_answer(p: &ai::UnitParams) -> ai::UnitOutput {
    let mut h = Sha256::new();
    for part in [
        p.model.as_str(),
        p.system.as_deref().unwrap_or(""),
        &p.prompt,
    ] {
        h.update(part.as_bytes());
        h.update([0]);
    }
    let digest = h.finalize();
    let n = (p.max_new_tokens as usize).min(8);
    let tokens: Vec<u32> = digest
        .chunks_exact(4)
        .take(n)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) % 50_000)
        .collect();
    ai::UnitOutput {
        text: format!("stand-in answer {}", hex_prefix(&digest)),
        margins: vec![1.0; tokens.len()],
        tokens,
    }
}

fn hex_prefix(bytes: &[u8]) -> String {
    bytes.iter().take(6).map(|b| format!("{b:02x}")).collect()
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
                max_buffer_size: opts.gpu_buffer,
            },
            client_id: Some(opts.client_id.unwrap_or_else(Uuid::new_v4)),
            join_code: opts.join_code.clone(),
            community: (!opts.community).then_some(false),
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
    let mut downloads = Downloads::new(&opts)?;
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
                if opts.ai_unfit && template.id == ai::ID {
                    let msg = WorkerMessage::Failed {
                        lease_id,
                        reason: "the model's check came out wrong on this device".into(),
                        unfit: true,
                    };
                    held.insert(lease_id, Instant::now());
                    send(&mut ws, &mut stats, msg).await?;
                    owed += 1;
                    continue;
                }
                let started = Instant::now();
                let cheat = opts.cheat;
                let fuel = opts.fuel;
                let input = downloads.input(&template, &params).await;
                let computed = tokio::task::spawn_blocking(move || {
                    match (cheat.falsify(&template.id, &params), input) {
                        (Some(fake), _) => Ok(fake),
                        (None, Some(input)) if template.id == tt::ID => {
                            compute_timetable(&input?, &params)
                        }
                        (None, Some(input)) => compute_program(&input?, &params, fuel),
                        (None, None) => compute(&template.id, template.version, &params),
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
                        unfit: false,
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

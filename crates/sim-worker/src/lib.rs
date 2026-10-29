//! Headless simulated worker: speaks the real worker protocol and computes
//! units with the CPU reference kernels. Used for tests, load generation and
//! fault injection.

use anyhow::{Context, bail};
use futures_util::{SinkExt, StreamExt};
use protocol::{
    Backend, CoordinatorMessage, DeviceInfo, PROTOCOL_VERSION, WorkerMessage,
    templates::{pi, pt},
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Options {
    /// Coordinator worker endpoint, e.g. `ws://localhost:7700/ws/worker`.
    pub url: String,
    /// Disconnect after this many completed units.
    pub max_units: Option<u32>,
    /// Disconnect the first time the queue is empty instead of polling.
    pub exit_when_idle: bool,
    /// The client id to report; `None` makes a new one per run.
    pub client_id: Option<Uuid>,
    /// Fault injection: return wrong results.
    pub cheat: Cheat,
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub completed: u32,
    pub accepted: u32,
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

pub async fn run(opts: Options) -> anyhow::Result<Stats> {
    let (mut ws, _) = connect_async(&opts.url)
        .await
        .with_context(|| format!("connecting to {}", opts.url))?;

    let send = |msg: WorkerMessage| Message::text(serde_json::to_string(&msg).unwrap());
    ws.send(send(WorkerMessage::Hello {
        protocol_version: PROTOCOL_VERSION,
        client_version: concat!("sim-worker-", env!("CARGO_PKG_VERSION")).into(),
        device: DeviceInfo {
            backend: Backend::Native,
            vendor: "cpu".into(),
            architecture: std::env::consts::ARCH.into(),
        },
        client_id: Some(opts.client_id.unwrap_or_else(Uuid::new_v4)),
    }))
    .await?;

    let mut stats = Stats::default();
    ws.send(send(WorkerMessage::RequestWork)).await?;

    while let Some(frame) = ws.next().await {
        let text = match frame? {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };
        match serde_json::from_str::<CoordinatorMessage>(&text)? {
            CoordinatorMessage::Welcome { .. } => {}
            CoordinatorMessage::Assign {
                lease_id,
                template,
                params,
                ..
            } => {
                let started = Instant::now();
                let computed = match opts.cheat.falsify(&template.id, &params) {
                    Some(fake) => Ok(fake),
                    None => compute(&template.id, template.version, &params),
                };
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
                ws.send(send(msg)).await?;
            }
            CoordinatorMessage::Ack { accepted, .. } => {
                stats.completed += 1;
                stats.accepted += u32::from(accepted);
                if opts.max_units.is_some_and(|max| stats.completed >= max) {
                    break;
                }
                ws.send(send(WorkerMessage::RequestWork)).await?;
            }
            CoordinatorMessage::NoWork { retry_after_ms } => {
                if opts.exit_when_idle {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(retry_after_ms.into())).await;
                ws.send(send(WorkerMessage::RequestWork)).await?;
            }
            CoordinatorMessage::Error { message } => bail!("coordinator error: {message}"),
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

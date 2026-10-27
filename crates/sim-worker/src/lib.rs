//! Headless simulated worker: speaks the real worker protocol and computes
//! units with the CPU reference kernels. Used for tests, load generation and
//! fault injection.

use anyhow::{Context, bail};
use futures_util::{SinkExt, StreamExt};
use protocol::{
    Backend, CoordinatorMessage, DeviceInfo, PROTOCOL_VERSION, WorkerMessage, templates::pi,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, Clone)]
pub struct Options {
    /// Coordinator worker endpoint, e.g. `ws://localhost:7700/ws/worker`.
    pub url: String,
    /// Disconnect after this many completed units.
    pub max_units: Option<u32>,
    /// Disconnect the first time the queue is empty instead of polling.
    pub exit_when_idle: bool,
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
                let msg = match compute(&template.id, template.version, &params) {
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

/// Convenience for building a pi job body.
pub fn pi_job(units: u32, samples_per_unit: u32, seed: u32) -> Value {
    json!({ "template": pi::ID, "params": { "units": units, "samples_per_unit": samples_per_unit, "seed": seed } })
}

//! WebSocket endpoints for workers and dashboards.

use crate::{
    db::{self, Completion},
    state::AppState,
};
use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use protocol::{
    CoordinatorMessage, DashboardEvent, PROTOCOL_VERSION, WorkerMessage, WorkerSummary,
};
use std::{collections::HashSet, time::Duration};
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;

/// Worker messages are small; anything larger is abuse.
const MAX_WORKER_MESSAGE_BYTES: usize = 64 * 1024;

pub async fn worker(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.max_message_size(MAX_WORKER_MESSAGE_BYTES)
        .on_upgrade(move |socket| worker_session(socket, state))
}

pub async fn dashboard(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| dashboard_session(socket, state))
}

enum Incoming {
    Message(WorkerMessage),
    Closed,
    Invalid(String),
}

async fn recv(socket: &mut WebSocket, timeout: Duration) -> Incoming {
    loop {
        let msg = match tokio::time::timeout(timeout, socket.recv()).await {
            Err(_) => return Incoming::Invalid("idle timeout".into()),
            Ok(None | Some(Err(_)) | Some(Ok(Message::Close(_)))) => return Incoming::Closed,
            Ok(Some(Ok(msg))) => msg,
        };
        return match msg {
            Message::Text(text) => match serde_json::from_str(&text) {
                Ok(m) => Incoming::Message(m),
                Err(e) => Incoming::Invalid(format!("malformed message: {e}")),
            },
            Message::Ping(_) | Message::Pong(_) => continue,
            Message::Binary(_) => Incoming::Invalid("binary messages are not supported".into()),
            Message::Close(_) => Incoming::Closed,
        };
    }
}

async fn send<T: serde::Serialize>(socket: &mut WebSocket, msg: &T) -> bool {
    let text = serde_json::to_string(msg).expect("protocol types always serialize");
    socket.send(Message::Text(text.into())).await.is_ok()
}

async fn reject(socket: &mut WebSocket, message: impl Into<String>) {
    let message = message.into();
    tracing::debug!(%message, "rejecting worker");
    send(socket, &CoordinatorMessage::Error { message }).await;
    let _ = socket.send(Message::Close(None)).await;
}

async fn worker_session(mut socket: WebSocket, state: AppState) {
    let cfg = &state.config;
    let (client_version, device) = match recv(&mut socket, cfg.idle_timeout).await {
        Incoming::Message(WorkerMessage::Hello {
            protocol_version,
            client_version,
            device,
        }) => {
            if protocol_version != PROTOCOL_VERSION {
                return reject(
                    &mut socket,
                    format!("protocol version {protocol_version} unsupported, expected {PROTOCOL_VERSION}"),
                )
                .await;
            }
            (client_version, device)
        }
        Incoming::Message(_) => return reject(&mut socket, "expected hello").await,
        Incoming::Invalid(e) => return reject(&mut socket, e).await,
        Incoming::Closed => return,
    };

    let worker_id = Uuid::now_v7();
    let connected_at = match db::insert_worker(&state.db, worker_id, &device, &client_version).await
    {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "failed to register worker");
            return reject(&mut socket, "internal error").await;
        }
    };
    let summary = WorkerSummary {
        id: worker_id,
        device,
        connected_at: db::unix_ms(connected_at),
        units_done: 0,
    };
    state
        .workers
        .lock()
        .unwrap()
        .insert(worker_id, summary.clone());
    state.publish(DashboardEvent::WorkerJoined { worker: summary });
    tracing::info!(%worker_id, %client_version, "worker joined");

    let welcome = CoordinatorMessage::Welcome {
        worker_id,
        heartbeat_interval_ms: cfg.heartbeat_interval.as_millis() as u32,
    };
    if send(&mut socket, &welcome).await
        && let Err(e) = worker_loop(&mut socket, &state, worker_id).await
    {
        tracing::error!(%worker_id, error = %e, "worker session failed");
    }

    // Whatever the reason the session ended, give its units back to the queue.
    match db::release_worker_leases(&state.db, worker_id).await {
        Ok(0) => {}
        Ok(n) => tracing::info!(%worker_id, released = n, "released leases of departed worker"),
        Err(e) => tracing::error!(%worker_id, error = %e, "failed to release leases"),
    }
    if let Err(e) = db::mark_worker_disconnected(&state.db, worker_id).await {
        tracing::error!(%worker_id, error = %e, "failed to mark worker disconnected");
    }
    state.workers.lock().unwrap().remove(&worker_id);
    state.publish(DashboardEvent::WorkerLeft { worker_id });
    tracing::info!(%worker_id, "worker left");
}

async fn worker_loop(
    socket: &mut WebSocket,
    state: &AppState,
    worker_id: Uuid,
) -> Result<(), sqlx::Error> {
    let cfg = &state.config;
    let mut active: HashSet<Uuid> = HashSet::new();

    loop {
        let msg = match recv(socket, cfg.idle_timeout).await {
            Incoming::Message(m) => m,
            Incoming::Closed => return Ok(()),
            Incoming::Invalid(e) => {
                reject(socket, e).await;
                return Ok(());
            }
        };

        let reply = match msg {
            WorkerMessage::Hello { .. } => {
                reject(socket, "duplicate hello").await;
                return Ok(());
            }

            WorkerMessage::RequestWork => {
                if active.len() >= cfg.max_leases_per_worker {
                    reject(socket, "lease limit reached").await;
                    return Ok(());
                }
                match db::claim_unit(&state.db, worker_id, cfg.lease_ttl).await? {
                    Some(claim) => {
                        active.insert(claim.lease_id);
                        if let Some(job) = claim.job_started {
                            state.publish(DashboardEvent::JobUpdated { job });
                        }
                        tracing::debug!(%worker_id, lease_id = %claim.lease_id, job_id = %claim.job_id, "unit assigned");
                        CoordinatorMessage::Assign {
                            lease_id: claim.lease_id,
                            unit_id: claim.unit_id,
                            template: claim.template,
                            params: claim.params,
                            lease_expires_at: db::unix_ms(claim.expires_at),
                        }
                    }
                    None => CoordinatorMessage::NoWork {
                        retry_after_ms: cfg.no_work_retry.as_millis() as u32,
                    },
                }
            }

            WorkerMessage::Heartbeat { lease_ids } => {
                let owned: Vec<Uuid> = lease_ids
                    .into_iter()
                    .filter(|id| active.contains(id))
                    .collect();
                db::heartbeat(&state.db, worker_id, &owned, cfg.lease_ttl).await?;
                continue;
            }

            WorkerMessage::Result {
                lease_id,
                output,
                elapsed_ms,
            } => {
                let accepted = active.remove(&lease_id)
                    && handle_completion(state, worker_id, lease_id, &output, elapsed_ms).await?;
                CoordinatorMessage::Ack { lease_id, accepted }
            }

            WorkerMessage::Failed { lease_id, reason } => {
                if active.remove(&lease_id) {
                    tracing::warn!(%worker_id, %lease_id, %reason, "worker failed a unit");
                    db::release_lease(&state.db, worker_id, lease_id).await?;
                }
                CoordinatorMessage::Ack {
                    lease_id,
                    accepted: false,
                }
            }
        };

        if !send(socket, &reply).await {
            return Ok(());
        }
    }
}

async fn handle_completion(
    state: &AppState,
    worker_id: Uuid,
    lease_id: Uuid,
    output: &serde_json::Value,
    elapsed_ms: u32,
) -> Result<bool, sqlx::Error> {
    match db::complete_lease(&state.db, worker_id, lease_id, output, elapsed_ms).await? {
        Completion::Accepted {
            job_id,
            unit_id,
            unit_params,
            units_done,
            units_total,
            job_finished,
        } => {
            state
                .renders
                .unit_accepted(job_id, unit_id, &unit_params, output)
                .await;
            if let Some(w) = state.workers.lock().unwrap().get_mut(&worker_id) {
                w.units_done += 1;
            }
            state.publish(DashboardEvent::UnitCompleted {
                job_id,
                unit_id,
                worker_id,
                units_done,
                units_total,
            });
            if let Some(job) = job_finished {
                tracing::info!(%job_id, status = ?job.status, "job finished");
                state.publish(DashboardEvent::JobUpdated { job });
            }
            Ok(true)
        }
        Completion::InvalidOutput(reason) => {
            tracing::warn!(%worker_id, %lease_id, %reason, "rejected invalid output");
            Ok(false)
        }
        Completion::StaleLease | Completion::AlreadyDone => Ok(false),
    }
}

async fn dashboard_session(mut socket: WebSocket, state: AppState) {
    // Subscribe before taking the snapshot so no event falls in between.
    let mut events = state.events.subscribe();
    if !send_snapshot(&mut socket, &state).await {
        return;
    }
    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                Some(Ok(_)) => {}
            },
            event = events.recv() => {
                let sent = match event {
                    Ok(event) => send(&mut socket, &event).await,
                    // Too slow to keep up: resynchronise with a fresh snapshot.
                    Err(RecvError::Lagged(_)) => send_snapshot(&mut socket, &state).await,
                    Err(RecvError::Closed) => return,
                };
                if !sent {
                    return;
                }
            }
        }
    }
}

async fn send_snapshot(socket: &mut WebSocket, state: &AppState) -> bool {
    let jobs = match db::recent_jobs(&state.db, 50).await {
        Ok(jobs) => jobs,
        Err(e) => {
            tracing::error!(error = %e, "failed to load jobs for snapshot");
            return false;
        }
    };
    send(
        socket,
        &DashboardEvent::Snapshot {
            workers: state.worker_snapshot(),
            jobs,
        },
    )
    .await
}

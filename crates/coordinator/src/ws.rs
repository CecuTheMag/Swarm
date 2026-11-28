//! WebSocket endpoints: workers, network dashboards, and the summary a joined device
//! sees of its network.

use crate::{
    api::AppError,
    auth::{self, PUBLIC_NETWORK},
    db::{self, Completion, DeviceRecord, Registration},
    networks::{self, NetworkRules},
    state::{AppState, Disruption, SessionCommand},
};
use axum::{
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, Method},
    response::{IntoResponse, Response},
};
use jiff::Timestamp;
use protocol::{
    CoordinatorMessage, DashboardEvent, DeviceControl, PROTOCOL_VERSION, WorkerMessage,
    WorkerSummary, api::Role,
};
use std::{collections::HashSet, time::Duration};
use tokio::sync::{broadcast::error::RecvError, mpsc};
use uuid::Uuid;

/// Sent to clients banned for wrong results (spec 0004).
const BLOCKED: &str = "this device is blocked: too many wrong results";

/// The largest worker message is a path-tracer result for the largest tile (base64 of
/// `MAX_TILE² × 3` f32, about 262 KiB); anything far beyond that is abuse.
pub const MAX_WORKER_MESSAGE_BYTES: usize = 512 * 1024;

#[utoipa::path(
    operation_id = "ws_worker",
    get,
    path = "/ws/worker",
    tag = "websocket",
    responses((status = 101, description = "WebSocket for a device: work units and results (protocol::WorkerMessage)")),
    security(())
)]
pub async fn worker(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.max_message_size(MAX_WORKER_MESSAGE_BYTES)
        .on_upgrade(move |socket| worker_session(socket, state))
}

/// The public network's live events, open to anyone (the home page shows them).
#[utoipa::path(
    operation_id = "ws_dashboard",
    get,
    path = "/ws/dashboard",
    tag = "websocket",
    responses((status = 101, description = "WebSocket of the public network's live events (protocol::DashboardEvent)")),
    security(())
)]
pub async fn dashboard(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| dashboard_session(socket, state, PUBLIC_NETWORK))
}

/// A network's live events, for its members (spec 0010).
#[utoipa::path(
    operation_id = "ws_network_live",
    get,
    path = "/ws/networks/{id}/live",
    tag = "websocket",
    params(("id" = uuid::Uuid, Path, description = "Network")),
    responses((status = 101, description = "WebSocket of a network's live events, for its members"), (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError)),
    security(("session" = []), ("api_key" = []))
)]
pub async fn network_live(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(network_id): Path<Uuid>,
) -> Result<Response, AppError> {
    auth::network_access(&state, &method, &headers, network_id, Role::Viewer).await?;
    Ok(ws.on_upgrade(move |socket| dashboard_session(socket, state, network_id)))
}

/// What a device that joined a network sees of it, once a second (spec 0010).
#[utoipa::path(
    operation_id = "ws_join_summary",
    get,
    path = "/ws/join/{code}/summary",
    tag = "websocket",
    params(("code" = String, Path, description = "Join code")),
    responses((status = 101, description = "WebSocket of what a joined device sees of its network"), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError)),
    security(())
)]
pub async fn join_summary(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Response, AppError> {
    let rules = networks::by_join_code(&state.db, &code)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(ws
        .on_upgrade(move |socket| summary_session(socket, state, rules.id, rules.name))
        .into_response())
}

async fn summary_session(mut socket: WebSocket, state: AppState, network_id: Uuid, name: String) {
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                Some(Ok(_)) => {}
            },
            _ = tick.tick() => {
                let summary = match state.network_summary(network_id, &name).await {
                    Ok(summary) => summary,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to build a network summary");
                        return;
                    }
                };
                if !send(&mut socket, &summary).await {
                    return;
                }
            }
        }
    }
}

enum Incoming {
    Message(WorkerMessage),
    Closed,
    Invalid(String),
}

/// The next message from the worker, or `Invalid` if none comes within `timeout`.
/// WebSocket pings and pongs do not count: a proxy or library may exchange them on a
/// connection whose worker is long gone, and would keep its units from the queue.
async fn recv(socket: &mut WebSocket, timeout: Duration) -> Incoming {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let msg = match tokio::time::timeout_at(deadline, socket.recv()).await {
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
    let (client_version, device, client_id, join_code, community) = match recv(
        &mut socket,
        cfg.idle_timeout,
    )
    .await
    {
        Incoming::Message(WorkerMessage::Hello {
            protocol_version,
            client_version,
            device,
            client_id,
            join_code,
            community,
        }) => {
            if protocol_version != PROTOCOL_VERSION {
                return reject(
                        &mut socket,
                        format!("protocol version {protocol_version} unsupported, expected {PROTOCOL_VERSION}"),
                    )
                    .await;
            }
            // Without an id of its own, the connection is a new client.
            (
                client_version,
                device,
                client_id.unwrap_or_else(Uuid::now_v7),
                join_code,
                community.unwrap_or(true),
            )
        }
        Incoming::Message(_) => return reject(&mut socket, "expected hello").await,
        Incoming::Invalid(e) => return reject(&mut socket, e).await,
        Incoming::Closed => return,
    };

    // The network of the join link, or the public one.
    let rules = match &join_code {
        Some(code) => networks::by_join_code(&state.db, code).await,
        None => networks::by_id(&state.db, PUBLIC_NETWORK).await,
    };
    let rules = match rules {
        Ok(Some(rules)) => rules,
        Ok(None) => return reject(&mut socket, UNKNOWN_NETWORK).await,
        Err(e) => {
            tracing::error!(error = %e, "failed to look up the network");
            return reject(&mut socket, "internal error").await;
        }
    };

    let worker_id = Uuid::now_v7();
    let approval = rules.join_mode == protocol::api::JoinMode::Approval;
    let registration = db::insert_worker(
        &state.db,
        worker_id,
        &device,
        &client_version,
        client_id,
        db::Joining {
            network_id: rules.id,
            approval,
            community,
        },
    )
    .await;
    let (connected_at, device_record) = match registration {
        Ok(Registration::Registered {
            connected_at,
            device,
        }) => (connected_at, device),
        Ok(Registration::Banned) => return reject(&mut socket, BLOCKED).await,
        Ok(Registration::Removed) => return reject(&mut socket, REMOVED).await,
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
    let mut commands = state.commands_of(worker_id);
    let network_id = rules.id;
    state.worker_joined(summary, client_id, network_id, device_record.id);
    tracing::info!(%worker_id, %client_id, %network_id, %client_version, "worker joined");

    let welcome = CoordinatorMessage::Welcome {
        worker_id,
        heartbeat_interval_ms: cfg.heartbeat_interval.as_millis() as u32,
    };
    let mut session = Session {
        worker_id,
        rules,
        device: device_record,
        control: DeviceControl::Running,
    };
    if send(&mut socket, &welcome).await
        && let Err(e) = worker_loop(&mut socket, &state, &mut session, &mut commands).await
    {
        tracing::error!(%worker_id, error = %e, "worker session failed");
    }

    // Whatever the reason the session ended, give its units back to the queue.
    let mut units_returned = 0;
    match db::retrying(|| db::release_worker_leases(&state.db, worker_id, &state.config)).await {
        Ok(released) => {
            units_returned = released.leases as u32;
            if released.leases > 0 {
                tracing::info!(%worker_id, released = released.leases, "released leases of departed worker");
            }
            state.publish_released(released);
        }
        Err(e) => tracing::error!(%worker_id, error = %e, "failed to release leases"),
    }
    if let Err(e) = db::mark_worker_disconnected(&state.db, worker_id).await {
        tracing::error!(%worker_id, error = %e, "failed to mark worker disconnected");
    }
    state.worker_left(worker_id, units_returned);
    tracing::info!(%worker_id, "worker left");
}

/// Sent to devices whose join link names no network (any more).
const UNKNOWN_NETWORK: &str = "this network does not exist: the link may be out of date";
/// Sent to devices removed from their network by its owner.
const REMOVED: &str = "this device was removed from the network";

/// A worker's network and device, and what they allow it.
struct Session {
    worker_id: Uuid,
    rules: NetworkRules,
    device: DeviceRecord,
    control: DeviceControl,
}

impl Session {
    /// What the device may do now; hours open and close as time passes.
    fn current(&self) -> DeviceControl {
        networks::control(&self.rules, &self.device, Timestamp::now())
    }

    /// Reloads the network and the device after the console changed them. A network
    /// or device that no longer exists means the device was removed.
    async fn reload(&mut self, state: &AppState) -> Result<(), sqlx::Error> {
        match networks::by_id(&state.db, self.rules.id).await? {
            Some(rules) => self.rules = rules,
            None => self.device.status = "removed".into(),
        }
        match db::device_record(&state.db, self.device.id).await? {
            Some(device) => self.device = device,
            None => self.device.status = "removed".into(),
        }
        Ok(())
    }

    fn message(&self) -> CoordinatorMessage {
        CoordinatorMessage::Control {
            control: self.control.clone(),
            max_power: self.rules.max_power,
        }
    }
}

/// Tells the device if what it may do changed (or `force`). Returns false when the
/// connection is gone.
async fn sync_control(socket: &mut WebSocket, session: &mut Session, force: bool) -> bool {
    let now = session.current();
    if !force && now == session.control {
        return true;
    }
    if now != session.control {
        tracing::debug!(worker_id = %session.worker_id, control = ?now, "device control changed");
    }
    session.control = now;
    send(socket, &session.message()).await
}

/// The next command from the chaos panel or the console; never, once none can come.
async fn next_command(
    commands: &mut Option<&mut mpsc::UnboundedReceiver<SessionCommand>>,
) -> SessionCommand {
    if let Some(rx) = commands {
        if let Some(command) = rx.recv().await {
            return command;
        }
        *commands = None;
    }
    std::future::pending().await
}

async fn worker_loop(
    socket: &mut WebSocket,
    state: &AppState,
    session: &mut Session,
    commands: &mut mpsc::UnboundedReceiver<SessionCommand>,
) -> Result<(), sqlx::Error> {
    let cfg = &state.config;
    let worker_id = session.worker_id;
    let mut active: HashSet<Uuid> = HashSet::new();
    let mut commands = Some(commands);
    let mut max_power = session.rules.max_power;
    if !sync_control(socket, session, true).await {
        return Ok(());
    }

    loop {
        let incoming = tokio::select! {
            incoming = recv(socket, cfg.idle_timeout) => incoming,
            command = next_command(&mut commands) => {
                match command {
                    SessionCommand::Disrupt(disruption) => {
                        tracing::info!(%worker_id, ?disruption, "worker disrupted by the chaos panel");
                        match disruption {
                            Disruption::Kill => {
                                let _ = socket.send(Message::Close(None)).await;
                            }
                            // Nothing more is read or sent; the session ends as a silent one does.
                            Disruption::CutOff => tokio::time::sleep(cfg.idle_timeout).await,
                        }
                        return Ok(());
                    }
                    SessionCommand::Refresh => {
                        session.reload(state).await?;
                        let force = session.rules.max_power != max_power;
                        max_power = session.rules.max_power;
                        if !sync_control(socket, session, force).await {
                            return Ok(());
                        }
                        if session.control == DeviceControl::Removed {
                            let _ = socket.send(Message::Close(None)).await;
                            return Ok(());
                        }
                        continue;
                    }
                }
            }
        };
        let msg = match incoming {
            Incoming::Message(m) => m,
            Incoming::Closed => return Ok(()),
            Incoming::Invalid(e) => {
                reject(socket, e).await;
                return Ok(());
            }
        };
        // Banned through another connection's result (an agreeing cheater, or the same
        // client on two devices).
        if state.is_banned(worker_id) {
            reject(socket, BLOCKED).await;
            return Ok(());
        }
        // Working hours open and close as time passes.
        if !sync_control(socket, session, false).await {
            return Ok(());
        }

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
                let claim = if session.control == DeviceControl::Running {
                    let (network, lend) = (session.rules.id, session.rules.lend_idle);
                    db::retrying(|| db::claim_unit(&state.db, worker_id, network, lend, cfg))
                        .await?
                } else {
                    None
                };
                match claim {
                    Some(claim) => {
                        active.insert(claim.lease_id);
                        if let Some(job) = claim.job_started {
                            state.publish(job.network_id, DashboardEvent::JobUpdated { job });
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
                db::retrying(|| db::heartbeat(&state.db, worker_id, &owned, cfg)).await?;
                continue;
            }

            WorkerMessage::Result {
                lease_id,
                output,
                elapsed_ms,
            } => {
                let (accepted, banned) = if active.remove(&lease_id) {
                    handle_completion(state, worker_id, lease_id, &output, elapsed_ms).await?
                } else {
                    (false, false)
                };
                if banned {
                    reject(socket, BLOCKED).await;
                    return Ok(());
                }
                CoordinatorMessage::Ack { lease_id, accepted }
            }

            WorkerMessage::Failed {
                lease_id,
                reason,
                unfit,
            } => {
                if active.remove(&lease_id) {
                    let released = if unfit {
                        tracing::warn!(%worker_id, %lease_id, %reason, "worker is unfit for a job");
                        db::retrying(|| db::release_unfit(&state.db, worker_id, lease_id)).await?
                    } else {
                        tracing::warn!(%worker_id, %lease_id, %reason, "worker failed a unit");
                        db::retrying(|| db::release_lease(&state.db, worker_id, lease_id, cfg))
                            .await?
                    };
                    state.publish_released(released);
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

/// Tells the render cache, the progress counters and the dashboards that `completion`
/// decided a unit, and that it finished its job if it did. Returns whether the worker
/// that produced the accepted result looks suspicious (its result was not in the
/// agreeing group). Anything but `Completion::Decided` announces nothing.
pub async fn announce_decided(state: &AppState, worker_id: Uuid, completion: Completion) -> bool {
    let Completion::Decided {
        job_id,
        network_id,
        unit_id,
        unit_params,
        output,
        agreed,
        units_done,
        units_total,
        job_finished,
        ..
    } = completion
    else {
        return false;
    };
    state
        .renders
        .unit_accepted(job_id, unit_id, &unit_params, &output)
        .await;
    if let Some(w) = state.workers.lock().unwrap().get_mut(&worker_id) {
        w.units_done += 1;
    }
    state.unit_decided(network_id, job_id, worker_id, units_done, units_total);
    if let Some(job) = job_finished {
        tracing::info!(%job_id, status = ?job.status, "job finished");
        state.publish(network_id, DashboardEvent::JobUpdated { job });
    }
    !agreed
}

/// Records a result and announces what it changed. Returns whether the worker is told
/// its result was accepted, and whether its client got banned by it.
async fn handle_completion(
    state: &AppState,
    worker_id: Uuid,
    lease_id: Uuid,
    output: &serde_json::Value,
    elapsed_ms: u32,
) -> Result<(bool, bool), sqlx::Error> {
    let completion = db::retrying(|| {
        db::complete_lease(
            &state.db,
            worker_id,
            lease_id,
            output,
            elapsed_ms,
            &state.config,
        )
    })
    .await?;
    let accepted = completion.accepted();

    if let Some(bans) = completion.bans() {
        state.clients_banned(&bans.clients);
    }
    // A ban reopened units in these jobs: their images and progress changed.
    for job_id in completion.reopened() {
        state.renders.forget(*job_id).await;
        if let Some(job) = db::get_job(&state.db, *job_id).await? {
            tracing::warn!(%job_id, units_done = job.units_done, "reopened units of a banned client");
            state.publish(job.network_id, DashboardEvent::JobUpdated { job });
        }
    }

    let suspicious = match completion {
        decided @ Completion::Decided { .. } => announce_decided(state, worker_id, decided).await,
        Completion::Unresolved { job } => {
            if let Some(job) = job {
                tracing::warn!(job_id = %job.id, error = job.error.as_deref().unwrap_or(""), "job failed");
                state.publish(job.network_id, DashboardEvent::JobUpdated { job });
            }
            false
        }
        Completion::Honeypot { correct, .. } => {
            if !correct {
                tracing::warn!(%worker_id, %lease_id, "wrong answer to a known-answer unit");
            }
            !correct
        }
        Completion::Late { agreed, .. } => !agreed,
        Completion::InvalidOutput {
            reason, released, ..
        } => {
            tracing::warn!(%worker_id, %lease_id, %reason, "rejected invalid output");
            state.publish_released(released);
            true
        }
        Completion::Recorded | Completion::Disputed { .. } | Completion::StaleLease => false,
    };
    let banned = suspicious && db::worker_banned(&state.db, worker_id).await?;
    Ok((accepted, banned))
}

async fn dashboard_session(mut socket: WebSocket, state: AppState, network_id: Uuid) {
    // Subscribe before taking the snapshot so no event falls in between.
    let mut events = state.events.subscribe();
    if !send_snapshot(&mut socket, &state, network_id).await {
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
                    Ok((network, _)) if network != network_id => true,
                    Ok((_, event)) => send(&mut socket, &event).await,
                    // Too slow to keep up: resynchronise with a fresh snapshot.
                    Err(RecvError::Lagged(_)) => send_snapshot(&mut socket, &state, network_id).await,
                    Err(RecvError::Closed) => return,
                };
                if !sent {
                    return;
                }
            }
        }
    }
}

async fn send_snapshot(socket: &mut WebSocket, state: &AppState, network_id: Uuid) -> bool {
    let jobs = match db::recent_jobs(&state.db, network_id, 50).await {
        Ok(jobs) => jobs,
        Err(e) => {
            tracing::error!(error = %e, "failed to load jobs for snapshot");
            return false;
        }
    };
    send(
        socket,
        &DashboardEvent::Snapshot {
            workers: state.worker_snapshot(network_id),
            jobs,
        },
    )
    .await
}

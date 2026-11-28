//! Resilience tests (spec 0008): a network's admins switch devices off, cut them off,
//! or add cheaters, live, and watch the network cope. `/api/chaos/*` acts on the public
//! network; `/api/networks/{id}/chaos/*` on any network (spec 0010).

use crate::{
    api::AppError,
    auth::{self, PUBLIC_NETWORK},
    flags, networks,
    state::{AppState, Disruption},
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, Method, StatusCode},
};
use protocol::{AddCheaters, api::Role};
use sim_worker::{Cheat, Options};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use uuid::Uuid;

/// Cheaters added by the panel that may run at once.
pub const MAX_CHEATERS: usize = 6;
/// How long a unit takes a panel cheater, so the room sees it join before it is caught.
const CHEATER_UNIT_TIME: Duration = Duration::from_secs(1);
/// A cheater that is never caught (no job running) leaves after this long.
const CHEATER_LIFETIME: Duration = Duration::from_secs(300);

/// Switches a device of the public network off.
#[utoipa::path(
    operation_id = "chaos_kill_worker",
    post,
    path = "/api/chaos/workers/{id}/kill",
    tag = "chaos",
    params(("id" = uuid::Uuid, Path, description = "Device connection")),
    responses((status = 204, description = "Switched off"), (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError), (status = 403, description = "Role too low, or a change from another site", body = protocol::ApiError), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError), (status = 409, description = "Conflict with the current state", body = protocol::ApiError)),
    security(("session" = []), ("api_key" = []))
)]
pub async fn kill_worker(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(worker): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    disrupt(
        &state,
        &method,
        &headers,
        PUBLIC_NETWORK,
        worker,
        Disruption::Kill,
    )
    .await
}

/// Cuts a device of the public network off, as if it lost its signal.
#[utoipa::path(
    operation_id = "chaos_cut_off_worker",
    post,
    path = "/api/chaos/workers/{id}/cut",
    tag = "chaos",
    params(("id" = uuid::Uuid, Path, description = "Device connection")),
    responses((status = 204, description = "Cut off"), (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError), (status = 403, description = "Role too low, or a change from another site", body = protocol::ApiError), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError), (status = 409, description = "Conflict with the current state", body = protocol::ApiError)),
    security(("session" = []), ("api_key" = []))
)]
pub async fn cut_off_worker(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(worker): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    disrupt(
        &state,
        &method,
        &headers,
        PUBLIC_NETWORK,
        worker,
        Disruption::CutOff,
    )
    .await
}

#[utoipa::path(
    operation_id = "chaos_add_cheaters",
    post,
    path = "/api/chaos/cheaters",
    tag = "chaos",
    request_body = protocol::AddCheaters,
    responses((status = 202, description = "The cheaters are joining"), (status = 400, description = "Invalid request", body = protocol::ApiError), (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError), (status = 403, description = "Role too low, or a change from another site", body = protocol::ApiError), (status = 409, description = "Conflict with the current state", body = protocol::ApiError)),
    security(("session" = []), ("api_key" = []))
)]
pub async fn add_cheaters(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Json(body): Json<AddCheaters>,
) -> Result<StatusCode, AppError> {
    cheaters(&state, &method, &headers, PUBLIC_NETWORK, body).await
}

#[utoipa::path(
    operation_id = "chaos_network_kill",
    post,
    path = "/api/networks/{id}/chaos/workers/{worker}/kill",
    tag = "chaos",
    params(("id" = uuid::Uuid, Path, description = "Network"), ("worker" = uuid::Uuid, Path, description = "Device connection")),
    responses((status = 204, description = "Switched off"), (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError), (status = 403, description = "Role too low, or a change from another site", body = protocol::ApiError), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError), (status = 409, description = "Conflict with the current state", body = protocol::ApiError)),
    security(("session" = []), ("api_key" = []))
)]
pub async fn network_kill(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((network, worker)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    disrupt(&state, &method, &headers, network, worker, Disruption::Kill).await
}

#[utoipa::path(
    operation_id = "chaos_network_cut_off",
    post,
    path = "/api/networks/{id}/chaos/workers/{worker}/cut",
    tag = "chaos",
    params(("id" = uuid::Uuid, Path, description = "Network"), ("worker" = uuid::Uuid, Path, description = "Device connection")),
    responses((status = 204, description = "Cut off"), (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError), (status = 403, description = "Role too low, or a change from another site", body = protocol::ApiError), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError), (status = 409, description = "Conflict with the current state", body = protocol::ApiError)),
    security(("session" = []), ("api_key" = []))
)]
pub async fn network_cut_off(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((network, worker)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    disrupt(
        &state,
        &method,
        &headers,
        network,
        worker,
        Disruption::CutOff,
    )
    .await
}

#[utoipa::path(
    operation_id = "chaos_network_cheaters",
    post,
    path = "/api/networks/{id}/chaos/cheaters",
    tag = "chaos",
    params(("id" = uuid::Uuid, Path, description = "Network")),
    request_body = protocol::AddCheaters,
    responses((status = 202, description = "The cheaters are joining"), (status = 400, description = "Invalid request", body = protocol::ApiError), (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError), (status = 403, description = "Role too low, or a change from another site", body = protocol::ApiError), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError), (status = 409, description = "Conflict with the current state", body = protocol::ApiError)),
    security(("session" = []), ("api_key" = []))
)]
pub async fn network_cheaters(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(network): Path<Uuid>,
    Json(body): Json<AddCheaters>,
) -> Result<StatusCode, AppError> {
    cheaters(&state, &method, &headers, network, body).await
}

async fn disrupt(
    state: &AppState,
    method: &Method,
    headers: &HeaderMap,
    network: Uuid,
    worker: Uuid,
    disruption: Disruption,
) -> Result<StatusCode, AppError> {
    auth::network_access(state, method, headers, network, Role::Admin).await?;
    flags::require(&state.db, flags::CHAOS_PANEL, "resilience tests").await?;
    if state.disrupt(network, worker, disruption) {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

/// Adds simulated devices that make up every result. They connect to this coordinator
/// like any device and go through the same verification, so catching them is real.
async fn cheaters(
    state: &AppState,
    method: &Method,
    headers: &HeaderMap,
    network: Uuid,
    body: AddCheaters,
) -> Result<StatusCode, AppError> {
    auth::network_access(state, method, headers, network, Role::Admin).await?;
    flags::require(&state.db, flags::CHAOS_PANEL, "resilience tests").await?;
    if !(1..=3).contains(&body.count) {
        return Err(AppError::BadRequest("count must be between 1 and 3".into()));
    }
    let Some(url) = state.config.self_url.clone() else {
        return Err(AppError::BadRequest(
            "this coordinator cannot add cheaters".into(),
        ));
    };
    let join_code = if network == PUBLIC_NETWORK {
        None
    } else {
        let rules = networks::by_id(&state.db, network)
            .await?
            .ok_or(AppError::NotFound)?;
        Some(rules.join_code)
    };
    let count = body.count as usize;
    state
        .chaos_cheaters
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |running| {
            (running + count <= MAX_CHEATERS).then_some(running + count)
        })
        .map_err(|_| AppError::BadRequest(format!("at most {MAX_CHEATERS} cheaters at once")))?;

    let cheat = if body.collude {
        Cheat::Constant { rate: 1.0 }
    } else {
        Cheat::Random { rate: 1.0 }
    };
    tracing::info!(%network, count, collude = body.collude, "chaos panel adds cheaters");
    for _ in 0..count {
        let state = state.clone();
        let opts = Options {
            url: url.clone(),
            cheat,
            join_code: join_code.clone(),
            delay: CHEATER_UNIT_TIME,
            deadline: Some(Instant::now() + CHEATER_LIFETIME),
            ..Options::default()
        };
        tokio::spawn(async move {
            // Being blocked ends a cheater with an error; that is the point.
            if let Err(e) = sim_worker::run(opts).await {
                tracing::debug!(error = %e, "chaos cheater ended");
            }
            state.chaos_cheaters.fetch_sub(1, Ordering::SeqCst);
        });
    }
    Ok(StatusCode::ACCEPTED)
}

//! The chaos panel (spec 0008): the organiser switches devices off, cuts them off, or
//! adds cheaters, live in front of the room. Every action needs the submit password.

use crate::{
    api::{AppError, authorize},
    state::{AppState, Disruption},
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use protocol::AddCheaters;
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

/// Switches a device off: its connection closes and its units go back to the queue.
pub async fn kill_worker(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    disrupt(&state, &headers, id, Disruption::Kill)
}

/// Cuts a device off, as if it lost its signal.
pub async fn cut_off_worker(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    disrupt(&state, &headers, id, Disruption::CutOff)
}

fn disrupt(
    state: &AppState,
    headers: &HeaderMap,
    id: Uuid,
    disruption: Disruption,
) -> Result<StatusCode, AppError> {
    authorize(state, headers)?;
    if state.disrupt(id, disruption) {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

/// Adds simulated devices that make up every result. They connect to this coordinator
/// like any device and go through the same verification, so catching them is real.
pub async fn add_cheaters(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<AddCheaters>,
) -> Result<StatusCode, AppError> {
    authorize(&state, &headers)?;
    if !(1..=3).contains(&body.count) {
        return Err(AppError::BadRequest("count must be between 1 and 3".into()));
    }
    let Some(url) = state.config.self_url.clone() else {
        return Err(AppError::BadRequest(
            "this coordinator cannot add cheaters".into(),
        ));
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
    tracing::info!(count, collude = body.collude, "chaos panel adds cheaters");
    for _ in 0..count {
        let state = state.clone();
        let opts = Options {
            url: url.clone(),
            cheat,
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

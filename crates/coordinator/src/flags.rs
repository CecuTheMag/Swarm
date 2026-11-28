//! Feature flags (spec 0020): features the platform admin switches on and off at run
//! time. The server refuses what is off (`409`), so a switched-off feature stays off
//! whatever a client sends; the console reads `GET /api/flags` to hide it as well.

use crate::{accounts::require_admin, api::AppError, db::unix_ms, state::AppState};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, Method},
};
use protocol::api::{FeatureFlag, UpdateFeatureFlag};
use sqlx::PgPool;
use std::collections::BTreeMap;
use time::OffsetDateTime;

/// Language-model jobs (spec 0017).
pub const AI_JOBS: &str = "ai_jobs";
/// The chaos panel: switching devices off, cutting them off, adding cheaters (spec 0008).
pub const CHAOS_PANEL: &str = "chaos_panel";
/// Networks' approved programs on the public network (spec 0012).
pub const PUBLIC_PROGRAMS: &str = "public_programs";

/// Whether `key` is on. A flag that does not exist is on: flags switch features off,
/// and a missing row must not take one away.
pub async fn enabled(db: &PgPool, key: &str) -> sqlx::Result<bool> {
    let on: Option<bool> = sqlx::query_scalar("SELECT enabled FROM feature_flags WHERE key = $1")
        .bind(key)
        .fetch_optional(db)
        .await?;
    Ok(on.unwrap_or(true))
}

/// Refuses the request when `key` is off; `what` names the feature in the error.
pub async fn require(db: &PgPool, key: &str, what: &str) -> Result<(), AppError> {
    if enabled(db, key).await? {
        Ok(())
    } else {
        Err(AppError::Conflict(format!("{what} are switched off")))
    }
}

/// `GET /api/flags`: every flag and whether it is on, for any page to read.
#[utoipa::path(
    operation_id = "flags_states",
    get,
    path = "/api/flags",
    tag = "feature flags",
    responses((status = 200, description = "Each flag's key and whether it is on", body = BTreeMap<String, bool>)),
    security(())
)]
pub async fn states(
    State(state): State<AppState>,
) -> Result<Json<BTreeMap<String, bool>>, AppError> {
    let rows: Vec<(String, bool)> = sqlx::query_as("SELECT key, enabled FROM feature_flags")
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().collect()))
}

type FlagRow = (String, bool, String, OffsetDateTime, Option<String>);

/// The flags with who switched each last, ending with `$rest` (a literal, so the query
/// stays a `&'static str`).
macro_rules! select {
    ($rest:literal) => {
        concat!(
            "SELECT f.key, f.enabled, f.description, f.updated_at, a.name ",
            "FROM feature_flags f LEFT JOIN accounts a ON a.id = f.updated_by ",
            $rest
        )
    };
}

fn view((key, enabled, description, updated_at, updated_by): FlagRow) -> FeatureFlag {
    FeatureFlag {
        key,
        enabled,
        description,
        updated_at: unix_ms(updated_at),
        updated_by,
    }
}

/// `GET /api/admin/flags` (platform admin): the flags with what they switch.
#[utoipa::path(
    operation_id = "flags_list",
    get,
    path = "/api/admin/flags",
    tag = "feature flags",
    responses(
        (status = 200, description = "Every flag", body = Vec<protocol::api::FeatureFlag>),
        (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError),
        (status = 403, description = "Role too low, or a change from another site", body = protocol::ApiError)
    ),
    security(("session" = []))
)]
pub async fn list(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
) -> Result<Json<Vec<FeatureFlag>>, AppError> {
    require_admin(&state, &method, &headers).await?;
    let rows: Vec<FlagRow> = sqlx::query_as(select!("ORDER BY f.key"))
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(view).collect()))
}

/// `PATCH /api/admin/flags/{key}` (platform admin): switches a flag on or off.
#[utoipa::path(
    operation_id = "flags_update",
    patch,
    path = "/api/admin/flags/{key}",
    tag = "feature flags",
    params(("key" = String, Path, description = "Flag")),
    request_body = protocol::api::UpdateFeatureFlag,
    responses(
        (status = 200, description = "The flag as it is now", body = protocol::api::FeatureFlag),
        (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError),
        (status = 403, description = "Role too low, or a change from another site", body = protocol::ApiError),
        (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError)
    ),
    security(("session" = []))
)]
pub async fn update(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(body): Json<UpdateFeatureFlag>,
) -> Result<Json<FeatureFlag>, AppError> {
    let admin = require_admin(&state, &method, &headers).await?;
    let changed = sqlx::query(
        "UPDATE feature_flags SET enabled = $2, updated_at = now(), updated_by = $3 WHERE key = $1",
    )
    .bind(&key)
    .bind(body.enabled)
    .bind(admin.id)
    .execute(&state.db)
    .await?;
    if changed.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    tracing::info!(account_id = %admin.id, flag = %key, enabled = body.enabled, "feature flag switched");
    let row: FlagRow = sqlx::query_as(select!("WHERE f.key = $1"))
        .bind(&key)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(view(row)))
}

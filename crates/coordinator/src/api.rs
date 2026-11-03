//! REST API for submitting and inspecting jobs.

use crate::{db, state::AppState, templates};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use protocol::{ApiError, CreateJob, DashboardEvent, JobSummary};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("wrong or missing password")]
    Unauthorized,
    #[error("not found")]
    NotFound,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Database(e) => {
                tracing::error!(error = %e, "database error");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        let error = match &self {
            Self::Database(_) => "internal error".to_owned(),
            other => other.to_string(),
        };
        (status, Json(ApiError { error })).into_response()
    }
}

/// Checks `Authorization: Bearer <token>` when submitting needs a password. Both sides
/// are hashed first so the comparison takes the same time whatever the guess.
pub(crate) fn authorize(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    let Some(token) = &state.config.submit_token else {
        return Ok(());
    };
    let given = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if Sha256::digest(given.as_bytes()) == Sha256::digest(token.as_bytes()) {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}

pub async fn create_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateJob>,
) -> Result<(StatusCode, Json<JobSummary>), AppError> {
    authorize(&state, &headers)?;
    let template = templates::get(&body.template)
        .ok_or_else(|| AppError::BadRequest(format!("unknown template {:?}", body.template)))?;
    let units = template
        .template
        .split(&body.params)
        .map_err(AppError::BadRequest)?;
    let job = db::create_job(
        &state.db,
        template,
        &body.params,
        units,
        state.config.redundancy,
    )
    .await?;
    tracing::info!(job_id = %job.id, template = %job.template, units = job.units_total, "job created");
    state.publish(DashboardEvent::JobUpdated { job: job.clone() });
    Ok((StatusCode::CREATED, Json(job)))
}

/// Stops a job (organiser only): for a demo, nothing must queue behind a forgotten job.
pub async fn cancel_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<JobSummary>, AppError> {
    authorize(&state, &headers)?;
    let job = db::cancel_job(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    if job.error.as_deref() == Some(db::CANCELLED) {
        tracing::info!(job_id = %id, "job cancelled");
        state.renders.forget(id).await;
        state.publish(DashboardEvent::JobUpdated { job: job.clone() });
    }
    Ok(Json(job))
}

pub async fn get_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<JobSummary>, AppError> {
    db::get_job(&state.db, id)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound)
}

/// The current image of a path-tracer job (spec 0003).
pub async fn job_image(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let png = state
        .renders
        .png(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        png,
    )
        .into_response())
}

pub async fn list_jobs(State(state): State<AppState>) -> Result<Json<Vec<JobSummary>>, AppError> {
    Ok(Json(db::recent_jobs(&state.db, 50).await?))
}

pub async fn healthz(State(state): State<AppState>) -> StatusCode {
    match sqlx::query("SELECT 1").execute(&state.db).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

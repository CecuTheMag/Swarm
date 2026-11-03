//! REST API for submitting and inspecting jobs.

use crate::{
    auth::{self, PUBLIC_NETWORK},
    db, networks,
    state::AppState,
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use protocol::{ApiError, CreateJob, DashboardEvent, JobSummary, api::Role};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("wrong or missing password")]
    Unauthorized,
    #[error("not allowed")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    Conflict(String),
    #[error("too many attempts; try again later")]
    TooManyRequests,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
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

/// The public network's jobs: started with the deployment's submit token, a public
/// network API key, or by its admins in the console. Other networks use
/// `/api/networks/{id}/jobs` (spec 0010).
pub async fn create_job(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Json(body): Json<CreateJob>,
) -> Result<(StatusCode, Json<JobSummary>), AppError> {
    let access =
        auth::network_access(&state, &method, &headers, PUBLIC_NETWORK, Role::Admin).await?;
    let job = networks::start_job(&state, &access, &body).await?;
    Ok((StatusCode::CREATED, Json(job)))
}

/// Stops a job (organiser only): for a demo, nothing must queue behind a forgotten job.
pub async fn cancel_job(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<JobSummary>, AppError> {
    auth::network_access(&state, &method, &headers, PUBLIC_NETWORK, Role::Admin).await?;
    networks::job_in(&state, PUBLIC_NETWORK, id).await?;
    Ok(Json(cancel(&state, id).await?))
}

/// Cancels a job whose access was checked.
pub async fn cancel(state: &AppState, id: Uuid) -> Result<JobSummary, AppError> {
    let job = db::cancel_job(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    if job.error.as_deref() == Some(db::CANCELLED) {
        tracing::info!(job_id = %id, "job cancelled");
        state.renders.forget(id).await;
        state.publish(
            job.network_id,
            DashboardEvent::JobUpdated { job: job.clone() },
        );
    }
    Ok(job)
}

pub async fn get_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<JobSummary>, AppError> {
    Ok(Json(networks::job_in(&state, PUBLIC_NETWORK, id).await?))
}

/// The current image of a path-tracer job of the public network (spec 0003).
pub async fn job_image(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    networks::job_in(&state, PUBLIC_NETWORK, id).await?;
    image(&state, id).await
}

/// The current image of a job whose access was checked.
pub async fn image(state: &AppState, id: Uuid) -> Result<Response, AppError> {
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
    Ok(Json(db::recent_jobs(&state.db, PUBLIC_NETWORK, 50).await?))
}

pub async fn healthz(State(state): State<AppState>) -> StatusCode {
    match sqlx::query("SELECT 1").execute(&state.db).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

//! REST API for submitting and inspecting jobs.

use crate::{db, state::AppState, templates};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use protocol::{ApiError, CreateJob, DashboardEvent, JobSummary};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("not found")]
    NotFound,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
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

pub async fn create_job(
    State(state): State<AppState>,
    Json(body): Json<CreateJob>,
) -> Result<(StatusCode, Json<JobSummary>), AppError> {
    let template = templates::get(&body.template)
        .ok_or_else(|| AppError::BadRequest(format!("unknown template {:?}", body.template)))?;
    let units = template
        .template
        .split(&body.params)
        .map_err(AppError::BadRequest)?;
    let job = db::create_job(&state.db, template, &body.params, units).await?;
    tracing::info!(job_id = %job.id, template = %job.template, units = job.units_total, "job created");
    state.publish(DashboardEvent::JobUpdated { job: job.clone() });
    Ok((StatusCode::CREATED, Json(job)))
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

pub async fn list_jobs(State(state): State<AppState>) -> Result<Json<Vec<JobSummary>>, AppError> {
    Ok(Json(db::recent_jobs(&state.db, 50).await?))
}

pub async fn healthz(State(state): State<AppState>) -> StatusCode {
    match sqlx::query("SELECT 1").execute(&state.db).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

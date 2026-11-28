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
use protocol::{ApiError, CreateJob, DashboardEvent, JobSummary, PlatformStats, api::Role};
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
#[utoipa::path(
    operation_id = "public_create_job",
    post,
    path = "/api/jobs",
    tag = "public jobs",
    request_body = protocol::CreateJob,
    responses((status = 201, description = "Queued", body = protocol::JobSummary), (status = 400, description = "Invalid request", body = protocol::ApiError), (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError), (status = 403, description = "Role too low, or a change from another site", body = protocol::ApiError), (status = 409, description = "Conflict with the current state", body = protocol::ApiError)),
    security(("session" = []), ("api_key" = []))
)]
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
#[utoipa::path(
    operation_id = "public_cancel_job",
    post,
    path = "/api/jobs/{id}/cancel",
    tag = "public jobs",
    params(("id" = uuid::Uuid, Path, description = "Job")),
    responses((status = 200, description = "Cancelled (or already finished)", body = protocol::JobSummary), (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError), (status = 403, description = "Role too low, or a change from another site", body = protocol::ApiError), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError)),
    security(("session" = []), ("api_key" = []))
)]
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

#[utoipa::path(
    operation_id = "public_get_job",
    get,
    path = "/api/jobs/{id}",
    tag = "public jobs",
    params(("id" = uuid::Uuid, Path, description = "Job")),
    responses((status = 200, description = "The job", body = protocol::JobSummary), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError)),
    security(())
)]
pub async fn get_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<JobSummary>, AppError> {
    Ok(Json(networks::job_in(&state, PUBLIC_NETWORK, id).await?))
}

/// The current image of a path-tracer job of the public network (spec 0003).
#[utoipa::path(
    operation_id = "public_job_image",
    get,
    path = "/api/jobs/{id}/image.png",
    tag = "public jobs",
    params(("id" = uuid::Uuid, Path, description = "Job")),
    responses((status = 200, description = "The image so far", body = Vec<u8>, content_type = "image/png"), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError)),
    security(())
)]
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

#[utoipa::path(
    operation_id = "public_list_jobs",
    get,
    path = "/api/jobs",
    tag = "public jobs",
    responses((status = 200, description = "The 50 latest jobs", body = Vec<protocol::JobSummary>)),
    security(())
)]
pub async fn list_jobs(State(state): State<AppState>) -> Result<Json<Vec<JobSummary>>, AppError> {
    Ok(Json(db::recent_jobs(&state.db, PUBLIC_NETWORK, 50).await?))
}

/// `GET /api/timetables/example?classes=N&seed=S`: an example school to try timetables
/// with, generated (spec 0013).
#[derive(serde::Deserialize)]
pub struct ExampleSchool {
    classes: usize,
    #[serde(default)]
    seed: u64,
}

#[utoipa::path(
    operation_id = "public_example_school",
    get,
    path = "/api/timetables/example",
    tag = "schools",
    params(("classes" = usize, Query, description = "Number of classes"), ("seed" = Option<u64>, Query, description = "Generator seed, 0 when left out")),
    responses((status = 200, description = "A generated school", body = Object), (status = 400, description = "Invalid request", body = protocol::ApiError)),
    security(())
)]
pub async fn example_school(
    axum::extract::Query(q): axum::extract::Query<ExampleSchool>,
) -> Result<Json<timetable::School>, AppError> {
    if !(1..=timetable::LIMITS.classes).contains(&q.classes) {
        return Err(AppError::BadRequest(format!(
            "classes must be between 1 and {}",
            timetable::LIMITS.classes
        )));
    }
    Ok(Json(timetable::sample::school(q.seed, q.classes)))
}

/// What the platform has computed so far, for the home page when no device is online.
#[utoipa::path(
    operation_id = "public_stats",
    get,
    path = "/api/stats",
    tag = "public jobs",
    responses((status = 200, description = "Totals of every network", body = PlatformStats)),
    security(())
)]
pub async fn stats(State(state): State<AppState>) -> Result<Response, AppError> {
    let (units_decided, jobs_done, devices): (i64, i64, i64) = sqlx::query_as(concat!(
        "SELECT (SELECT COALESCE(SUM(units_done), 0) FROM jobs)::bigint, ",
        "(SELECT count(*) FROM jobs WHERE status = 'done'), ",
        "(SELECT count(*) FROM devices WHERE backend <> 'native')"
    ))
    .fetch_one(&state.db)
    .await?;
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=30")],
        Json(PlatformStats {
            units_decided,
            jobs_done,
            devices,
        }),
    )
        .into_response())
}

#[utoipa::path(
    operation_id = "public_healthz",
    get,
    path = "/healthz",
    tag = "health",
    responses((status = 200, description = "The coordinator and its database answer"), (status = 503, description = "The database does not answer")),
    security(())
)]
pub async fn healthz(State(state): State<AppState>) -> StatusCode {
    match sqlx::query("SELECT 1").execute(&state.db).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

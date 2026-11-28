//! A found timetable as a person edited it (spec 0015): one per timetable job, versioned
//! like a school, its cost always the coordinator's own.

use crate::{
    api::AppError,
    auth::{self, Access},
    db::unix_ms,
    networks::job_in,
    state::AppState,
    templates::tt::decode_slots,
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, Method, StatusCode},
};
use protocol::{
    JobStatus, JobSummary, SaveTimetableEdit, TimetableEdit,
    api::Role,
    templates::tt::{Cost, ID, JobParams},
};
use sqlx::{FromRow, PgPool, types::Json as Jsonb};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(FromRow)]
struct EditRow {
    slots: String,
    pinned: Vec<i32>,
    cost: Jsonb<Cost>,
    version: i32,
    updated_at: OffsetDateTime,
    updated_by: Option<String>,
}

impl EditRow {
    fn edit(self) -> TimetableEdit {
        TimetableEdit {
            slots: self.slots,
            pinned: self.pinned.into_iter().map(|h| h as u32).collect(),
            cost: self.cost.0,
            version: self.version as u32,
            updated_at: unix_ms(self.updated_at),
            updated_by: self.updated_by,
        }
    }
}

async fn edit_of(db: &PgPool, job_id: Uuid) -> sqlx::Result<Option<EditRow>> {
    sqlx::query_as(
        "SELECT e.slots, e.pinned, e.cost, e.version, e.updated_at, a.name AS updated_by
         FROM timetable_edits e LEFT JOIN accounts a ON a.id = e.updated_by
         WHERE e.job_id = $1",
    )
    .bind(job_id)
    .fetch_optional(db)
    .await
}

/// A timetable job of the network; refused for any other kind of job.
async fn timetable_job(
    state: &AppState,
    network_id: Uuid,
    job_id: Uuid,
) -> Result<JobSummary, AppError> {
    let job = job_in(state, network_id, job_id).await?;
    if job.template != ID {
        return Err(AppError::BadRequest(
            "the job is not a timetable search".into(),
        ));
    }
    Ok(job)
}

/// The cost of `body`'s timetable for the job's school, after checking that it is a
/// timetable of that school and that its pinned hours are hours the search may move.
fn checked(job: &JobSummary, body: &SaveTimetableEdit) -> Result<Cost, AppError> {
    if job.status != JobStatus::Done {
        return Err(AppError::BadRequest(
            "the search has not found its timetable yet".into(),
        ));
    }
    let params: JobParams = serde_json::from_value(job.params.clone())
        .map_err(|e| AppError::BadRequest(format!("the job's school cannot be read: {e}")))?;
    let problem = timetable::Problem::new(&params.school).map_err(AppError::BadRequest)?;
    let slots = decode_slots(&body.slots).map_err(AppError::BadRequest)?;
    let cost = timetable::cost(&problem, &slots).map_err(AppError::BadRequest)?;
    let mut seen = vec![false; problem.hours.len()];
    for &h in &body.pinned {
        let Some(hour) = problem.hours.get(h as usize) else {
            return Err(AppError::BadRequest(format!(
                "hour {h} does not exist: the timetable has {}",
                problem.hours.len()
            )));
        };
        if hour.pin.is_some() {
            return Err(AppError::BadRequest(format!(
                "hour {h} is pinned in the school already"
            )));
        }
        if std::mem::replace(&mut seen[h as usize], true) {
            return Err(AppError::BadRequest(format!("hour {h} is pinned twice")));
        }
    }
    Ok(cost)
}

fn editor(access: &Access) -> Option<Uuid> {
    access.account.as_ref().map(|a| a.id)
}

/// `GET /api/networks/{id}/jobs/{job}/timetable` (members): the edit, or null.
#[utoipa::path(
    operation_id = "timetable_get",
    get,
    path = "/api/networks/{id}/jobs/{job}/timetable",
    tag = "schools",
    params(("id" = uuid::Uuid, Path, description = "Network"), ("job" = uuid::Uuid, Path, description = "Job")),
    responses((status = 200, description = "The edited timetable, or null before the first edit", body = Option<protocol::TimetableEdit>), (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError)),
    security(("session" = []), ("api_key" = []))
)]
pub async fn get(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Option<TimetableEdit>>, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Viewer).await?;
    timetable_job(&state, id, job_id).await?;
    Ok(Json(edit_of(&state.db, job_id).await?.map(EditRow::edit)))
}

/// `PUT /api/networks/{id}/jobs/{job}/timetable` (admins): saves an edit of `version`,
/// or the first edit without one; 409 when someone saved since.
#[utoipa::path(
    operation_id = "timetable_save",
    put,
    path = "/api/networks/{id}/jobs/{job}/timetable",
    tag = "schools",
    params(("id" = uuid::Uuid, Path, description = "Network"), ("job" = uuid::Uuid, Path, description = "Job")),
    request_body = protocol::SaveTimetableEdit,
    responses((status = 200, description = "Saved, with its cost", body = protocol::TimetableEdit), (status = 400, description = "Invalid request", body = protocol::ApiError), (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError), (status = 403, description = "Role too low, or a change from another site", body = protocol::ApiError), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError), (status = 409, description = "Conflict with the current state", body = protocol::ApiError)),
    security(("session" = []), ("api_key" = []))
)]
pub async fn save(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((id, job_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<SaveTimetableEdit>,
) -> Result<Json<TimetableEdit>, AppError> {
    let access = auth::network_access(&state, &method, &headers, id, Role::Admin).await?;
    let job = timetable_job(&state, id, job_id).await?;
    let cost = checked(&job, &body)?;
    let pinned: Vec<i32> = body.pinned.iter().map(|&h| h as i32).collect();
    let saved = match body.version {
        None => sqlx::query(
            "INSERT INTO timetable_edits (job_id, slots, pinned, cost, updated_by)
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT (job_id) DO NOTHING",
        )
        .bind(job_id)
        .bind(&body.slots)
        .bind(&pinned)
        .bind(Jsonb(&cost))
        .bind(editor(&access))
        .execute(&state.db)
        .await?
        .rows_affected(),
        Some(version) => sqlx::query(
            "UPDATE timetable_edits
             SET slots = $3, pinned = $4, cost = $5, version = version + 1,
                 updated_at = now(), updated_by = $6
             WHERE job_id = $1 AND version = $2",
        )
        .bind(job_id)
        .bind(i32::try_from(version).unwrap_or(i32::MAX))
        .bind(&body.slots)
        .bind(&pinned)
        .bind(Jsonb(&cost))
        .bind(editor(&access))
        .execute(&state.db)
        .await?
        .rows_affected(),
    };
    let row = edit_of(&state.db, job_id).await?;
    if saved == 0 {
        let since = body
            .version
            .map_or("the found timetable".to_owned(), |v| format!("version {v}"));
        return Err(AppError::Conflict(match row {
            Some(row) => format!(
                "the timetable changed since {since}: it is at version {}{}",
                row.version,
                row.updated_by
                    .as_ref()
                    .map(|by| format!(", saved by {by}"))
                    .unwrap_or_default()
            ),
            None => format!("the timetable changed since {since}: its edit was dropped"),
        }));
    }
    tracing::info!(%job_id, network_id = %id, "timetable edited");
    Ok(Json(row.ok_or(AppError::NotFound)?.edit()))
}

/// `DELETE /api/networks/{id}/jobs/{job}/timetable` (admins): back to the timetable the
/// search found.
#[utoipa::path(
    operation_id = "timetable_drop",
    delete,
    path = "/api/networks/{id}/jobs/{job}/timetable",
    tag = "schools",
    params(("id" = uuid::Uuid, Path, description = "Network"), ("job" = uuid::Uuid, Path, description = "Job")),
    responses((status = 204, description = "Back to the found timetable"), (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError), (status = 403, description = "Role too low, or a change from another site", body = protocol::ApiError), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError)),
    security(("session" = []), ("api_key" = []))
)]
pub async fn drop(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Admin).await?;
    timetable_job(&state, id, job_id).await?;
    let dropped = sqlx::query("DELETE FROM timetable_edits WHERE job_id = $1")
        .bind(job_id)
        .execute(&state.db)
        .await?
        .rows_affected();
    if dropped == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

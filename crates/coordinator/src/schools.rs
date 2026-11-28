//! The schools a network keeps for its timetables, as its deputy director entered them
//! (spec 0014).

use crate::{
    api::AppError,
    auth::{self, Access},
    db::unix_ms,
    state::AppState,
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, Method, StatusCode},
};
use protocol::{SaveSchool, SchoolRecord, SchoolSummary, api::Role, templates::tt::School};
use sqlx::{FromRow, PgPool, types::Json as Jsonb};
use time::OffsetDateTime;
use uuid::Uuid;

/// Schools a network may keep: every term of several years, few enough that one network
/// cannot fill the database.
pub const MAX_SCHOOLS_PER_NETWORK: i64 = 50;
/// The largest school's JSON, well above what the limits of a school allow in practice.
pub const MAX_SCHOOL_BYTES: usize = 2 << 20;

#[derive(FromRow)]
struct SchoolRow {
    id: Uuid,
    name: String,
    version: i32,
    data: Jsonb<School>,
    updated_at: OffsetDateTime,
    updated_by: Option<String>,
}

impl SchoolRow {
    fn summary(&self) -> SchoolSummary {
        let school = &self.data.0;
        SchoolSummary {
            id: self.id,
            name: self.name.clone(),
            version: self.version as u32,
            classes: school.classes.len() as u32,
            teachers: school.teachers.len() as u32,
            hours: school.lessons.iter().map(|l| u32::from(l.per_week)).sum(),
            updated_at: unix_ms(self.updated_at),
            updated_by: self.updated_by.clone(),
        }
    }

    fn record(self) -> SchoolRecord {
        SchoolRecord {
            summary: self.summary(),
            school: self.data.0,
        }
    }
}

/// A query of schools with who saved each last, ending with `$rest`.
macro_rules! select {
    ($rest:literal) => {
        concat!(
            "SELECT s.id, s.name, s.version, s.data, s.updated_at, a.name AS updated_by
             FROM schools s LEFT JOIN accounts a ON a.id = s.updated_by ",
            $rest
        )
    };
}

/// A school fit to keep: one a timetable can be searched for (spec 0013), of a size the
/// database takes.
fn check(school: &School) -> Result<(), AppError> {
    timetable::Problem::new(school).map_err(AppError::BadRequest)?;
    let bytes = serde_json::to_vec(school).map_err(|e| AppError::BadRequest(e.to_string()))?;
    if bytes.len() > MAX_SCHOOL_BYTES {
        return Err(AppError::BadRequest(format!(
            "a school may take at most {} MiB",
            MAX_SCHOOL_BYTES >> 20
        )));
    }
    Ok(())
}

fn editor(access: &Access) -> Option<Uuid> {
    access.account.as_ref().map(|a| a.id)
}

pub async fn of_network(db: &PgPool, network_id: Uuid) -> sqlx::Result<Vec<SchoolSummary>> {
    let rows: Vec<SchoolRow> = sqlx::query_as(select!(
        "WHERE s.network_id = $1 ORDER BY s.updated_at DESC, s.id"
    ))
    .bind(network_id)
    .fetch_all(db)
    .await?;
    Ok(rows.iter().map(SchoolRow::summary).collect())
}

async fn get_in(db: &PgPool, network_id: Uuid, id: Uuid) -> sqlx::Result<Option<SchoolRow>> {
    sqlx::query_as(select!("WHERE s.network_id = $1 AND s.id = $2"))
        .bind(network_id)
        .bind(id)
        .fetch_optional(db)
        .await
}

/// `GET /api/networks/{id}/schools` (members).
#[utoipa::path(
    operation_id = "schools_list",
    get,
    path = "/api/networks/{id}/schools",
    tag = "schools",
    params(("id" = uuid::Uuid, Path, description = "Network")),
    responses((status = 200, description = "The network's schools", body = Vec<protocol::SchoolSummary>), (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError)),
    security(("session" = []), ("api_key" = []))
)]
pub async fn list(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<SchoolSummary>>, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Viewer).await?;
    Ok(Json(of_network(&state.db, id).await?))
}

/// `POST /api/networks/{id}/schools` (admins).
#[utoipa::path(
    operation_id = "schools_create",
    post,
    path = "/api/networks/{id}/schools",
    tag = "schools",
    params(("id" = uuid::Uuid, Path, description = "Network")),
    request_body = protocol::SaveSchool,
    responses((status = 201, description = "Saved as version 1", body = protocol::SchoolRecord), (status = 400, description = "Invalid request", body = protocol::ApiError), (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError), (status = 403, description = "Role too low, or a change from another site", body = protocol::ApiError), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError), (status = 409, description = "Conflict with the current state", body = protocol::ApiError)),
    security(("session" = []), ("api_key" = []))
)]
pub async fn create(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<SaveSchool>,
) -> Result<(StatusCode, Json<SchoolRecord>), AppError> {
    let access = auth::network_access(&state, &method, &headers, id, Role::Admin).await?;
    if body.version.is_some() {
        return Err(AppError::BadRequest("a new school has no version".into()));
    }
    check(&body.school)?;
    let school_id = Uuid::now_v7();
    // The network's row is locked while counting, so two creates at once cannot both
    // pass the limit.
    let mut tx = state.db.begin().await?;
    sqlx::query("SELECT 1 FROM networks WHERE id = $1 FOR UPDATE")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM schools WHERE network_id = $1")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    if count >= MAX_SCHOOLS_PER_NETWORK {
        return Err(AppError::Conflict(format!(
            "a network may keep at most {MAX_SCHOOLS_PER_NETWORK} schools"
        )));
    }
    sqlx::query(
        "INSERT INTO schools (id, network_id, name, data, updated_by) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(school_id)
    .bind(id)
    .bind(&body.school.name)
    .bind(Jsonb(&body.school))
    .bind(editor(&access))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!(%school_id, network_id = %id, "school created");
    let row = get_in(&state.db, id, school_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok((StatusCode::CREATED, Json(row.record())))
}

/// `GET /api/networks/{id}/schools/{school}` (members).
#[utoipa::path(
    operation_id = "schools_get",
    get,
    path = "/api/networks/{id}/schools/{school}",
    tag = "schools",
    params(("id" = uuid::Uuid, Path, description = "Network"), ("school" = uuid::Uuid, Path, description = "School")),
    responses((status = 200, description = "The school with its data", body = protocol::SchoolRecord), (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError)),
    security(("session" = []), ("api_key" = []))
)]
pub async fn get(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((id, school_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<SchoolRecord>, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Viewer).await?;
    let row = get_in(&state.db, id, school_id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(row.record()))
}

/// `PUT /api/networks/{id}/schools/{school}` (admins): saves an edit of `version`; 409
/// when someone saved since.
#[utoipa::path(
    operation_id = "schools_save",
    put,
    path = "/api/networks/{id}/schools/{school}",
    tag = "schools",
    params(("id" = uuid::Uuid, Path, description = "Network"), ("school" = uuid::Uuid, Path, description = "School")),
    request_body = protocol::SaveSchool,
    responses((status = 200, description = "Saved as the next version", body = protocol::SchoolRecord), (status = 400, description = "Invalid request", body = protocol::ApiError), (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError), (status = 403, description = "Role too low, or a change from another site", body = protocol::ApiError), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError), (status = 409, description = "Conflict with the current state", body = protocol::ApiError)),
    security(("session" = []), ("api_key" = []))
)]
pub async fn save(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((id, school_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<SaveSchool>,
) -> Result<Json<SchoolRecord>, AppError> {
    let access = auth::network_access(&state, &method, &headers, id, Role::Admin).await?;
    let version = body
        .version
        .ok_or_else(|| AppError::BadRequest("say which version the edit started from".into()))?;
    check(&body.school)?;
    let saved = sqlx::query(
        "UPDATE schools
         SET name = $4, data = $5, version = version + 1, updated_at = now(), updated_by = $6
         WHERE network_id = $1 AND id = $2 AND version = $3",
    )
    .bind(id)
    .bind(school_id)
    .bind(i32::try_from(version).unwrap_or(i32::MAX))
    .bind(&body.school.name)
    .bind(Jsonb(&body.school))
    .bind(editor(&access))
    .execute(&state.db)
    .await?
    .rows_affected();
    let row = get_in(&state.db, id, school_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if saved == 0 {
        return Err(AppError::Conflict(format!(
            "the school changed since version {version}: it is at version {}{}",
            row.version,
            row.updated_by
                .as_ref()
                .map(|by| format!(", saved by {by}"))
                .unwrap_or_default()
        )));
    }
    Ok(Json(row.record()))
}

/// `DELETE /api/networks/{id}/schools/{school}` (admins). Jobs searched for it keep
/// their own copy.
#[utoipa::path(
    operation_id = "schools_delete",
    delete,
    path = "/api/networks/{id}/schools/{school}",
    tag = "schools",
    params(("id" = uuid::Uuid, Path, description = "Network"), ("school" = uuid::Uuid, Path, description = "School")),
    responses((status = 204, description = "Deleted"), (status = 401, description = "Not signed in or unknown API key", body = protocol::ApiError), (status = 403, description = "Role too low, or a change from another site", body = protocol::ApiError), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError)),
    security(("session" = []), ("api_key" = []))
)]
pub async fn delete(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((id, school_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Admin).await?;
    let deleted = sqlx::query("DELETE FROM schools WHERE network_id = $1 AND id = $2")
        .bind(id)
        .bind(school_id)
        .execute(&state.db)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(AppError::NotFound);
    }
    tracing::info!(%school_id, network_id = %id, "school deleted");
    Ok(StatusCode::NO_CONTENT)
}

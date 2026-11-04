//! Programs that networks uploaded, their modules and their review (spec 0012).

use crate::{
    accounts::require_admin,
    api::AppError,
    auth::{self, Access, PUBLIC_NETWORK},
    db::{self, unix_ms},
    state::AppState,
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use protocol::{
    CreateJob, JobOutputs, ProgramSummary, Review, ReviewDecision, UploadProgram, api::Role,
    templates::program,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

/// Programs a network may keep: enough for any class, few enough that one network
/// cannot fill the database.
pub const MAX_PROGRAMS_PER_NETWORK: i64 = 50;
pub const MAX_NAME_CHARS: usize = 80;
pub const MAX_DESCRIPTION_CHARS: usize = 2000;
pub const MAX_SOURCE_URL_CHARS: usize = 300;
pub const MAX_NOTE_CHARS: usize = 2000;

#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    #[error("{0}")]
    BadRequest(String),
    #[error(transparent)]
    Rejected(#[from] programs::Rejection),
    #[error("a network may keep at most {MAX_PROGRAMS_PER_NETWORK} programs")]
    TooMany,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(FromRow)]
struct ProgramRow {
    id: Uuid,
    network_id: Uuid,
    network_name: String,
    sha256: String,
    size: i32,
    name: String,
    description: String,
    source_url: Option<String>,
    review: String,
    review_note: Option<String>,
    created_at: OffsetDateTime,
}

impl From<ProgramRow> for ProgramSummary {
    fn from(r: ProgramRow) -> Self {
        ProgramSummary {
            id: r.id,
            network_id: r.network_id,
            network_name: r.network_name,
            name: r.name,
            description: r.description,
            source_url: r.source_url,
            sha256: r.sha256,
            size: r.size as u32,
            // The CHECK constraint guarantees one of the known values.
            review: Review::parse(&r.review).unwrap_or(Review::Private),
            review_note: r.review_note,
            created_at: unix_ms(r.created_at),
        }
    }
}

/// The select list and joins of `ProgramRow`; a macro so queries stay `&'static str`.
macro_rules! program_select {
    () => {
        "SELECT p.id, p.network_id, n.name AS network_name, p.sha256, length(m.bytes) AS size,
                p.name, p.description, p.source_url, p.review, p.review_note, p.created_at
         FROM programs p JOIN networks n ON n.id = p.network_id
         JOIN modules m ON m.sha256 = p.sha256"
    };
}

fn text(field: &str, value: &str, max: usize, required: bool) -> Result<String, UploadError> {
    let value = value.trim();
    if required && value.is_empty() {
        return Err(UploadError::BadRequest(format!("{field} is required")));
    }
    if value.chars().count() > max {
        return Err(UploadError::BadRequest(format!(
            "{field} may be at most {max} characters"
        )));
    }
    Ok(value.to_owned())
}

/// Checks and stores a network's program. Uploading a module the network already has
/// returns that program unchanged, with `false`.
pub async fn upload(
    db: &PgPool,
    network_id: Uuid,
    upload: &UploadProgram,
) -> Result<(ProgramSummary, bool), UploadError> {
    let name = text("name", &upload.name, MAX_NAME_CHARS, true)?;
    if name.chars().any(char::is_control) {
        return Err(UploadError::BadRequest(
            "name may not contain control characters".into(),
        ));
    }
    let description = text(
        "description",
        &upload.description,
        MAX_DESCRIPTION_CHARS,
        false,
    )?;
    let source_url = match upload.source_url.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(url) => {
            let url = text("source_url", url, MAX_SOURCE_URL_CHARS, true)?;
            if !url.starts_with("https://") {
                return Err(UploadError::BadRequest(
                    "source_url must be an https:// address".into(),
                ));
            }
            Some(url)
        }
    };
    // Refuse what cannot decode to an acceptable size before decoding it.
    if upload.module.len() > programs::MAX_MODULE_BYTES.div_ceil(3) * 4 {
        return Err(programs::Rejection::TooLarge {
            size: upload.module.len() / 4 * 3,
        }
        .into());
    }
    let module = STANDARD
        .decode(upload.module.trim())
        .map_err(|e| UploadError::BadRequest(format!("module is not base64: {e}")))?;
    programs::check(&module)?;
    let sha256 = hex::encode(Sha256::digest(&module));

    let mut tx = db.begin().await?;
    // Serialises uploads of one network, so its count cannot be raced past the limit.
    sqlx::query("SELECT 1 FROM networks WHERE id = $1 FOR UPDATE")
        .bind(network_id)
        .execute(&mut *tx)
        .await?;
    let existing: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM programs WHERE network_id = $1 AND sha256 = $2")
            .bind(network_id)
            .bind(&sha256)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(id) = existing {
        tx.commit().await?;
        let program = get(db, id).await?.expect("found above");
        return Ok((program, false));
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM programs WHERE network_id = $1")
        .bind(network_id)
        .fetch_one(&mut *tx)
        .await?;
    if count >= MAX_PROGRAMS_PER_NETWORK {
        return Err(UploadError::TooMany);
    }
    sqlx::query("INSERT INTO modules (sha256, bytes) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(&sha256)
        .bind(&module)
        .execute(&mut *tx)
        .await?;
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO programs (id, network_id, sha256, name, description, source_url)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(network_id)
    .bind(&sha256)
    .bind(&name)
    .bind(&description)
    .bind(&source_url)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let program = get(db, id).await?.expect("inserted above");
    Ok((program, true))
}

pub async fn get(db: &PgPool, id: Uuid) -> sqlx::Result<Option<ProgramSummary>> {
    let row: Option<ProgramRow> = sqlx::query_as(concat!(program_select!(), " WHERE p.id = $1"))
        .bind(id)
        .fetch_optional(db)
        .await?;
    Ok(row.map(Into::into))
}

pub async fn of_network(db: &PgPool, network_id: Uuid) -> sqlx::Result<Vec<ProgramSummary>> {
    let rows: Vec<ProgramRow> = sqlx::query_as(concat!(
        program_select!(),
        " WHERE p.network_id = $1 ORDER BY p.created_at DESC"
    ))
    .bind(network_id)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// Programs in a review state, oldest first: the review queue, or the approved
/// programs a contributor may allow.
pub async fn in_review_state(db: &PgPool, review: Review) -> sqlx::Result<Vec<ProgramSummary>> {
    let rows: Vec<ProgramRow> = sqlx::query_as(concat!(
        program_select!(),
        " WHERE p.review = $1 ORDER BY p.created_at"
    ))
    .bind(review.as_str())
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// Asks for review of a private or rejected program; an approved or pending one stays
/// as it is. Returns the program as it now is.
pub async fn request_review(db: &PgPool, id: Uuid) -> sqlx::Result<Option<ProgramSummary>> {
    sqlx::query(
        "UPDATE programs SET review = 'pending', review_note = NULL, reviewed_at = NULL
         WHERE id = $1 AND review IN ('private', 'rejected')",
    )
    .bind(id)
    .execute(db)
    .await?;
    get(db, id).await
}

/// Approves or rejects a program waiting for review. Returns `None` if there is no
/// such program waiting.
pub async fn decide(
    db: &PgPool,
    id: Uuid,
    approve: bool,
    note: Option<&str>,
) -> Result<Option<ProgramSummary>, UploadError> {
    let note = match note {
        Some(note) => Some(text("note", note, MAX_NOTE_CHARS, false)?).filter(|n| !n.is_empty()),
        None => None,
    };
    let decided = sqlx::query(
        "UPDATE programs SET review = $2, review_note = $3, reviewed_at = now()
         WHERE id = $1 AND review = 'pending'",
    )
    .bind(id)
    .bind(if approve { "approved" } else { "rejected" })
    .bind(note)
    .execute(db)
    .await?
    .rows_affected();
    if decided == 0 {
        return Ok(None);
    }
    Ok(get(db, id).await?)
}

/// A module's bytes by their hash.
pub async fn module(db: &PgPool, sha256: &str) -> sqlx::Result<Option<Vec<u8>>> {
    sqlx::query_scalar("SELECT bytes FROM modules WHERE sha256 = $1")
        .bind(sha256)
        .fetch_optional(db)
        .await
}

// The API (spec 0012).

impl From<UploadError> for AppError {
    fn from(e: UploadError) -> Self {
        match e {
            UploadError::BadRequest(_) | UploadError::Rejected(_) => {
                Self::BadRequest(e.to_string())
            }
            UploadError::TooMany => Self::Conflict(e.to_string()),
            UploadError::Database(e) => Self::Database(e),
        }
    }
}

/// A program of `network_id`, or "not found" (also for another network's program).
async fn program_in(
    state: &AppState,
    network_id: Uuid,
    id: Uuid,
) -> Result<ProgramSummary, AppError> {
    get(&state.db, id)
        .await?
        .filter(|p| p.network_id == network_id)
        .ok_or(AppError::NotFound)
}

/// `GET /api/networks/{id}/programs` (viewers).
pub async fn list(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ProgramSummary>>, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Viewer).await?;
    Ok(Json(of_network(&state.db, id).await?))
}

/// `POST /api/networks/{id}/programs` (admins): 201 for a new program, 200 when the
/// network already had this module.
pub async fn create(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<UploadProgram>,
) -> Result<(StatusCode, Json<ProgramSummary>), AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Admin).await?;
    let (program, created) = upload(&state.db, id, &body).await?;
    if created {
        tracing::info!(program_id = %program.id, network_id = %id, sha256 = %program.sha256, "program uploaded");
    }
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(program)))
}

/// `POST /api/networks/{id}/programs/{program}/review` (admins): asks for review for the
/// public network.
pub async fn ask_review(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((id, program_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ProgramSummary>, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Admin).await?;
    program_in(&state, id, program_id).await?;
    let program = request_review(&state.db, program_id)
        .await?
        .ok_or(AppError::NotFound)?;
    tracing::info!(%program_id, "review requested");
    Ok(Json(program))
}

/// `GET /api/admin/programs` (platform admin): programs waiting for review.
pub async fn review_queue(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
) -> Result<Json<Vec<ProgramSummary>>, AppError> {
    require_admin(&state, &method, &headers).await?;
    Ok(Json(in_review_state(&state.db, Review::Pending).await?))
}

/// `POST /api/admin/programs/{program}/decision` (platform admin).
pub async fn decision(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(program_id): Path<Uuid>,
    Json(body): Json<ReviewDecision>,
) -> Result<Json<ProgramSummary>, AppError> {
    require_admin(&state, &method, &headers).await?;
    let program = decide(&state.db, program_id, body.approve, body.note.as_deref())
        .await?
        .ok_or(AppError::NotFound)?;
    tracing::info!(%program_id, review = ?program.review, "program reviewed");
    Ok(Json(program))
}

/// `GET /api/programs/approved`: what a contributor allows by allowing community
/// programs.
pub async fn approved(
    State(state): State<AppState>,
) -> Result<Json<Vec<ProgramSummary>>, AppError> {
    Ok(Json(in_review_state(&state.db, Review::Approved).await?))
}

/// `GET /api/modules/{sha256}.wasm`, for workers about to run it. The bytes never change
/// for a hash, so browsers may keep them for good.
pub async fn module_file(
    State(state): State<AppState>,
    Path(file): Path<String>,
) -> Result<Response, AppError> {
    let sha256 = file
        .strip_suffix(".wasm")
        .filter(|h| h.len() == 64 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
        .ok_or(AppError::NotFound)?;
    let bytes = module(&state.db, sha256).await?.ok_or(AppError::NotFound)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/wasm"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        bytes,
    )
        .into_response())
}

/// `GET /api/networks/{id}/jobs/{job}/outputs` (viewers): a program job's outputs.
pub async fn outputs(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<JobOutputs>, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Viewer).await?;
    let job = crate::networks::job_in(&state, id, job_id).await?;
    if job.template != program::ID {
        return Err(AppError::BadRequest(
            "only program jobs have outputs to download".into(),
        ));
    }
    let outputs = db::unit_outputs(&state.db, job_id)
        .await?
        .into_iter()
        .map(|output| {
            output
                .and_then(|o| serde_json::from_value::<program::UnitOutput>(o).ok())
                .map(|o| o.output)
        })
        .collect();
    Ok(Json(JobOutputs { outputs }))
}

/// Largest quorum a network may ask for.
pub const MAX_QUORUM: u32 = 3;
/// Jobs a network may have on the public network at once, so one network cannot fill
/// it.
pub const MAX_PUBLIC_JOBS_PER_NETWORK: i64 = 2;

/// Where and how a new job runs.
pub struct Placement {
    /// The network whose devices compute it.
    pub network_id: Uuid,
    pub quorum: u32,
    /// The network that submitted it, when it runs on the public network for it.
    pub owner_network_id: Option<Uuid>,
    /// The program it runs, and the job's params as shown: the inputs stay with the
    /// units.
    pub program: Option<(ProgramSummary, Value)>,
}

/// Decides where a job asked for by `access` runs and what it may run (spec 0012). Only
/// a network's own programs run in it; only approved ones, on the public network.
pub async fn place(
    state: &AppState,
    access: &Access,
    body: &CreateJob,
    units: u32,
) -> Result<Placement, AppError> {
    let bad = |message: &str| Err(AppError::BadRequest(message.into()));
    let program = if body.template == program::ID {
        let params: program::JobParams = serde_json::from_value(body.params.clone())
            .map_err(|e| AppError::BadRequest(format!("invalid params: {e}")))?;
        let found = program_in(state, access.network_id, params.program)
            .await
            .map_err(|_| AppError::BadRequest(format!("unknown program {}", params.program)))?;
        let shown = serde_json::to_value(program::JobInfo {
            program: found.id,
            name: found.name.clone(),
            units,
        })
        .expect("plain structs always serialize");
        Some((found, shown))
    } else {
        None
    };

    let to_public = body.public == Some(true) && access.network_id != PUBLIC_NETWORK;
    if to_public {
        match &program {
            Some((p, _)) if p.review == Review::Approved => {}
            _ => return bad("only programs approved in review run on the public network"),
        }
        if body.quorum.is_some() {
            return bad("jobs on the public network use the coordinator's quorum");
        }
        if db::public_jobs_of(&state.db, access.network_id).await? >= MAX_PUBLIC_JOBS_PER_NETWORK {
            return Err(AppError::Conflict(format!(
                "a network may have at most {MAX_PUBLIC_JOBS_PER_NETWORK} jobs on the public network at once"
            )));
        }
        return Ok(Placement {
            network_id: PUBLIC_NETWORK,
            quorum: state.config.redundancy,
            owner_network_id: Some(access.network_id),
            program,
        });
    }
    let quorum = match body.quorum {
        None => state.config.redundancy,
        Some(_) if access.network_id == PUBLIC_NETWORK => {
            return bad("jobs on the public network use the coordinator's quorum");
        }
        Some(q) if (1..=MAX_QUORUM).contains(&q) => q,
        Some(_) => {
            return Err(AppError::BadRequest(format!(
                "quorum must be between 1 and {MAX_QUORUM}"
            )));
        }
    };
    Ok(Placement {
        network_id: access.network_id,
        quorum,
        owner_network_id: None,
        program,
    })
}

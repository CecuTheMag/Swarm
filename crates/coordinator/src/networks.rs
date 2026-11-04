//! Networks of devices that clients own and operate (spec 0010): their rules, devices,
//! members, API keys and jobs, as the console manages them.

use crate::{
    api::AppError,
    auth::{self, Access, PUBLIC_NETWORK},
    catalog,
    db::{self, DeviceRecord},
    hours,
    state::AppState,
    templates,
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, Method, StatusCode},
};
use jiff::Timestamp;
use protocol::{
    CreateJob, DashboardEvent, DeviceControl, JobSummary,
    api::{
        ApiKeyView, CreateApiKey, CreateInvite, CreateNetwork, CreatedApiKey, CreatedInvite,
        DeviceStatus, DeviceView, Hours, JoinMode, MemberView, NetworkView, Role, UpdateDevice,
        UpdateMember, UpdateNetwork,
    },
};
use serde::Serialize;
use sqlx::{FromRow, PgPool, types::Json as SqlJson};
use time::OffsetDateTime;
use uuid::Uuid;

/// How long an invitation link stays valid.
pub const INVITE_DAYS: i32 = 14;
const MAX_NAME: usize = 80;

/// A network's settings, as sessions and the console need them.
#[derive(Debug, Clone)]
pub struct NetworkRules {
    pub id: Uuid,
    pub name: String,
    pub join_code: String,
    pub join_mode: JoinMode,
    pub max_power: u32,
    pub hours: Option<Hours>,
    pub paused: bool,
    pub lend_idle: bool,
    pub created_at: OffsetDateTime,
}

#[derive(FromRow)]
struct NetworkRow {
    id: Uuid,
    name: String,
    join_code: String,
    join_mode: String,
    max_power: i32,
    hours: Option<SqlJson<Hours>>,
    paused: bool,
    lend_idle: bool,
    created_at: OffsetDateTime,
}

impl From<NetworkRow> for NetworkRules {
    fn from(r: NetworkRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            join_code: r.join_code,
            join_mode: if r.join_mode == "approval" {
                JoinMode::Approval
            } else {
                JoinMode::Open
            },
            max_power: r.max_power as u32,
            hours: r.hours.map(|h| h.0),
            paused: r.paused,
            lend_idle: r.lend_idle,
            created_at: r.created_at,
        }
    }
}

/// Columns of `NetworkRow`; a macro so queries stay `&'static str`.
macro_rules! network_columns {
    () => {
        "id, name, join_code, join_mode, max_power, hours, paused, lend_idle, created_at"
    };
}

pub async fn by_id(db: &PgPool, id: Uuid) -> sqlx::Result<Option<NetworkRules>> {
    let row: Option<NetworkRow> = sqlx::query_as(concat!(
        "SELECT ",
        network_columns!(),
        " FROM networks WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(db)
    .await?;
    Ok(row.map(Into::into))
}

pub async fn by_join_code(db: &PgPool, code: &str) -> sqlx::Result<Option<NetworkRules>> {
    let row: Option<NetworkRow> = sqlx::query_as(concat!(
        "SELECT ",
        network_columns!(),
        " FROM networks WHERE join_code = $1"
    ))
    .bind(code)
    .fetch_optional(db)
    .await?;
    Ok(row.map(Into::into))
}

/// What a device may do now, from its network's rules and its own state.
pub fn control(rules: &NetworkRules, device: &DeviceRecord, now: Timestamp) -> DeviceControl {
    match device.status.as_str() {
        "removed" => return DeviceControl::Removed,
        "pending" => return DeviceControl::PendingApproval,
        _ => {}
    }
    if rules.paused {
        DeviceControl::NetworkPaused
    } else if device.paused {
        DeviceControl::PausedByOwner
    } else if !hours::open_at(rules.hours.as_ref(), now) {
        DeviceControl::OutsideHours {
            resumes_at: rules
                .hours
                .as_ref()
                .and_then(|h| hours::next_open(h, now))
                .map(|t| t.as_millisecond()),
        }
    } else {
        DeviceControl::Running
    }
}

/// A join code: short enough to type, from letters and digits that cannot be confused.
pub fn new_join_code() -> String {
    const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    let mut bytes = [0u8; 10];
    getrandom::fill(&mut bytes).expect("the OS has a random source");
    bytes
        .iter()
        .map(|b| ALPHABET[usize::from(*b) % ALPHABET.len()] as char)
        .collect()
}

fn clean_name(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err(AppError::BadRequest(format!(
            "a name of 1 to {MAX_NAME} characters"
        )));
    }
    Ok(name.to_owned())
}

async fn view(state: &AppState, rules: NetworkRules, role: Role) -> Result<NetworkView, AppError> {
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM devices WHERE network_id = $1 AND status <> 'removed'",
    )
    .bind(rules.id)
    .fetch_one(&state.db)
    .await?;
    Ok(NetworkView {
        id: rules.id,
        name: rules.name,
        join_code: rules.join_code,
        role,
        join_mode: rules.join_mode,
        max_power: rules.max_power,
        hours: rules.hours,
        paused: rules.paused,
        lend_idle: rules.lend_idle,
        public: rules.id == PUBLIC_NETWORK,
        devices_online: state.online_devices(rules.id).len() as u32,
        devices_total: total as u32,
        created_at: db::unix_ms(rules.created_at),
    })
}

async fn rules_of(state: &AppState, id: Uuid) -> Result<NetworkRules, AppError> {
    by_id(&state.db, id).await?.ok_or(AppError::NotFound)
}

// ---------- Networks ----------

/// `GET /api/networks`: the signed-in account's networks.
pub async fn list(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
) -> Result<Json<Vec<NetworkView>>, AppError> {
    let account = auth::require_account(&state, &method, &headers).await?;
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT m.network_id, m.role FROM memberships m JOIN networks n ON n.id = m.network_id
         WHERE m.account_id = $1 ORDER BY n.created_at",
    )
    .bind(account.id)
    .fetch_all(&state.db)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for (id, role) in rows {
        if let (Some(rules), Some(role)) = (by_id(&state.db, id).await?, Role::parse(&role)) {
            out.push(view(&state, rules, role).await?);
        }
    }
    Ok(Json(out))
}

/// `POST /api/networks`: a new network owned by the signed-in account.
pub async fn create(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Json(body): Json<CreateNetwork>,
) -> Result<(StatusCode, Json<NetworkView>), AppError> {
    let account = auth::require_account(&state, &method, &headers).await?;
    let name = clean_name(&body.name)?;
    let id = Uuid::now_v7();
    let mut tx = state.db.begin().await?;
    sqlx::query("INSERT INTO networks (id, name, join_code) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(&name)
        .bind(new_join_code())
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO memberships (network_id, account_id, role) VALUES ($1, $2, 'owner')")
        .bind(id)
        .bind(account.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    tracing::info!(network_id = %id, account_id = %account.id, "network created");
    let rules = rules_of(&state, id).await?;
    Ok((
        StatusCode::CREATED,
        Json(view(&state, rules, Role::Owner).await?),
    ))
}

pub async fn get(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<NetworkView>, AppError> {
    let access = auth::network_access(&state, &method, &headers, id, Role::Viewer).await?;
    let rules = rules_of(&state, id).await?;
    Ok(Json(view(&state, rules, access.role).await?))
}

/// `PATCH /api/networks/{id}`: rules (admin); devices hear about it at once.
pub async fn update(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateNetwork>,
) -> Result<Json<NetworkView>, AppError> {
    let access = auth::network_access(&state, &method, &headers, id, Role::Admin).await?;
    let mut rules = rules_of(&state, id).await?;
    if let Some(name) = &body.name {
        rules.name = clean_name(name)?;
    }
    if let Some(mode) = body.join_mode {
        rules.join_mode = mode;
    }
    if let Some(power) = body.max_power {
        if ![25, 50, 75, 100].contains(&power) {
            return Err(AppError::BadRequest(
                "max_power is 25, 50, 75 or 100".into(),
            ));
        }
        rules.max_power = power;
    }
    if let Some(hours) = body.hours {
        if let Some(h) = &hours {
            hours::validate(h).map_err(AppError::BadRequest)?;
        }
        rules.hours = hours;
    }
    if let Some(paused) = body.paused {
        rules.paused = paused;
    }
    if let Some(lend) = body.lend_idle {
        rules.lend_idle = lend && id != PUBLIC_NETWORK;
    }
    if body.new_join_code == Some(true) {
        if id == PUBLIC_NETWORK {
            return Err(AppError::BadRequest(
                "the public network keeps its code".into(),
            ));
        }
        rules.join_code = new_join_code();
    }
    sqlx::query(
        "UPDATE networks SET name = $2, join_mode = $3, max_power = $4, hours = $5, paused = $6,
             lend_idle = $7, join_code = $8
         WHERE id = $1",
    )
    .bind(id)
    .bind(&rules.name)
    .bind(match rules.join_mode {
        JoinMode::Open => "open",
        JoinMode::Approval => "approval",
    })
    .bind(rules.max_power as i32)
    .bind(rules.hours.as_ref().map(SqlJson))
    .bind(rules.paused)
    .bind(rules.lend_idle)
    .bind(&rules.join_code)
    .execute(&state.db)
    .await?;
    state.refresh_network(id);
    Ok(Json(view(&state, rules, access.role).await?))
}

/// `DELETE /api/networks/{id}` (owner): the network, its devices and jobs.
pub async fn delete(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Owner).await?;
    if id == PUBLIC_NETWORK {
        return Err(AppError::BadRequest(
            "the public network cannot be deleted".into(),
        ));
    }
    sqlx::query("DELETE FROM networks WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;
    // Connected devices find their device gone and leave.
    state.refresh_network(id);
    Ok(StatusCode::NO_CONTENT)
}

// ---------- Devices ----------

#[derive(FromRow)]
struct DeviceRow {
    id: Uuid,
    name: Option<String>,
    status: String,
    paused: bool,
    backend: String,
    vendor: String,
    architecture: String,
    units_done: i32,
    first_seen_at: OffsetDateTime,
    last_seen_at: OffsetDateTime,
}

/// `GET /api/networks/{id}/devices`: waiting ones first, then by last seen.
pub async fn devices(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<DeviceView>>, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Viewer).await?;
    let rows: Vec<DeviceRow> = sqlx::query_as(
        "SELECT id, name, status, paused, backend, vendor, architecture, units_done,
                first_seen_at, last_seen_at
         FROM devices WHERE network_id = $1 AND status <> 'removed'
         ORDER BY status = 'pending' DESC, last_seen_at DESC LIMIT 2000",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let online = state.online_devices(id);
    Ok(Json(
        rows.into_iter()
            .map(|r| DeviceView {
                id: r.id,
                name: r.name,
                status: if r.status == "pending" {
                    DeviceStatus::Pending
                } else {
                    DeviceStatus::Active
                },
                paused: r.paused,
                backend: serde_json::from_value(serde_json::Value::String(r.backend))
                    .unwrap_or(protocol::Backend::Native),
                vendor: r.vendor,
                architecture: r.architecture,
                units_done: r.units_done.max(0) as u32,
                online: online.contains(&r.id),
                first_seen_at: db::unix_ms(r.first_seen_at),
                last_seen_at: db::unix_ms(r.last_seen_at),
            })
            .collect(),
    ))
}

/// `PATCH /api/networks/{id}/devices/{device}`: rename, pause, resume, approve (admin).
pub async fn update_device(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((id, device_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateDevice>,
) -> Result<StatusCode, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Admin).await?;
    let name = match &body.name {
        Some(n) if n.trim().is_empty() => Some(None),
        Some(n) => Some(Some(clean_name(n)?)),
        None => None,
    };
    let updated = sqlx::query(
        "UPDATE devices SET
             name = CASE WHEN $3 THEN $4 ELSE name END,
             paused = COALESCE($5, paused),
             status = CASE WHEN $6 AND status = 'pending' THEN 'active' ELSE status END
         WHERE id = $2 AND network_id = $1 AND status <> 'removed'",
    )
    .bind(id)
    .bind(device_id)
    .bind(name.is_some())
    .bind(name.flatten())
    .bind(body.paused)
    .bind(body.approve == Some(true))
    .execute(&state.db)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    state.refresh_device(device_id);
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/networks/{id}/devices/{device}` (admin): the device leaves and cannot
/// join this network again with the same browser.
pub async fn remove_device(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((id, device_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Admin).await?;
    let removed = sqlx::query(
        "UPDATE devices SET status = 'removed' WHERE id = $2 AND network_id = $1 AND status <> 'removed'",
    )
    .bind(id)
    .bind(device_id)
    .execute(&state.db)
    .await?;
    if removed.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    state.refresh_device(device_id);
    Ok(StatusCode::NO_CONTENT)
}

// ---------- Members ----------

pub async fn members(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<MemberView>>, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Viewer).await?;
    let rows: Vec<(Uuid, String, String, String)> = sqlx::query_as(
        "SELECT a.id, a.name, a.email, m.role FROM memberships m
         JOIN accounts a ON a.id = m.account_id WHERE m.network_id = $1 ORDER BY m.created_at",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .filter_map(|(account_id, name, email, role)| {
                Some(MemberView {
                    account_id,
                    name,
                    email,
                    role: Role::parse(&role)?,
                })
            })
            .collect(),
    ))
}

async fn owners(state: &AppState, id: Uuid) -> Result<i64, AppError> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM memberships WHERE network_id = $1 AND role = 'owner'",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?)
}

/// `PATCH /api/networks/{id}/members/{account}` (owner). A network keeps an owner.
pub async fn update_member(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((id, account_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateMember>,
) -> Result<StatusCode, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Owner).await?;
    let current: Option<String> = sqlx::query_scalar(
        "SELECT role FROM memberships WHERE network_id = $1 AND account_id = $2",
    )
    .bind(id)
    .bind(account_id)
    .fetch_optional(&state.db)
    .await?;
    let current = current.ok_or(AppError::NotFound)?;
    if current == "owner" && body.role != Role::Owner && owners(&state, id).await? <= 1 {
        return Err(AppError::BadRequest("a network needs an owner".into()));
    }
    sqlx::query("UPDATE memberships SET role = $3 WHERE network_id = $1 AND account_id = $2")
        .bind(id)
        .bind(account_id)
        .bind(body.role.as_str())
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/networks/{id}/members/{account}`: an owner removes anyone, anyone leaves.
pub async fn remove_member(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((id, account_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let access = auth::network_access(&state, &method, &headers, id, Role::Viewer).await?;
    let me = access.account.as_ref().map(|a| a.id);
    if me != Some(account_id) && access.role < Role::Owner {
        return Err(AppError::Forbidden);
    }
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM memberships WHERE network_id = $1 AND account_id = $2",
    )
    .bind(id)
    .bind(account_id)
    .fetch_optional(&state.db)
    .await?;
    if role.as_deref() == Some("owner") && owners(&state, id).await? <= 1 {
        return Err(AppError::BadRequest("a network needs an owner".into()));
    }
    sqlx::query("DELETE FROM memberships WHERE network_id = $1 AND account_id = $2")
        .bind(id)
        .bind(account_id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/networks/{id}/invites` (admin): a link that adds a member with a role no
/// higher than the inviter's, and creates their account if they have none.
pub async fn invite(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<CreateInvite>,
) -> Result<(StatusCode, Json<CreatedInvite>), AppError> {
    let access = auth::network_access(&state, &method, &headers, id, Role::Admin).await?;
    let role = body.role.unwrap_or(Role::Viewer);
    if role > access.role {
        return Err(AppError::Forbidden);
    }
    let created_by = access.account.as_ref().map(|a| a.id);
    let created =
        create_invite(&state, body.email.as_deref(), Some((id, role)), created_by).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// Stores an invitation and returns its one-time link token.
pub async fn create_invite(
    state: &AppState,
    email: Option<&str>,
    network: Option<(Uuid, Role)>,
    created_by: Option<Uuid>,
) -> Result<CreatedInvite, AppError> {
    let token = auth::random_token();
    let email = email.map(str::trim).filter(|e| !e.is_empty());
    let expires_at: OffsetDateTime = sqlx::query_scalar(
        "INSERT INTO invites (token_hash, email, network_id, role, created_by, expires_at)
         VALUES ($1, $2, $3, $4, $5, now() + make_interval(days => $6)) RETURNING expires_at",
    )
    .bind(auth::token_hash(&token))
    .bind(email)
    .bind(network.map(|(id, _)| id))
    .bind(network.map(|(_, role)| role.as_str()))
    .bind(created_by)
    .bind(INVITE_DAYS)
    .fetch_one(&state.db)
    .await?;
    Ok(CreatedInvite {
        token,
        expires_at: db::unix_ms(expires_at),
    })
}

// ---------- API keys ----------

pub async fn api_keys(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ApiKeyView>>, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Admin).await?;
    let rows: Vec<(Uuid, String, OffsetDateTime, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT id, label, created_at, last_used_at FROM api_keys
         WHERE network_id = $1 AND revoked_at IS NULL ORDER BY created_at",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, label, created_at, last_used_at)| ApiKeyView {
                id,
                label,
                created_at: db::unix_ms(created_at),
                last_used_at: last_used_at.map(db::unix_ms),
            })
            .collect(),
    ))
}

pub async fn create_api_key(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<CreateApiKey>,
) -> Result<(StatusCode, Json<CreatedApiKey>), AppError> {
    let access = auth::network_access(&state, &method, &headers, id, Role::Admin).await?;
    let label = clean_name(&body.label)?;
    let key = format!("{}{}", auth::API_KEY_PREFIX, auth::random_token());
    let key_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO api_keys (id, network_id, token_hash, label, created_by) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(key_id)
    .bind(id)
    .bind(auth::token_hash(&key))
    .bind(&label)
    .bind(access.account.as_ref().map(|a| a.id))
    .execute(&state.db)
    .await?;
    Ok((StatusCode::CREATED, Json(CreatedApiKey { id: key_id, key })))
}

pub async fn revoke_api_key(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((id, key_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Admin).await?;
    let done = sqlx::query(
        "UPDATE api_keys SET revoked_at = now() WHERE id = $2 AND network_id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(key_id)
    .execute(&state.db)
    .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

// ---------- Jobs ----------

pub async fn jobs(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<JobSummary>>, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Viewer).await?;
    Ok(Json(db::recent_jobs(&state.db, id, 50).await?))
}

/// Creates a job in a network (the console, an API key, or for the public network the
/// submit token).
pub async fn start_job(
    state: &AppState,
    access: &Access,
    body: &CreateJob,
) -> Result<JobSummary, AppError> {
    let template = templates::get(&body.template)
        .ok_or_else(|| AppError::BadRequest(format!("unknown template {:?}", body.template)))?;
    let units = template
        .template
        .split(&body.params)
        .map_err(AppError::BadRequest)?;
    let placement = catalog::place(state, access, body, units.len() as u32).await?;
    let (program, params) = match &placement.program {
        Some((program, shown)) => (Some(program), shown),
        None => (None, &body.params),
    };
    let job = db::create_job(
        &state.db,
        db::NewJob {
            kernel_sha256: program
                .map(|p| p.sha256.clone())
                .or_else(|| template.sha256.clone()),
            program_id: program.map(|p| p.id),
            owner_network_id: placement.owner_network_id,
            ..db::NewJob::new(
                placement.network_id,
                template,
                params,
                units,
                placement.quorum,
            )
        },
    )
    .await?;
    tracing::info!(job_id = %job.id, network_id = %job.network_id, template = %job.template, units = job.units_total, "job created");
    state.publish(
        job.network_id,
        DashboardEvent::JobUpdated { job: job.clone() },
    );
    Ok(job)
}

pub async fn create_job(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<CreateJob>,
) -> Result<(StatusCode, Json<JobSummary>), AppError> {
    let access = auth::network_access(&state, &method, &headers, id, Role::Admin).await?;
    let job = start_job(&state, &access, &body).await?;
    Ok((StatusCode::CREATED, Json(job)))
}

/// A job of `network_id`, or one it runs on the public network; "not found" otherwise
/// (also for another network's job).
pub async fn job_in(
    state: &AppState,
    network_id: Uuid,
    job_id: Uuid,
) -> Result<JobSummary, AppError> {
    db::get_job(&state.db, job_id)
        .await?
        .filter(|j| j.network_id == network_id || j.owner_network_id == Some(network_id))
        .ok_or(AppError::NotFound)
}

pub async fn get_job(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<JobSummary>, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Viewer).await?;
    Ok(Json(job_in(&state, id, job_id).await?))
}

pub async fn cancel_job(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<JobSummary>, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Admin).await?;
    job_in(&state, id, job_id).await?;
    Ok(Json(crate::api::cancel(&state, job_id).await?))
}

pub async fn job_image(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<axum::response::Response, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Viewer).await?;
    job_in(&state, id, job_id).await?;
    crate::api::image(&state, job_id).await
}

/// `GET /api/join/{code}`: what a device sees before it joins.
#[derive(Serialize)]
pub struct JoinInfo {
    pub name: String,
    pub approval: bool,
}

pub async fn join_info(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<JoinInfo>, AppError> {
    let rules = by_join_code(&state.db, &code)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(JoinInfo {
        name: rules.name,
        approval: rules.join_mode == JoinMode::Approval,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::api::HoursWindow;

    fn rules() -> NetworkRules {
        NetworkRules {
            id: Uuid::now_v7(),
            name: "Lab".into(),
            join_code: "abc".into(),
            join_mode: JoinMode::Open,
            max_power: 100,
            hours: None,
            paused: false,
            lend_idle: false,
            created_at: OffsetDateTime::now_utc(),
        }
    }

    fn device(status: &str, paused: bool) -> DeviceRecord {
        DeviceRecord {
            id: Uuid::now_v7(),
            name: None,
            status: status.into(),
            paused,
        }
    }

    #[test]
    fn control_follows_the_owners_decisions_in_order() {
        let now = Timestamp::now();
        let mut r = rules();
        assert_eq!(
            control(&r, &device("active", false), now),
            DeviceControl::Running
        );
        assert_eq!(
            control(&r, &device("active", true), now),
            DeviceControl::PausedByOwner
        );
        assert_eq!(
            control(&r, &device("pending", false), now),
            DeviceControl::PendingApproval
        );
        assert_eq!(
            control(&r, &device("removed", false), now),
            DeviceControl::Removed
        );
        r.paused = true;
        assert_eq!(
            control(&r, &device("active", false), now),
            DeviceControl::NetworkPaused
        );
        r.paused = false;
        // Hours that never open: always outside, with no next window.
        r.hours = Some(Hours {
            timezone: "UTC".into(),
            windows: vec![],
        });
        assert_eq!(
            control(&r, &device("active", false), now),
            DeviceControl::OutsideHours { resumes_at: None }
        );
        r.hours = Some(Hours {
            timezone: "UTC".into(),
            windows: vec![HoursWindow {
                days: vec![1, 2, 3, 4, 5, 6, 7],
                from: "00:00".into(),
                to: "00:00".into(),
            }],
        });
        assert_eq!(
            control(&r, &device("active", false), now),
            DeviceControl::Running
        );
    }

    #[test]
    fn join_codes_avoid_confusable_characters() {
        let code = new_join_code();
        assert_eq!(code.len(), 10);
        assert!(!code.contains(['0', 'o', '1', 'l', 'i']));
        assert_ne!(code, new_join_code());
    }
}

/// The network sweep job `job_id` of network `id` (spec 0011), and its parameters.
async fn sweep_in(
    state: &AppState,
    id: Uuid,
    job_id: Uuid,
) -> Result<protocol::templates::nn::JobParams, AppError> {
    let job = job_in(state, id, job_id).await?;
    if job.template != protocol::templates::nn::ID {
        return Err(AppError::NotFound);
    }
    serde_json::from_value(job.params).map_err(|e| AppError::BadRequest(e.to_string()))
}

/// `GET …/jobs/{job}/leaderboard`: the configurations ranked by the units decided so
/// far, so the console can show the sweep while it runs.
pub async fn leaderboard(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<protocol::templates::nn::JobResult>, AppError> {
    auth::network_access(&state, &method, &headers, id, Role::Viewer).await?;
    let params = sweep_in(&state, id, job_id).await?;
    let rows: Vec<(serde_json::Value, serde_json::Value)> = sqlx::query_as(
        "SELECT params, output FROM units WHERE job_id = $1 AND status = 'done' AND output IS NOT NULL",
    )
    .bind(job_id)
    .fetch_all(&state.db)
    .await?;
    let decided = rows
        .into_iter()
        .filter_map(|(p, o)| {
            Some((
                serde_json::from_value(p).ok()?,
                serde_json::from_value(o).ok()?,
            ))
        })
        .collect::<Vec<_>>();
    let board =
        crate::templates::nn::leaderboard(&params, &decided).map_err(AppError::BadRequest)?;
    Ok(Json(board))
}

/// One layer of a downloaded model: `weights[i * outputs + o]` connects input `i` to
/// output `o`; values are the trained master weights as real numbers.
#[derive(Serialize)]
pub struct ModelLayer {
    pub inputs: usize,
    pub outputs: usize,
    pub activation: &'static str,
    pub weights: Vec<f64>,
    pub biases: Vec<f64>,
}

#[derive(Serialize)]
pub struct Model {
    pub dataset: String,
    pub input: &'static str,
    pub hidden: u32,
    pub layers: u32,
    pub learning_rate: f64,
    pub batch: u32,
    pub steps: u32,
    pub seed: u32,
    /// Validation accuracy of exactly these weights (0..1).
    pub accuracy: f64,
    /// SHA-256 of the integer weights, as the devices reported it.
    pub digest: String,
    pub network: Vec<ModelLayer>,
}

/// `GET …/jobs/{job}/model.json`: the best configuration's network. Devices never
/// upload weights; training is deterministic, so the coordinator retrains the first
/// run of the best configuration and gets the very weights the devices agreed on.
pub async fn model(
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    Path((id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<axum::response::Response, AppError> {
    use axum::response::IntoResponse;
    use protocol::templates::nn::UnitParams;

    auth::network_access(&state, &method, &headers, id, Role::Viewer).await?;
    let job = job_in(&state, id, job_id).await?;
    let params = sweep_in(&state, id, job_id).await?;
    let result: protocol::templates::nn::JobResult = job
        .result
        .and_then(|r| serde_json::from_value(r).ok())
        .ok_or_else(|| AppError::Conflict("the sweep has not finished".into()))?;
    let best = result.rows.first().ok_or(AppError::NotFound)?;
    let units = crate::templates::get(protocol::templates::nn::ID)
        .expect("registered")
        .template
        .split(&job.params)
        .map_err(AppError::BadRequest)?;
    let unit: UnitParams = units
        .into_iter()
        .filter_map(|u| serde_json::from_value::<UnitParams>(u).ok())
        .find(|u| u.config == best.config)
        .ok_or(AppError::NotFound)?;
    let learning_rate = best.learning_rate;
    // Seconds of CPU: off the async runtime.
    let (output, trained) =
        tokio::task::spawn_blocking(move || crate::templates::nn::train_unit(&unit))
            .await
            .map_err(|e| AppError::BadRequest(e.to_string()))?
            .map_err(AppError::BadRequest)?;
    let n = trained.layers.len();
    let real = |v: &i32| f64::from(*v) / f64::from(1 << 24);
    let model = Model {
        dataset: params.dataset.clone(),
        input: "14 × 14 grey pixels, row by row, each 0..1 (byte / 256)",
        hidden: best.hidden,
        layers: best.layers,
        learning_rate,
        batch: best.batch,
        steps: params.steps,
        seed: params.seed,
        accuracy: f64::from(output.correct) / f64::from(result.validation),
        digest: output.digest,
        network: trained
            .layers
            .iter()
            .enumerate()
            .map(|(l, layer)| ModelLayer {
                inputs: layer.fan_in,
                outputs: layer.fan_out,
                activation: if l + 1 < n { "relu" } else { "softmax" },
                weights: layer.w.iter().map(real).collect(),
                biases: layer.b.iter().map(real).collect(),
            })
            .collect(),
    };
    let name = format!("attachment; filename=\"royak-model-{job_id}.json\"");
    Ok((
        [(axum::http::header::CONTENT_DISPOSITION, name)],
        Json(model),
    )
        .into_response())
}

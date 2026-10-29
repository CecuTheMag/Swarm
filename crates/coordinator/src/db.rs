//! PostgreSQL persistence. Every state transition that must be atomic is a
//! single transaction here, so correctness does not depend on how many
//! coordinator instances run.

use crate::templates;
use protocol::{DeviceInfo, JobStatus, JobSummary, TemplateRef};
use serde_json::Value;
use sqlx::{FromRow, PgPool, Postgres, Transaction, types::Json};
use std::time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;

pub fn unix_ms(t: OffsetDateTime) -> i64 {
    (t.unix_timestamp_nanos() / 1_000_000) as i64
}

#[derive(FromRow)]
struct JobRow {
    id: Uuid,
    template: String,
    status: String,
    params: Json<Value>,
    units_total: i32,
    units_done: i32,
    result: Option<Json<Value>>,
    created_at: OffsetDateTime,
    finished_at: Option<OffsetDateTime>,
}

impl From<JobRow> for JobSummary {
    fn from(r: JobRow) -> Self {
        JobSummary {
            id: r.id,
            template: r.template,
            // The CHECK constraint guarantees one of the known values.
            status: JobStatus::parse(&r.status).unwrap_or(JobStatus::Failed),
            params: r.params.0,
            units_total: r.units_total as u32,
            units_done: r.units_done as u32,
            result: r.result.map(|j| j.0),
            created_at: unix_ms(r.created_at),
            finished_at: r.finished_at.map(unix_ms),
        }
    }
}

/// Columns of `JobRow`; a macro so queries stay `&'static str` (checked by sqlx).
macro_rules! job_columns {
    () => {
        "id, template, status, params, units_total, units_done, result, created_at, finished_at"
    };
}

/// Creates a job and all its units in one transaction.
pub async fn create_job(
    db: &PgPool,
    template: &templates::Registered,
    params: &Value,
    unit_params: Vec<Value>,
) -> sqlx::Result<JobSummary> {
    let job_id = Uuid::now_v7();
    let mut tx = db.begin().await?;
    let job: JobRow = sqlx::query_as(concat!(
        "INSERT INTO jobs (id, template, template_version, params, status, units_total)
         VALUES ($1, $2, $3, $4, 'queued', $5)
         RETURNING ",
        job_columns!(),
        ""
    ))
    .bind(job_id)
    .bind(template.template.id())
    .bind(template.template.version() as i32)
    .bind(Json(params))
    .bind(unit_params.len() as i32)
    .fetch_one(&mut *tx)
    .await?;

    let ids: Vec<Uuid> = (0..unit_params.len()).map(|_| Uuid::now_v7()).collect();
    let seqs: Vec<i32> = (0..unit_params.len() as i32).collect();
    sqlx::query(
        "INSERT INTO units (id, job_id, seq, params, status, created_at)
         SELECT id, $2, seq, params, 'pending', $5
         FROM unnest($1::uuid[], $3::int[], $4::jsonb[]) AS u(id, seq, params)",
    )
    .bind(&ids)
    .bind(job_id)
    .bind(&seqs)
    .bind(&unit_params)
    .bind(job.created_at)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(job.into())
}

pub async fn get_job(db: &PgPool, id: Uuid) -> sqlx::Result<Option<JobSummary>> {
    let row: Option<JobRow> = sqlx::query_as(concat!(
        "SELECT ",
        job_columns!(),
        " FROM jobs WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(db)
    .await?;
    Ok(row.map(Into::into))
}

pub async fn recent_jobs(db: &PgPool, limit: i64) -> sqlx::Result<Vec<JobSummary>> {
    let rows: Vec<JobRow> = sqlx::query_as(concat!(
        "SELECT ",
        job_columns!(),
        " FROM jobs ORDER BY created_at DESC LIMIT $1"
    ))
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn insert_worker(
    db: &PgPool,
    id: Uuid,
    device: &DeviceInfo,
    client_version: &str,
) -> sqlx::Result<OffsetDateTime> {
    let backend = serde_json::to_value(device.backend).expect("enum serializes");
    sqlx::query_scalar(
        "INSERT INTO workers (id, backend, vendor, architecture, client_version)
         VALUES ($1, $2, $3, $4, $5) RETURNING connected_at",
    )
    .bind(id)
    .bind(backend.as_str().unwrap_or_default())
    .bind(&device.vendor)
    .bind(&device.architecture)
    .bind(client_version)
    .fetch_one(db)
    .await
}

pub async fn mark_worker_disconnected(db: &PgPool, id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE workers SET disconnected_at = now(), last_seen_at = now() WHERE id = $1")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

pub struct Claim {
    pub lease_id: Uuid,
    pub unit_id: Uuid,
    pub job_id: Uuid,
    pub template: TemplateRef,
    pub params: Value,
    pub expires_at: OffsetDateTime,
    /// Set when this claim moved the job from queued to running.
    pub job_started: Option<JobSummary>,
}

/// Leases the next pending unit to `worker_id`, or returns `None` if the queue
/// is empty. `SKIP LOCKED` lets concurrent claimers pass over rows another
/// transaction is taking, so no unit is ever handed out twice.
pub async fn claim_unit(
    db: &PgPool,
    worker_id: Uuid,
    lease_ttl: Duration,
) -> sqlx::Result<Option<Claim>> {
    let mut tx = db.begin().await?;
    let unit: Option<(Uuid, Uuid, Json<Value>)> = sqlx::query_as(
        "WITH next AS (
             SELECT id FROM units
             WHERE status = 'pending'
             ORDER BY created_at, job_id, seq
             LIMIT 1
             FOR UPDATE SKIP LOCKED
         )
         UPDATE units SET status = 'leased'
         FROM next WHERE units.id = next.id
         RETURNING units.id, units.job_id, units.params",
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some((unit_id, job_id, Json(params))) = unit else {
        return Ok(None);
    };

    let (template_id, template_version): (String, i32) =
        sqlx::query_as("SELECT template, template_version FROM jobs WHERE id = $1")
            .bind(job_id)
            .fetch_one(&mut *tx)
            .await?;

    let lease_id = Uuid::now_v7();
    let expires_at: OffsetDateTime = sqlx::query_scalar(
        "INSERT INTO leases (id, unit_id, worker_id, expires_at)
         VALUES ($1, $2, $3, now() + $4)
         RETURNING expires_at",
    )
    .bind(lease_id)
    .bind(unit_id)
    .bind(worker_id)
    .bind(lease_ttl)
    .fetch_one(&mut *tx)
    .await?;

    let job_started: Option<JobRow> = sqlx::query_as(concat!(
        "UPDATE jobs SET status = 'running' WHERE id = $1 AND status = 'queued'
         RETURNING ",
        job_columns!(),
        ""
    ))
    .bind(job_id)
    .fetch_optional(&mut *tx)
    .await?;

    tx.commit().await?;

    let sha256 = templates::get(&template_id)
        .map(|t| t.sha256.clone())
        .unwrap_or_default();
    Ok(Some(Claim {
        lease_id,
        unit_id,
        job_id,
        template: TemplateRef {
            id: template_id,
            version: template_version as u32,
            sha256,
        },
        params,
        expires_at,
        job_started: job_started.map(Into::into),
    }))
}

#[derive(Debug)]
pub enum Completion {
    /// The unit's result was recorded.
    Accepted {
        job_id: Uuid,
        unit_id: Uuid,
        unit_params: Value,
        units_done: u32,
        units_total: u32,
        /// Present when this completion finished the job.
        job_finished: Option<JobSummary>,
    },
    /// The lease is unknown, not owned by this worker, or no longer active.
    StaleLease,
    /// The output failed template validation; the unit was returned to the queue.
    InvalidOutput(String),
    /// Another lease already completed this unit (possible once units are
    /// executed redundantly); the result was stored but not counted.
    AlreadyDone,
}

/// Records a worker's result for a lease. Progress is counted with an atomic
/// increment on the job row: the row lock serialises concurrent completions,
/// so exactly one of them observes the final count and aggregates the job.
pub async fn complete_lease(
    db: &PgPool,
    worker_id: Uuid,
    lease_id: Uuid,
    output: &Value,
    elapsed_ms: u32,
) -> sqlx::Result<Completion> {
    let mut tx = db.begin().await?;

    let lease: Option<(Uuid, Uuid, String, Json<Value>)> = sqlx::query_as(
        "SELECT u.id, u.job_id, j.template, u.params
         FROM leases l
         JOIN units u ON u.id = l.unit_id
         JOIN jobs j ON j.id = u.job_id
         WHERE l.id = $1 AND l.worker_id = $2
           AND l.completed_at IS NULL AND l.released_at IS NULL
         FOR UPDATE OF l",
    )
    .bind(lease_id)
    .bind(worker_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((unit_id, job_id, template_id, Json(unit_params))) = lease else {
        return Ok(Completion::StaleLease);
    };

    let Some(template) = templates::get(&template_id) else {
        return Ok(Completion::InvalidOutput(format!(
            "unknown template {template_id}"
        )));
    };
    if let Err(reason) = template.template.validate_output(&unit_params, output) {
        release_leases(&mut tx, &[lease_id]).await?;
        tx.commit().await?;
        return Ok(Completion::InvalidOutput(reason));
    }

    sqlx::query(
        "UPDATE leases SET completed_at = now(), output = $2, elapsed_ms = $3 WHERE id = $1",
    )
    .bind(lease_id)
    .bind(Json(output))
    .bind(elapsed_ms as i32)
    .execute(&mut *tx)
    .await?;

    let newly_done = sqlx::query(
        "UPDATE units SET status = 'done', output = $2 WHERE id = $1 AND status <> 'done'",
    )
    .bind(unit_id)
    .bind(Json(output))
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    if !newly_done {
        tx.commit().await?;
        return Ok(Completion::AlreadyDone);
    }

    let (units_done, units_total, job_params): (i32, i32, Json<Value>) = sqlx::query_as(
        "UPDATE jobs SET units_done = units_done + 1 WHERE id = $1
         RETURNING units_done, units_total, params",
    )
    .bind(job_id)
    .fetch_one(&mut *tx)
    .await?;

    let job_finished = if units_done == units_total {
        let outputs: Vec<Json<Value>> =
            sqlx::query_scalar("SELECT output FROM units WHERE job_id = $1 ORDER BY seq")
                .bind(job_id)
                .fetch_all(&mut *tx)
                .await?;
        let outputs: Vec<Value> = outputs.into_iter().map(|j| j.0).collect();
        let (status, result) = match template.template.aggregate(&job_params.0, &outputs) {
            Ok(result) => ("done", Some(result)),
            Err(e) => {
                tracing::error!(%job_id, error = %e, "aggregation failed");
                ("failed", None)
            }
        };
        let row: JobRow = sqlx::query_as(concat!(
            "UPDATE jobs SET status = $2, result = $3, finished_at = now() WHERE id = $1
             RETURNING ",
            job_columns!(),
            ""
        ))
        .bind(job_id)
        .bind(status)
        .bind(result.map(Json))
        .fetch_one(&mut *tx)
        .await?;
        Some(row.into())
    } else {
        None
    };

    tx.commit().await?;
    Ok(Completion::Accepted {
        job_id,
        unit_id,
        unit_params,
        units_done: units_done as u32,
        units_total: units_total as u32,
        job_finished,
    })
}

/// Releases one lease held by `worker_id` (the worker reported failure).
/// Returns whether a lease was released.
pub async fn release_lease(db: &PgPool, worker_id: Uuid, lease_id: Uuid) -> sqlx::Result<bool> {
    let mut tx = db.begin().await?;
    let owned: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM leases WHERE id = $1 AND worker_id = $2
           AND completed_at IS NULL AND released_at IS NULL FOR UPDATE",
    )
    .bind(lease_id)
    .bind(worker_id)
    .fetch_optional(&mut *tx)
    .await?;
    let released = match owned {
        Some(id) => release_leases(&mut tx, &[id]).await? == 1,
        None => false,
    };
    tx.commit().await?;
    Ok(released)
}

/// Releases every active lease of a worker (it disconnected). Returns the count.
pub async fn release_worker_leases(db: &PgPool, worker_id: Uuid) -> sqlx::Result<u64> {
    let mut tx = db.begin().await?;
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM leases WHERE worker_id = $1
           AND completed_at IS NULL AND released_at IS NULL FOR UPDATE",
    )
    .bind(worker_id)
    .fetch_all(&mut *tx)
    .await?;
    let n = release_leases(&mut tx, &ids).await?;
    tx.commit().await?;
    Ok(n)
}

/// Marks leases released and returns their units to the queue unless another
/// lease already completed them.
async fn release_leases(tx: &mut Transaction<'_, Postgres>, ids: &[Uuid]) -> sqlx::Result<u64> {
    if ids.is_empty() {
        return Ok(0);
    }
    let units: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE leases SET released_at = now() WHERE id = ANY($1) RETURNING unit_id",
    )
    .bind(ids)
    .fetch_all(&mut **tx)
    .await?;
    sqlx::query("UPDATE units SET status = 'pending' WHERE id = ANY($1) AND status = 'leased'")
        .bind(&units)
        .execute(&mut **tx)
        .await?;
    Ok(units.len() as u64)
}

/// Extends the given leases if they are still active and owned by the worker.
pub async fn heartbeat(
    db: &PgPool,
    worker_id: Uuid,
    lease_ids: &[Uuid],
    lease_ttl: Duration,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE workers SET last_seen_at = now() WHERE id = $1")
        .bind(worker_id)
        .execute(db)
        .await?;
    if !lease_ids.is_empty() {
        sqlx::query(
            "UPDATE leases SET expires_at = now() + $3
             WHERE id = ANY($1) AND worker_id = $2
               AND completed_at IS NULL AND released_at IS NULL",
        )
        .bind(lease_ids)
        .bind(worker_id)
        .bind(lease_ttl)
        .execute(db)
        .await?;
    }
    Ok(())
}

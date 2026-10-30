//! PostgreSQL persistence. Every state transition that must be atomic is a
//! single transaction here, so correctness does not depend on how many
//! coordinator instances run.

use crate::{state::Config, templates};
use protocol::{DeviceInfo, JobStatus, JobSummary, TemplateRef};
use serde_json::Value;
use sqlx::{FromRow, PgPool, Postgres, Transaction, types::Json};
use time::OffsetDateTime;
use uuid::Uuid;

/// Runs a transaction again when PostgreSQL aborted it to resolve a deadlock or a
/// serialization conflict. Such a transaction was rolled back completely, so running
/// it again is safe. Concurrent completions, releases and bans touch overlapping rows
/// (a ban reverses verdicts on other clients' leases), so deadlocks can happen.
pub async fn retrying<T, F, Fut>(mut tx: F) -> sqlx::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = sqlx::Result<T>>,
{
    const ATTEMPTS: u32 = 5;
    let mut attempt = 1;
    loop {
        match tx().await {
            Err(sqlx::Error::Database(e))
                if attempt < ATTEMPTS && matches!(e.code().as_deref(), Some("40P01" | "40001")) =>
            {
                tracing::debug!(attempt, code = ?e.code(), "retrying transaction");
                // Random back-off so the two sides do not collide again.
                let ms = fastrand::u64(1..=(5 << attempt));
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                attempt += 1;
            }
            result => return result,
        }
    }
}

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
    error: Option<String>,
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
            error: r.error,
            created_at: unix_ms(r.created_at),
            finished_at: r.finished_at.map(unix_ms),
        }
    }
}

/// Columns of `JobRow`; a macro so queries stay `&'static str` (checked by sqlx).
macro_rules! job_columns {
    () => {
        "id, template, status, params, units_total, units_done, result, error, created_at, finished_at"
    };
}

/// Creates a job and all its units in one transaction. Each unit needs `redundancy`
/// agreeing results (spec 0004).
pub async fn create_job(
    db: &PgPool,
    template: &templates::Registered,
    params: &Value,
    unit_params: Vec<Value>,
    redundancy: u32,
) -> sqlx::Result<JobSummary> {
    let job_id = Uuid::now_v7();
    let mut tx = db.begin().await?;
    let job: JobRow = sqlx::query_as(concat!(
        "INSERT INTO jobs (id, template, template_version, template_sha256, params, status,
                           units_total)
         VALUES ($1, $2, $3, $4, $5, 'queued', $6)
         RETURNING ",
        job_columns!(),
        ""
    ))
    .bind(job_id)
    .bind(template.template.id())
    .bind(template.template.version() as i32)
    .bind(&template.sha256)
    .bind(Json(params))
    .bind(unit_params.len() as i32)
    .fetch_one(&mut *tx)
    .await?;

    let ids: Vec<Uuid> = (0..unit_params.len()).map(|_| Uuid::now_v7()).collect();
    let seqs: Vec<i32> = (0..unit_params.len() as i32).collect();
    sqlx::query(
        "INSERT INTO units (id, job_id, seq, params, status, created_at, target)
         SELECT id, $2, seq, params, 'pending', $5, $6
         FROM unnest($1::uuid[], $3::int[], $4::jsonb[]) AS u(id, seq, params)",
    )
    .bind(&ids)
    .bind(job_id)
    .bind(&seqs)
    .bind(&unit_params)
    .bind(job.created_at)
    .bind(redundancy.max(1) as i32)
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

/// Registers a worker connection under its client, creating the client on first
/// sight. Returns when the connection started, or `None` if the client is banned.
pub async fn insert_worker(
    db: &PgPool,
    id: Uuid,
    device: &DeviceInfo,
    client_version: &str,
    client_id: Uuid,
) -> sqlx::Result<Option<OffsetDateTime>> {
    let mut tx = db.begin().await?;
    sqlx::query("INSERT INTO clients (id) VALUES ($1) ON CONFLICT (id) DO NOTHING")
        .bind(client_id)
        .execute(&mut *tx)
        .await?;
    let banned: Option<OffsetDateTime> =
        sqlx::query_scalar("SELECT banned_at FROM clients WHERE id = $1")
            .bind(client_id)
            .fetch_one(&mut *tx)
            .await?;
    if banned.is_some() {
        tx.commit().await?;
        return Ok(None);
    }
    let backend = serde_json::to_value(device.backend).expect("enum serializes");
    let connected_at = sqlx::query_scalar(
        "INSERT INTO workers (id, backend, vendor, architecture, client_version, client_id)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING connected_at",
    )
    .bind(id)
    .bind(backend.as_str().unwrap_or_default())
    .bind(&device.vendor)
    .bind(&device.architecture)
    .bind(client_version)
    .bind(client_id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some(connected_at))
}

/// Whether the worker's client is banned.
pub async fn worker_banned(db: &PgPool, worker_id: Uuid) -> sqlx::Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM workers w JOIN clients c ON c.id = w.client_id
                        WHERE w.id = $1 AND c.banned_at IS NOT NULL)",
    )
    .bind(worker_id)
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

/// A worker's client and its standing.
struct ClientStanding {
    id: Uuid,
    correct: i32,
    incorrect: i32,
    banned: bool,
}

async fn client_of(
    tx: &mut Transaction<'_, Postgres>,
    worker_id: Uuid,
) -> sqlx::Result<ClientStanding> {
    let row: Option<(Uuid, i32, i32, bool)> = sqlx::query_as(
        "SELECT c.id, c.correct, c.incorrect, c.banned_at IS NOT NULL
         FROM workers w JOIN clients c ON c.id = w.client_id WHERE w.id = $1",
    )
    .bind(worker_id)
    .fetch_optional(&mut **tx)
    .await?;
    // Workers registered before spec 0004 have no client: each is its own, untrusted.
    let (id, correct, incorrect, banned) = row.unwrap_or((worker_id, 0, 0, false));
    Ok(ClientStanding {
        id,
        correct,
        incorrect,
        banned,
    })
}

/// Leases a unit to `worker_id`, or returns `None` if there is nothing for it (or its
/// client is banned).
///
/// Units are taken in job order. A unit stays claimable until enough copies are out
/// to reach its `target` number of results, so redundant copies run in parallel, and a
/// client never gets a unit it holds or has answered: one client cannot verify itself.
/// A client whose lease of a unit was released may get it again.
/// `SKIP LOCKED` lets concurrent claimers pass over rows another transaction is taking.
/// Sometimes the answer is a copy of an already verified unit instead (a honeypot), and
/// a trusted client may get a unit to compute alone (spec 0004).
pub async fn claim_unit(db: &PgPool, worker_id: Uuid, cfg: &Config) -> sqlx::Result<Option<Claim>> {
    let mut tx = db.begin().await?;
    let client = client_of(&mut tx, worker_id).await?;
    if client.banned {
        return Ok(None);
    }
    let trusted = cfg.is_trusted(client.correct, client.incorrect);

    let honeypot_rate = if trusted {
        cfg.honeypot_rate
    } else {
        cfg.new_client_honeypot_rate
    };
    if fastrand::f64() < honeypot_rate
        && let Some(claim) = claim_honeypot(&mut tx, worker_id, client.id, cfg).await?
    {
        tx.commit().await?;
        return Ok(Some(claim));
    }

    let unit: Option<(Uuid, Uuid, Json<Value>, i32)> = sqlx::query_as(
        "SELECT u.id, u.job_id, u.params, u.target FROM units u
         WHERE u.status = 'pending'
           AND NOT EXISTS (
               SELECT 1 FROM leases l JOIN workers w ON w.id = l.worker_id
               WHERE l.unit_id = u.id AND l.released_at IS NULL
                 AND (w.client_id = $1 OR l.worker_id = $2)
           )
         ORDER BY u.created_at, u.job_id, u.seq
         LIMIT 1
         FOR UPDATE OF u SKIP LOCKED",
    )
    .bind(client.id)
    .bind(worker_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((unit_id, job_id, Json(params), target)) = unit else {
        return Ok(None);
    };

    let (active, results) = unit_copies(&mut tx, unit_id).await?;
    let solo = cfg.redundancy > 1
        && trusted
        && active == 0
        && results == 0
        && target == cfg.redundancy as i32
        && fastrand::f64() >= cfg.spot_check_rate;
    if solo || active + results + 1 >= i64::from(target) {
        sqlx::query("UPDATE units SET status = 'leased' WHERE id = $1")
            .bind(unit_id)
            .execute(&mut *tx)
            .await?;
    }

    let (lease_id, expires_at) =
        insert_lease(&mut tx, unit_id, worker_id, cfg, false, solo).await?;

    let job_started: Option<JobRow> = sqlx::query_as(concat!(
        "UPDATE jobs SET status = 'running' WHERE id = $1 AND status = 'queued'
         RETURNING ",
        job_columns!(),
        ""
    ))
    .bind(job_id)
    .fetch_optional(&mut *tx)
    .await?;
    let template = template_ref(&mut tx, job_id).await?;
    tx.commit().await?;

    Ok(Some(Claim {
        lease_id,
        unit_id,
        job_id,
        template,
        params,
        expires_at,
        job_started: job_started.map(Into::into),
    }))
}

/// A copy of a unit verified in a finished job, as long as this client has not
/// answered it.
async fn claim_honeypot(
    tx: &mut Transaction<'_, Postgres>,
    worker_id: Uuid,
    client_id: Uuid,
    cfg: &Config,
) -> sqlx::Result<Option<Claim>> {
    // A few random picks: a client that has answered many of the known units would
    // otherwise rarely be given one.
    let mut unit = None;
    for _ in 0..HONEYPOT_PICKS {
        unit = pick_honeypot(tx, worker_id, client_id).await?;
        if unit.is_some() {
            break;
        }
    }
    let Some((unit_id, job_id, Json(params))) = unit else {
        return Ok(None);
    };
    let template = template_ref(tx, job_id).await?;
    if template.sha256.is_empty() {
        // The template is no longer registered.
        return Ok(None);
    }
    let (lease_id, expires_at) = insert_lease(tx, unit_id, worker_id, cfg, true, false).await?;
    Ok(Some(Claim {
        lease_id,
        unit_id,
        job_id,
        template,
        params,
        expires_at,
        job_started: None,
    }))
}

const HONEYPOT_PICKS: usize = 4;

/// A random verified unit of a random finished job that this client has not answered.
/// Only jobs computed with today's kernels qualify: a known answer from another kernel
/// would fail honest workers.
async fn pick_honeypot(
    tx: &mut Transaction<'_, Postgres>,
    worker_id: Uuid,
    client_id: Uuid,
) -> sqlx::Result<Option<(Uuid, Uuid, Json<Value>)>> {
    sqlx::query_as(
        "SELECT u.id, u.job_id, u.params
         FROM (SELECT id, units_total FROM jobs
               WHERE status = 'done' AND template_sha256 = ANY($3)
               ORDER BY random() LIMIT 1) j
         JOIN units u ON u.job_id = j.id
             AND u.seq = floor(random() * j.units_total)::int
             AND u.status = 'done'
         WHERE NOT EXISTS (
             SELECT 1 FROM leases l JOIN workers w ON w.id = l.worker_id
             WHERE l.unit_id = u.id AND l.released_at IS NULL
                 AND (w.client_id = $1 OR l.worker_id = $2)
         )",
    )
    .bind(client_id)
    .bind(worker_id)
    .bind(templates::current_hashes())
    .fetch_optional(&mut **tx)
    .await
}

async fn insert_lease(
    tx: &mut Transaction<'_, Postgres>,
    unit_id: Uuid,
    worker_id: Uuid,
    cfg: &Config,
    honeypot: bool,
    solo: bool,
) -> sqlx::Result<(Uuid, OffsetDateTime)> {
    let lease_id = Uuid::now_v7();
    let expires_at = sqlx::query_scalar(
        "INSERT INTO leases (id, unit_id, worker_id, expires_at, honeypot, solo)
         VALUES ($1, $2, $3, now() + $4, $5, $6)
         RETURNING expires_at",
    )
    .bind(lease_id)
    .bind(unit_id)
    .bind(worker_id)
    .bind(cfg.lease_ttl)
    .bind(honeypot)
    .bind(solo)
    .fetch_one(&mut **tx)
    .await?;
    Ok((lease_id, expires_at))
}

async fn template_ref(
    tx: &mut Transaction<'_, Postgres>,
    job_id: Uuid,
) -> sqlx::Result<TemplateRef> {
    let (id, version): (String, i32) =
        sqlx::query_as("SELECT template, template_version FROM jobs WHERE id = $1")
            .bind(job_id)
            .fetch_one(&mut **tx)
            .await?;
    let sha256 = templates::get(&id)
        .map(|t| t.sha256.clone())
        .unwrap_or_default();
    Ok(TemplateRef {
        id,
        version: version as u32,
        sha256,
    })
}

/// Copies of a unit still out, and results that still count towards deciding it
/// (undecided, not from a banned client).
async fn unit_copies(
    tx: &mut Transaction<'_, Postgres>,
    unit_id: Uuid,
) -> sqlx::Result<(i64, i64)> {
    sqlx::query_as(
        "SELECT
             count(*) FILTER (WHERE l.completed_at IS NULL AND l.released_at IS NULL),
             count(*) FILTER (WHERE l.completed_at IS NOT NULL AND l.verdict IS NULL
                                    AND c.banned_at IS NULL)
         FROM leases l
         JOIN workers w ON w.id = l.worker_id
         LEFT JOIN clients c ON c.id = w.client_id
         WHERE l.unit_id = $1 AND NOT l.honeypot",
    )
    .bind(unit_id)
    .fetch_one(&mut **tx)
    .await
}

#[derive(Debug)]
pub enum Completion {
    /// The result was recorded; the unit waits for more results to compare with.
    Recorded,
    /// This result decided the unit. `output` is the accepted one; `agreed` says
    /// whether this worker's result was among those that agreed.
    Decided {
        job_id: Uuid,
        unit_id: Uuid,
        unit_params: Value,
        output: Value,
        agreed: bool,
        units_done: u32,
        units_total: u32,
        /// Present when this completion finished the job.
        job_finished: Option<JobSummary>,
        /// Jobs whose solo results were reopened because their client was banned.
        reopened: Vec<Uuid>,
    },
    /// The results disagree; the unit asks one more client to break the tie.
    Disputed { reopened: Vec<Uuid> },
    /// The results never agreed: the unit and its job failed.
    Unresolved { job: Option<JobSummary> },
    /// A copy of a verified unit; `correct` says whether the result matched.
    Honeypot { correct: bool, reopened: Vec<Uuid> },
    /// The unit was already decided; the result only counted for reputation.
    Late { agreed: bool, reopened: Vec<Uuid> },
    /// The lease is unknown, not owned by this worker, or no longer active.
    StaleLease,
    /// The output failed template validation; the unit was returned to the queue
    /// (or, released too often, failed its job).
    InvalidOutput {
        reason: String,
        released: Released,
        reopened: Vec<Uuid>,
    },
}

impl Completion {
    /// Whether the worker is told its result was accepted. Honeypots always say yes,
    /// so they cannot be told apart from real work.
    pub fn accepted(&self) -> bool {
        match self {
            Self::Recorded | Self::Disputed { .. } | Self::Honeypot { .. } => true,
            Self::Decided { agreed, .. } | Self::Late { agreed, .. } => *agreed,
            Self::Unresolved { .. } | Self::StaleLease | Self::InvalidOutput { .. } => false,
        }
    }

    /// Jobs whose accepted results were reopened by a ban in this completion.
    pub fn reopened(&self) -> &[Uuid] {
        match self {
            Self::Decided { reopened, .. }
            | Self::Disputed { reopened }
            | Self::Honeypot { reopened, .. }
            | Self::Late { reopened, .. }
            | Self::InvalidOutput { reopened, .. } => reopened,
            _ => &[],
        }
    }
}

/// Records a worker's result for a lease and, when enough results are in, decides the
/// unit (spec 0004). The unit row lock serialises completions of one unit; the job row
/// lock serialises progress, so exactly one completion finishes and aggregates the job.
pub async fn complete_lease(
    db: &PgPool,
    worker_id: Uuid,
    lease_id: Uuid,
    output: &Value,
    elapsed_ms: u32,
    cfg: &Config,
) -> sqlx::Result<Completion> {
    let mut tx = db.begin().await?;

    let lease: Option<(Uuid, Uuid, String, Json<Value>, bool, bool)> = sqlx::query_as(
        "SELECT u.id, u.job_id, j.template, u.params, l.honeypot, l.solo
         FROM leases l
         JOIN units u ON u.id = l.unit_id
         JOIN jobs j ON j.id = u.job_id
         WHERE l.id = $1 AND l.worker_id = $2
           AND l.completed_at IS NULL AND l.released_at IS NULL
           AND (l.honeypot OR j.status IN ('queued', 'running'))
         FOR UPDATE OF l",
    )
    .bind(lease_id)
    .bind(worker_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((unit_id, job_id, template_id, Json(unit_params), honeypot, solo)) = lease else {
        return Ok(Completion::StaleLease);
    };
    let client = client_of(&mut tx, worker_id).await?;

    let template = templates::get(&template_id);
    let invalid = match template {
        None => Some(format!("unknown template {template_id}")),
        Some(t) => t.template.validate_output(&unit_params, output).err(),
    };
    if let Some(reason) = invalid {
        let released = release_leases(&mut tx, &[lease_id], ReleaseReason::Invalid, cfg).await?;
        let change = Standing {
            incorrect: 1,
            ..Standing::default()
        };
        let reopened = adjust_client(&mut tx, client.id, change, cfg).await?;
        tx.commit().await?;
        return Ok(Completion::InvalidOutput {
            reason,
            released,
            reopened,
        });
    }
    let template = template.expect("checked above");

    sqlx::query(
        "UPDATE leases SET completed_at = now(), output = $2, elapsed_ms = $3 WHERE id = $1",
    )
    .bind(lease_id)
    .bind(Json(output))
    .bind(elapsed_ms as i32)
    .execute(&mut *tx)
    .await?;

    let (status, accepted, target, seq): (String, Option<Json<Value>>, i32, i32) =
        sqlx::query_as("SELECT status, output, target, seq FROM units WHERE id = $1 FOR UPDATE")
            .bind(unit_id)
            .fetch_one(&mut *tx)
            .await?;

    // A honeypot, or a result for a unit that others already decided: compare it with
    // the known output, for reputation only.
    if honeypot || status == "done" {
        let Some(Json(known)) = accepted else {
            tx.commit().await?;
            return Ok(Completion::StaleLease);
        };
        let agreed = template.template.agree(&unit_params, output, &known);
        let verdict = match (honeypot, agreed) {
            (true, true) => "honeypot_ok",
            (true, false) => "honeypot_bad",
            (false, true) => "agreed",
            (false, false) => "disagreed",
        };
        let reopened = record_verdicts(&mut tx, &[(lease_id, verdict)], cfg).await?;
        tx.commit().await?;
        return Ok(if honeypot {
            Completion::Honeypot {
                correct: agreed,
                reopened,
            }
        } else {
            Completion::Late { agreed, reopened }
        });
    }
    if status == "failed" {
        tx.commit().await?;
        return Ok(Completion::StaleLease);
    }

    // A trusted client's solo unit, or no verification at all: one result decides.
    if solo || cfg.redundancy <= 1 {
        let reopened = record_verdicts(&mut tx, &[(lease_id, "trusted")], cfg).await?;
        let decided = decide(
            &mut tx,
            template,
            job_id,
            unit_id,
            unit_params,
            output.clone(),
            true,
            reopened,
        )
        .await?;
        tx.commit().await?;
        return Ok(decided);
    }

    let results: Vec<(Uuid, Json<Value>)> = sqlx::query_as(
        "SELECT l.id, l.output FROM leases l
         JOIN workers w ON w.id = l.worker_id
         LEFT JOIN clients c ON c.id = w.client_id
         WHERE l.unit_id = $1 AND l.completed_at IS NOT NULL AND l.verdict IS NULL
           AND NOT l.honeypot AND c.banned_at IS NULL
         ORDER BY l.completed_at, l.id",
    )
    .bind(unit_id)
    .fetch_all(&mut *tx)
    .await?;
    let quorum = cfg.redundancy as usize;

    if results.len() >= quorum {
        // The largest group of mutually agreeing results; the earliest wins ties.
        let groups: Vec<Vec<usize>> = (0..results.len())
            .map(|i| {
                (0..results.len())
                    .filter(|&j| {
                        template
                            .template
                            .agree(&unit_params, &results[i].1.0, &results[j].1.0)
                    })
                    .collect()
            })
            .collect();
        let best = (0..groups.len())
            .max_by_key(|&i| (groups[i].len(), std::cmp::Reverse(i)))
            .expect("at least one result");
        if groups[best].len() >= quorum {
            let verdicts: Vec<(Uuid, &str)> = results
                .iter()
                .enumerate()
                .map(|(i, (id, _))| {
                    (
                        *id,
                        if groups[best].contains(&i) {
                            "agreed"
                        } else {
                            "disagreed"
                        },
                    )
                })
                .collect();
            let agreed = verdicts
                .iter()
                .any(|(id, v)| *id == lease_id && *v == "agreed");
            let reopened = record_verdicts(&mut tx, &verdicts, cfg).await?;
            let accepted = results[best].1.0.clone();
            let decided = decide(
                &mut tx,
                template,
                job_id,
                unit_id,
                unit_params,
                accepted,
                agreed,
                reopened,
            )
            .await?;
            tx.commit().await?;
            return Ok(decided);
        }
        if results.len() >= quorum + cfg.max_extra_results as usize {
            let error = format!(
                "results for unit {seq} never agreed ({} results)",
                results.len()
            );
            let job = fail_job(&mut tx, job_id, &error).await?;
            tx.commit().await?;
            return Ok(Completion::Unresolved { job });
        }
        // No agreement yet: ask one more client.
        let target = results.len() as i32 + 1;
        sqlx::query("UPDATE units SET target = $2 WHERE id = $1")
            .bind(unit_id)
            .bind(target)
            .execute(&mut *tx)
            .await?;
        update_assignment(&mut tx, unit_id, target).await?;
        tx.commit().await?;
        return Ok(Completion::Disputed {
            reopened: Vec::new(),
        });
    }

    update_assignment(&mut tx, unit_id, target).await?;
    tx.commit().await?;
    Ok(Completion::Recorded)
}

/// Marks an undecided unit `leased` when enough copies are out to reach `target`
/// results, and `pending` (claimable) otherwise.
async fn update_assignment(
    tx: &mut Transaction<'_, Postgres>,
    unit_id: Uuid,
    target: i32,
) -> sqlx::Result<()> {
    let (active, results) = unit_copies(tx, unit_id).await?;
    let status = if active + results >= i64::from(target) {
        "leased"
    } else {
        "pending"
    };
    sqlx::query("UPDATE units SET status = $2 WHERE id = $1 AND status IN ('pending', 'leased')")
        .bind(unit_id)
        .bind(status)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Accepts `output` for the unit and counts it towards the job; the completion that
/// counts the last unit aggregates the job.
#[allow(clippy::too_many_arguments)]
async fn decide(
    tx: &mut Transaction<'_, Postgres>,
    template: &templates::Registered,
    job_id: Uuid,
    unit_id: Uuid,
    unit_params: Value,
    output: Value,
    agreed: bool,
    reopened: Vec<Uuid>,
) -> sqlx::Result<Completion> {
    sqlx::query("UPDATE units SET status = 'done', output = $2 WHERE id = $1")
        .bind(unit_id)
        .bind(Json(&output))
        .execute(&mut **tx)
        .await?;

    let (units_done, units_total, Json(job_params)): (i32, i32, Json<Value>) = sqlx::query_as(
        "UPDATE jobs SET units_done = units_done + 1 WHERE id = $1
         RETURNING units_done, units_total, params",
    )
    .bind(job_id)
    .fetch_one(&mut **tx)
    .await?;

    let job_finished = if units_done == units_total {
        let outputs: Vec<Json<Value>> =
            sqlx::query_scalar("SELECT output FROM units WHERE job_id = $1 ORDER BY seq")
                .bind(job_id)
                .fetch_all(&mut **tx)
                .await?;
        let outputs: Vec<Value> = outputs.into_iter().map(|j| j.0).collect();
        let (status, result, error) = match template.template.aggregate(&job_params, &outputs) {
            Ok(result) => ("done", Some(result), None),
            Err(e) => {
                tracing::error!(%job_id, error = %e, "aggregation failed");
                ("failed", None, Some(format!("aggregation failed: {e}")))
            }
        };
        let row: JobRow = sqlx::query_as(concat!(
            "UPDATE jobs SET status = $2, result = $3, error = $4, finished_at = now()
             WHERE id = $1 RETURNING ",
            job_columns!(),
            ""
        ))
        .bind(job_id)
        .bind(status)
        .bind(result.map(Json))
        .bind(error)
        .fetch_one(&mut **tx)
        .await?;
        Some(row.into())
    } else {
        None
    };

    Ok(Completion::Decided {
        job_id,
        unit_id,
        unit_params,
        output,
        agreed,
        units_done: units_done as u32,
        units_total: units_total as u32,
        job_finished,
        reopened,
    })
}

/// Fails a running job and stops handing out its units.
async fn fail_job(
    tx: &mut Transaction<'_, Postgres>,
    job_id: Uuid,
    error: &str,
) -> sqlx::Result<Option<JobSummary>> {
    let job: Option<JobRow> = sqlx::query_as(concat!(
        "UPDATE jobs SET status = 'failed', error = $2, finished_at = now()
         WHERE id = $1 AND status IN ('queued', 'running')
         RETURNING ",
        job_columns!(),
        ""
    ))
    .bind(job_id)
    .bind(error)
    .fetch_optional(&mut **tx)
    .await?;
    if job.is_some() {
        // Results still in flight become stale.
        sqlx::query(
            "UPDATE units SET status = 'failed' WHERE job_id = $1 AND status IN ('pending', 'leased')",
        )
        .bind(job_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(job.map(Into::into))
}

/// Stores verdicts on leases and updates their clients' reputation. Returns jobs
/// reopened by bans this caused.
async fn record_verdicts(
    tx: &mut Transaction<'_, Postgres>,
    verdicts: &[(Uuid, &str)],
    cfg: &Config,
) -> sqlx::Result<Vec<Uuid>> {
    let ids: Vec<Uuid> = verdicts.iter().map(|(id, _)| *id).collect();
    let values: Vec<String> = verdicts.iter().map(|(_, v)| (*v).to_owned()).collect();
    let per_client: Vec<(Uuid, i64, i64, i64)> = sqlx::query_as(
        "WITH v AS (
             UPDATE leases l SET verdict = x.verdict
             FROM unnest($1::uuid[], $2::text[]) AS x(id, verdict)
             WHERE l.id = x.id
             RETURNING l.worker_id, x.verdict
         )
         SELECT w.client_id,
                count(*) FILTER (WHERE v.verdict IN ('agreed', 'honeypot_ok')),
                count(*) FILTER (WHERE v.verdict IN ('disagreed', 'honeypot_bad')),
                count(*) FILTER (WHERE v.verdict = 'honeypot_bad')
         FROM v JOIN workers w ON w.id = v.worker_id
         WHERE w.client_id IS NOT NULL
         GROUP BY w.client_id",
    )
    .bind(&ids)
    .bind(&values)
    .fetch_all(&mut **tx)
    .await?;
    let mut reopened = Vec::new();
    for (client_id, correct, incorrect, honeypot_failures) in per_client {
        if correct > 0 || incorrect > 0 {
            let change = Standing {
                correct: correct as i32,
                incorrect: incorrect as i32,
                honeypot_failures: honeypot_failures as i32,
            };
            reopened.extend(adjust_client(tx, client_id, change, cfg).await?);
        }
    }
    Ok(reopened)
}

/// Adds to a client's counts of correct and incorrect results and bans it if it falls
/// below the bar. Returns the jobs a ban reopened (see [`reopen_after_ban`]).
/// Changes to a client's record.
#[derive(Default)]
struct Standing {
    correct: i32,
    incorrect: i32,
    honeypot_failures: i32,
}

async fn adjust_client(
    tx: &mut Transaction<'_, Postgres>,
    client_id: Uuid,
    change: Standing,
    cfg: &Config,
) -> sqlx::Result<Vec<Uuid>> {
    let row: Option<(i32, i32, i32, bool)> = sqlx::query_as(
        "UPDATE clients SET correct = correct + $2, incorrect = incorrect + $3,
                            honeypot_failures = honeypot_failures + $4
         WHERE id = $1 RETURNING correct, incorrect, honeypot_failures, banned_at IS NOT NULL",
    )
    .bind(client_id)
    .bind(change.correct)
    .bind(change.incorrect)
    .bind(change.honeypot_failures)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((correct, incorrect, honeypot_failures, banned)) = row else {
        return Ok(Vec::new());
    };
    if banned || !cfg.should_ban(correct, incorrect, honeypot_failures) {
        return Ok(Vec::new());
    }
    sqlx::query("UPDATE clients SET banned_at = now() WHERE id = $1")
        .bind(client_id)
        .execute(&mut **tx)
        .await?;
    tracing::warn!(%client_id, correct, incorrect, honeypot_failures, "client banned");

    reopen_after_ban(tx, client_id, cfg).await
}

/// Undoes what a banned client's results decided in jobs still running: every unit it
/// helped decide (alone, or by agreeing) that no longer has `redundancy` agreeing
/// results from clients in good standing goes back to the queue. The other clients'
/// verdicts on those units are reversed, since they were measured against a result that
/// may be wrong, and their results count again when the unit is decided anew. This is
/// what limits colluding cheaters: what they got accepted before being caught is redone.
async fn reopen_after_ban(
    tx: &mut Transaction<'_, Postgres>,
    client_id: Uuid,
    cfg: &Config,
) -> sqlx::Result<Vec<Uuid>> {
    let units: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT DISTINCT u.id, u.job_id FROM leases l
         JOIN workers w ON w.id = l.worker_id
         JOIN units u ON u.id = l.unit_id
         JOIN jobs j ON j.id = u.job_id
         WHERE w.client_id = $1 AND l.verdict IN ('agreed', 'trusted') AND NOT l.honeypot
           AND u.status = 'done' AND j.status = 'running'
           AND (SELECT count(*) FROM leases l2
                JOIN workers w2 ON w2.id = l2.worker_id
                JOIN clients c2 ON c2.id = w2.client_id
                WHERE l2.unit_id = u.id AND l2.verdict IN ('agreed', 'trusted')
                  AND c2.banned_at IS NULL) < $2",
    )
    .bind(client_id)
    .bind(cfg.redundancy as i32)
    .fetch_all(&mut **tx)
    .await?;
    if units.is_empty() {
        return Ok(Vec::new());
    }
    let unit_ids: Vec<Uuid> = units.iter().map(|(u, _)| *u).collect();

    // Reverse the other clients' verdicts on these units; their results count again.
    // RETURNING shows rows after the update, so the old verdicts are read first.
    let reversed: Vec<(Uuid, i64, i64)> = sqlx::query_as(
        "WITH old AS (
             SELECT l.id, w.client_id, l.verdict
             FROM leases l
             JOIN workers w ON w.id = l.worker_id
             JOIN clients c ON c.id = w.client_id
             WHERE l.unit_id = ANY($1) AND c.banned_at IS NULL
               AND l.verdict IN ('agreed', 'disagreed')
             FOR UPDATE OF l
         ), r AS (
             UPDATE leases SET verdict = NULL FROM old WHERE leases.id = old.id
             RETURNING old.client_id, old.verdict
         )
         SELECT client_id,
                count(*) FILTER (WHERE verdict = 'agreed'),
                count(*) FILTER (WHERE verdict = 'disagreed')
         FROM r GROUP BY client_id",
    )
    .bind(&unit_ids)
    .fetch_all(&mut **tx)
    .await?;
    for (client, correct, incorrect) in reversed {
        sqlx::query(
            "UPDATE clients SET correct = correct - $2, incorrect = incorrect - $3 WHERE id = $1",
        )
        .bind(client)
        .bind(correct as i32)
        .bind(incorrect as i32)
        .execute(&mut **tx)
        .await?;
    }

    // Back to the queue. The results that count again plus one more are needed, so the
    // next result triggers a fresh decision over all of them.
    sqlx::query(
        "UPDATE units u SET status = 'pending', output = NULL,
             target = GREATEST($2, 1 + (
                 SELECT count(*) FROM leases l
                 JOIN workers w ON w.id = l.worker_id
                 JOIN clients c ON c.id = w.client_id
                 WHERE l.unit_id = u.id AND l.completed_at IS NOT NULL AND l.verdict IS NULL
                   AND NOT l.honeypot AND c.banned_at IS NULL))
         WHERE u.id = ANY($1)",
    )
    .bind(&unit_ids)
    .bind(cfg.redundancy as i32)
    .execute(&mut **tx)
    .await?;

    let mut per_job: std::collections::BTreeMap<Uuid, i32> = Default::default();
    for (_, job_id) in &units {
        *per_job.entry(*job_id).or_default() += 1;
    }
    for (job_id, n) in &per_job {
        sqlx::query("UPDATE jobs SET units_done = units_done - $2 WHERE id = $1")
            .bind(job_id)
            .bind(n)
            .execute(&mut **tx)
            .await?;
    }
    Ok(per_job.into_keys().collect())
}

/// Why a lease ended without a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseReason {
    /// The worker's connection ended.
    Disconnected,
    /// The worker reported that it could not compute the unit.
    Failed,
    /// The worker's output did not pass the template's validation.
    Invalid,
    /// No heartbeat in time, or the lease reached its maximum lifetime.
    Expired,
}

impl ReleaseReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Disconnected => "disconnected",
            Self::Failed => "failed",
            Self::Invalid => "invalid",
            Self::Expired => "expired",
        }
    }
}

/// The effect of releasing leases.
#[derive(Debug, Default)]
pub struct Released {
    /// Leases released.
    pub leases: u64,
    /// Jobs that failed because one of the released units ran out of releases.
    pub failed_jobs: Vec<JobSummary>,
}

/// Releases one lease held by `worker_id` (the worker reported failure).
pub async fn release_lease(
    db: &PgPool,
    worker_id: Uuid,
    lease_id: Uuid,
    cfg: &Config,
) -> sqlx::Result<Released> {
    let mut tx = db.begin().await?;
    let owned: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM leases WHERE id = $1 AND worker_id = $2
           AND completed_at IS NULL AND released_at IS NULL FOR UPDATE",
    )
    .bind(lease_id)
    .bind(worker_id)
    .fetch_all(&mut *tx)
    .await?;
    let released = release_leases(&mut tx, &owned, ReleaseReason::Failed, cfg).await?;
    tx.commit().await?;
    Ok(released)
}

/// Releases every active lease of a worker (it disconnected).
pub async fn release_worker_leases(
    db: &PgPool,
    worker_id: Uuid,
    cfg: &Config,
) -> sqlx::Result<Released> {
    let mut tx = db.begin().await?;
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM leases WHERE worker_id = $1
           AND completed_at IS NULL AND released_at IS NULL FOR UPDATE",
    )
    .bind(worker_id)
    .fetch_all(&mut *tx)
    .await?;
    let released = release_leases(&mut tx, &ids, ReleaseReason::Disconnected, cfg).await?;
    tx.commit().await?;
    Ok(released)
}

/// Returns every expired lease's unit to the queue: the worker stopped sending
/// heartbeats, the lease outlived `max_lease_duration`, or its coordinator went away.
/// `SKIP LOCKED` lets several coordinators reap concurrently without blocking.
pub async fn reap_expired(db: &PgPool, cfg: &Config) -> sqlx::Result<Released> {
    let mut tx = db.begin().await?;
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM leases
         WHERE completed_at IS NULL AND released_at IS NULL AND expires_at < now()
         FOR UPDATE SKIP LOCKED",
    )
    .fetch_all(&mut *tx)
    .await?;
    let released = release_leases(&mut tx, &ids, ReleaseReason::Expired, cfg).await?;
    tx.commit().await?;
    Ok(released)
}

/// Marks leases released and returns their units to the queue, unless another lease
/// already completed them. A unit released `max_releases` times fails instead, and so
/// does its job: one missing unit makes an exact result impossible.
async fn release_leases(
    tx: &mut Transaction<'_, Postgres>,
    ids: &[Uuid],
    reason: ReleaseReason,
    cfg: &Config,
) -> sqlx::Result<Released> {
    if ids.is_empty() {
        return Ok(Released::default());
    }
    let released: Vec<(Uuid, bool)> = sqlx::query_as(
        "UPDATE leases SET released_at = now(), release_reason = $2
         WHERE id = ANY($1) RETURNING unit_id, honeypot",
    )
    .bind(ids)
    .bind(reason.as_str())
    .fetch_all(&mut **tx)
    .await?;
    // A honeypot's unit was decided long ago; losing the copy costs it nothing.
    let units: Vec<Uuid> = released
        .iter()
        .filter(|(_, honeypot)| !honeypot)
        .map(|(unit, _)| *unit)
        .collect();
    let exhausted: Vec<(Uuid, i32, i32)> = sqlx::query_as(
        "UPDATE units
         SET releases = releases + 1,
             status = CASE WHEN releases + 1 >= $2 THEN 'failed' ELSE 'pending' END
         WHERE id = ANY($1) AND status IN ('pending', 'leased')
         RETURNING job_id, seq, releases",
    )
    .bind(&units)
    .bind(cfg.max_releases as i32)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .filter(|(_, _, releases)| *releases >= cfg.max_releases as i32)
    .collect();

    let mut failed_jobs = Vec::new();
    for (job_id, seq, releases) in exhausted {
        let error = format!(
            "unit {seq} was given out {releases} times without a result (last: {})",
            reason.as_str()
        );
        let job: Option<JobRow> = sqlx::query_as(concat!(
            "UPDATE jobs SET status = 'failed', error = $2, finished_at = now()
             WHERE id = $1 AND status IN ('queued', 'running')
             RETURNING ",
            job_columns!(),
            ""
        ))
        .bind(job_id)
        .bind(&error)
        .fetch_optional(&mut **tx)
        .await?;
        if let Some(job) = job {
            // Nothing else of the job is handed out; results still in flight are stale.
            sqlx::query(
                "UPDATE units SET status = 'failed'
                 WHERE job_id = $1 AND status IN ('pending', 'leased')",
            )
            .bind(job_id)
            .execute(&mut **tx)
            .await?;
            failed_jobs.push(job.into());
        }
    }
    Ok(Released {
        leases: released.len() as u64,
        failed_jobs,
    })
}

/// Extends the given leases if they are still active and owned by the worker, but
/// never beyond `max_lease_duration` after they were issued.
pub async fn heartbeat(
    db: &PgPool,
    worker_id: Uuid,
    lease_ids: &[Uuid],
    cfg: &Config,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE workers SET last_seen_at = now() WHERE id = $1")
        .bind(worker_id)
        .execute(db)
        .await?;
    if !lease_ids.is_empty() {
        sqlx::query(
            "UPDATE leases SET expires_at = LEAST(now() + $3, issued_at + $4)
             WHERE id = ANY($1) AND worker_id = $2
               AND completed_at IS NULL AND released_at IS NULL",
        )
        .bind(lease_ids)
        .bind(worker_id)
        .bind(cfg.lease_ttl)
        .bind(cfg.max_lease_duration)
        .execute(db)
        .await?;
    }
    Ok(())
}

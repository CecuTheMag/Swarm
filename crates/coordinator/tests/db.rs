//! Scheduling and completion invariants, tested against a real PostgreSQL.
//! Each test gets a fresh database with migrations applied.

use coordinator::{
    Config,
    db::{self, Completion},
    templates,
};
use protocol::{Backend, DeviceInfo, JobStatus, templates::pi};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{collections::HashSet, time::Duration};
use uuid::Uuid;

/// Scheduling as in spec 0001: one result decides a unit, no honeypots. Verification
/// (spec 0004) has its own tests with `verifying()`.
fn cfg() -> Config {
    Config {
        redundancy: 1,
        honeypot_rate: 0.0,
        new_client_honeypot_rate: 0.0,
        ..Config::default()
    }
}

/// A worker connection of a new client.
async fn worker(db: &PgPool) -> Uuid {
    worker_of(db, Uuid::now_v7()).await
}

/// A worker connection of the given client.
async fn worker_of(db: &PgPool, client_id: Uuid) -> Uuid {
    let id = Uuid::now_v7();
    let device = DeviceInfo {
        backend: Backend::Native,
        vendor: "test".into(),
        architecture: "test".into(),
    };
    db::insert_worker(db, id, &device, "test", client_id)
        .await
        .unwrap()
        .expect("client is not banned");
    id
}

async fn pi_job(db: &PgPool, units: u32, samples: u32, seed: u32) -> (Uuid, Value) {
    let params = json!({ "units": units, "samples_per_unit": samples, "seed": seed });
    let t = templates::get(pi::ID).unwrap();
    let job = db::create_job(db, t, &params, t.template.split(&params).unwrap(), 1)
        .await
        .unwrap();
    assert_eq!(job.status, JobStatus::Queued);
    assert_eq!(job.units_total, units);
    (job.id, params)
}

fn reference_inside(units: u32, samples: u32, seed: u32) -> u64 {
    (0..units)
        .map(|unit_index| {
            u64::from(kernels::pi::count_inside(kernels::pi::UnitParams {
                seed,
                unit_index,
                samples,
            }))
        })
        .sum()
}

async fn unit_status(db: &PgPool, unit_id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM units WHERE id = $1")
        .bind(unit_id)
        .fetch_one(db)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn claims_are_exclusive_under_concurrency(db: PgPool) {
    pi_job(&db, 20, 10, 1).await;
    let mut tasks = Vec::new();
    for _ in 0..40 {
        let db = db.clone();
        let w = worker(&db).await;
        tasks.push(tokio::spawn(async move {
            db::claim_unit(&db, w, &cfg()).await.unwrap()
        }));
    }
    let mut units = HashSet::new();
    let mut empty = 0;
    for t in tasks {
        match t.await.unwrap() {
            Some(c) => assert!(units.insert(c.unit_id), "unit handed out twice"),
            None => empty += 1,
        }
    }
    assert_eq!(units.len(), 20);
    assert_eq!(empty, 20);
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn claims_follow_job_then_unit_order(db: PgPool) {
    let (first, _) = pi_job(&db, 3, 10, 1).await;
    let (second, _) = pi_job(&db, 2, 10, 2).await;
    let w = worker(&db).await;
    let mut order = Vec::new();
    while let Some(c) = db::claim_unit(&db, w, &cfg()).await.unwrap() {
        let p: pi::UnitParams = serde_json::from_value(c.params).unwrap();
        order.push((c.job_id, p.unit_index));
    }
    assert_eq!(
        order,
        vec![(first, 0), (first, 1), (first, 2), (second, 0), (second, 1)]
    );
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn job_result_equals_single_machine_reference(db: PgPool) {
    let (units, samples, seed) = (10, 20_000, 42);
    let (job_id, _) = pi_job(&db, units, samples, seed).await;
    let w = worker(&db).await;

    let first = db::claim_unit(&db, w, &cfg()).await.unwrap().unwrap();
    let started = first
        .job_started
        .as_ref()
        .expect("first claim starts the job");
    assert_eq!(started.status, JobStatus::Running);

    let mut claim = Some(first);
    let mut finished = None;
    while let Some(c) = claim {
        let output = sim_worker::compute(&c.template.id, c.template.version, &c.params).unwrap();
        match db::complete_lease(&db, w, c.lease_id, &output, 1, &cfg())
            .await
            .unwrap()
        {
            Completion::Decided { job_finished, .. } => finished = finished.or(job_finished),
            other => panic!("unexpected {other:?}"),
        }
        claim = db::claim_unit(&db, w, &cfg()).await.unwrap();
    }

    let job = finished.expect("last completion finishes the job");
    assert_eq!(job.status, JobStatus::Done);
    assert_eq!(job.units_done, units);
    let result: pi::JobResult = serde_json::from_value(job.result.unwrap()).unwrap();
    assert_eq!(result.inside, reference_inside(units, samples, seed));
    assert_eq!(result.total, u64::from(units * samples));

    let stored = db::get_job(&db, job_id).await.unwrap().unwrap();
    assert_eq!(stored.status, JobStatus::Done);
    assert!(stored.finished_at.is_some());
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn concurrent_completions_finish_job_exactly_once(db: PgPool) {
    let units = 50;
    let (job_id, _) = pi_job(&db, units, 100, 7).await;
    let mut claims = Vec::new();
    for _ in 0..units {
        let w = worker(&db).await;
        claims.push((w, db::claim_unit(&db, w, &cfg()).await.unwrap().unwrap()));
    }
    let tasks: Vec<_> = claims
        .into_iter()
        .map(|(w, c)| {
            let db = db.clone();
            tokio::spawn(async move {
                let out =
                    sim_worker::compute(&c.template.id, c.template.version, &c.params).unwrap();
                db::complete_lease(&db, w, c.lease_id, &out, 1, &cfg())
                    .await
                    .unwrap()
            })
        })
        .collect();

    let mut finished = 0;
    let mut counts = HashSet::new();
    for t in tasks {
        match t.await.unwrap() {
            Completion::Decided {
                units_done,
                job_finished,
                ..
            } => {
                assert!(counts.insert(units_done), "progress count repeated");
                finished += usize::from(job_finished.is_some());
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(finished, 1);
    assert_eq!(counts.len(), units as usize);
    let job = db::get_job(&db, job_id).await.unwrap().unwrap();
    assert_eq!((job.status, job.units_done), (JobStatus::Done, units));
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn foreign_and_repeated_results_are_rejected(db: PgPool) {
    pi_job(&db, 2, 100, 1).await;
    let (owner, other) = (worker(&db).await, worker(&db).await);
    let c = db::claim_unit(&db, owner, &cfg()).await.unwrap().unwrap();
    let out = sim_worker::compute(&c.template.id, c.template.version, &c.params).unwrap();

    let foreign = db::complete_lease(&db, other, c.lease_id, &out, 1, &cfg())
        .await
        .unwrap();
    assert!(matches!(foreign, Completion::StaleLease));
    let unknown = db::complete_lease(&db, owner, Uuid::now_v7(), &out, 1, &cfg())
        .await
        .unwrap();
    assert!(matches!(unknown, Completion::StaleLease));

    let first = db::complete_lease(&db, owner, c.lease_id, &out, 1, &cfg())
        .await
        .unwrap();
    assert!(matches!(first, Completion::Decided { units_done: 1, .. }));
    let again = db::complete_lease(&db, owner, c.lease_id, &out, 1, &cfg())
        .await
        .unwrap();
    assert!(matches!(again, Completion::StaleLease));
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn invalid_output_returns_unit_to_queue(db: PgPool) {
    pi_job(&db, 1, 100, 1).await;
    let w = worker(&db).await;
    for bad in [json!({ "inside": 101 }), json!({ "nope": 1 }), json!("x")] {
        let c = db::claim_unit(&db, w, &cfg())
            .await
            .unwrap()
            .expect("unit is back in the queue");
        let r = db::complete_lease(&db, w, c.lease_id, &bad, 1, &cfg())
            .await
            .unwrap();
        assert!(matches!(r, Completion::InvalidOutput { .. }), "{bad}");
        assert_eq!(unit_status(&db, c.unit_id).await, "pending");
    }
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn departed_worker_leases_are_requeued(db: PgPool) {
    pi_job(&db, 2, 100, 1).await;
    let (gone, next) = (worker(&db).await, worker(&db).await);
    let c = db::claim_unit(&db, gone, &cfg()).await.unwrap().unwrap();

    assert_eq!(
        db::release_worker_leases(&db, gone, &cfg())
            .await
            .unwrap()
            .leases,
        1
    );
    assert_eq!(unit_status(&db, c.unit_id).await, "pending");
    assert_eq!(
        db::release_worker_leases(&db, gone, &cfg())
            .await
            .unwrap()
            .leases,
        0
    );

    // The released unit is handed out again, and a late result from the
    // departed worker no longer counts.
    let again = db::claim_unit(&db, next, &cfg()).await.unwrap().unwrap();
    assert_eq!(again.unit_id, c.unit_id);
    let out = sim_worker::compute(&c.template.id, c.template.version, &c.params).unwrap();
    let late = db::complete_lease(&db, gone, c.lease_id, &out, 1, &cfg())
        .await
        .unwrap();
    assert!(matches!(late, Completion::StaleLease));
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn failed_unit_is_released_only_by_its_owner(db: PgPool) {
    pi_job(&db, 1, 100, 1).await;
    let (owner, other) = (worker(&db).await, worker(&db).await);
    let c = db::claim_unit(&db, owner, &cfg()).await.unwrap().unwrap();
    assert_eq!(
        db::release_lease(&db, other, c.lease_id, &cfg())
            .await
            .unwrap()
            .leases,
        0
    );
    assert_eq!(unit_status(&db, c.unit_id).await, "leased");
    assert_eq!(
        db::release_lease(&db, owner, c.lease_id, &cfg())
            .await
            .unwrap()
            .leases,
        1
    );
    assert_eq!(unit_status(&db, c.unit_id).await, "pending");
}

async fn expires_at(db: &PgPool, lease_id: Uuid) -> time::OffsetDateTime {
    sqlx::query_scalar("SELECT expires_at FROM leases WHERE id = $1")
        .bind(lease_id)
        .fetch_one(db)
        .await
        .unwrap()
}

/// Moves a lease's clock back as if it had been issued `age` ago and last renewed
/// `since_renewal` ago.
async fn age_lease(db: &PgPool, lease_id: Uuid, age: Duration, since_renewal: Duration) {
    sqlx::query(
        "UPDATE leases SET issued_at = now() - $2, expires_at = now() - $3 + interval '60 seconds'
         WHERE id = $1",
    )
    .bind(lease_id)
    .bind(age)
    .bind(since_renewal)
    .execute(db)
    .await
    .unwrap();
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn heartbeat_extends_only_own_active_leases(db: PgPool) {
    pi_job(&db, 1, 100, 1).await;
    let (owner, other) = (worker(&db).await, worker(&db).await);
    let short = Config {
        lease_ttl: Duration::from_secs(5),
        ..cfg()
    };
    let c = db::claim_unit(&db, owner, &short).await.unwrap().unwrap();
    let before = expires_at(&db, c.lease_id).await;
    let long = Config {
        lease_ttl: Duration::from_secs(300),
        ..cfg()
    };
    db::heartbeat(&db, other, &[c.lease_id], &long)
        .await
        .unwrap();
    assert_eq!(expires_at(&db, c.lease_id).await, before);
    db::heartbeat(&db, owner, &[c.lease_id], &long)
        .await
        .unwrap();
    assert!(expires_at(&db, c.lease_id).await > before + time::Duration::minutes(4));
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn heartbeats_cannot_extend_a_lease_past_its_maximum_lifetime(db: PgPool) {
    pi_job(&db, 1, 100, 1).await;
    let w = worker(&db).await;
    let c = db::claim_unit(&db, w, &cfg()).await.unwrap().unwrap();
    // A worker that stays connected and keeps sending heartbeats, but has held the
    // unit for longer than allowed: its heartbeat no longer renews the lease.
    age_lease(
        &db,
        c.lease_id,
        cfg().max_lease_duration + Duration::from_secs(1),
        Duration::ZERO,
    )
    .await;
    db::heartbeat(&db, w, &[c.lease_id], &cfg()).await.unwrap();
    assert!(expires_at(&db, c.lease_id).await < time::OffsetDateTime::now_utc());

    let reaped = db::reap_expired(&db, &cfg()).await.unwrap();
    assert_eq!(reaped.leases, 1);
    assert_eq!(unit_status(&db, c.unit_id).await, "pending");
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn expired_leases_are_reaped_and_late_results_ignored(db: PgPool) {
    pi_job(&db, 2, 100, 1).await;
    let (slow, next) = (worker(&db).await, worker(&db).await);
    let c = db::claim_unit(&db, slow, &cfg()).await.unwrap().unwrap();
    let live = db::claim_unit(&db, slow, &cfg()).await.unwrap().unwrap();

    // Nothing has expired yet.
    assert_eq!(db::reap_expired(&db, &cfg()).await.unwrap().leases, 0);
    // One lease missed its heartbeats.
    age_lease(
        &db,
        c.lease_id,
        Duration::from_secs(90),
        Duration::from_secs(61),
    )
    .await;
    assert_eq!(db::reap_expired(&db, &cfg()).await.unwrap().leases, 1);
    assert_eq!(unit_status(&db, c.unit_id).await, "pending");
    assert_eq!(unit_status(&db, live.unit_id).await, "leased");
    let reason: String = sqlx::query_scalar("SELECT release_reason FROM leases WHERE id = $1")
        .bind(c.lease_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(reason, "expired");

    // Another worker gets the unit; the slow worker's late result does not count.
    let again = db::claim_unit(&db, next, &cfg()).await.unwrap().unwrap();
    assert_eq!(again.unit_id, c.unit_id);
    let out = sim_worker::compute(&c.template.id, c.template.version, &c.params).unwrap();
    let late = db::complete_lease(&db, slow, c.lease_id, &out, 1, &cfg())
        .await
        .unwrap();
    assert!(matches!(late, Completion::StaleLease));
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn a_unit_released_too_often_fails_its_job(db: PgPool) {
    let (job_id, _) = pi_job(&db, 3, 100, 1).await;
    let limited = Config {
        max_releases: 2,
        ..cfg()
    };
    let w = worker(&db).await;

    // The first unit keeps failing on every worker that tries it.
    let first = db::claim_unit(&db, w, &limited).await.unwrap().unwrap();
    let r = db::release_lease(&db, w, first.lease_id, &limited)
        .await
        .unwrap();
    assert!(r.failed_jobs.is_empty());
    let second = db::claim_unit(&db, w, &limited).await.unwrap().unwrap();
    assert_eq!(second.unit_id, first.unit_id);
    // Meanwhile another unit of the job is being computed.
    let other = db::claim_unit(&db, w, &limited).await.unwrap().unwrap();
    assert_ne!(other.unit_id, first.unit_id);

    let r = db::release_lease(&db, w, second.lease_id, &limited)
        .await
        .unwrap();
    assert_eq!(r.failed_jobs.len(), 1);
    let job = &r.failed_jobs[0];
    assert_eq!((job.id, job.status), (job_id, JobStatus::Failed));
    assert!(
        job.error.as_deref().unwrap().contains("unit 0"),
        "{:?}",
        job.error
    );
    assert!(job.finished_at.is_some());

    // Nothing more of the job is handed out, and results still in flight are stale.
    assert!(db::claim_unit(&db, w, &limited).await.unwrap().is_none());
    let out =
        sim_worker::compute(&other.template.id, other.template.version, &other.params).unwrap();
    let late = db::complete_lease(&db, w, other.lease_id, &out, 1, &limited)
        .await
        .unwrap();
    assert!(matches!(late, Completion::StaleLease));
    assert_eq!(
        db::get_job(&db, job_id).await.unwrap().unwrap().status,
        JobStatus::Failed
    );
}

// ---- Verification (spec 0004) -------------------------------------------------------

/// Two agreeing results per unit, no random honeypots or spot checks unless a test
/// turns them on.
fn verifying() -> Config {
    Config {
        redundancy: 2,
        honeypot_rate: 0.0,
        new_client_honeypot_rate: 0.0,
        spot_check_rate: 0.0,
        ..Config::default()
    }
}

async fn pi_job_with(db: &PgPool, cfg: &Config, units: u32, samples: u32, seed: u32) -> Uuid {
    let params = json!({ "units": units, "samples_per_unit": samples, "seed": seed });
    let t = templates::get(pi::ID).unwrap();
    db::create_job(
        db,
        t,
        &params,
        t.template.split(&params).unwrap(),
        cfg.redundancy,
    )
    .await
    .unwrap()
    .id
}

fn honest(c: &db::Claim) -> Value {
    sim_worker::compute(&c.template.id, c.template.version, &c.params).unwrap()
}

fn wrong(c: &db::Claim) -> Value {
    json!({ "inside": honest(c)["inside"].as_u64().unwrap() + 1 })
}

async fn standing(db: &PgPool, worker_id: Uuid) -> (i32, i32, bool) {
    sqlx::query_as(
        "SELECT c.correct, c.incorrect, c.banned_at IS NOT NULL
         FROM workers w JOIN clients c ON c.id = w.client_id WHERE w.id = $1",
    )
    .bind(worker_id)
    .fetch_one(db)
    .await
    .unwrap()
}

async fn set_standing(db: &PgPool, worker_id: Uuid, correct: i32, incorrect: i32) {
    sqlx::query(
        "UPDATE clients SET correct = $2, incorrect = $3
         WHERE id = (SELECT client_id FROM workers WHERE id = $1)",
    )
    .bind(worker_id)
    .bind(correct)
    .bind(incorrect)
    .execute(db)
    .await
    .unwrap();
}

async fn complete(db: &PgPool, w: Uuid, c: &db::Claim, out: &Value, cfg: &Config) -> Completion {
    db::complete_lease(db, w, c.lease_id, out, 1, cfg)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn two_clients_must_agree(db: PgPool) {
    let cfg = verifying();
    let job_id = pi_job_with(&db, &cfg, 1, 1000, 3).await;
    let (a, b) = (worker(&db).await, worker(&db).await);

    // Both copies are handed out at once, to different clients.
    let ca = db::claim_unit(&db, a, &cfg).await.unwrap().unwrap();
    assert_eq!(unit_status(&db, ca.unit_id).await, "pending");
    assert!(db::claim_unit(&db, a, &cfg).await.unwrap().is_none());
    let cb = db::claim_unit(&db, b, &cfg).await.unwrap().unwrap();
    assert_eq!(cb.unit_id, ca.unit_id);
    assert_eq!(unit_status(&db, ca.unit_id).await, "leased");

    // One result alone decides nothing.
    let first = complete(&db, a, &ca, &honest(&ca), &cfg).await;
    assert!(matches!(first, Completion::Recorded));
    assert!(first.accepted());
    assert_eq!(
        db::get_job(&db, job_id).await.unwrap().unwrap().units_done,
        0
    );

    let second = complete(&db, b, &cb, &honest(&cb), &cfg).await;
    assert!(matches!(
        second,
        Completion::Decided {
            agreed: true,
            job_finished: Some(_),
            ..
        }
    ));
    assert_eq!(standing(&db, a).await, (1, 0, false));
    assert_eq!(standing(&db, b).await, (1, 0, false));
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn a_client_cannot_verify_itself_from_another_connection(db: PgPool) {
    let cfg = verifying();
    pi_job_with(&db, &cfg, 1, 1000, 3).await;
    let client = Uuid::now_v7();
    let (tab1, tab2) = (worker_of(&db, client).await, worker_of(&db, client).await);
    db::claim_unit(&db, tab1, &cfg).await.unwrap().unwrap();
    assert!(db::claim_unit(&db, tab2, &cfg).await.unwrap().is_none());
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn a_dispute_asks_for_a_tiebreaker_and_the_majority_wins(db: PgPool) {
    let cfg = verifying();
    let job_id = pi_job_with(&db, &cfg, 1, 1000, 8).await;
    let (a, cheat, c) = (worker(&db).await, worker(&db).await, worker(&db).await);
    let ca = db::claim_unit(&db, a, &cfg).await.unwrap().unwrap();
    let cx = db::claim_unit(&db, cheat, &cfg).await.unwrap().unwrap();
    // No third copy while two are out.
    assert!(db::claim_unit(&db, c, &cfg).await.unwrap().is_none());

    assert!(matches!(
        complete(&db, a, &ca, &honest(&ca), &cfg).await,
        Completion::Recorded
    ));
    assert!(matches!(
        complete(&db, cheat, &cx, &wrong(&cx), &cfg).await,
        Completion::Disputed { .. }
    ));

    // A third client breaks the tie.
    let cc = db::claim_unit(&db, c, &cfg).await.unwrap().unwrap();
    assert_eq!(cc.unit_id, ca.unit_id);
    let decided = complete(&db, c, &cc, &honest(&cc), &cfg).await;
    let Completion::Decided {
        output,
        agreed: true,
        ..
    } = decided
    else {
        panic!("expected a decision, got {decided:?}");
    };
    assert_eq!(output, honest(&ca));
    assert_eq!(standing(&db, a).await, (1, 0, false));
    assert_eq!(standing(&db, cheat).await, (0, 1, false));
    assert_eq!(standing(&db, c).await, (1, 0, false));

    let job = db::get_job(&db, job_id).await.unwrap().unwrap();
    assert_eq!(job.status, JobStatus::Done);
    let result: pi::JobResult = serde_json::from_value(job.result.unwrap()).unwrap();
    assert_eq!(result.inside, reference_inside(1, 1000, 8));
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn results_that_never_agree_fail_the_job(db: PgPool) {
    let cfg = Config {
        max_extra_results: 1,
        ..verifying()
    };
    let job_id = pi_job_with(&db, &cfg, 2, 1000, 8).await;
    let mut last = None;
    for k in 0..3u64 {
        let w = worker(&db).await;
        let c = db::claim_unit(&db, w, &cfg).await.unwrap().unwrap();
        let out = json!({ "inside": 100 + k });
        last = Some(complete(&db, w, &c, &out, &cfg).await);
    }
    let Some(Completion::Unresolved { job: Some(job) }) = last else {
        panic!("expected the job to fail, got {last:?}");
    };
    assert_eq!((job.id, job.status), (job_id, JobStatus::Failed));
    assert!(job.error.unwrap().contains("never agreed"));
    // Nobody is blamed: it is unknown which result was right.
    let blamed: i64 = sqlx::query_scalar("SELECT count(*) FROM leases WHERE verdict IS NOT NULL")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(blamed, 0);
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn late_results_count_for_reputation(db: PgPool) {
    let cfg = verifying();
    // Two units, so the job is still running when the late results arrive.
    pi_job_with(&db, &cfg, 2, 1000, 2).await;
    let (a, b, late_good, late_bad) = (
        worker(&db).await,
        worker(&db).await,
        worker(&db).await,
        worker(&db).await,
    );
    let ca = db::claim_unit(&db, a, &cfg).await.unwrap().unwrap();
    let cb = db::claim_unit(&db, b, &cfg).await.unwrap().unwrap();
    // Two extra copies of the same unit are out (as after a dispute)...
    sqlx::query("UPDATE units SET target = 4, status = 'pending' WHERE id = $1")
        .bind(ca.unit_id)
        .execute(&db)
        .await
        .unwrap();
    let cg = db::claim_unit(&db, late_good, &cfg).await.unwrap().unwrap();
    let cx = db::claim_unit(&db, late_bad, &cfg).await.unwrap().unwrap();
    assert!(cg.unit_id == ca.unit_id && cx.unit_id == ca.unit_id);
    sqlx::query("UPDATE units SET target = 2 WHERE id = $1")
        .bind(ca.unit_id)
        .execute(&db)
        .await
        .unwrap();
    // ...when the first two decide it.
    complete(&db, a, &ca, &honest(&ca), &cfg).await;
    assert!(matches!(
        complete(&db, b, &cb, &honest(&cb), &cfg).await,
        Completion::Decided { .. }
    ));

    let good = complete(&db, late_good, &cg, &honest(&cg), &cfg).await;
    let bad = complete(&db, late_bad, &cx, &wrong(&cx), &cfg).await;
    assert!(matches!(good, Completion::Late { agreed: true, .. }) && good.accepted());
    assert!(matches!(bad, Completion::Late { agreed: false, .. }) && !bad.accepted());
    assert_eq!(standing(&db, late_good).await, (1, 0, false));
    assert_eq!(standing(&db, late_bad).await, (0, 1, false));
    // The decided output is untouched.
    let output: Value = sqlx::query_scalar("SELECT output FROM units WHERE id = $1")
        .bind(ca.unit_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(output, honest(&ca));
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn honeypots_are_checked_against_the_known_answer(db: PgPool) {
    let cfg = verifying();
    // A verified, finished job provides the known answers.
    pi_job_with(&db, &cfg, 2, 1000, 5).await;
    let (a, b) = (worker(&db).await, worker(&db).await);
    for _ in 0..2 {
        let ca = db::claim_unit(&db, a, &cfg).await.unwrap().unwrap();
        let cb = db::claim_unit(&db, b, &cfg).await.unwrap().unwrap();
        complete(&db, a, &ca, &honest(&ca), &cfg).await;
        complete(&db, b, &cb, &honest(&cb), &cfg).await;
    }

    let traps = Config {
        new_client_honeypot_rate: 1.0,
        ..cfg.clone()
    };
    let (good, bad) = (worker(&db).await, worker(&db).await);
    let cg = db::claim_unit(&db, good, &traps).await.unwrap().unwrap();
    let cb = db::claim_unit(&db, bad, &traps).await.unwrap().unwrap();
    for c in [&cg, &cb] {
        assert_eq!(unit_status(&db, c.unit_id).await, "done", "a verified unit");
    }
    let g = complete(&db, good, &cg, &honest(&cg), &traps).await;
    let x = complete(&db, bad, &cb, &wrong(&cb), &traps).await;
    assert!(matches!(g, Completion::Honeypot { correct: true, .. }));
    assert!(matches!(x, Completion::Honeypot { correct: false, .. }));
    // The worker cannot tell: both are acknowledged as accepted.
    assert!(g.accepted() && x.accepted());
    assert_eq!(standing(&db, good).await, (1, 0, false));
    assert_eq!(standing(&db, bad).await, (0, 1, false));

    // A honeypot copy that is never answered costs its unit nothing.
    let ct = db::claim_unit(&db, worker(&db).await, &traps)
        .await
        .unwrap()
        .unwrap();
    db::reap_expired(&db, &traps).await.unwrap();
    sqlx::query("UPDATE leases SET expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(ct.lease_id)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(db::reap_expired(&db, &traps).await.unwrap().leases, 1);
    let (status, releases): (String, i32) =
        sqlx::query_as("SELECT status, releases FROM units WHERE id = $1")
            .bind(ct.unit_id)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!((status.as_str(), releases), ("done", 0));
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn honeypots_come_only_from_jobs_computed_with_todays_kernel(db: PgPool) {
    let cfg = verifying();
    let job = pi_job_with(&db, &cfg, 2, 1000, 5).await;
    let (a, b) = (worker(&db).await, worker(&db).await);
    for _ in 0..2 {
        let ca = db::claim_unit(&db, a, &cfg).await.unwrap().unwrap();
        let cb = db::claim_unit(&db, b, &cfg).await.unwrap().unwrap();
        complete(&db, a, &ca, &honest(&ca), &cfg).await;
        complete(&db, b, &cb, &honest(&cb), &cfg).await;
    }
    let traps = Config {
        new_client_honeypot_rate: 1.0,
        ..cfg.clone()
    };
    assert!(
        db::claim_unit(&db, worker(&db).await, &traps)
            .await
            .unwrap()
            .is_some(),
        "the finished job provides honeypots"
    );

    // The kernel changed since: its known answers no longer apply.
    sqlx::query("UPDATE jobs SET template_sha256 = 'an older kernel' WHERE id = $1")
        .bind(job)
        .execute(&db)
        .await
        .unwrap();
    for _ in 0..20 {
        let claim = db::claim_unit(&db, worker(&db).await, &traps)
            .await
            .unwrap();
        assert!(
            claim.is_none(),
            "a known answer from another kernel was handed out"
        );
    }
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn honeypots_are_drawn_from_the_whole_job(db: PgPool) {
    // A finished job of 64 units, decided by one result each.
    let once = Config {
        redundancy: 1,
        ..verifying()
    };
    pi_job_with(&db, &once, 64, 1000, 8).await;
    let w = worker(&db).await;
    while let Some(c) = db::claim_unit(&db, w, &once).await.unwrap() {
        complete(&db, w, &c, &honest(&c), &once).await;
    }

    // With nothing else to do, every new client asking for work gets a known unit, and
    // over many clients they come from all over the job.
    let traps = Config {
        new_client_honeypot_rate: 1.0,
        ..verifying()
    };
    let mut seqs = std::collections::BTreeSet::new();
    for _ in 0..100 {
        let c = db::claim_unit(&db, worker(&db).await, &traps)
            .await
            .unwrap()
            .expect("a honeypot for a client that has answered nothing");
        let seq: i32 = sqlx::query_scalar("SELECT seq FROM units WHERE id = $1")
            .bind(c.unit_id)
            .fetch_one(&db)
            .await
            .unwrap();
        seqs.insert(seq);
    }
    assert!(seqs.len() > 32, "{} distinct units", seqs.len());
    assert!(*seqs.last().unwrap() >= 56, "{seqs:?}");
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn trusted_clients_work_alone_except_for_spot_checks(db: PgPool) {
    let cfg = verifying();
    pi_job_with(&db, &cfg, 2, 1000, 6).await;
    let veteran = worker(&db).await;
    set_standing(&db, veteran, 40, 0).await;

    // Trusted and not spot-checked: one copy, decided by its result alone.
    let c = db::claim_unit(&db, veteran, &cfg).await.unwrap().unwrap();
    assert_eq!(unit_status(&db, c.unit_id).await, "leased");
    let done = complete(&db, veteran, &c, &honest(&c), &cfg).await;
    assert!(matches!(done, Completion::Decided { .. }));
    let verdict: String = sqlx::query_scalar("SELECT verdict FROM leases WHERE id = $1")
        .bind(c.lease_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(verdict, "trusted");
    // An unverified result earns nothing.
    assert_eq!(standing(&db, veteran).await, (40, 0, false));

    // Always spot-checked: the unit still needs a second client.
    let always = Config {
        spot_check_rate: 1.0,
        ..cfg.clone()
    };
    let c = db::claim_unit(&db, veteran, &always)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unit_status(&db, c.unit_id).await, "pending");
    assert!(matches!(
        complete(&db, veteran, &c, &honest(&c), &always).await,
        Completion::Recorded
    ));

    // Without enough correct results there is no trust: never a solo copy.
    let newcomer = worker(&db).await;
    set_standing(&db, newcomer, 5, 0).await;
    pi_job_with(&db, &cfg, 1, 1000, 7).await;
    let c = db::claim_unit(&db, newcomer, &cfg).await.unwrap().unwrap();
    let solo: bool = sqlx::query_scalar("SELECT solo FROM leases WHERE id = $1")
        .bind(c.lease_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(!solo);
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn a_banned_client_is_blocked_and_what_it_decided_is_redone(db: PgPool) {
    let cfg = verifying();
    // A trusted client computes two units of a running job alone...
    let job_id = pi_job_with(&db, &cfg, 3, 1000, 9).await;
    let mole = worker(&db).await;
    set_standing(&db, mole, 20, 0).await;
    for _ in 0..2 {
        let c = db::claim_unit(&db, mole, &cfg).await.unwrap().unwrap();
        complete(&db, mole, &c, &wrong(&c), &cfg).await;
    }
    assert_eq!(
        db::get_job(&db, job_id).await.unwrap().unwrap().units_done,
        2
    );

    // ...then turns out to be lying: its record collapses and the next wrong answer
    // gets it banned.
    set_standing(&db, mole, 1, 2).await;
    let honest_client = worker(&db).await;
    let ch = db::claim_unit(&db, honest_client, &cfg)
        .await
        .unwrap()
        .unwrap();
    let cm = db::claim_unit(&db, mole, &cfg).await.unwrap().unwrap();
    assert_eq!(ch.unit_id, cm.unit_id);
    complete(&db, honest_client, &ch, &honest(&ch), &cfg).await;
    let caught = complete(&db, mole, &cm, &wrong(&cm), &cfg).await;
    assert!(matches!(caught, Completion::Disputed { .. }), "{caught:?}");
    // Disputes do not blame anyone yet; a third client decides.
    let third = worker(&db).await;
    let ct = db::claim_unit(&db, third, &cfg).await.unwrap().unwrap();
    let decided = complete(&db, third, &ct, &honest(&ct), &cfg).await;
    assert_eq!(decided.reopened(), &[job_id]);
    assert!(standing(&db, mole).await.2, "the client is banned");

    // The two units it decided alone are back in the queue.
    let job = db::get_job(&db, job_id).await.unwrap().unwrap();
    assert_eq!((job.status, job.units_done), (JobStatus::Running, 1));
    // It gets no more work, and cannot reconnect.
    assert!(db::claim_unit(&db, mole, &cfg).await.unwrap().is_none());
    let client: Uuid = sqlx::query_scalar("SELECT client_id FROM workers WHERE id = $1")
        .bind(mole)
        .fetch_one(&db)
        .await
        .unwrap();
    let device = DeviceInfo {
        backend: Backend::Native,
        vendor: "test".into(),
        architecture: "test".into(),
    };
    assert!(
        db::insert_worker(&db, Uuid::now_v7(), &device, "test", client)
            .await
            .unwrap()
            .is_none()
    );

    // Honest clients finish the job, and the result is exact.
    for _ in 0..4 {
        let (x, y) = (worker(&db).await, worker(&db).await);
        let (Some(cx), Some(cy)) = (
            db::claim_unit(&db, x, &cfg).await.unwrap(),
            db::claim_unit(&db, y, &cfg).await.unwrap(),
        ) else {
            break;
        };
        complete(&db, x, &cx, &honest(&cx), &cfg).await;
        complete(&db, y, &cy, &honest(&cy), &cfg).await;
    }
    let job = db::get_job(&db, job_id).await.unwrap().unwrap();
    assert_eq!(job.status, JobStatus::Done);
    let result: pi::JobResult = serde_json::from_value(job.result.unwrap()).unwrap();
    assert_eq!(result.inside, reference_inside(3, 1000, 9));
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn colluders_who_agree_on_a_wrong_answer_are_undone_when_caught(db: PgPool) {
    let cfg = verifying();
    let job_id = pi_job_with(&db, &cfg, 2, 1000, 4).await;
    let (m1, m2) = (worker(&db).await, worker(&db).await);
    // Two colluders agree on the same wrong answer for the first unit.
    let c1 = db::claim_unit(&db, m1, &cfg).await.unwrap().unwrap();
    let c2 = db::claim_unit(&db, m2, &cfg).await.unwrap().unwrap();
    // An honest client got a third copy (as after a dispute) and answers after the
    // colluders decided the unit: it is marked as disagreeing, unfairly.
    sqlx::query("UPDATE units SET target = 3, status = 'pending' WHERE id = $1")
        .bind(c1.unit_id)
        .execute(&db)
        .await
        .unwrap();
    let h = worker(&db).await;
    let ch = db::claim_unit(&db, h, &cfg).await.unwrap().unwrap();
    assert_eq!(ch.unit_id, c1.unit_id);
    sqlx::query("UPDATE units SET target = 2 WHERE id = $1")
        .bind(c1.unit_id)
        .execute(&db)
        .await
        .unwrap();
    complete(&db, m1, &c1, &wrong(&c1), &cfg).await;
    let fooled = complete(&db, m2, &c2, &wrong(&c2), &cfg).await;
    assert!(matches!(fooled, Completion::Decided { agreed: true, .. }));
    assert_eq!(
        db::get_job(&db, job_id).await.unwrap().unwrap().units_done,
        1
    );
    let late = complete(&db, h, &ch, &honest(&ch), &cfg).await;
    assert!(matches!(late, Completion::Late { agreed: false, .. }));
    assert_eq!(standing(&db, h).await, (0, 1, false));
    assert_eq!(standing(&db, m2).await, (1, 0, false));

    // Honeypots expose one of them (its record is set up so that one more wrong answer
    // bans it).
    set_standing(&db, m1, 0, 2).await;
    let finished = pi_job_with(&db, &cfg, 1, 1000, 11).await;
    sqlx::query("UPDATE units SET status = 'done', output = $2 WHERE job_id = $1")
        .bind(finished)
        .bind(json!({ "inside": reference_inside(1, 1000, 11) }))
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("UPDATE jobs SET status = 'done', units_done = units_total WHERE id = $1")
        .bind(finished)
        .execute(&db)
        .await
        .unwrap();
    let traps = Config {
        new_client_honeypot_rate: 1.0,
        ..cfg.clone()
    };
    let trap = db::claim_unit(&db, m1, &traps).await.unwrap().unwrap();
    let caught = complete(&db, m1, &trap, &wrong(&trap), &traps).await;
    assert!(matches!(
        caught,
        Completion::Honeypot { correct: false, .. }
    ));
    assert_eq!(caught.reopened(), &[job_id]);

    // The unit they faked is redone. The honest client's mark is withdrawn, and so is
    // the other colluder's credit for agreeing; the honest result counts again.
    assert_eq!(
        db::get_job(&db, job_id).await.unwrap().unwrap().units_done,
        0
    );
    assert_eq!(standing(&db, h).await, (0, 0, false));
    assert_eq!(standing(&db, m2).await, (0, 0, false));
    let verdict: Option<String> = sqlx::query_scalar("SELECT verdict FROM leases WHERE id = $1")
        .bind(ch.lease_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(verdict, None);
    // One more honest result decides it, in agreement with the earlier honest one.
    let h2 = worker(&db).await;
    let c = db::claim_unit(&db, h2, &cfg).await.unwrap().unwrap();
    assert_eq!(c.unit_id, c1.unit_id);
    let redone = complete(&db, h2, &c, &honest(&c), &cfg).await;
    assert!(
        matches!(redone, Completion::Decided { agreed: true, .. }),
        "{redone:?}"
    );
    assert_eq!(standing(&db, h).await, (1, 0, false));

    for _ in 0..4 {
        let (x, y) = (worker(&db).await, worker(&db).await);
        let (Some(cx), Some(cy)) = (
            db::claim_unit(&db, x, &cfg).await.unwrap(),
            db::claim_unit(&db, y, &cfg).await.unwrap(),
        ) else {
            break;
        };
        complete(&db, x, &cx, &honest(&cx), &cfg).await;
        complete(&db, y, &cy, &honest(&cy), &cfg).await;
    }
    let job = db::get_job(&db, job_id).await.unwrap().unwrap();
    assert_eq!(job.status, JobStatus::Done);
    let result: pi::JobResult = serde_json::from_value(job.result.unwrap()).unwrap();
    assert_eq!(result.inside, reference_inside(2, 1000, 4));
}

// ---- Live images (spec 0003) -----------------------------------------------------------

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn images_are_shared_until_a_unit_changes_them(db: PgPool) {
    let cfg = Config {
        redundancy: 1,
        ..verifying()
    };
    // 2 × 1 tiles of 16 pixels, one pass.
    let params = json!({ "width": 32, "height": 16, "tile": 16, "spp": 1, "passes": 1, "seed": 3 });
    let t = templates::get(protocol::templates::pt::ID).unwrap();
    let job = db::create_job(&db, t, &params, t.template.split(&params).unwrap(), 1)
        .await
        .unwrap();
    let w = worker(&db).await;
    let first = db::claim_unit(&db, w, &cfg).await.unwrap().unwrap();
    complete(&db, w, &first, &honest(&first), &cfg).await;

    // Loaded from the database, then the same encoded image for every viewer.
    let renders = coordinator::render::Renders::default();
    let a = renders.png(&db, job.id).await.unwrap().unwrap();
    let b = renders.png(&db, job.id).await.unwrap().unwrap();
    assert_eq!(a.as_ptr(), b.as_ptr(), "encoded once");

    // A newly accepted unit changes the image.
    let second = db::claim_unit(&db, w, &cfg).await.unwrap().unwrap();
    let out = honest(&second);
    complete(&db, w, &second, &out, &cfg).await;
    renders
        .unit_accepted(job.id, second.unit_id, &second.params, &out)
        .await;
    let c = renders.png(&db, job.id).await.unwrap().unwrap();
    assert_ne!(a, c);
    // The same as an image built from scratch from the database.
    let fresh = coordinator::render::Renders::default();
    assert_eq!(c, fresh.png(&db, job.id).await.unwrap().unwrap());

    // Not a path-tracer job: no image.
    let pi = pi_job_with(&db, &cfg, 1, 100, 1).await;
    assert!(renders.png(&db, pi).await.unwrap().is_none());
}

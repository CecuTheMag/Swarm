//! Scheduling and completion invariants, tested against a real PostgreSQL.
//! Each test gets a fresh database with migrations applied.

use coordinator::{
    db::{self, Completion},
    templates,
};
use protocol::{Backend, DeviceInfo, JobStatus, templates::pi};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{collections::HashSet, time::Duration};
use uuid::Uuid;

const TTL: Duration = Duration::from_secs(60);

async fn worker(db: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    let device = DeviceInfo {
        backend: Backend::Native,
        vendor: "test".into(),
        architecture: "test".into(),
    };
    db::insert_worker(db, id, &device, "test").await.unwrap();
    id
}

async fn pi_job(db: &PgPool, units: u32, samples: u32, seed: u32) -> (Uuid, Value) {
    let params = json!({ "units": units, "samples_per_unit": samples, "seed": seed });
    let t = templates::get(pi::ID).unwrap();
    let job = db::create_job(db, t, &params, t.template.split(&params).unwrap())
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
            db::claim_unit(&db, w, TTL).await.unwrap()
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
    while let Some(c) = db::claim_unit(&db, w, TTL).await.unwrap() {
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

    let first = db::claim_unit(&db, w, TTL).await.unwrap().unwrap();
    let started = first
        .job_started
        .as_ref()
        .expect("first claim starts the job");
    assert_eq!(started.status, JobStatus::Running);

    let mut claim = Some(first);
    let mut finished = None;
    while let Some(c) = claim {
        let output = sim_worker::compute(&c.template.id, c.template.version, &c.params).unwrap();
        match db::complete_lease(&db, w, c.lease_id, &output, 1)
            .await
            .unwrap()
        {
            Completion::Accepted { job_finished, .. } => finished = finished.or(job_finished),
            other => panic!("unexpected {other:?}"),
        }
        claim = db::claim_unit(&db, w, TTL).await.unwrap();
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
        claims.push((w, db::claim_unit(&db, w, TTL).await.unwrap().unwrap()));
    }
    let tasks: Vec<_> = claims
        .into_iter()
        .map(|(w, c)| {
            let db = db.clone();
            tokio::spawn(async move {
                let out =
                    sim_worker::compute(&c.template.id, c.template.version, &c.params).unwrap();
                db::complete_lease(&db, w, c.lease_id, &out, 1)
                    .await
                    .unwrap()
            })
        })
        .collect();

    let mut finished = 0;
    let mut counts = HashSet::new();
    for t in tasks {
        match t.await.unwrap() {
            Completion::Accepted {
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
    let c = db::claim_unit(&db, owner, TTL).await.unwrap().unwrap();
    let out = sim_worker::compute(&c.template.id, c.template.version, &c.params).unwrap();

    let foreign = db::complete_lease(&db, other, c.lease_id, &out, 1)
        .await
        .unwrap();
    assert!(matches!(foreign, Completion::StaleLease));
    let unknown = db::complete_lease(&db, owner, Uuid::now_v7(), &out, 1)
        .await
        .unwrap();
    assert!(matches!(unknown, Completion::StaleLease));

    let first = db::complete_lease(&db, owner, c.lease_id, &out, 1)
        .await
        .unwrap();
    assert!(matches!(first, Completion::Accepted { units_done: 1, .. }));
    let again = db::complete_lease(&db, owner, c.lease_id, &out, 1)
        .await
        .unwrap();
    assert!(matches!(again, Completion::StaleLease));
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn invalid_output_returns_unit_to_queue(db: PgPool) {
    pi_job(&db, 1, 100, 1).await;
    let w = worker(&db).await;
    for bad in [json!({ "inside": 101 }), json!({ "nope": 1 }), json!("x")] {
        let c = db::claim_unit(&db, w, TTL)
            .await
            .unwrap()
            .expect("unit is back in the queue");
        let r = db::complete_lease(&db, w, c.lease_id, &bad, 1)
            .await
            .unwrap();
        assert!(matches!(r, Completion::InvalidOutput(_)), "{bad}");
        assert_eq!(unit_status(&db, c.unit_id).await, "pending");
    }
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn departed_worker_leases_are_requeued(db: PgPool) {
    pi_job(&db, 2, 100, 1).await;
    let (gone, next) = (worker(&db).await, worker(&db).await);
    let c = db::claim_unit(&db, gone, TTL).await.unwrap().unwrap();

    assert_eq!(db::release_worker_leases(&db, gone).await.unwrap(), 1);
    assert_eq!(unit_status(&db, c.unit_id).await, "pending");
    assert_eq!(db::release_worker_leases(&db, gone).await.unwrap(), 0);

    // The released unit is handed out again, and a late result from the
    // departed worker no longer counts.
    let again = db::claim_unit(&db, next, TTL).await.unwrap().unwrap();
    assert_eq!(again.unit_id, c.unit_id);
    let out = sim_worker::compute(&c.template.id, c.template.version, &c.params).unwrap();
    let late = db::complete_lease(&db, gone, c.lease_id, &out, 1)
        .await
        .unwrap();
    assert!(matches!(late, Completion::StaleLease));
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn failed_unit_is_released_only_by_its_owner(db: PgPool) {
    pi_job(&db, 1, 100, 1).await;
    let (owner, other) = (worker(&db).await, worker(&db).await);
    let c = db::claim_unit(&db, owner, TTL).await.unwrap().unwrap();
    assert!(!db::release_lease(&db, other, c.lease_id).await.unwrap());
    assert_eq!(unit_status(&db, c.unit_id).await, "leased");
    assert!(db::release_lease(&db, owner, c.lease_id).await.unwrap());
    assert_eq!(unit_status(&db, c.unit_id).await, "pending");
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn heartbeat_extends_only_own_active_leases(db: PgPool) {
    pi_job(&db, 1, 100, 1).await;
    let (owner, other) = (worker(&db).await, worker(&db).await);
    let c = db::claim_unit(&db, owner, Duration::from_secs(5))
        .await
        .unwrap()
        .unwrap();
    let expires = |db: PgPool| async move {
        sqlx::query_scalar::<_, time::OffsetDateTime>("SELECT expires_at FROM leases")
            .fetch_one(&db)
            .await
            .unwrap()
    };
    let before = expires(db.clone()).await;
    db::heartbeat(&db, other, &[c.lease_id], Duration::from_secs(600))
        .await
        .unwrap();
    assert_eq!(expires(db.clone()).await, before);
    db::heartbeat(&db, owner, &[c.lease_id], Duration::from_secs(600))
        .await
        .unwrap();
    assert!(expires(db.clone()).await > before + time::Duration::minutes(5));
}

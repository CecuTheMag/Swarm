//! School timetables (spec 0013), end to end: searches on simulated devices, the school
//! served by its hash, forged and lazy results.

use base64::{Engine, engine::general_purpose::STANDARD};
use coordinator::{
    Config, State, accounts,
    auth::PUBLIC_NETWORK,
    db::{self, Completion, NewJob},
    templates,
};
use protocol::{
    Backend, DeviceInfo, JobStatus, JobSummary,
    api::NetworkView,
    templates::tt::{self, JobResult},
};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::time::Duration;
use timetable::{Problem, Search, sample, solve};
use uuid::Uuid;

const ADMIN: (&str, &str) = ("admin@swarm.test", "admin password 1");

fn config() -> Config {
    Config {
        honeypot_rate: 0.0,
        new_client_honeypot_rate: 0.0,
        trust_min_correct: u32::MAX,
        no_work_retry: Duration::from_millis(100),
        ..Config::default()
    }
}

async fn serve(db: PgPool, config: Config) -> String {
    accounts::ensure_admin(&db, ADMIN.0, ADMIN.1).await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let state = State::new(db, config);
    coordinator::spawn_reaper(state.clone());
    coordinator::spawn_progress(state.clone());
    let app = coordinator::router(state);
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

/// The platform admin's browser.
async fn admin(addr: &str) -> Client {
    let http = Client::builder().cookie_store(true).build().unwrap();
    let res = http
        .post(format!("http://{addr}/api/auth/login"))
        .header("origin", format!("http://{addr}"))
        .json(&json!({ "email": ADMIN.0, "password": ADMIN.1 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    http
}

async fn post(http: &Client, addr: &str, path: &str, body: Value) -> reqwest::Response {
    http.post(format!("http://{addr}{path}"))
        .header("origin", format!("http://{addr}"))
        .json(&body)
        .send()
        .await
        .unwrap()
}

fn tt_job(classes: usize, searches: u32, iterations: u32) -> Value {
    json!({
        "template": tt::ID,
        "params": {
            "school": sample::school(5, classes),
            "searches": searches,
            "iterations": iterations,
            "seed": 2026,
        },
    })
}

/// The cost an honest device finds for unit `index`.
fn reference_cost(classes: usize, index: u32, iterations: u32) -> u64 {
    let p = Problem::new(&sample::school(5, classes)).unwrap();
    let slots = solve(&p, Search::unit(2026, index, iterations));
    timetable::cost(&p, &slots).unwrap().total
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn a_schools_searches_run_on_its_devices_and_the_best_timetable_wins(db: PgPool) {
    let addr = serve(db.clone(), config()).await;
    let http = admin(&addr).await;
    let res = post(&http, &addr, "/api/networks", json!({ "name": "ПМГ" })).await;
    let school: NetworkView = res.json().await.unwrap();

    let mut body = tt_job(6, 8, 60_000);
    body["quorum"] = json!(1);
    let res = post(
        &http,
        &addr,
        &format!("/api/networks/{}/jobs", school.id),
        body,
    )
    .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let job: JobSummary = res.json().await.unwrap();

    // Devices fetch the school once, by its hash.
    let bytes = tt::school_bytes(&sample::school(5, 6));
    let sha = hex::encode(Sha256::digest(&bytes));
    let served = http
        .get(format!("http://{addr}/api/blobs/{sha}.json"))
        .send()
        .await
        .unwrap();
    assert_eq!(served.status(), StatusCode::OK);
    assert_eq!(served.bytes().await.unwrap().as_ref(), &bytes[..]);
    for missing in [format!("{}.json", "0".repeat(64)), sha.clone()] {
        let res = http
            .get(format!("http://{addr}/api/blobs/{missing}"))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{missing}");
    }

    let workers: Vec<_> = (0..3)
        .map(|_| {
            tokio::spawn(sim_worker::run(sim_worker::Options {
                url: format!("ws://{addr}/ws/worker"),
                exit_when_idle: true,
                join_code: Some(school.join_code.clone()),
                ..Default::default()
            }))
        })
        .collect();
    let mut accepted = 0;
    for w in workers {
        accepted += w.await.unwrap().unwrap().accepted;
    }
    assert_eq!(accepted, 8);

    let done: JobSummary = http
        .get(format!(
            "http://{addr}/api/networks/{}/jobs/{}",
            school.id, job.id
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(done.status, JobStatus::Done);
    let result: JobResult = serde_json::from_value(done.result.unwrap()).unwrap();
    let expected: Vec<u64> = (0..8).map(|i| reference_cost(6, i, 60_000)).collect();
    assert_eq!(result.costs, expected);
    assert_eq!(result.cost.total, *expected.iter().min().unwrap());
    assert_eq!(
        result.cost.hard, 0,
        "the example school has a valid timetable"
    );
    // The timetable itself re-scores to the cost reported.
    let slots: Vec<u16> = STANDARD
        .decode(&result.slots)
        .unwrap()
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    let p = Problem::new(&sample::school(5, 6)).unwrap();
    assert_eq!(timetable::cost(&p, &slots).unwrap(), result.cost);
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn a_device_that_forges_timetables_is_refused_and_banned(db: PgPool) {
    let addr = serve(db.clone(), config()).await;
    let http = admin(&addr).await;
    let res = post(&http, &addr, "/api/networks", json!({ "name": "СУ" })).await;
    let school: NetworkView = res.json().await.unwrap();
    let mut body = tt_job(3, 12, 20_000);
    body["quorum"] = json!(1);
    let job: JobSummary = post(
        &http,
        &addr,
        &format!("/api/networks/{}/jobs", school.id),
        body,
    )
    .await
    .json()
    .await
    .unwrap();

    let forger = Uuid::now_v7();
    let forging = sim_worker::run(sim_worker::Options {
        url: format!("ws://{addr}/ws/worker"),
        exit_when_idle: true,
        join_code: Some(school.join_code.clone()),
        client_id: Some(forger),
        cheat: sim_worker::Cheat::Random { rate: 1.0 },
        ..Default::default()
    })
    .await;
    // Blocked once its forgeries add up; none of them was accepted.
    assert!(forging.map_or(true, |s| s.accepted == 0));
    let banned: bool =
        sqlx::query_scalar("SELECT banned_at IS NOT NULL FROM clients WHERE id = $1")
            .bind(forger)
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(banned);

    // Honest devices finish the job with nothing of the forger's in it.
    for _ in 0..20 {
        sim_worker::run(sim_worker::Options {
            url: format!("ws://{addr}/ws/worker"),
            exit_when_idle: true,
            join_code: Some(school.join_code.clone()),
            ..Default::default()
        })
        .await
        .unwrap();
        let j: JobSummary = http
            .get(format!(
                "http://{addr}/api/networks/{}/jobs/{}",
                school.id, job.id
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if j.status == JobStatus::Done {
            let result: JobResult = serde_json::from_value(j.result.unwrap()).unwrap();
            let expected: Vec<u64> = (0..12).map(|i| reference_cost(3, i, 20_000)).collect();
            assert_eq!(result.costs, expected);
            return;
        }
    }
    panic!("the job did not finish");
}

async fn worker(db: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    let device = DeviceInfo {
        backend: Backend::Native,
        vendor: "test".into(),
        architecture: "test".into(),
    };
    let registration = db::insert_worker(
        db,
        id,
        &device,
        "test",
        Uuid::now_v7(),
        db::Joining::open(PUBLIC_NETWORK),
    )
    .await
    .unwrap();
    assert!(matches!(registration, db::Registration::Registered { .. }));
    id
}

fn output(classes: usize, index: u32, iterations: u32) -> Value {
    let p = Problem::new(&sample::school(5, classes)).unwrap();
    let slots = solve(&p, Search::unit(2026, index, iterations));
    let bytes: Vec<u8> = slots.iter().flat_map(|s| s.to_le_bytes()).collect();
    json!({
        "slots": STANDARD.encode(bytes),
        "cost": timetable::cost(&p, &slots).unwrap().total,
    })
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn a_lazy_device_is_outvoted_by_honest_ones(db: PgPool) {
    let cfg = config();
    assert_eq!(cfg.redundancy, 2);
    let body = tt_job(12, 1, 30_000);
    let t = templates::get(tt::ID).unwrap();
    let units = t.template.split(&body["params"]).unwrap();
    db::put_blob(&db, &t.template.blob(&body["params"]).unwrap())
        .await
        .unwrap();
    let job = db::create_job(
        &db,
        NewJob::new(PUBLIC_NETWORK, t, &body["params"], units, 2),
    )
    .await
    .unwrap();

    // A lazy device searches less: a valid timetable, honestly scored, but worse than the
    // one the unit asks for. Honest devices agree on the right one.
    let honest = output(12, 0, 30_000);
    let lazy = output(12, 0, 3_000);
    assert!(lazy["cost"].as_u64() > honest["cost"].as_u64());
    let (a, b, c) = (worker(&db).await, worker(&db).await, worker(&db).await);
    let claim = |w| {
        let db = db.clone();
        let cfg = cfg.clone();
        async move {
            db::claim_unit(&db, w, PUBLIC_NETWORK, false, &cfg)
                .await
                .unwrap()
                .unwrap()
        }
    };
    let (ca, cb) = (claim(a).await, claim(b).await);
    let first = db::complete_lease(&db, a, ca.lease_id, &honest, 1, &cfg)
        .await
        .unwrap();
    assert!(matches!(first, Completion::Recorded));
    let second = db::complete_lease(&db, b, cb.lease_id, &lazy, 1, &cfg)
        .await
        .unwrap();
    assert!(matches!(second, Completion::Disputed { .. }));
    let cc = claim(c).await;
    let third = db::complete_lease(&db, c, cc.lease_id, &honest, 1, &cfg)
        .await
        .unwrap();
    assert!(matches!(third, Completion::Decided { agreed: true, .. }));

    let verdict: String = sqlx::query_scalar("SELECT verdict FROM leases WHERE id = $1")
        .bind(cb.lease_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(verdict, "disagreed");
    let done = db::get_job(&db, job.id).await.unwrap().unwrap();
    let result: JobResult = serde_json::from_value(done.result.unwrap()).unwrap();
    assert_eq!(result.cost.total, honest["cost"].as_u64().unwrap());
}

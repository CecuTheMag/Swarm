//! End-to-end: real HTTP server, WebSocket workers and dashboard stream.

use coordinator::{Config, State};
use futures_util::{SinkExt, StreamExt};
use protocol::{
    CoordinatorMessage, DashboardEvent, JobStatus, JobSummary, PROTOCOL_VERSION, templates::pi,
};
use serde_json::json;
use sqlx::PgPool;
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Starts the coordinator on an ephemeral port and returns its address.
async fn serve(db: PgPool, config: Config) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let app = coordinator::router(State::new(db, config));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

fn sim(addr: &str) -> sim_worker::Options {
    sim_worker::Options {
        url: format!("ws://{addr}/ws/worker"),
        max_units: None,
        exit_when_idle: true,
    }
}

async fn create_pi_job(addr: &str, units: u32, samples: u32, seed: u32) -> JobSummary {
    let res = reqwest::Client::new()
        .post(format!("http://{addr}/api/jobs"))
        .json(&sim_worker::pi_job(units, samples, seed))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    res.json().await.unwrap()
}

async fn get_job(addr: &str, id: uuid::Uuid) -> JobSummary {
    reqwest::get(format!("http://{addr}/api/jobs/{id}"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn several_workers_complete_a_job_exactly(db: PgPool) {
    let addr = serve(db, Config::default()).await;
    let (units, samples, seed) = (24, 50_000, 2026);
    let job = create_pi_job(&addr, units, samples, seed).await;
    assert_eq!(job.status, JobStatus::Queued);

    let workers: Vec<_> = (0..3)
        .map(|_| tokio::spawn(sim_worker::run(sim(&addr))))
        .collect();
    let mut accepted = 0;
    for w in workers {
        accepted += w.await.unwrap().unwrap().accepted;
    }
    assert_eq!(accepted, units);

    let job = get_job(&addr, job.id).await;
    assert_eq!(job.status, JobStatus::Done);
    let result: pi::JobResult = serde_json::from_value(job.result.unwrap()).unwrap();
    let expected: u64 = (0..units)
        .map(|unit_index| {
            u64::from(kernels::pi::count_inside(kernels::pi::UnitParams {
                seed,
                unit_index,
                samples,
            }))
        })
        .sum();
    assert_eq!(result.inside, expected);
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn worker_that_disconnects_mid_job_loses_its_unit(db: PgPool) {
    let addr = serve(db, Config::default()).await;
    let job = create_pi_job(&addr, 5, 1000, 1).await;

    // A raw worker takes a unit and vanishes without answering.
    let (mut ws, _) = connect_async(format!("ws://{addr}/ws/worker"))
        .await
        .unwrap();
    hello(&mut ws).await;
    ws.send(Message::text(r#"{"type":"request_work"}"#))
        .await
        .unwrap();
    assert!(matches!(
        next(&mut ws).await,
        CoordinatorMessage::Assign { .. }
    ));
    drop(ws);

    // The remaining worker still finishes every unit, including the abandoned one.
    let mut stats = sim_worker::Stats::default();
    for _ in 0..50 {
        let s = sim_worker::run(sim(&addr)).await.unwrap();
        stats.accepted += s.accepted;
        if get_job(&addr, job.id).await.status == JobStatus::Done {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(stats.accepted, 5);
    assert_eq!(get_job(&addr, job.id).await.status, JobStatus::Done);
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn protocol_violations_are_rejected(db: PgPool) {
    let addr = serve(db, Config::default()).await;
    seed_job(&addr).await;

    // Work request before hello.
    let (mut ws, _) = connect_async(format!("ws://{addr}/ws/worker"))
        .await
        .unwrap();
    ws.send(Message::text(r#"{"type":"request_work"}"#))
        .await
        .unwrap();
    assert!(matches!(
        next(&mut ws).await,
        CoordinatorMessage::Error { .. }
    ));

    // Unknown message type.
    let (mut ws, _) = connect_async(format!("ws://{addr}/ws/worker"))
        .await
        .unwrap();
    hello(&mut ws).await;
    ws.send(Message::text(r#"{"type":"run_code","code":"x"}"#))
        .await
        .unwrap();
    assert!(matches!(
        next(&mut ws).await,
        CoordinatorMessage::Error { .. }
    ));

    // Wrong protocol version.
    let (mut ws, _) = connect_async(format!("ws://{addr}/ws/worker"))
        .await
        .unwrap();
    ws.send(Message::text(
        json!({"type":"hello","protocol_version":999,"client_version":"x",
               "device":{"backend":"native","vendor":"x","architecture":"x"}})
        .to_string(),
    ))
    .await
    .unwrap();
    assert!(matches!(
        next(&mut ws).await,
        CoordinatorMessage::Error { .. }
    ));

    // Hoarding: a second request while holding the maximum number of leases.
    let (mut ws, _) = connect_async(format!("ws://{addr}/ws/worker"))
        .await
        .unwrap();
    hello(&mut ws).await;
    ws.send(Message::text(r#"{"type":"request_work"}"#))
        .await
        .unwrap();
    assert!(matches!(
        next(&mut ws).await,
        CoordinatorMessage::Assign { .. }
    ));
    ws.send(Message::text(r#"{"type":"request_work"}"#))
        .await
        .unwrap();
    assert!(matches!(
        next(&mut ws).await,
        CoordinatorMessage::Error { .. }
    ));
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn idle_connections_are_closed(db: PgPool) {
    let config = Config {
        idle_timeout: Duration::from_millis(200),
        ..Config::default()
    };
    let addr = serve(db, config).await;
    let (mut ws, _) = connect_async(format!("ws://{addr}/ws/worker"))
        .await
        .unwrap();
    hello(&mut ws).await;
    assert!(matches!(
        next(&mut ws).await,
        CoordinatorMessage::Error { .. }
    ));
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn rest_api_validates_input(db: PgPool) {
    let addr = serve(db, Config::default()).await;
    let client = reqwest::Client::new();
    let post = |body: serde_json::Value| {
        client
            .post(format!("http://{addr}/api/jobs"))
            .json(&body)
            .send()
    };
    assert_eq!(
        post(json!({"template":"miner","params":{}}))
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        post(json!({"template":"pi","params":{"units":0,"samples_per_unit":1,"seed":0}}))
            .await
            .unwrap()
            .status(),
        400
    );
    let missing = reqwest::get(format!("http://{addr}/api/jobs/{}", uuid::Uuid::now_v7()))
        .await
        .unwrap();
    assert_eq!(missing.status(), 404);
    let health = reqwest::get(format!("http://{addr}/healthz"))
        .await
        .unwrap();
    assert_eq!(health.status(), 200);
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn dashboard_streams_live_events(db: PgPool) {
    let addr = serve(db, Config::default()).await;
    let (mut dash, _) = connect_async(format!("ws://{addr}/ws/dashboard"))
        .await
        .unwrap();
    assert!(matches!(
        next_event(&mut dash).await,
        DashboardEvent::Snapshot { workers, jobs } if workers.is_empty() && jobs.is_empty()
    ));

    let job = create_pi_job(&addr, 2, 1000, 5).await;
    assert!(matches!(
        next_event(&mut dash).await,
        DashboardEvent::JobUpdated { job: j } if j.id == job.id && j.status == JobStatus::Queued
    ));

    sim_worker::run(sim(&addr)).await.unwrap();

    let mut saw = (false, false, 0, false, false);
    while !saw.4 {
        match next_event(&mut dash).await {
            DashboardEvent::WorkerJoined { .. } => saw.0 = true,
            DashboardEvent::JobUpdated { job: j } if j.status == JobStatus::Running => saw.1 = true,
            DashboardEvent::UnitCompleted { .. } => saw.2 += 1,
            DashboardEvent::JobUpdated { job: j } if j.status == JobStatus::Done => saw.3 = true,
            DashboardEvent::WorkerLeft { .. } => saw.4 = true,
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(saw, (true, true, 2, true, true));
}

async fn seed_job(addr: &str) {
    create_pi_job(addr, 3, 100, 0).await;
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn hello(ws: &mut Ws) {
    ws.send(Message::text(
        json!({"type":"hello","protocol_version":PROTOCOL_VERSION,"client_version":"test",
               "device":{"backend":"native","vendor":"x","architecture":"x"}})
        .to_string(),
    ))
    .await
    .unwrap();
    assert!(matches!(next(ws).await, CoordinatorMessage::Welcome { .. }));
}

async fn next_text(ws: &mut Ws) -> String {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("timed out waiting for a message")
            .expect("connection closed")
            .unwrap();
        if let Message::Text(t) = frame {
            return t.to_string();
        }
    }
}

async fn next(ws: &mut Ws) -> CoordinatorMessage {
    serde_json::from_str(&next_text(ws).await).unwrap()
}

async fn next_event(ws: &mut Ws) -> DashboardEvent {
    serde_json::from_str(&next_text(ws).await).unwrap()
}

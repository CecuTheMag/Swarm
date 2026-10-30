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
    let state = State::new(db, config);
    coordinator::spawn_reaper(state.clone());
    coordinator::spawn_progress(state.clone());
    let app = coordinator::router(state);
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

/// One result decides a unit and there are no honeypots: for tests about scheduling
/// with a single worker, which cannot verify its own results (spec 0004).
fn unverified() -> Config {
    Config {
        redundancy: 1,
        honeypot_rate: 0.0,
        new_client_honeypot_rate: 0.0,
        ..Config::default()
    }
}

/// Full verification, but no honeypots: once a job is done, a worker asking for more
/// work may otherwise get a copy of one of its units, which would skew exact counts.
fn without_honeypots() -> Config {
    Config {
        honeypot_rate: 0.0,
        new_client_honeypot_rate: 0.0,
        ..Config::default()
    }
}

fn sim(addr: &str) -> sim_worker::Options {
    sim_worker::Options {
        url: format!("ws://{addr}/ws/worker"),
        exit_when_idle: true,
        ..Default::default()
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
    let addr = serve(db, without_honeypots()).await;
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
    // Every unit was computed by two different workers whose results agreed.
    assert_eq!(accepted, units * Config::default().redundancy);

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
    let addr = serve(db, without_honeypots()).await;
    let job = create_pi_job(&addr, 5, 1000, 1).await;
    let (mut dash, _) = connect_async(format!("ws://{addr}/ws/dashboard"))
        .await
        .unwrap();

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
    // The projector shows the unit going back to the queue.
    loop {
        if let DashboardEvent::WorkerLeft { units_returned, .. } = next_event(&mut dash).await {
            assert_eq!(units_returned, 1);
            break;
        }
    }

    // The remaining workers still finish every unit, including the abandoned one; each
    // unit is verified by two of them.
    let mut stats = sim_worker::Stats::default();
    for _ in 0..50 {
        let s = sim_worker::run(sim(&addr)).await.unwrap();
        stats.accepted += s.accepted;
        if get_job(&addr, job.id).await.status == JobStatus::Done {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(stats.accepted, 5 * Config::default().redundancy);
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

    // Hoarding: one more request while holding the maximum number of leases.
    create_pi_job(&addr, 20, 100, 1).await;
    let (mut ws, _) = connect_async(format!("ws://{addr}/ws/worker"))
        .await
        .unwrap();
    hello(&mut ws).await;
    for _ in 0..Config::default().max_leases_per_worker {
        ws.send(Message::text(r#"{"type":"request_work"}"#))
            .await
            .unwrap();
        assert!(matches!(
            next(&mut ws).await,
            CoordinatorMessage::Assign { .. }
        ));
    }
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
async fn pings_do_not_keep_a_silent_worker_connected(db: PgPool) {
    let config = Config {
        idle_timeout: Duration::from_millis(300),
        ..Config::default()
    };
    let addr = serve(db, config).await;
    let (mut ws, _) = connect_async(format!("ws://{addr}/ws/worker"))
        .await
        .unwrap();
    hello(&mut ws).await;
    // Only WebSocket pings, as a proxy might send for a worker that is gone.
    let started = std::time::Instant::now();
    let closing = loop {
        ws.send(Message::Ping(Vec::new().into())).await.unwrap();
        match tokio::time::timeout(Duration::from_millis(50), ws.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => break t,
            Ok(Some(Ok(_))) | Err(_) => {}
            other => panic!("connection ended without a message: {other:?}"),
        }
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "still connected"
        );
    };
    let msg: CoordinatorMessage = serde_json::from_str(&closing).unwrap();
    assert!(matches!(msg, CoordinatorMessage::Error { .. }), "{msg:?}");
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn the_organiser_can_stop_a_job(db: PgPool) {
    let config = Config {
        submit_token: Some("pw".into()),
        ..unverified()
    };
    let addr = serve(db, config).await;
    let client = reqwest::Client::new();
    let job: JobSummary = client
        .post(format!("http://{addr}/api/jobs"))
        .header("authorization", "Bearer pw")
        .json(&sim_worker::pi_job(50, 1000, 3))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let cancel = |auth: &'static str| {
        client
            .post(format!("http://{addr}/api/jobs/{}/cancel", job.id))
            .header("authorization", auth)
            .send()
    };
    assert_eq!(cancel("Bearer guess").await.unwrap().status(), 401);

    let res = cancel("Bearer pw").await.unwrap();
    assert_eq!(res.status(), 200);
    let stopped: JobSummary = res.json().await.unwrap();
    assert_eq!(stopped.status, JobStatus::Failed);
    assert_eq!(stopped.error.as_deref(), Some(coordinator::db::CANCELLED));

    // None of its units are handed out any more.
    let stats = sim_worker::run(sim(&addr)).await.unwrap();
    assert_eq!(stats.completed, 0);
    // Stopping it again changes nothing; an unknown job is not found.
    assert_eq!(cancel("Bearer pw").await.unwrap().status(), 200);
    let missing = client
        .post(format!(
            "http://{addr}/api/jobs/{}/cancel",
            uuid::Uuid::now_v7()
        ))
        .header("authorization", "Bearer pw")
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 404);
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
async fn submitting_needs_the_password_when_set(db: PgPool) {
    let config = Config {
        submit_token: Some("correct horse".into()),
        ..Config::default()
    };
    let addr = serve(db, config).await;
    let client = reqwest::Client::new();
    let post = |auth: Option<&str>| {
        let mut req = client
            .post(format!("http://{addr}/api/jobs"))
            .json(&sim_worker::pi_job(4, 1024, 1));
        if let Some(auth) = auth {
            req = req.header("authorization", auth);
        }
        req.send()
    };
    for wrong in [
        None,
        Some("Bearer wrong"),
        Some("correct horse"),
        Some("Bearer "),
    ] {
        let res = post(wrong).await.unwrap();
        assert_eq!(res.status(), 401, "{wrong:?}");
        let body: protocol::ApiError = res.json().await.unwrap();
        assert!(!body.error.is_empty());
    }
    assert_eq!(
        post(Some("Bearer correct horse")).await.unwrap().status(),
        201
    );
    // Reading stays open: the render page and dashboard are public.
    let jobs = reqwest::get(format!("http://{addr}/api/jobs"))
        .await
        .unwrap();
    assert_eq!(jobs.status(), 200);
    let jobs: Vec<JobSummary> = jobs.json().await.unwrap();
    assert_eq!(jobs.len(), 1);
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn dashboard_streams_live_events(db: PgPool) {
    let addr = serve(db, unverified()).await;
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
            DashboardEvent::UnitsCompleted {
                units,
                jobs,
                workers,
            } => {
                saw.2 += units;
                assert!(
                    jobs.iter()
                        .all(|j| j.job_id == job.id && j.units_total == 2)
                );
                assert_eq!(workers.iter().map(|w| w.units).sum::<u32>(), units);
            }
            DashboardEvent::JobUpdated { job: j } if j.status == JobStatus::Done => saw.3 = true,
            DashboardEvent::WorkerLeft { units_returned, .. } => {
                // It finished everything it held.
                assert_eq!(units_returned, 0);
                saw.4 = true;
            }
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

fn decode_png(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut rgba).unwrap();
    (info.width, info.height, rgba)
}

async fn image(addr: &str, id: uuid::Uuid) -> reqwest::Response {
    reqwest::get(format!("http://{addr}/api/jobs/{id}/image.png"))
        .await
        .unwrap()
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn several_workers_render_an_image(db: PgPool) {
    let addr = serve(db.clone(), Config::default()).await;
    let (width, height, spp, passes, seed) = (40, 24, 2, 2, 9);
    let res = reqwest::Client::new()
        .post(format!("http://{addr}/api/jobs"))
        .json(&sim_worker::pt_job(width, height, 16, spp, passes, seed))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    let job: JobSummary = res.json().await.unwrap();

    // Viewing the image before any work starts the live accumulator: nothing rendered.
    let before = image(&addr, job.id).await;
    assert_eq!(before.status(), 200);
    assert_eq!(before.headers()["content-type"], "image/png");
    let (w, h, rgba) = decode_png(&before.bytes().await.unwrap());
    assert_eq!((w, h), (width, height));
    assert!(rgba.chunks(4).all(|px| px[3] == 0));

    let workers: Vec<_> = (0..3)
        .map(|_| tokio::spawn(sim_worker::run(sim(&addr))))
        .collect();
    for w in workers {
        w.await.unwrap().unwrap();
    }
    let job = get_job(&addr, job.id).await;
    assert_eq!(job.status, JobStatus::Done);
    let result: protocol::templates::pt::JobResult =
        serde_json::from_value(job.result.unwrap()).unwrap();
    assert_eq!(result.samples_per_pixel, spp * passes);

    // The live image equals one rendered on a single machine (up to 8-bit rounding,
    // since passes add their sums separately).
    let live = image(&addr, job.id).await.bytes().await.unwrap();
    let (_, _, rgba) = decode_png(&live);
    let mut whole = vec![0.0; (width * height * 3) as usize];
    kernels::pt::render_tile(
        &kernels::pt::TileParams {
            width,
            height,
            x: 0,
            y: 0,
            w: width,
            h: height,
            spp: spp * passes,
            pass: 0,
            seed,
        },
        &mut whole,
    );
    for (px, sums) in rgba.chunks(4).zip(whole.chunks(3)) {
        assert_eq!(px[3], 255);
        for c in 0..3 {
            let mean = sums[c] / (spp * passes) as f32;
            let expected = (mean.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0 + 0.5) as i32;
            assert!(
                (i32::from(px[c]) - expected).abs() <= 1,
                "{px:?} vs {sums:?}"
            );
        }
    }

    // A coordinator that rebuilds the image from the database gets the same picture.
    let other = serve(db, Config::default()).await;
    assert_eq!(image(&other, job.id).await.bytes().await.unwrap(), live);

    // Only path-tracer jobs have images.
    let pi_job = create_pi_job(&addr, 1, 10, 0).await;
    assert_eq!(image(&addr, pi_job.id).await.status(), 404);
    assert_eq!(image(&addr, uuid::Uuid::now_v7()).await.status(), 404);
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn a_hung_worker_loses_its_unit_to_a_healthy_one(db: PgPool) {
    let config = Config {
        lease_ttl: Duration::from_secs(1),
        max_lease_duration: Duration::from_secs(2),
        reap_interval: Duration::from_millis(200),
        no_work_retry: Duration::from_millis(200),
        ..unverified()
    };
    let addr = serve(db.clone(), config).await;
    let job = create_pi_job(&addr, 1, 1000, 4).await;

    // Takes the only unit, then stays connected and keeps sending heartbeats for it,
    // but never sends a result: a GPU that hung inside a tab, say.
    let (mut hung, _) = connect_async(format!("ws://{addr}/ws/worker"))
        .await
        .unwrap();
    hello(&mut hung).await;
    hung.send(Message::text(r#"{"type":"request_work"}"#))
        .await
        .unwrap();
    let CoordinatorMessage::Assign {
        lease_id, params, ..
    } = next(&mut hung).await
    else {
        panic!("expected an assignment");
    };
    let beats = tokio::spawn(async move {
        for _ in 0..20 {
            let beat = json!({"type": "heartbeat", "lease_ids": [lease_id]}).to_string();
            if hung.send(Message::text(beat)).await.is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        hung
    });

    // A healthy worker waits for work; it gets the unit once the lease reaches its
    // maximum lifetime and the reaper returns it to the queue.
    let healthy = sim_worker::run(sim_worker::Options {
        max_units: Some(1),
        exit_when_idle: false,
        ..sim(&addr)
    });
    let stats = tokio::time::timeout(Duration::from_secs(10), healthy)
        .await
        .expect("the unit was never handed to the healthy worker")
        .unwrap();
    assert_eq!(stats.accepted, 1);
    assert_eq!(get_job(&addr, job.id).await.status, JobStatus::Done);

    let reason: String = sqlx::query_scalar("SELECT release_reason FROM leases WHERE id = $1")
        .bind(lease_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(reason, "expired");

    // The hung worker's result, if it ever comes, is not counted.
    let mut hung = beats.await.unwrap();
    let output = sim_worker::compute(pi::ID, pi::VERSION, &params).unwrap();
    hung.send(Message::text(
        json!({"type": "result", "lease_id": lease_id, "output": output, "elapsed_ms": 1})
            .to_string(),
    ))
    .await
    .unwrap();
    let reply = next(&mut hung).await;
    assert!(
        matches!(
            reply,
            CoordinatorMessage::Ack {
                accepted: false,
                ..
            }
        ),
        "unexpected {reply:?}"
    );
}

async fn client_banned(db: &PgPool, client_id: uuid::Uuid) -> bool {
    sqlx::query_scalar("SELECT banned_at IS NOT NULL FROM clients WHERE id = $1")
        .bind(client_id)
        .fetch_one(db)
        .await
        .unwrap()
}

/// Runs fresh honest workers (each a new client) until the job is decided.
async fn finish_with_honest_workers(addr: &str, job: uuid::Uuid) -> JobSummary {
    for _ in 0..40 {
        let workers: Vec<_> = (0..3)
            .map(|_| tokio::spawn(sim_worker::run(sim(addr))))
            .collect();
        for w in workers {
            w.await.unwrap().unwrap();
        }
        let j = get_job(addr, job).await;
        if j.status != JobStatus::Running && j.status != JobStatus::Queued {
            return j;
        }
    }
    panic!("job {job} did not finish");
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn an_honest_majority_outvotes_cheaters_and_bans_them(db: PgPool) {
    let addr = serve(db.clone(), Config::default()).await;
    let (units, samples, seed) = (30, 20_000, 77);
    let job = create_pi_job(&addr, units, samples, seed).await;
    // The projector watches: it must see the cheaters caught.
    let (dash, _) = connect_async(format!("ws://{addr}/ws/dashboard"))
        .await
        .unwrap();
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    tokio::spawn(record_events(dash, seen.clone()));

    // Two workers that always make up their results, alongside honest ones.
    let cheaters: Vec<uuid::Uuid> = (0..2).map(|_| uuid::Uuid::now_v7()).collect();
    let cheating: Vec<_> = cheaters
        .iter()
        .map(|id| {
            tokio::spawn(sim_worker::run(sim_worker::Options {
                client_id: Some(*id),
                cheat: sim_worker::Cheat::Random { rate: 1.0 },
                ..sim(&addr)
            }))
        })
        .collect();
    let job = finish_with_honest_workers(&addr, job.id).await;
    for c in cheating {
        // Blocked mid-session, or done: either way it stops.
        let _ = c.await.unwrap();
    }

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
    assert_eq!(result.inside, expected, "a made-up result got through");
    for id in &cheaters {
        assert!(client_banned(&db, *id).await, "cheater {id} was not banned");
    }
    // A banned client cannot reconnect.
    let blocked = sim_worker::run(sim_worker::Options {
        client_id: Some(cheaters[0]),
        ..sim(&addr)
    })
    .await;
    assert!(blocked.unwrap_err().to_string().contains("blocked"));

    // Each cheater appeared on the dashboard as banned, with its wrong results, and
    // then left; nobody else was banned.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let seen = seen.lock().unwrap().clone();
    let banned: Vec<uuid::Uuid> = seen
        .iter()
        .filter_map(|e| match e {
            DashboardEvent::WorkerBanned {
                worker_id,
                wrong_results,
                ..
            } => {
                assert!(*wrong_results >= 1);
                Some(*worker_id)
            }
            _ => None,
        })
        .collect();
    assert_eq!(banned.len(), cheaters.len(), "{banned:?}");
    for worker in &banned {
        assert!(seen.iter().any(
            |e| matches!(e, DashboardEvent::WorkerLeft { worker_id, .. } if worker_id == worker)
        ));
    }
}

/// Keeps every dashboard event in `seen`.
async fn record_events(mut dash: Ws, seen: std::sync::Arc<std::sync::Mutex<Vec<DashboardEvent>>>) {
    while let Some(Ok(frame)) = dash.next().await {
        if let Message::Text(t) = frame {
            seen.lock().unwrap().push(serde_json::from_str(&t).unwrap());
        }
    }
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn colluders_are_caught_by_honeypots_and_their_work_redone(db: PgPool) {
    // One coordinator without honeypots, one where every request from an untrusted
    // client is a honeypot; both serve the same database.
    let plain = serve(
        db.clone(),
        Config {
            honeypot_rate: 0.0,
            new_client_honeypot_rate: 0.0,
            ..Config::default()
        },
    )
    .await;
    let traps = serve(
        db.clone(),
        Config {
            honeypot_rate: 1.0,
            new_client_honeypot_rate: 1.0,
            ..Config::default()
        },
    )
    .await;

    // A finished, verified job provides the known answers.
    let seeded = create_pi_job(&plain, 6, 5_000, 1).await;
    assert_eq!(
        finish_with_honest_workers(&plain, seeded.id).await.status,
        JobStatus::Done
    );

    // Two colluders return the same wrong answer, so they agree with each other and get
    // units of the next job accepted.
    let (units, samples, seed) = (10, 5_000, 2);
    let job = create_pi_job(&plain, units, samples, seed).await;
    let colluders: Vec<uuid::Uuid> = (0..2).map(|_| uuid::Uuid::now_v7()).collect();
    let collude = |addr: String, client: uuid::Uuid, max_units: u32| {
        tokio::spawn(async move {
            sim_worker::run(sim_worker::Options {
                client_id: Some(client),
                cheat: sim_worker::Cheat::Constant { rate: 1.0 },
                max_units: Some(max_units),
                ..sim(&addr)
            })
            .await
        })
    };
    let first: Vec<_> = colluders
        .iter()
        .map(|c| collude(plain.clone(), *c, 4))
        .collect();
    for c in first {
        c.await.unwrap().unwrap();
    }
    let fooled = get_job(&plain, job.id).await.units_done;
    assert!(
        fooled > 0,
        "the colluders should have fooled the vote on some units"
    );

    // Through the second coordinator they only get honeypots, fail them, and are banned.
    let second: Vec<_> = colluders
        .iter()
        .map(|c| collude(traps.clone(), *c, 30))
        .collect();
    for c in second {
        let _ = c.await.unwrap();
    }
    for id in &colluders {
        assert!(
            client_banned(&db, *id).await,
            "colluder {id} was not banned"
        );
    }
    // What they decided is back in the queue...
    assert!(get_job(&plain, job.id).await.units_done < fooled);

    // ...and honest workers finish the job exactly.
    let job = finish_with_honest_workers(&plain, job.id).await;
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
    assert_eq!(result.inside, expected, "a colluders' result survived");
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn the_largest_tiles_can_be_delivered(db: PgPool) {
    // One 128 × 128 tile: its result is about 262 KiB on the wire.
    let addr = serve(db, unverified()).await;
    let res = reqwest::Client::new()
        .post(format!("http://{addr}/api/jobs"))
        .json(&sim_worker::pt_job(128, 128, 128, 1, 1, 3))
        .send()
        .await
        .unwrap();
    let job: JobSummary = res.json().await.unwrap();
    let stats = sim_worker::run(sim(&addr)).await.unwrap();
    assert_eq!(stats.accepted, 1);
    let job = get_job(&addr, job.id).await;
    assert_eq!(job.status, JobStatus::Done, "{:?}", job.error);
}

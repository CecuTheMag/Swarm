//! A found timetable edited by hand (spec 0015): kept per job, versioned, scored by the
//! coordinator, refused when it is not a timetable of the job's school.

use coordinator::{
    Config, State, accounts,
    templates::tt::{decode_slots, encode_slots},
};
use protocol::{
    JobStatus, JobSummary, TimetableEdit,
    api::{CreatedInvite, NetworkView},
    templates::tt::{self, JobResult},
};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::time::Duration;
use timetable::{Problem, School, Slot, sample};
use uuid::Uuid;

const ADMIN: (&str, &str) = ("admin@swarm.test", "admin password 1");

async fn serve(db: PgPool) -> String {
    accounts::ensure_admin(&db, ADMIN.0, ADMIN.1).await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let state = State::new(
        db,
        Config {
            honeypot_rate: 0.0,
            new_client_honeypot_rate: 0.0,
            no_work_retry: Duration::from_millis(100),
            // As on a development machine (SWARM_OPEN_PUBLIC=1).
            open_public: true,
            ..Config::default()
        },
    );
    coordinator::spawn_reaper(state.clone());
    let app = coordinator::router(state);
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

/// A browser: keeps cookies and sends this site's Origin, as pages do.
struct Browser {
    http: Client,
    addr: String,
}

impl Browser {
    fn new(addr: &str) -> Self {
        Self {
            http: Client::builder().cookie_store(true).build().unwrap(),
            addr: addr.to_owned(),
        }
    }

    async fn send(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> reqwest::Response {
        let mut req = self
            .http
            .request(method, format!("http://{}{path}", self.addr))
            .header("origin", format!("http://{}", self.addr));
        if let Some(body) = body {
            req = req.json(&body);
        }
        req.send().await.unwrap()
    }

    async fn get(&self, path: &str) -> reqwest::Response {
        self.send(reqwest::Method::GET, path, None).await
    }

    async fn post(&self, path: &str, body: Value) -> reqwest::Response {
        self.send(reqwest::Method::POST, path, Some(body)).await
    }

    async fn put(&self, path: &str, body: Value) -> reqwest::Response {
        self.send(reqwest::Method::PUT, path, Some(body)).await
    }

    async fn delete(&self, path: &str) -> reqwest::Response {
        self.send(reqwest::Method::DELETE, path, None).await
    }

    async fn admin(addr: &str) -> Self {
        let b = Self::new(addr);
        let res = b
            .post(
                "/api/auth/login",
                json!({ "email": ADMIN.0, "password": ADMIN.1 }),
            )
            .await;
        assert_eq!(res.status(), StatusCode::OK);
        b
    }

    async fn create_network(&self, name: &str) -> NetworkView {
        let res = self.post("/api/networks", json!({ "name": name })).await;
        assert_eq!(res.status(), StatusCode::CREATED);
        res.json().await.unwrap()
    }

    /// Someone with a new account, invited into `network` with `role`.
    async fn invite(&self, network: Uuid, role: &str, email: &str) -> Browser {
        let res = self
            .post(
                &format!("/api/networks/{network}/invites"),
                json!({ "role": role }),
            )
            .await;
        assert_eq!(res.status(), StatusCode::CREATED);
        let invite: CreatedInvite = res.json().await.unwrap();
        let them = Browser::new(&self.addr);
        let res = them
            .post(
                &format!("/api/invites/{}", invite.token),
                json!({ "email": email, "name": "Петя Колева", "password": "member password" }),
            )
            .await;
        assert_eq!(res.status(), StatusCode::OK);
        them
    }

    async fn job(&self, network: Uuid, body: Value) -> JobSummary {
        let res = self
            .post(&format!("/api/networks/{network}/jobs"), body)
            .await;
        assert_eq!(res.status(), StatusCode::CREATED, "{:?}", res.text().await);
        res.json().await.unwrap()
    }
}

/// A small example school with its first lesson's first hour pinned.
fn school() -> School {
    let mut school = sample::school(5, 2);
    school.lessons[0].pinned = vec![Slot { day: 0, period: 0 }];
    school
}

fn tt_job() -> Value {
    json!({
        "template": tt::ID,
        "quorum": 1,
        "params": { "school": school(), "searches": 2, "iterations": 10_000, "seed": 7 },
    })
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn an_edit_is_kept_versioned_and_scored_by_the_coordinator(db: PgPool) {
    let addr = serve(db).await;
    let admin = Browser::admin(&addr).await;
    let net = admin.create_network("ПМГ").await;
    let job = admin.job(net.id, tt_job()).await;
    let path = format!("/api/networks/{}/jobs/{}/timetable", net.id, job.id);

    // Nothing to edit before the search is done.
    let none: Option<TimetableEdit> = admin.get(&path).await.json().await.unwrap();
    assert_eq!(none, None);
    let res = admin.put(&path, json!({ "slots": "", "pinned": [] })).await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(
        res.text()
            .await
            .unwrap()
            .contains("not found its timetable yet")
    );

    let run = sim_worker::run(sim_worker::Options {
        url: format!("ws://{addr}/ws/worker"),
        exit_when_idle: true,
        join_code: Some(net.join_code.clone()),
        ..Default::default()
    });
    assert_eq!(run.await.unwrap().accepted, 2);
    let done: JobSummary = admin
        .get(&format!("/api/networks/{}/jobs/{}", net.id, job.id))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(done.status, JobStatus::Done);
    let result: JobResult = serde_json::from_value(done.result.unwrap()).unwrap();
    let found = decode_slots(&result.slots).unwrap();
    assert_eq!(found[0], 0, "the pinned hour is where it was pinned");
    let none: Option<TimetableEdit> = admin.get(&path).await.json().await.unwrap();
    assert_eq!(none, None);

    // Two free hours of the first class trade places; one of them is pinned.
    let problem = Problem::new(&school()).unwrap();
    let (a, b) = (
        problem.class_hours[0][0] as usize,
        problem.class_hours[0][1] as usize,
    );
    let mut edited = found.clone();
    edited.swap(a, b);
    let expected = timetable::cost(&problem, &edited).unwrap();
    let res = admin
        .put(
            &path,
            json!({ "slots": encode_slots(&edited), "pinned": [b] }),
        )
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    let first: TimetableEdit = res.json().await.unwrap();
    assert_eq!(first.slots, encode_slots(&edited));
    assert_eq!(first.pinned, vec![b as u32]);
    assert_eq!(first.cost, expected, "the coordinator's own cost");
    assert_eq!(
        (first.version, first.updated_by.as_deref()),
        (1, Some("admin"))
    );
    let read: Option<TimetableEdit> = admin.get(&path).await.json().await.unwrap();
    assert_eq!(read.as_ref(), Some(&first));

    // A second first edit, and a save from an old version, are told who saved since.
    let res = admin
        .put(&path, json!({ "slots": result.slots, "pinned": [] }))
        .await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let msg = res.text().await.unwrap();
    assert!(
        msg.contains("since the found timetable: it is at version 1, saved by admin"),
        "{msg}"
    );
    let deputy = admin.invite(net.id, "admin", "deputy@school.bg").await;
    let res = deputy
        .put(
            &path,
            json!({ "slots": result.slots, "pinned": [], "version": 1 }),
        )
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    let second: TimetableEdit = res.json().await.unwrap();
    assert_eq!(
        (second.version, second.updated_by.as_deref()),
        (2, Some("Петя Колева"))
    );
    assert_eq!(second.cost, result.cost);
    let res = admin
        .put(
            &path,
            json!({ "slots": encode_slots(&edited), "pinned": [], "version": 1 }),
        )
        .await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let msg = res.text().await.unwrap();
    assert!(
        msg.contains("since version 1: it is at version 2, saved by Петя Колева"),
        "{msg}"
    );
    let read: Option<TimetableEdit> = admin.get(&path).await.json().await.unwrap();
    assert_eq!(
        read.as_ref(),
        Some(&second),
        "nothing of the refused save was kept"
    );

    // Back to the found timetable; a save from the dropped edit is told so.
    assert_eq!(admin.delete(&path).await.status(), StatusCode::NO_CONTENT);
    assert_eq!(admin.delete(&path).await.status(), StatusCode::NOT_FOUND);
    let none: Option<TimetableEdit> = admin.get(&path).await.json().await.unwrap();
    assert_eq!(none, None);
    let res = deputy
        .put(
            &path,
            json!({ "slots": result.slots, "pinned": [], "version": 2 }),
        )
        .await;
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert!(res.text().await.unwrap().contains("its edit was dropped"));
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn only_timetables_of_the_jobs_school_are_kept(db: PgPool) {
    let addr = serve(db).await;
    let admin = Browser::admin(&addr).await;
    let net = admin.create_network("СУ").await;
    let job = admin.job(net.id, tt_job()).await;
    let run = sim_worker::run(sim_worker::Options {
        url: format!("ws://{addr}/ws/worker"),
        exit_when_idle: true,
        join_code: Some(net.join_code.clone()),
        ..Default::default()
    });
    assert_eq!(run.await.unwrap().accepted, 2);
    let path = format!("/api/networks/{}/jobs/{}/timetable", net.id, job.id);
    let done: JobSummary = admin
        .get(&format!("/api/networks/{}/jobs/{}", net.id, job.id))
        .await
        .json()
        .await
        .unwrap();
    let result: JobResult = serde_json::from_value(done.result.unwrap()).unwrap();
    let found = decode_slots(&result.slots).unwrap();
    let problem = Problem::new(&school()).unwrap();
    let (hours, week) = (problem.hours.len(), problem.slots() as u16);
    let free = problem.free[0];

    let mut outside = found.clone();
    outside[3] = week;
    for (body, reason) in [
        (
            json!({ "slots": encode_slots(&found[1..]), "pinned": [] }),
            format!("{} slots for {hours}", hours - 1),
        ),
        (
            json!({ "slots": encode_slots(&outside), "pinned": [] }),
            format!("hour 3: slot {week} is outside the week"),
        ),
        (json!({ "slots": "%%", "pinned": [] }), "not base64".into()),
        (
            json!({ "slots": "AQ==", "pinned": [] }),
            "whole number".into(),
        ),
        (
            json!({ "slots": result.slots, "pinned": [hours] }),
            format!("hour {hours} does not exist"),
        ),
        (
            json!({ "slots": result.slots, "pinned": [free, free] }),
            format!("hour {free} is pinned twice"),
        ),
        (
            json!({ "slots": result.slots, "pinned": [0] }),
            "hour 0 is pinned in the school already".into(),
        ),
    ] {
        let res = admin.put(&path, body.clone()).await;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST, "{body}");
        let msg = res.text().await.unwrap();
        assert!(msg.contains(&reason), "{reason}: {msg}");
    }
    let res = admin
        .put(
            &path,
            json!({ "slots": result.slots, "pinned": [], "cost": 0 }),
        )
        .await;
    assert!(res.status().is_client_error(), "unknown fields are refused");
    let none: Option<TimetableEdit> = admin.get(&path).await.json().await.unwrap();
    assert_eq!(none, None, "nothing refused was kept");

    // A hand moved off its pin is kept, with the hard violation it is.
    let mut moved = found.clone();
    moved[0] = 1;
    let res = admin
        .put(
            &path,
            json!({ "slots": encode_slots(&moved), "pinned": [] }),
        )
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    let kept: TimetableEdit = res.json().await.unwrap();
    assert_eq!(kept.cost.breakdown.pins_moved, 1);
    assert!(kept.cost.hard >= 1);

    // Viewers read but do not change it; another network does not see it.
    let viewer = admin.invite(net.id, "viewer", "teacher@school.bg").await;
    assert_eq!(viewer.get(&path).await.status(), StatusCode::OK);
    let res = viewer
        .put(
            &path,
            json!({ "slots": result.slots, "pinned": [], "version": 1 }),
        )
        .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(viewer.delete(&path).await.status(), StatusCode::FORBIDDEN);
    let other = admin.create_network("Друго училище").await;
    let outsider = admin.invite(other.id, "admin", "other@school.bg").await;
    let elsewhere = format!("/api/networks/{}/jobs/{}/timetable", other.id, job.id);
    assert_eq!(
        outsider.get(&elsewhere).await.status(),
        StatusCode::NOT_FOUND
    );
    let res = outsider
        .put(&elsewhere, json!({ "slots": result.slots, "pinned": [] }))
        .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert!(!outsider.get(&path).await.status().is_success());

    // Only timetable jobs have a timetable.
    let pi = admin.job(net.id, sim_worker::pi_job(1, 1000, 7)).await;
    let res = admin
        .get(&format!(
            "/api/networks/{}/jobs/{}/timetable",
            net.id, pi.id
        ))
        .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    assert!(res.text().await.unwrap().contains("not a timetable search"));
    let res = admin
        .get(&format!(
            "/api/networks/{}/jobs/{}/timetable",
            net.id,
            Uuid::now_v7()
        ))
        .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

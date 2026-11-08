//! Networks that clients own and operate (spec 0010), end to end: accounts and
//! invitations, roles, isolation between networks, lending, the owner's controls as a
//! connected device sees them, and the live streams.

use coordinator::{
    AppState, Config, State, accounts,
    auth::{self, PUBLIC_NETWORK},
};
use futures_util::{SinkExt, StreamExt};
use protocol::{
    CoordinatorMessage, DashboardEvent, DeviceControl, JobSummary, NetworkSummary,
    PROTOCOL_VERSION,
    api::{CreatedApiKey, CreatedInvite, DeviceView, Me, NetworkView},
};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

const ADMIN: (&str, &str) = ("admin@swarm.test", "admin password 1");

/// One result decides a unit and there are no honeypots: these tests are about who
/// gets which unit, not about verification.
fn config() -> Config {
    Config {
        redundancy: 1,
        honeypot_rate: 0.0,
        new_client_honeypot_rate: 0.0,
        no_work_retry: Duration::from_millis(100),
        ..Config::default()
    }
}

async fn serve(db: PgPool, config: Config) -> String {
    accounts::ensure_admin(&db, ADMIN.0, ADMIN.1).await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    // As on a development machine (SWARM_OPEN_PUBLIC=1): the public network takes jobs
    // from anyone, unless a test sets a submit token.
    let state = State::new(
        db,
        Config {
            open_public: true,
            ..config
        },
    );
    coordinator::spawn_reaper(state.clone());
    coordinator::spawn_progress(state.clone());
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

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }

    fn origin(&self) -> String {
        format!("http://{}", self.addr)
    }

    async fn get(&self, path: &str) -> reqwest::Response {
        self.http.get(self.url(path)).send().await.unwrap()
    }

    async fn send(&self, method: reqwest::Method, path: &str, body: Value) -> reqwest::Response {
        self.http
            .request(method, self.url(path))
            .header("origin", self.origin())
            .json(&body)
            .send()
            .await
            .unwrap()
    }

    async fn post(&self, path: &str, body: Value) -> reqwest::Response {
        self.send(reqwest::Method::POST, path, body).await
    }

    async fn patch(&self, path: &str, body: Value) -> reqwest::Response {
        self.send(reqwest::Method::PATCH, path, body).await
    }

    async fn login(&self, email: &str, password: &str) -> reqwest::Response {
        self.post(
            "/api/auth/login",
            json!({ "email": email, "password": password }),
        )
        .await
    }

    /// Signs in as the platform admin.
    async fn admin(addr: &str) -> Self {
        let b = Self::new(addr);
        assert_eq!(b.login(ADMIN.0, ADMIN.1).await.status(), StatusCode::OK);
        b
    }

    async fn create_network(&self, name: &str) -> NetworkView {
        let res = self.post("/api/networks", json!({ "name": name })).await;
        assert_eq!(res.status(), StatusCode::CREATED);
        res.json().await.unwrap()
    }

    /// Invites someone with a new account into `network` with `role`; returns their
    /// signed-in browser.
    async fn invite_member(&self, network: Uuid, role: &str, email: &str) -> Browser {
        let res = self
            .post(
                &format!("/api/networks/{network}/invites"),
                json!({ "role": role }),
            )
            .await;
        assert_eq!(res.status(), StatusCode::CREATED, "{:?}", res.text().await);
        let invite: CreatedInvite = res.json().await.unwrap();
        let them = Browser::new(&self.addr);
        let res = them
            .post(
                &format!("/api/invites/{}", invite.token),
                json!({ "email": email, "name": "Member", "password": "member password" }),
            )
            .await;
        assert_eq!(res.status(), StatusCode::OK);
        them
    }

    async fn pi_job(&self, network: Uuid, units: u32) -> JobSummary {
        let res = self
            .post(
                &format!("/api/networks/{network}/jobs"),
                sim_worker::pi_job(units, 1000, 7),
            )
            .await;
        assert_eq!(res.status(), StatusCode::CREATED, "{:?}", res.text().await);
        res.json().await.unwrap()
    }

    async fn job(&self, network: Uuid, job: Uuid) -> JobSummary {
        self.get(&format!("/api/networks/{network}/jobs/{job}"))
            .await
            .json()
            .await
            .unwrap()
    }
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

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

/// A raw device that joins with `join_code` and returns what the network allows it.
async fn device(addr: &str, join_code: Option<&str>, client: Uuid) -> (Ws, DeviceControl) {
    let (mut ws, _) = connect_async(format!("ws://{addr}/ws/worker"))
        .await
        .unwrap();
    let mut hello = json!({"type":"hello","protocol_version":PROTOCOL_VERSION,"client_version":"test",
        "device":{"backend":"native","vendor":"x","architecture":"x"},"client_id":client});
    if let Some(code) = join_code {
        hello["join_code"] = json!(code);
    }
    ws.send(Message::text(hello.to_string())).await.unwrap();
    assert!(matches!(
        next(&mut ws).await,
        CoordinatorMessage::Welcome { .. }
    ));
    match next(&mut ws).await {
        CoordinatorMessage::Control { control, .. } => (ws, control),
        other => panic!("expected control, got {other:?}"),
    }
}

async fn request_work(ws: &mut Ws) -> CoordinatorMessage {
    ws.send(Message::text(r#"{"type":"request_work"}"#))
        .await
        .unwrap();
    next(ws).await
}

fn sim(addr: &str, join_code: Option<&str>) -> sim_worker::Options {
    sim_worker::Options {
        url: format!("ws://{addr}/ws/worker"),
        exit_when_idle: true,
        join_code: join_code.map(str::to_owned),
        ..Default::default()
    }
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn accounts_exist_only_by_invitation_and_sessions_are_cookies(db: PgPool) {
    let addr = serve(db, config()).await;
    let anon = Browser::new(&addr);
    assert_eq!(anon.get("/api/me").await.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        anon.login(ADMIN.0, "wrong password").await.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        anon.login("nobody@swarm.test", "whatever 123")
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );

    let admin = Browser::admin(&addr).await;
    let me: Me = admin.get("/api/me").await.json().await.unwrap();
    assert!(me.is_admin);

    // A request from another site is refused even with the cookie.
    let forged = admin
        .http
        .post(admin.url("/api/networks"))
        .header("origin", "https://evil.example")
        .json(&json!({ "name": "x" }))
        .send()
        .await
        .unwrap();
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);

    // Only the platform admin invites new accounts.
    let res = admin.post("/api/admin/invites", json!({})).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let invite: CreatedInvite = res.json().await.unwrap();
    let info: Value = anon
        .get(&format!("/api/invites/{}", invite.token))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(info["network_name"], Value::Null);

    let new = Browser::new(&addr);
    let weak = new
        .post(
            &format!("/api/invites/{}", invite.token),
            json!({ "email": "ana@school.bg", "name": "Ана", "password": "short" }),
        )
        .await;
    assert_eq!(weak.status(), StatusCode::BAD_REQUEST);
    let ok = new
        .post(
            &format!("/api/invites/{}", invite.token),
            json!({ "email": "ana@school.bg", "name": "Ана", "password": "a good long password" }),
        )
        .await;
    assert_eq!(ok.status(), StatusCode::OK);
    let me: Me = new.get("/api/me").await.json().await.unwrap();
    assert_eq!((me.name.as_str(), me.is_admin), ("Ана", false));
    // Used once.
    let again = Browser::new(&addr)
        .post(
            &format!("/api/invites/{}", invite.token),
            json!({ "email": "eve@school.bg", "name": "Eve", "password": "another password" }),
        )
        .await;
    assert_eq!(again.status(), StatusCode::NOT_FOUND);
    // Not an admin.
    assert_eq!(
        new.post("/api/admin/invites", json!({})).await.status(),
        StatusCode::FORBIDDEN
    );

    // Signing out ends the session.
    assert_eq!(
        new.post("/api/auth/logout", json!({})).await.status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(new.get("/api/me").await.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        new.login("ANA@school.bg", "a good long password")
            .await
            .status(),
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn roles_decide_who_may_do_what(db: PgPool) {
    let addr = serve(db, config()).await;
    let owner = Browser::admin(&addr).await;
    let lab = owner.create_network("Кабинет 12").await;
    let viewer = owner.invite_member(lab.id, "viewer", "v@school.bg").await;
    let admin = owner.invite_member(lab.id, "admin", "a@school.bg").await;
    let outsider = owner
        .invite_member(owner.create_network("Other").await.id, "owner", "o@x.bg")
        .await;

    let path = format!("/api/networks/{}", lab.id);
    assert_eq!(viewer.get(&path).await.status(), StatusCode::OK);
    assert_eq!(
        viewer
            .patch(&path, json!({ "paused": true }))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    // A network one cannot see does not exist.
    assert_eq!(outsider.get(&path).await.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        outsider
            .patch(&path, json!({ "paused": true }))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    // Admins run the network but cannot hand out more power than they have.
    assert_eq!(
        admin
            .patch(&path, json!({ "max_power": 50 }))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        admin
            .post(&format!("{path}/invites"), json!({ "role": "owner" }))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        admin
            .send(reqwest::Method::DELETE, &path, json!({}))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    // The last owner stays.
    let me: Me = owner.get("/api/me").await.json().await.unwrap();
    assert_eq!(
        owner
            .patch(
                &format!("{path}/members/{}", me.id),
                json!({ "role": "admin" })
            )
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );

    // An API key submits jobs to its own network only.
    let res = admin
        .post(
            &format!("{path}/keys"),
            json!({ "label": "research script" }),
        )
        .await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let key: CreatedApiKey = res.json().await.unwrap();
    assert!(key.key.starts_with("swk_"));
    let http = Client::new();
    let ok = http
        .post(owner.url(&format!("{path}/jobs")))
        .bearer_auth(&key.key)
        .json(&sim_worker::pi_job(2, 1000, 1))
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::CREATED);
    let elsewhere = http
        .post(owner.url("/api/jobs"))
        .bearer_auth(&key.key)
        .json(&sim_worker::pi_job(2, 1000, 1))
        .send()
        .await
        .unwrap();
    assert_eq!(elsewhere.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn a_networks_devices_work_only_for_it_unless_it_lends_them(db: PgPool) {
    let addr = serve(db, config()).await;
    let owner = Browser::admin(&addr).await;
    let (a, b) = (
        owner.create_network("A").await,
        owner.create_network("B").await,
    );
    let job_a = owner.pi_job(a.id, 4).await;
    let job_b = owner.pi_job(b.id, 4).await;
    let public = owner.pi_job(PUBLIC_NETWORK, 4).await;

    // A's worker finishes A's job and nothing else.
    sim_worker::run(sim(&addr, Some(&a.join_code)))
        .await
        .unwrap();
    assert_eq!(owner.job(a.id, job_a.id).await.units_done, 4);
    assert_eq!(owner.job(b.id, job_b.id).await.units_done, 0);
    let p: JobSummary = owner
        .get(&format!(
            "/api/networks/{PUBLIC_NETWORK}/jobs/{}",
            public.id
        ))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(p.units_done, 0);
    // Another network's job is invisible from here.
    assert_eq!(
        owner
            .get(&format!("/api/networks/{}/jobs/{}", a.id, job_b.id))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );

    // Lending: idle A devices help the public network.
    owner
        .patch(
            &format!("/api/networks/{}", a.id),
            json!({ "lend_idle": true }),
        )
        .await;
    sim_worker::run(sim(&addr, Some(&a.join_code)))
        .await
        .unwrap();
    let p: JobSummary = owner
        .get(&format!(
            "/api/networks/{PUBLIC_NETWORK}/jobs/{}",
            public.id
        ))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(p.units_done, 4);
    assert_eq!(owner.job(b.id, job_b.id).await.units_done, 0, "never B");

    // A device with an unknown join code is turned away.
    let (mut ws, _) = connect_async(format!("ws://{addr}/ws/worker"))
        .await
        .unwrap();
    ws.send(Message::text(
        json!({"type":"hello","protocol_version":PROTOCOL_VERSION,"client_version":"t",
            "device":{"backend":"native","vendor":"x","architecture":"x"},"join_code":"nope"})
        .to_string(),
    ))
    .await
    .unwrap();
    assert!(matches!(
        next(&mut ws).await,
        CoordinatorMessage::Error { .. }
    ));
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn known_answers_never_come_from_another_clients_network(db: PgPool) {
    let addr = serve(
        db.clone(),
        Config {
            new_client_honeypot_rate: 1.0,
            ..config()
        },
    )
    .await;
    let owner = Browser::admin(&addr).await;
    let (a, b) = (
        owner.create_network("A").await,
        owner.create_network("B").await,
    );
    // B has a finished, verified job: candidate known answers.
    let job_b = owner.pi_job(b.id, 6).await;
    sim_worker::run(sim(&addr, Some(&b.join_code)))
        .await
        .unwrap();
    assert_eq!(owner.job(b.id, job_b.id).await.units_done, 6);

    // A device of A asking for work is never handed one of B's units.
    for _ in 0..10 {
        let (mut ws, _) = device(&addr, Some(&a.join_code), Uuid::now_v7()).await;
        assert!(matches!(
            request_work(&mut ws).await,
            CoordinatorMessage::NoWork { .. }
        ));
    }
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn the_owners_controls_reach_a_connected_device(db: PgPool) {
    let addr = serve(db, config()).await;
    let owner = Browser::admin(&addr).await;
    let lab = owner.create_network("Lab").await;
    owner.pi_job(lab.id, 50).await;
    let client = Uuid::now_v7();
    let (mut ws, control) = device(&addr, Some(&lab.join_code), client).await;
    assert_eq!(control, DeviceControl::Running);
    assert!(matches!(
        request_work(&mut ws).await,
        CoordinatorMessage::Assign { .. }
    ));

    let devices: Vec<DeviceView> = owner
        .get(&format!("/api/networks/{}/devices", lab.id))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(devices.len(), 1);
    assert!(devices[0].online);
    let dev = format!("/api/networks/{}/devices/{}", lab.id, devices[0].id);

    // Paused: told at once, and no work.
    assert_eq!(
        owner
            .patch(&dev, json!({ "paused": true, "name": "PC 5" }))
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    assert!(matches!(
        next(&mut ws).await,
        CoordinatorMessage::Control {
            control: DeviceControl::PausedByOwner,
            ..
        }
    ));
    assert!(matches!(
        request_work(&mut ws).await,
        CoordinatorMessage::NoWork { .. }
    ));
    owner.patch(&dev, json!({ "paused": false })).await;
    assert!(matches!(
        next(&mut ws).await,
        CoordinatorMessage::Control {
            control: DeviceControl::Running,
            ..
        }
    ));

    // The whole network, and a lower power limit.
    let net = format!("/api/networks/{}", lab.id);
    owner
        .patch(&net, json!({ "paused": true, "max_power": 25 }))
        .await;
    assert!(matches!(
        next(&mut ws).await,
        CoordinatorMessage::Control {
            control: DeviceControl::NetworkPaused,
            max_power: 25
        }
    ));
    owner.patch(&net, json!({ "paused": false })).await;
    assert!(matches!(
        next(&mut ws).await,
        CoordinatorMessage::Control {
            control: DeviceControl::Running,
            ..
        }
    ));

    // Hours that never open: outside hours.
    owner
        .patch(
            &net,
            json!({ "hours": { "timezone": "Europe/Sofia", "windows": [] } }),
        )
        .await;
    assert!(matches!(
        next(&mut ws).await,
        CoordinatorMessage::Control {
            control: DeviceControl::OutsideHours { resumes_at: None },
            ..
        }
    ));
    owner.patch(&net, json!({ "hours": null })).await;
    assert!(matches!(
        next(&mut ws).await,
        CoordinatorMessage::Control {
            control: DeviceControl::Running,
            ..
        }
    ));

    // Approval: a new device waits until admitted.
    owner.patch(&net, json!({ "join_mode": "approval" })).await;
    let (mut newcomer, control) = device(&addr, Some(&lab.join_code), Uuid::now_v7()).await;
    assert_eq!(control, DeviceControl::PendingApproval);
    assert!(matches!(
        request_work(&mut newcomer).await,
        CoordinatorMessage::NoWork { .. }
    ));
    let devices: Vec<DeviceView> = owner
        .get(&format!("/api/networks/{}/devices", lab.id))
        .await
        .json()
        .await
        .unwrap();
    let pending = devices
        .iter()
        .find(|d| d.status == protocol::api::DeviceStatus::Pending)
        .unwrap();
    owner
        .patch(
            &format!("/api/networks/{}/devices/{}", lab.id, pending.id),
            json!({ "approve": true }),
        )
        .await;
    assert!(matches!(
        next(&mut newcomer).await,
        CoordinatorMessage::Control {
            control: DeviceControl::Running,
            ..
        }
    ));

    // Removed: told, disconnected, and kept out.
    assert_eq!(
        owner
            .send(reqwest::Method::DELETE, &dev, json!({}))
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    assert!(matches!(
        next(&mut ws).await,
        CoordinatorMessage::Control {
            control: DeviceControl::Removed,
            ..
        }
    ));
    let (mut again, _) = connect_async(format!("ws://{addr}/ws/worker"))
        .await
        .unwrap();
    again
        .send(Message::text(
            json!({"type":"hello","protocol_version":PROTOCOL_VERSION,"client_version":"t",
                "device":{"backend":"native","vendor":"x","architecture":"x"},
                "client_id": client, "join_code": lab.join_code})
            .to_string(),
        ))
        .await
        .unwrap();
    assert!(matches!(
        next(&mut again).await,
        CoordinatorMessage::Error { .. }
    ));
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn live_streams_carry_only_their_network(db: PgPool) {
    let addr = serve(db, config()).await;
    let owner = Browser::admin(&addr).await;
    let (a, b) = (
        owner.create_network("A").await,
        owner.create_network("B").await,
    );
    let viewer = owner.invite_member(a.id, "viewer", "v@school.bg").await;

    // The members-only stream needs a session.
    let anonymous = connect_async(format!("ws://{addr}/ws/networks/{}/live", a.id)).await;
    assert!(anonymous.is_err(), "no session, no stream");
    let login = Client::new()
        .post(format!("http://{addr}/api/auth/login"))
        .header("origin", format!("http://{addr}"))
        .json(&json!({ "email": "v@school.bg", "password": "member password" }))
        .send()
        .await
        .unwrap();
    let cookie = login.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let request = |network: Uuid| {
        let mut r = tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(
            format!("ws://{addr}/ws/networks/{network}/live"),
        )
        .unwrap();
        r.headers_mut().insert("cookie", cookie.parse().unwrap());
        r
    };
    assert!(
        connect_async(request(b.id)).await.is_err(),
        "not a member of B"
    );
    let (mut live, _) = connect_async(request(a.id)).await.unwrap();
    let snapshot: DashboardEvent = serde_json::from_str(&next_text(&mut live).await).unwrap();
    assert!(matches!(snapshot, DashboardEvent::Snapshot { .. }));

    // A device of B, then one of A: A's stream and summary see only A's.
    let summary_url = format!("ws://{addr}/ws/join/{}/summary", a.join_code);
    let (mut summary, _) = connect_async(&summary_url).await.unwrap();
    let (_in_b, _) = device(&addr, Some(&b.join_code), Uuid::now_v7()).await;
    let (_in_a, _) = device(&addr, Some(&a.join_code), Uuid::now_v7()).await;
    let joined: DashboardEvent = serde_json::from_str(&next_text(&mut live).await).unwrap();
    assert!(
        matches!(joined, DashboardEvent::WorkerJoined { .. }),
        "the first event is A's device, not B's: {joined:?}"
    );
    let mut last = None;
    for _ in 0..4 {
        let s: NetworkSummary = serde_json::from_str(&next_text(&mut summary).await).unwrap();
        let done = s.devices_server == 1;
        last = Some(s);
        if done {
            break;
        }
    }
    let last = last.unwrap();
    assert_eq!((last.name.as_str(), last.devices_server), ("A", 1));
    drop(viewer);

    // The public dashboard stream never shows other networks' devices.
    let (mut public, _) = connect_async(format!("ws://{addr}/ws/dashboard"))
        .await
        .unwrap();
    let snapshot: DashboardEvent = serde_json::from_str(&next_text(&mut public).await).unwrap();
    assert!(matches!(snapshot, DashboardEvent::Snapshot { workers, .. } if workers.is_empty()));
    assert_eq!(
        reqwest::get(format!("http://{addr}/api/join/{}", a.join_code))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        reqwest::get(format!("http://{addr}/api/join/unknown"))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
}

/// Like [`serve`], with the configuration exactly as given (the public network open only
/// if it says so), and the coordinator's state at hand.
async fn serve_as_given(db: PgPool, config: Config) -> (String, AppState) {
    accounts::ensure_admin(&db, ADMIN.0, ADMIN.1).await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let state = State::new(db, config);
    let app = coordinator::router(state.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (addr, state)
}

/// A coordinator that lacks its submit token keeps the public network closed to anyone
/// who does not sign in; its admins still run it. Open only on a development machine
/// that asks for it, and never once a token is set (spec 0016).
#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn the_public_network_stays_closed_without_its_token(db: PgPool) {
    let job =
        json!({ "template": "pi", "params": { "units": 2, "samples_per_unit": 1024, "seed": 1 } });
    let public = format!("/api/networks/{PUBLIC_NETWORK}");

    let (addr, _) = serve_as_given(db.clone(), config()).await;
    let stranger = Browser::new(&addr);
    assert_eq!(
        stranger.post("/api/jobs", job.clone()).await.status(),
        StatusCode::UNAUTHORIZED
    );
    for path in ["devices", "keys", "members"] {
        let res = stranger.get(&format!("{public}/{path}")).await;
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "{path}");
    }
    let cheaters = json!({ "count": 1, "collude": false });
    assert_eq!(
        stranger
            .post(&format!("{public}/chaos/cheaters"), cheaters)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let admin = Browser::admin(&addr).await;
    assert_eq!(
        admin.post("/api/jobs", job.clone()).await.status(),
        StatusCode::CREATED
    );

    let open = Config {
        open_public: true,
        ..config()
    };
    let (dev, _) = serve_as_given(db.clone(), open).await;
    assert_eq!(
        Browser::new(&dev)
            .post("/api/jobs", job.clone())
            .await
            .status(),
        StatusCode::CREATED
    );

    let with_token = Config {
        open_public: true,
        submit_token: Some("organiser password".into()),
        ..config()
    };
    let (prod, _) = serve_as_given(db, with_token).await;
    assert_eq!(
        Browser::new(&prod).post("/api/jobs", job).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

/// Sign-ins check passwords a few at a time. While every check is taken, a sign-in
/// waits, then is told to try again, and that never counts as a wrong password against
/// the account (spec 0016).
#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn sign_ins_wait_for_a_free_password_check(db: PgPool) {
    let config = Config {
        sign_in_wait: Duration::from_millis(200),
        ..config()
    };
    let (addr, state) = serve_as_given(db, config).await;
    let browser = Browser::new(&addr);

    let every_check = state
        .sign_ins
        .acquire_many(auth::SIGN_IN_CHECKS as u32)
        .await
        .unwrap();
    // More than the eight wrong passwords that lock an email for a while.
    for _ in 0..10 {
        assert_eq!(
            browser.login(ADMIN.0, ADMIN.1).await.status(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }
    drop(every_check);
    assert_eq!(
        browser.login(ADMIN.0, ADMIN.1).await.status(),
        StatusCode::OK
    );
}

/// Every coordinator response, errors included, says it is no page: not to be framed,
/// sniffed into a script, or followed with a Referer (spec 0016).
#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn responses_carry_security_headers(db: PgPool) {
    let (addr, _) = serve_as_given(db, config()).await;
    let browser = Browser::new(&addr);
    for path in [
        "/healthz",
        "/api/me",
        "/api/jobs/not-a-job",
        "/api/no-such-route",
    ] {
        let res = browser.get(path).await;
        let headers = res.headers();
        assert_eq!(
            headers["content-security-policy"], "default-src 'none'; frame-ancestors 'none'",
            "{path}"
        );
        assert_eq!(headers["x-content-type-options"], "nosniff", "{path}");
        assert_eq!(headers["x-frame-options"], "DENY", "{path}");
        assert_eq!(headers["referrer-policy"], "no-referrer", "{path}");
    }
}

/// A sign-in given up halfway (the client hung up and its request was dropped) keeps its
/// password check's slot until the check is done, so clients that hang up at once
/// cannot run more checks than the limit (spec 0016).
#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn an_abandoned_sign_in_keeps_its_slot_until_its_check_ends(db: PgPool) {
    let state = State::new(db, config());
    let sign_ins: Vec<_> = (0..auth::SIGN_IN_CHECKS)
        .map(|_| {
            let state = state.clone();
            tokio::spawn(async move { auth::check_sign_in(&state, "a guess".into(), None).await })
        })
        .collect();
    while state.sign_ins.available_permits() > 0 {
        tokio::task::yield_now().await;
    }
    for sign_in in &sign_ins {
        sign_in.abort();
    }
    for sign_in in sign_ins {
        assert!(sign_in.await.unwrap_err().is_cancelled());
    }
    assert_eq!(
        state.sign_ins.available_permits(),
        0,
        "the checks still run"
    );

    let all_free = async {
        while state.sign_ins.available_permits() < auth::SIGN_IN_CHECKS {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(30), all_free)
        .await
        .expect("every slot is free once the checks end");
}

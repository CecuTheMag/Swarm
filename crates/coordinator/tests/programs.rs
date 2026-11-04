//! Programs (spec 0012), end to end: upload and checks, roles, running on a network's
//! devices at every quorum, cheaters, review for the public network and contributors'
//! consent.

use base64::{Engine, engine::general_purpose::STANDARD};
use coordinator::{Config, State, accounts, auth::PUBLIC_NETWORK};
use futures_util::{SinkExt, StreamExt};
use programs::fixtures::{endless, fnv, fnv_of};
use protocol::{
    CoordinatorMessage, JobOutputs, JobStatus, JobSummary, PROTOCOL_VERSION, ProgramSummary,
    Review,
    api::{CreatedInvite, NetworkView},
    templates::program,
};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

const ADMIN: (&str, &str) = ("admin@swarm.test", "admin password 1");

/// Verification on, honeypots off, nobody trusted enough to compute alone.
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

    async fn get(&self, path: &str) -> reqwest::Response {
        self.http.get(self.url(path)).send().await.unwrap()
    }

    async fn post(&self, path: &str, body: Value) -> reqwest::Response {
        self.http
            .post(self.url(path))
            .header("origin", format!("http://{}", self.addr))
            .json(&body)
            .send()
            .await
            .unwrap()
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

    /// Invites someone with a new account, into `network` with `role` or into none.
    async fn invite(&self, network: Option<Uuid>, role: &str, email: &str) -> Browser {
        let res = match network {
            Some(n) => {
                self.post(
                    &format!("/api/networks/{n}/invites"),
                    json!({ "role": role }),
                )
                .await
            }
            None => self.post("/api/admin/invites", json!({})).await,
        };
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

    async fn upload(&self, network: Uuid, name: &str, module: &[u8]) -> reqwest::Response {
        self.post(
            &format!("/api/networks/{network}/programs"),
            json!({ "name": name, "description": "test", "module": STANDARD.encode(module) }),
        )
        .await
    }

    async fn program(&self, network: Uuid, module: &[u8]) -> ProgramSummary {
        let res = self.upload(network, "FNV", module).await;
        assert_eq!(res.status(), StatusCode::CREATED);
        res.json().await.unwrap()
    }

    async fn submit(&self, network: Uuid, body: Value) -> reqwest::Response {
        self.post(&format!("/api/networks/{network}/jobs"), body)
            .await
    }

    async fn job(&self, network: Uuid, job: Uuid) -> JobSummary {
        self.get(&format!("/api/networks/{network}/jobs/{job}"))
            .await
            .json()
            .await
            .unwrap()
    }

    async fn outputs(&self, network: Uuid, job: Uuid) -> Vec<Vec<u8>> {
        let res = self
            .get(&format!("/api/networks/{network}/jobs/{job}/outputs"))
            .await;
        assert_eq!(res.status(), StatusCode::OK);
        let outputs: JobOutputs = res.json().await.unwrap();
        outputs
            .outputs
            .into_iter()
            .map(|o| STANDARD.decode(o.expect("every unit decided")).unwrap())
            .collect()
    }
}

fn program_job(program: Uuid, params: Value) -> Value {
    let mut params = params;
    params["program"] = json!(program);
    json!({ "template": program::ID, "params": params })
}

fn sim(addr: &str, join_code: Option<&str>) -> sim_worker::Options {
    sim_worker::Options {
        url: format!("ws://{addr}/ws/worker"),
        exit_when_idle: true,
        join_code: join_code.map(Into::into),
        ..Default::default()
    }
}

/// Runs rounds of fresh workers (each a new client) until the job is decided.
async fn finish(
    b: &Browser,
    network: Uuid,
    job: Uuid,
    join_code: Option<&str>,
    workers: usize,
) -> JobSummary {
    for _ in 0..40 {
        let running: Vec<_> = (0..workers)
            .map(|_| tokio::spawn(sim_worker::run(sim(&b.addr, join_code))))
            .collect();
        for w in running {
            w.await.unwrap().unwrap();
        }
        let j = b.job(network, job).await;
        if !matches!(j.status, JobStatus::Queued | JobStatus::Running) {
            return j;
        }
    }
    panic!("job {job} did not finish");
}

fn counted(i: u32, args: &[u8]) -> Vec<u8> {
    let mut input = i.to_le_bytes().to_vec();
    input.extend_from_slice(args);
    input
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn uploads_are_checked_kept_once_and_need_an_admin(db: PgPool) {
    let addr = serve(db, config()).await;
    let admin = Browser::admin(&addr).await;
    let lab = admin.create_network("Лаборатория").await;
    let viewer = admin
        .invite(Some(lab.id), "viewer", "viewer@swarm.test")
        .await;
    let outsider = admin.invite(None, "", "outsider@swarm.test").await;

    // Only an admin of the network uploads.
    assert_eq!(
        viewer.upload(lab.id, "x", &fnv()).await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        outsider.upload(lab.id, "x", &fnv()).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        Browser::new(&addr)
            .upload(lab.id, "x", &fnv())
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );

    // A module that may grow without bound is refused, with what to change.
    let res = admin
        .upload(lab.id, "x", &programs::fixtures::unbounded())
        .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let error: Value = res.json().await.unwrap();
    assert!(error["error"].as_str().unwrap().contains("--max-memory"));
    for (name, module) in [("", fnv()), ("x", b"not wasm".to_vec())] {
        let status = admin.upload(lab.id, name, &module).await.status();
        assert_eq!(status, StatusCode::BAD_REQUEST, "{name:?}");
    }
    let res = admin
        .post(
            &format!("/api/networks/{}/programs", lab.id),
            json!({ "name": "x", "module": STANDARD.encode(fnv()), "source_url": "http://x" }),
        )
        .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let first = admin.program(lab.id, &fnv()).await;
    assert_eq!(first.review, Review::Private);
    assert_eq!(first.network_id, lab.id);
    assert_eq!(first.network_name, "Лаборатория");
    assert_eq!(first.sha256, hex::encode(Sha256::digest(fnv())));
    assert_eq!(first.size as usize, fnv().len());
    // The same module again is the same program; another network gets its own.
    let again = admin.upload(lab.id, "FNV again", &fnv()).await;
    assert_eq!(again.status(), StatusCode::OK);
    assert_eq!(again.json::<ProgramSummary>().await.unwrap(), first);
    let other = admin.create_network("Друга").await;
    assert_ne!(admin.program(other.id, &fnv()).await.id, first.id);

    // Viewers see the programs; outsiders do not learn the network exists.
    let seen: Vec<ProgramSummary> = viewer
        .get(&format!("/api/networks/{}/programs", lab.id))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(seen, std::slice::from_ref(&first));
    let res = outsider
        .get(&format!("/api/networks/{}/programs", lab.id))
        .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // Devices download the module by its hash.
    let res = outsider
        .get(&format!("/api/modules/{}.wasm", first.sha256))
        .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["content-type"], "application/wasm");
    let cache = res.headers()["cache-control"].to_str().unwrap().to_owned();
    assert!(cache.contains("immutable"));
    assert_eq!(res.bytes().await.unwrap().as_ref(), fnv());
    for missing in [
        format!("{}.wasm", "0".repeat(64)),
        first.sha256.clone(),
        format!("{}.wasm", first.sha256.to_uppercase()),
        "..%2F..%2Fetc%2Fpasswd".into(),
    ] {
        let res = outsider.get(&format!("/api/modules/{missing}")).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{missing}");
    }

    // A network cannot run another network's program, and viewers start no jobs.
    let res = admin
        .submit(other.id, program_job(first.id, json!({ "count": 1 })))
        .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let res = viewer
        .submit(lab.id, program_job(first.id, json!({ "count": 1 })))
        .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn a_program_runs_on_its_networks_devices_at_every_quorum(db: PgPool) {
    let addr = serve(db.clone(), config()).await;
    let admin = Browser::admin(&addr).await;
    let class = admin.create_network("10А клас").await;
    let p = admin.program(class.id, &fnv()).await;
    let args = b"shared arguments";

    // Quorum is between 1 and 3, and not chosen for the public network.
    for bad in [0, 4] {
        let mut body = program_job(p.id, json!({ "count": 1 }));
        body["quorum"] = json!(bad);
        let status = admin.submit(class.id, body).await.status();
        assert_eq!(status, StatusCode::BAD_REQUEST, "quorum {bad}");
    }
    let mut pi = sim_worker::pi_job(1, 100, 1);
    pi["quorum"] = json!(1);
    let status = admin.submit(PUBLIC_NETWORK, pi).await.status();
    assert_eq!(status, StatusCode::BAD_REQUEST);

    for quorum in 1..=3u32 {
        let mut body = program_job(p.id, json!({ "count": 12, "args": STANDARD.encode(args) }));
        body["quorum"] = json!(quorum);
        let res = admin.submit(class.id, body).await;
        assert_eq!(res.status(), StatusCode::CREATED);
        let job: JobSummary = res.json().await.unwrap();
        assert_eq!(job.quorum, quorum);
        assert_eq!(job.network_id, class.id);
        // The job shows what it runs, not its inputs.
        assert_eq!(
            job.params,
            json!({ "program": p.id, "name": "FNV", "units": 12 })
        );

        // A device of the public network never gets its units.
        let outsider = sim_worker::run(sim(&addr, None)).await.unwrap();
        assert_eq!(outsider.completed, 0);

        let done = finish(&admin, class.id, job.id, Some(&class.join_code), 3).await;
        assert_eq!(done.status, JobStatus::Done, "quorum {quorum}");
        assert_eq!(done.result, Some(json!({ "outputs": 12, "bytes": 96 })));
        let expected: Vec<Vec<u8>> = (0..12).map(|i| fnv_of(&counted(i, args))).collect();
        assert_eq!(admin.outputs(class.id, job.id).await, expected);
        // Each unit was decided by `quorum` agreeing results (one alone is `trusted`).
        let decided: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM leases l JOIN units u ON u.id = l.unit_id
             WHERE u.job_id = $1 AND l.verdict IN ('agreed', 'trusted')",
        )
        .bind(job.id)
        .fetch_one(&db)
        .await
        .unwrap();
        assert!(
            decided >= i64::from(quorum) * 12,
            "quorum {quorum}: {decided}"
        );
    }
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn outputs_come_back_in_order_for_the_networks_members_only(db: PgPool) {
    let addr = serve(db, config()).await;
    let admin = Browser::admin(&addr).await;
    let lab = admin.create_network("Входове").await;
    let viewer = admin
        .invite(Some(lab.id), "viewer", "viewer@swarm.test")
        .await;
    let outsider = admin.invite(None, "", "outsider@swarm.test").await;
    let p = admin.program(lab.id, &fnv()).await;
    let inputs: Vec<Vec<u8>> = vec![vec![], b"a".to_vec(), vec![7; 5000], b"swarm".to_vec()];
    let mut body = program_job(
        p.id,
        json!({ "inputs": inputs.iter().map(|i| STANDARD.encode(i)).collect::<Vec<_>>() }),
    );
    body["quorum"] = json!(1);
    let job: JobSummary = admin.submit(lab.id, body).await.json().await.unwrap();

    let path = format!("/api/networks/{}/jobs/{}/outputs", lab.id, job.id);
    let pending: JobOutputs = admin.get(&path).await.json().await.unwrap();
    assert_eq!(pending.outputs, vec![None; 4]);

    finish(&admin, lab.id, job.id, Some(&lab.join_code), 1).await;
    let expected: Vec<Vec<u8>> = inputs.iter().map(|i| fnv_of(i)).collect();
    assert_eq!(admin.outputs(lab.id, job.id).await, expected);
    assert_eq!(viewer.outputs(lab.id, job.id).await, expected);
    assert_eq!(outsider.get(&path).await.status(), StatusCode::NOT_FOUND);

    // Other templates have no outputs to download.
    let pi: JobSummary = admin
        .submit(lab.id, sim_worker::pi_job(1, 100, 1))
        .await
        .json()
        .await
        .unwrap();
    let res = admin
        .get(&format!("/api/networks/{}/jobs/{}/outputs", lab.id, pi.id))
        .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn a_cheating_member_is_caught_and_its_results_never_count(db: PgPool) {
    let addr = serve(db.clone(), config()).await;
    let admin = Browser::admin(&addr).await;
    let class = admin.create_network("Със съученик, който мами").await;
    let p = admin.program(class.id, &fnv()).await;
    let mut body = program_job(p.id, json!({ "count": 40 }));
    body["quorum"] = json!(2);
    let job: JobSummary = admin.submit(class.id, body).await.json().await.unwrap();

    let cheater = Uuid::now_v7();
    let cheating = tokio::spawn(sim_worker::run(sim_worker::Options {
        client_id: Some(cheater),
        cheat: sim_worker::Cheat::Random { rate: 1.0 },
        ..sim(&addr, Some(&class.join_code))
    }));
    let done = finish(&admin, class.id, job.id, Some(&class.join_code), 3).await;
    let _ = cheating.await.unwrap();

    assert_eq!(done.status, JobStatus::Done);
    let expected: Vec<Vec<u8>> = (0..40).map(|i| fnv_of(&counted(i, b""))).collect();
    assert_eq!(admin.outputs(class.id, job.id).await, expected);
    let banned: bool =
        sqlx::query_scalar("SELECT banned_at IS NOT NULL FROM clients WHERE id = $1")
            .bind(cheater)
            .fetch_one(&db)
            .await
            .unwrap();
    assert!(banned, "the cheating member was not banned");
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn workers_run_exactly_the_module_the_job_names(db: PgPool) {
    let addr = serve(db, config()).await;
    let admin = Browser::admin(&addr).await;
    let lab = admin.create_network("Хеш").await;
    let p = admin.program(lab.id, &fnv()).await;
    let mut body = program_job(p.id, json!({ "count": 1 }));
    body["quorum"] = json!(1);
    admin.submit(lab.id, body).await;

    let (mut ws, _) = connect_async(format!("ws://{addr}/ws/worker"))
        .await
        .unwrap();
    let hello = json!({
        "type": "hello",
        "protocol_version": PROTOCOL_VERSION,
        "client_version": "test",
        "device": {"backend": "native", "vendor": "test", "architecture": "test"},
        "join_code": lab.join_code,
    });
    ws.send(Message::text(hello.to_string())).await.unwrap();
    ws.send(Message::text(r#"{"type":"request_work"}"#))
        .await
        .unwrap();
    let (template, params) = loop {
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        if let Ok(CoordinatorMessage::Assign {
            template, params, ..
        }) = serde_json::from_str(msg.to_text().unwrap())
        {
            break (template, params);
        }
    };
    assert_eq!(template.id, program::ID);
    assert_eq!(template.version, program::VERSION);
    assert_eq!(template.sha256, p.sha256);
    assert_eq!(
        params,
        json!({ "input": STANDARD.encode(0u32.to_le_bytes()) })
    );
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn a_program_that_never_finishes_fails_its_job(db: PgPool) {
    let config = Config {
        max_releases: 3,
        ..config()
    };
    let addr = serve(db, config).await;
    let admin = Browser::admin(&addr).await;
    let lab = admin.create_network("Безкраен цикъл").await;
    let p = admin.program(lab.id, &endless()).await;
    let mut body = program_job(p.id, json!({ "count": 1 }));
    body["quorum"] = json!(1);
    let job: JobSummary = admin.submit(lab.id, body).await.json().await.unwrap();

    for _ in 0..3 {
        let stats = sim_worker::run(sim_worker::Options {
            fuel: 1_000_000,
            max_units: Some(1),
            ..sim(&addr, Some(&lab.join_code))
        })
        .await
        .unwrap();
        assert_eq!(stats.accepted, 0);
    }
    let failed = admin.job(lab.id, job.id).await;
    assert_eq!(failed.status, JobStatus::Failed);
    assert!(failed.error.unwrap().contains("given out 3 times"));
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn reviewed_programs_reach_the_public_network_with_consent(db: PgPool) {
    let addr = serve(db, config()).await;
    let admin = Browser::admin(&addr).await;
    let science = admin.create_network("Отворена наука").await;
    // Its owner is an ordinary account, not the platform admin.
    let owner = admin
        .invite(Some(science.id), "admin", "owner@swarm.test")
        .await;
    let p = owner.program(science.id, &fnv()).await;
    let public = |count: u32| {
        let mut body = program_job(p.id, json!({ "count": count }));
        body["public"] = json!(true);
        body
    };

    // Not before review.
    let status = owner.submit(science.id, public(4)).await.status();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let review = format!("/api/networks/{}/programs/{}/review", science.id, p.id);
    let pending: ProgramSummary = owner.post(&review, json!({})).await.json().await.unwrap();
    assert_eq!(pending.review, Review::Pending);

    // Only the platform admin decides.
    let res = owner.get("/api/admin/programs").await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let queue: Vec<ProgramSummary> = admin.get("/api/admin/programs").await.json().await.unwrap();
    assert_eq!(queue, [pending]);
    let decision = format!("/api/admin/programs/{}/decision", p.id);
    let res = owner.post(&decision, json!({ "approve": true })).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let rejected: ProgramSummary = admin
        .post(
            &decision,
            json!({ "approve": false, "note": "Липсва описание." }),
        )
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(rejected.review, Review::Rejected);
    assert_eq!(rejected.review_note.as_deref(), Some("Липсва описание."));
    // A decision needs a pending review.
    let res = admin.post(&decision, json!({ "approve": true })).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    owner.post(&review, json!({})).await;
    let approved: ProgramSummary = admin
        .post(&decision, json!({ "approve": true }))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(approved.review, Review::Approved);
    assert_eq!(approved.review_note, None);
    let community: Vec<ProgramSummary> = Browser::new(&addr)
        .get("/api/programs/approved")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(community, [approved]);

    // Now it runs on the public network, at the coordinator's quorum, and stays the
    // network's.
    let mut with_quorum = public(4);
    with_quorum["quorum"] = json!(1);
    let status = owner.submit(science.id, with_quorum).await.status();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let res = owner.submit(science.id, public(4)).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let job: JobSummary = res.json().await.unwrap();
    assert_eq!(job.network_id, PUBLIC_NETWORK);
    assert_eq!(job.owner_network_id, Some(science.id));
    assert_eq!(job.quorum, config().redundancy);
    let listed: Vec<JobSummary> = owner
        .get(&format!("/api/networks/{}/jobs", science.id))
        .await
        .json()
        .await
        .unwrap();
    assert!(listed.iter().any(|j| j.id == job.id));

    // A device that did not allow community programs leaves it alone.
    let refusing = sim_worker::run(sim_worker::Options {
        community: false,
        ..sim(&addr, None)
    })
    .await
    .unwrap();
    assert_eq!(refusing.completed, 0);
    let done = finish(&owner, science.id, job.id, None, 3).await;
    assert_eq!(done.status, JobStatus::Done);
    let expected: Vec<Vec<u8>> = (0..4).map(|i| fnv_of(&counted(i, b""))).collect();
    assert_eq!(owner.outputs(science.id, job.id).await, expected);

    // At most two jobs of a network on the public network at a time.
    for status in [
        StatusCode::CREATED,
        StatusCode::CREATED,
        StatusCode::CONFLICT,
    ] {
        assert_eq!(owner.submit(science.id, public(1)).await.status(), status);
    }
    // Only programs reach the public network from a network.
    let mut pi = sim_worker::pi_job(1, 100, 1);
    pi["public"] = json!(true);
    let status = owner.submit(science.id, pi).await.status();
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn a_lent_device_runs_community_programs_only_with_consent(db: PgPool) {
    let addr = serve(db, config()).await;
    let admin = Browser::admin(&addr).await;
    let science = admin.create_network("Наука").await;
    let p = admin.program(science.id, &fnv()).await;
    admin
        .post(
            &format!("/api/networks/{}/programs/{}/review", science.id, p.id),
            json!({}),
        )
        .await;
    admin
        .post(
            &format!("/api/admin/programs/{}/decision", p.id),
            json!({ "approve": true }),
        )
        .await;
    let mut body = program_job(p.id, json!({ "count": 2 }));
    body["public"] = json!(true);
    let job: JobSummary = admin.submit(science.id, body).await.json().await.unwrap();

    // A school that lends its idle devices to the public network.
    let school = admin.create_network("Училище").await;
    let res = admin
        .http
        .patch(admin.url(&format!("/api/networks/{}", school.id)))
        .header("origin", format!("http://{addr}"))
        .json(&json!({ "lend_idle": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let refusing = sim_worker::run(sim_worker::Options {
        community: false,
        ..sim(&addr, Some(&school.join_code))
    })
    .await
    .unwrap();
    assert_eq!(refusing.completed, 0);
    let done = finish(&admin, science.id, job.id, Some(&school.join_code), 3).await;
    assert_eq!(done.status, JobStatus::Done);
}

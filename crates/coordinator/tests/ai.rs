//! Language models on the swarm (spec 0017), end to end: a real server, simulated devices.
//! Simulated devices run no model; they answer with `sim_worker::stand_in_answer`, a pure
//! function of the unit, as devices decoding greedily with the same model agree.

use coordinator::{Config, State};
use protocol::{
    JobStatus, JobSummary,
    templates::ai::{self, UnitParams},
};
use serde_json::{Value, json};
use sqlx::PgPool;

const SMOL: &str = "HuggingFaceTB/SmolLM2-360M-Instruct";
const TINY: &str = "hf-internal-testing/tiny-random-LlamaForCausalLM";
/// A GPU that holds SmolLM2 (its catalog entry asks for 256 MiB).
const ENOUGH: u64 = 1 << 30;

async fn serve(db: PgPool, config: Config) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
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

fn unverified() -> Config {
    Config {
        redundancy: 1,
        honeypot_rate: 0.0,
        new_client_honeypot_rate: 0.0,
        ..Config::default()
    }
}

fn sim(addr: &str, gpu_buffer: Option<u64>) -> sim_worker::Options {
    sim_worker::Options {
        url: format!("ws://{addr}/ws/worker"),
        exit_when_idle: true,
        gpu_buffer,
        ..Default::default()
    }
}

fn job_params(model: &str, prompts: &[String]) -> Value {
    json!({ "model": model, "prompts": prompts, "system": "Answer in one sentence.",
            "max_new_tokens": 16 })
}

async fn submit(addr: &str, params: &Value) -> reqwest::Response {
    reqwest::Client::new()
        .post(format!("http://{addr}/api/jobs"))
        .json(&json!({ "template": "ai", "params": params }))
        .send()
        .await
        .unwrap()
}

async fn create(addr: &str, params: &Value) -> JobSummary {
    let res = submit(addr, params).await;
    assert_eq!(res.status(), 201, "{}", res.text().await.unwrap());
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

async fn client_banned(db: &PgPool, client_id: uuid::Uuid) -> bool {
    sqlx::query_scalar("SELECT banned_at IS NOT NULL FROM clients WHERE id = $1")
        .bind(client_id)
        .fetch_one(db)
        .await
        .unwrap()
}

fn prompts(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| format!("Summarise essay number {i}."))
        .collect()
}

/// What an honest device answers for each prompt, in order.
fn expected(params: &Value) -> Vec<ai::UnitOutput> {
    coordinator::templates::get("ai")
        .unwrap()
        .template
        .split(params)
        .unwrap()
        .into_iter()
        .map(|u| sim_worker::stand_in_answer(&serde_json::from_value::<UnitParams>(u).unwrap()))
        .collect()
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn answers_come_back_in_prompt_order_and_a_liar_is_outvoted_and_banned(db: PgPool) {
    let addr = serve(db.clone(), Config::default()).await;
    let params = job_params(SMOL, &prompts(12));
    let job = create(&addr, &params).await;

    let liar = uuid::Uuid::now_v7();
    let lying = tokio::spawn(sim_worker::run(sim_worker::Options {
        client_id: Some(liar),
        cheat: sim_worker::Cheat::Random { rate: 1.0 },
        ..sim(&addr, Some(ENOUGH))
    }));
    let mut job = get_job(&addr, job.id).await;
    for _ in 0..40 {
        let honest: Vec<_> = (0..3)
            .map(|_| tokio::spawn(sim_worker::run(sim(&addr, Some(ENOUGH)))))
            .collect();
        for h in honest {
            h.await.unwrap().unwrap();
        }
        job = get_job(&addr, job.id).await;
        if job.status == JobStatus::Done {
            break;
        }
    }
    let _ = lying.await.unwrap();

    assert_eq!(job.status, JobStatus::Done);
    let result: ai::JobResult = serde_json::from_value(job.result.unwrap()).unwrap();
    assert_eq!(result.model, SMOL);
    let want = expected(&params);
    assert_eq!(result.answers.len(), want.len());
    for (got, want) in result.answers.iter().zip(&want) {
        assert_eq!(got.text, want.text, "a made-up answer got through");
        assert_eq!(got.tokens as usize, want.tokens.len());
    }
    assert!(client_banned(&db, liar).await, "the liar was not banned");
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn only_devices_that_can_hold_the_model_get_its_units(db: PgPool) {
    let addr = serve(db.clone(), unverified()).await;
    let job = create(&addr, &job_params(SMOL, &prompts(3))).await;
    let pi = reqwest::Client::new()
        .post(format!("http://{addr}/api/jobs"))
        .json(&sim_worker::pi_job(2, 1000, 5))
        .send()
        .await
        .unwrap();
    assert_eq!(pi.status(), 201);
    let pi: JobSummary = pi.json().await.unwrap();

    // No GPU: the π job, queued after the model's, is all it gets.
    let stats = sim_worker::run(sim(&addr, None)).await.unwrap();
    assert_eq!(stats.completed, 2);
    assert_eq!(get_job(&addr, pi.id).await.status, JobStatus::Done);
    // A GPU too small for the model gets nothing.
    let stats = sim_worker::run(sim(&addr, Some(128 << 20))).await.unwrap();
    assert_eq!(stats.completed, 0);
    let waiting = get_job(&addr, job.id).await;
    assert_eq!(waiting.status, JobStatus::Queued);
    assert_eq!(waiting.units_done, 0);
    // One that can hold it answers every prompt.
    let stats = sim_worker::run(sim(&addr, Some(ENOUGH))).await.unwrap();
    assert_eq!(stats.completed, 3);
    assert_eq!(get_job(&addr, job.id).await.status, JobStatus::Done);
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn a_model_job_needs_a_catalog_model_and_test_models_their_setting(db: PgPool) {
    let addr = serve(db.clone(), unverified()).await;
    let unknown = submit(&addr, &job_params("someone/else", &prompts(1))).await;
    assert_eq!(unknown.status(), 400);
    let test_model = submit(&addr, &job_params(TINY, &prompts(1))).await;
    assert_eq!(test_model.status(), 400);
    assert!(
        test_model
            .text()
            .await
            .unwrap()
            .contains("SWARM_TEST_MODELS")
    );

    let addr = serve(
        db.clone(),
        Config {
            test_models: true,
            ..unverified()
        },
    )
    .await;
    // The test model needs no GPU: any device runs it.
    let job = create(&addr, &job_params(TINY, &prompts(2))).await;
    let stats = sim_worker::run(sim(&addr, None)).await.unwrap();
    assert_eq!(stats.completed, 2);
    assert_eq!(get_job(&addr, job.id).await.status, JobStatus::Done);
}

#[sqlx::test(migrator = "coordinator::MIGRATOR")]
async fn model_files_are_served_only_from_the_catalog(db: PgPool) {
    let dir = std::env::temp_dir().join(format!("swarm-models-{}", uuid::Uuid::now_v7()));
    let smol = dir.join(SMOL);
    let tiny = dir.join(TINY);
    std::fs::create_dir_all(smol.join("onnx")).unwrap();
    std::fs::create_dir_all(&tiny).unwrap();
    std::fs::write(smol.join("config.json"), b"{\"model_type\":\"llama\"}").unwrap();
    std::fs::write(smol.join("secret.txt"), b"not in the catalog").unwrap();
    std::fs::write(tiny.join("config.json"), b"{}").unwrap();
    let get = async |addr: &str, path: &str| {
        reqwest::get(format!("http://{addr}/api/models/{path}"))
            .await
            .unwrap()
    };

    let addr = serve(
        db.clone(),
        Config {
            models_dir: dir.clone(),
            ..unverified()
        },
    )
    .await;
    let res = get(&addr, &format!("{SMOL}/config.json")).await;
    assert_eq!(res.status(), 200);
    assert_eq!(res.headers()["content-type"], "application/json");
    assert_eq!(res.headers()["content-length"], "22");
    assert_eq!(
        res.bytes().await.unwrap().as_ref(),
        b"{\"model_type\":\"llama\"}"
    );
    for path in [
        format!("{SMOL}/secret.txt"),
        // In the catalog, but not downloaded.
        format!("{SMOL}/onnx/model_q4f16.onnx"),
        format!("{SMOL}/../{TINY}/config.json"),
        format!("{TINY}/config.json"),
    ] {
        assert_eq!(get(&addr, &path).await.status(), 404, "{path}");
    }

    let addr = serve(
        db.clone(),
        Config {
            models_dir: dir.clone(),
            test_models: true,
            ..unverified()
        },
    )
    .await;
    assert_eq!(
        get(&addr, &format!("{TINY}/config.json")).await.status(),
        200
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

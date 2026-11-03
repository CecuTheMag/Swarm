//! Рояк coordinator: accounts and networks, job API, work-unit scheduling and result
//! aggregation.

pub mod accounts;
pub mod api;
pub mod auth;
pub mod chaos;
pub mod db;
pub mod hours;
pub mod networks;
pub mod render;
pub mod state;
pub mod templates;
pub mod ws;

use axum::{
    Router,
    routing::{get, patch, post},
};
use sqlx::{
    PgPool, Postgres,
    migrate::{MigrateDatabase, Migrator},
    postgres::{PgConnectOptions, PgPoolOptions},
};
pub use state::{AppState, Config, State};
use std::str::FromStr;

pub static MIGRATOR: Migrator = sqlx::migrate!();

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(api::healthz))
        .route("/api/jobs", post(api::create_job).get(api::list_jobs))
        .route("/api/jobs/{id}", get(api::get_job))
        .route("/api/jobs/{id}/cancel", post(api::cancel_job))
        .route("/api/jobs/{id}/image.png", get(api::job_image))
        .route("/api/chaos/workers/{id}/kill", post(chaos::kill_worker))
        .route("/api/chaos/workers/{id}/cut", post(chaos::cut_off_worker))
        .route("/api/chaos/cheaters", post(chaos::add_cheaters))
        // Accounts (spec 0010).
        .route("/api/auth/login", post(accounts::login))
        .route("/api/auth/logout", post(accounts::logout))
        .route("/api/me", get(accounts::current))
        .route(
            "/api/invites/{token}",
            get(accounts::invite_info).post(accounts::accept_invite),
        )
        .route("/api/admin/invites", post(accounts::admin_invite))
        .route("/api/admin/accounts", get(accounts::admin_accounts))
        // Networks.
        .route("/api/join/{code}", get(networks::join_info))
        .route("/api/networks", get(networks::list).post(networks::create))
        .route(
            "/api/networks/{id}",
            get(networks::get)
                .patch(networks::update)
                .delete(networks::delete),
        )
        .route("/api/networks/{id}/devices", get(networks::devices))
        .route(
            "/api/networks/{id}/devices/{device}",
            patch(networks::update_device).delete(networks::remove_device),
        )
        .route("/api/networks/{id}/members", get(networks::members))
        .route(
            "/api/networks/{id}/members/{account}",
            patch(networks::update_member).delete(networks::remove_member),
        )
        .route("/api/networks/{id}/invites", post(networks::invite))
        .route(
            "/api/networks/{id}/keys",
            get(networks::api_keys).post(networks::create_api_key),
        )
        .route(
            "/api/networks/{id}/keys/{key}",
            axum::routing::delete(networks::revoke_api_key),
        )
        .route(
            "/api/networks/{id}/jobs",
            get(networks::jobs).post(networks::create_job),
        )
        .route("/api/networks/{id}/jobs/{job}", get(networks::get_job))
        .route(
            "/api/networks/{id}/jobs/{job}/cancel",
            post(networks::cancel_job),
        )
        .route(
            "/api/networks/{id}/jobs/{job}/image.png",
            get(networks::job_image),
        )
        .route(
            "/api/networks/{id}/chaos/workers/{worker}/kill",
            post(chaos::network_kill),
        )
        .route(
            "/api/networks/{id}/chaos/workers/{worker}/cut",
            post(chaos::network_cut_off),
        )
        .route(
            "/api/networks/{id}/chaos/cheaters",
            post(chaos::network_cheaters),
        )
        .route("/ws/worker", get(ws::worker))
        .route("/ws/dashboard", get(ws::dashboard))
        .route("/ws/networks/{id}/live", get(ws::network_live))
        .route("/ws/join/{code}/summary", get(ws::join_summary))
        .with_state(state)
}

/// Writes the units each device decided to the database every few seconds, rather than
/// one update per unit (spec 0010).
pub fn spawn_device_counts(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(5));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            let counts = state.take_device_units();
            if let Err(e) = db::add_device_units(&state.db, &counts).await {
                tracing::error!(error = %e, "failed to record device units");
            }
        }
    })
}

/// Returns expired leases to the queue every `reap_interval` (spec 0002). Several
/// coordinators may run it at once.
pub fn spawn_reaper(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(state.config.reap_interval);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            match db::retrying(|| db::reap_expired(&state.db, &state.config)).await {
                Ok(released) => {
                    if released.leases > 0 {
                        tracing::info!(released = released.leases, "reaped expired leases");
                    }
                    state.publish_released(released);
                }
                Err(e) => tracing::error!(error = %e, "reaping expired leases failed"),
            }
        }
    })
}

/// Announces decided units to dashboards in batches (see [`State::flush_progress`]).
pub fn spawn_progress(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(state::PROGRESS_INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            state.flush_progress();
        }
    })
}

/// Connects to PostgreSQL, creating the database if needed, and applies
/// pending migrations.
pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    if !Postgres::database_exists(database_url).await? {
        Postgres::create_database(database_url).await?;
    }
    // Every query is short; compiling one with JIT costs more than running it.
    //
    // Commits do not wait for the disk. Each decided unit updates its job's row, so with
    // waiting, a job's completions would go one at a time at the disk's flush rate. A
    // crash of PostgreSQL (not of the coordinator) may lose the last fraction of a
    // second of results, never consistency: their leases expire and the units are
    // computed again. Creating a job still waits (`create_job`).
    let options = PgConnectOptions::from_str(database_url)?
        .options([("jit", "off"), ("synchronous_commit", "off")]);
    let db = PgPoolOptions::new()
        .max_connections(20)
        .connect_with(options)
        .await?;
    MIGRATOR.run(&db).await?;
    Ok(db)
}

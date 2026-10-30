//! Рояк coordinator: job API, work-unit scheduling and result aggregation.

pub mod api;
pub mod db;
pub mod render;
pub mod state;
pub mod templates;
pub mod ws;

use axum::{
    Router,
    routing::{get, post},
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
        .route("/ws/worker", get(ws::worker))
        .route("/ws/dashboard", get(ws::dashboard))
        .with_state(state)
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

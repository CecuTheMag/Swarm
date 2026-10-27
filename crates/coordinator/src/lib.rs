//! Рояк coordinator: job API, work-unit scheduling and result aggregation.

pub mod api;
pub mod db;
pub mod state;
pub mod templates;
pub mod ws;

use axum::{
    Router,
    routing::{get, post},
};
use sqlx::{PgPool, migrate::Migrator, postgres::PgPoolOptions};
pub use state::{AppState, Config, State};

pub static MIGRATOR: Migrator = sqlx::migrate!();

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(api::healthz))
        .route("/api/jobs", post(api::create_job).get(api::list_jobs))
        .route("/api/jobs/{id}", get(api::get_job))
        .route("/ws/worker", get(ws::worker))
        .route("/ws/dashboard", get(ws::dashboard))
        .with_state(state)
}

/// Connects to PostgreSQL and applies pending migrations.
pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let db = PgPoolOptions::new()
        .max_connections(20)
        .connect(database_url)
        .await?;
    MIGRATOR.run(&db).await?;
    Ok(db)
}

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
    postgres::PgPoolOptions,
};
pub use state::{AppState, Config, State};

pub static MIGRATOR: Migrator = sqlx::migrate!();

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(api::healthz))
        .route("/api/jobs", post(api::create_job).get(api::list_jobs))
        .route("/api/jobs/{id}", get(api::get_job))
        .route("/api/jobs/{id}/image.png", get(api::job_image))
        .route("/ws/worker", get(ws::worker))
        .route("/ws/dashboard", get(ws::dashboard))
        .with_state(state)
}

/// Connects to PostgreSQL, creating the database if needed, and applies
/// pending migrations.
pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    if !Postgres::database_exists(database_url).await? {
        Postgres::create_database(database_url).await?;
    }
    let db = PgPoolOptions::new()
        .max_connections(20)
        .connect(database_url)
        .await?;
    MIGRATOR.run(&db).await?;
    Ok(db)
}

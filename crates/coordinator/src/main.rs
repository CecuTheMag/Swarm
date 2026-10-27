use axum::{Router, routing::get};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let addr = std::env::var("COORDINATOR_ADDR").unwrap_or_else(|_| "0.0.0.0:7700".into());
    let app = Router::new().route("/healthz", get(|| async { "ok" }));

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("coordinator listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

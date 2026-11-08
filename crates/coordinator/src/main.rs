use anyhow::Context;
use coordinator::{Config, State};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL is not set")?;
    let addr = std::env::var("COORDINATOR_ADDR").unwrap_or_else(|_| "0.0.0.0:7700".into());

    let db = coordinator::connect(&database_url)
        .await
        .context("connecting to the database")?;

    // `coordinator admin-create EMAIL`: a platform admin with a new random password,
    // printed once (spec 0010). Run on the server, so the password never leaves it.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("admin-create") {
        let email = args
            .get(2)
            .context("usage: coordinator admin-create EMAIL")?;
        let password = &coordinator::auth::random_token()[..20];
        coordinator::accounts::ensure_admin(&db, email, password).await?;
        println!("Admin {email} can sign in with the password: {password}");
        return Ok(());
    }
    // Development and tests: SWARM_BOOTSTRAP_ADMIN=email:password.
    if let Ok(spec) = std::env::var("SWARM_BOOTSTRAP_ADMIN")
        && let Some((email, password)) = spec.split_once(':')
    {
        coordinator::accounts::ensure_admin(&db, email, password).await?;
        tracing::info!(%email, "bootstrap admin ready");
    }
    let mut config = Config::from_env();
    config.self_url = self_url(&addr);
    match (&config.submit_token, config.open_public) {
        (None, true) => tracing::warn!(
            "SWARM_OPEN_PUBLIC: anyone may start jobs and use the chaos panel on the public \
             network; never on a public address"
        ),
        (None, false) => tracing::info!(
            "no SWARM_SUBMIT_TOKEN: only the public network's admins start jobs on it"
        ),
        (Some(_), _) => {}
    }
    let state = State::new(db, config);
    coordinator::spawn_reaper(state.clone());
    coordinator::spawn_progress(state.clone());
    coordinator::spawn_device_counts(state.clone());
    let app = coordinator::router(state);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    tracing::info!("coordinator listening on {addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// The worker endpoint of this process, as reached from inside it (spec 0008).
fn self_url(addr: &str) -> Option<String> {
    let mut addr: std::net::SocketAddr = addr.parse().ok()?;
    if addr.ip().is_unspecified() {
        addr.set_ip(std::net::Ipv4Addr::LOCALHOST.into());
    }
    Some(format!("ws://{addr}/ws/worker"))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutting down");
}

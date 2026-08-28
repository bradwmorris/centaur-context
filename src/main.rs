use anyhow::{Context, Result};
use centaur_os::{
    api::{self, AppState},
    config::Config,
    db,
};
use sqlx::postgres::PgPoolOptions;
use tokio::{net::TcpListener, signal};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "centaur_os=info,tower_http=info".into()),
        )
        .init();

    let config = Config::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .context("connect to centaur_os database")?;
    db::migrate(&pool)
        .await
        .context("run centaur_os migrations")?;

    let human_listener = TcpListener::bind(config.human_addr)
        .await
        .context("bind human listener")?;
    let agent_listener = TcpListener::bind(config.agent_addr)
        .await
        .context("bind agent listener")?;
    let ingest_listener = TcpListener::bind(config.ingest_addr)
        .await
        .context("bind chat ingestion listener")?;
    let state = AppState { pool };
    let human = api::human_router(state.clone(), config.static_dir);
    let agent = api::agent_router(state.clone(), config.agent_api_token);
    let ingest = centaur_os::ingest::router(
        state.clone(),
        config.chat_ingest_api_token,
        config.approved_slack_surfaces,
    );
    let inactivity_pool = state.pool.clone();
    let inactivity_duration = config.interaction_inactivity;
    let poll_interval = config.inactivity_poll_interval;
    let inactivity_worker = async move {
        let mut interval = tokio::time::interval(poll_interval);
        loop {
            interval.tick().await;
            match centaur_os::ingest::queue_inactive_interactions(
                &inactivity_pool,
                inactivity_duration,
            )
            .await
            {
                Ok(queued) if queued > 0 => info!(queued, "queued inactive Slack interactions"),
                Ok(_) => {}
                Err(error) => tracing::error!(%error, "inactivity queue pass failed"),
            }
        }
    };

    info!(address = %config.human_addr, "human UI listener ready");
    info!(address = %config.agent_addr, "agent API listener ready");
    info!(address = %config.ingest_addr, "chat ingestion listener ready");

    tokio::select! {
        result = axum::serve(human_listener, human) => result.context("human server stopped")?,
        result = axum::serve(agent_listener, agent) => result.context("agent server stopped")?,
        result = axum::serve(ingest_listener, ingest) => result.context("chat ingestion server stopped")?,
        _ = inactivity_worker => unreachable!("inactivity worker runs until shutdown"),
        _ = shutdown_signal() => info!("shutdown signal received"),
    }
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async { signal::ctrl_c().await.expect("install Ctrl+C handler") };
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
}

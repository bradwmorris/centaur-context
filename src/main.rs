use anyhow::{Context, Result};
use centaur_context::{
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
                .unwrap_or_else(|_| "centaur_context=info,tower_http=info".into()),
        )
        .init();

    let config = Config::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .context("connect to centaur_context database")?;
    db::migrate(&pool)
        .await
        .context("run centaur_context migrations")?;
    let embedding_client = config
        .embedding
        .as_ref()
        .map(centaur_context::embeddings::EmbeddingClient::new)
        .transpose()?;
    if let Some(client) = embedding_client.as_ref() {
        centaur_context::embeddings::prepare(&pool, client).await?;
    } else {
        info!("Object embeddings disabled; full-text search remains available");
    }

    let human_listener = TcpListener::bind(config.human_addr)
        .await
        .context("bind human listener")?;
    let agent_listener = TcpListener::bind(config.agent_addr)
        .await
        .context("bind agent listener")?;
    let ingest_listener = TcpListener::bind(config.ingest_addr)
        .await
        .context("bind chat ingestion listener")?;
    let curator_listener = TcpListener::bind(config.curator_addr)
        .await
        .context("bind context curator listener")?;
    let state = AppState {
        pool,
        embeddings: embedding_client.clone(),
        text_search_config: config.text_search_config,
    };
    let human = api::human_router(state.clone(), config.static_dir);
    let agent = api::agent_router(state.clone(), config.agent_api_token);
    let ingest = centaur_context::ingest::router(
        state.clone(),
        config.chat_ingest_api_token,
        config.approved_slack_surfaces,
    );
    let curator = centaur_context::curator::router(state.clone(), config.curator_api_token);
    let inactivity_pool = state.pool.clone();
    let inactivity_duration = config.interaction_inactivity;
    let poll_interval = config.inactivity_poll_interval;
    let inactivity_worker = async move {
        let mut interval = tokio::time::interval(poll_interval);
        loop {
            interval.tick().await;
            match centaur_context::ingest::queue_inactive_interactions(
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
    let embedding_pool = state.pool.clone();
    let embedding_poll_interval = config
        .embedding
        .as_ref()
        .map(|embedding| embedding.poll_interval);
    let embedding_worker = async move {
        if let (Some(client), Some(poll_interval)) = (embedding_client, embedding_poll_interval) {
            centaur_context::embeddings::run_worker(embedding_pool, client, poll_interval).await;
        } else {
            std::future::pending::<()>().await;
        }
    };
    let curator_pool = state.pool.clone();
    let curator_embeddings = state.embeddings.clone();
    let curator_model = config.curator_model.clone();
    let curator_text_search_config = config.text_search_config;
    let curator_worker = async move {
        if let Some(curator_model) = curator_model {
            centaur_context::curator::run_worker(
                curator_pool,
                curator_embeddings,
                curator_model,
                curator_text_search_config,
            )
            .await;
        } else {
            info!(
                "Context Curator model disabled; queued runs remain available for the internal curator API"
            );
            std::future::pending::<()>().await;
        }
    };

    info!(address = %config.human_addr, "human UI listener ready");
    info!(address = %config.agent_addr, "agent API listener ready");
    info!(address = %config.ingest_addr, "chat ingestion listener ready");
    info!(address = %config.curator_addr, "context curator listener ready");

    tokio::select! {
        result = axum::serve(human_listener, human) => result.context("human server stopped")?,
        result = axum::serve(agent_listener, agent) => result.context("agent server stopped")?,
        result = axum::serve(ingest_listener, ingest) => result.context("chat ingestion server stopped")?,
        result = axum::serve(curator_listener, curator) => result.context("context curator server stopped")?,
        _ = inactivity_worker => unreachable!("inactivity worker runs until shutdown"),
        _ = embedding_worker => unreachable!("embedding worker runs until shutdown"),
        _ = curator_worker => unreachable!("curator worker runs until shutdown"),
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

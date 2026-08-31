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
    let note_write_listener = TcpListener::bind(config.note_write_addr)
        .await
        .context("bind Note-write listener")?;
    let ingest_listener = TcpListener::bind(config.ingest_addr)
        .await
        .context("bind chat ingestion listener")?;
    let curator_listener = TcpListener::bind(config.curator_addr)
        .await
        .context("bind context curator listener")?;
    let intake_listener = if let Some(intake) = config.intake.as_ref() {
        Some((
            TcpListener::bind(intake.addr)
                .await
                .context("bind Context intake listener")?,
            intake.clone(),
        ))
    } else {
        None
    };
    let source_intake_listener = if let Some(source_intake) = config.source_intake.as_ref() {
        Some((
            TcpListener::bind(source_intake.addr)
                .await
                .context("bind permanent Source intake listener")?,
            source_intake.clone(),
        ))
    } else {
        None
    };
    let theme_proposal_listener = if let Some(theme_proposals) = config.theme_proposals.as_ref() {
        Some((
            TcpListener::bind(theme_proposals.addr)
                .await
                .context("bind Theme proposal listener")?,
            theme_proposals.clone(),
        ))
    } else {
        None
    };
    let external_action_listener = if let Some(external_actions) = config.external_actions.as_ref()
    {
        Some((
            TcpListener::bind(external_actions.addr)
                .await
                .context("bind External-action listener")?,
            external_actions.clone(),
        ))
    } else {
        None
    };
    let state = AppState {
        pool,
        embeddings: embedding_client.clone(),
        text_search_config: config.text_search_config,
    };
    let human = api::human_router(state.clone(), config.static_dir, config.identity_assets_dir);
    let agent = api::agent_router(state.clone(), config.agent_api_token);
    let note_write = api::note_write_router(state.clone(), config.note_write_api_token);
    let ingest = centaur_context::ingest::router(
        state.clone(),
        config.chat_ingest_api_token,
        config.approved_slack_surfaces,
    );
    let curator = centaur_context::curator::router(state.clone(), config.curator_api_token);
    let intake = intake_listener.as_ref().map(|(_, config)| {
        centaur_context::intake::router(
            state.clone(),
            config.api_token.clone(),
            config.approved_manifest_sha256.clone(),
        )
    });
    let source_intake = source_intake_listener.as_ref().map(|(_, config)| {
        centaur_context::source_intake::router(state.clone(), config.api_token.clone())
    });
    let theme_proposals = theme_proposal_listener
        .as_ref()
        .map(|(_, config)| api::theme_proposal_router(state.clone(), config.api_token.clone()));
    let external_actions = external_action_listener.as_ref().map(|(_, config)| {
        centaur_context::external_actions::router(
            state.clone(),
            config.api_token.clone(),
            config.allowed_principals.clone(),
        )
    });
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
    info!(address = %config.note_write_addr, "Note-write API listener ready");
    info!(address = %config.ingest_addr, "chat ingestion listener ready");
    info!(address = %config.curator_addr, "context curator listener ready");
    if let Some((_, config)) = intake_listener.as_ref() {
        info!(address = %config.addr, "private Context intake listener ready");
    } else {
        info!("private Context intake listener disabled");
    }
    if let Some((_, config)) = source_intake_listener.as_ref() {
        info!(address = %config.addr, "permanent Source intake listener ready");
    } else {
        info!("permanent Source intake listener disabled");
    }
    if let Some((_, config)) = theme_proposal_listener.as_ref() {
        info!(address = %config.addr, "Theme proposal listener ready");
    } else {
        info!("Theme proposal listener disabled");
    }
    if let Some((_, config)) = external_action_listener.as_ref() {
        info!(address = %config.addr, "External-action listener ready");
    } else {
        info!("External-action listener disabled");
    }

    let intake_server = async move {
        if let (Some((listener, _)), Some(router)) = (intake_listener, intake) {
            axum::serve(listener, router)
                .await
                .context("Context intake server stopped")
        } else {
            std::future::pending::<Result<()>>().await
        }
    };

    let source_intake_server = async move {
        if let (Some((listener, _)), Some(router)) = (source_intake_listener, source_intake) {
            axum::serve(listener, router)
                .await
                .context("permanent Source intake server stopped")
        } else {
            std::future::pending::<Result<()>>().await
        }
    };

    let theme_proposal_server = async move {
        if let (Some((listener, _)), Some(router)) = (theme_proposal_listener, theme_proposals) {
            axum::serve(listener, router)
                .await
                .context("Theme proposal server stopped")
        } else {
            std::future::pending::<Result<()>>().await
        }
    };

    let external_action_server = async move {
        if let (Some((listener, _)), Some(router)) = (external_action_listener, external_actions) {
            axum::serve(listener, router)
                .await
                .context("External-action server stopped")
        } else {
            std::future::pending::<Result<()>>().await
        }
    };

    tokio::select! {
        result = axum::serve(human_listener, human) => result.context("human server stopped")?,
        result = axum::serve(agent_listener, agent) => result.context("agent server stopped")?,
        result = axum::serve(note_write_listener, note_write) => result.context("Note-write server stopped")?,
        result = axum::serve(ingest_listener, ingest) => result.context("chat ingestion server stopped")?,
        result = axum::serve(curator_listener, curator) => result.context("context curator server stopped")?,
        result = intake_server => result?,
        result = source_intake_server => result?,
        result = theme_proposal_server => result?,
        result = external_action_server => result?,
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

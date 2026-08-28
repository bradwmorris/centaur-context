use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result, bail};

use crate::ingest::ApprovedSlackSurfaces;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub human_addr: SocketAddr,
    pub agent_addr: SocketAddr,
    pub agent_api_token: String,
    pub ingest_addr: SocketAddr,
    pub chat_ingest_api_token: String,
    pub curator_addr: SocketAddr,
    pub curator_api_token: String,
    pub approved_slack_surfaces: ApprovedSlackSurfaces,
    pub interaction_inactivity: std::time::Duration,
    pub inactivity_poll_interval: std::time::Duration,
    pub embedding: Option<EmbeddingConfig>,
    pub curator_model: Option<CuratorModelConfig>,
    pub static_dir: PathBuf,
}

#[derive(Clone)]
pub struct EmbeddingConfig {
    pub endpoint: String,
    pub api_token: String,
    pub model: String,
    pub dimensions: i32,
    pub poll_interval: std::time::Duration,
}

#[derive(Clone)]
pub struct CuratorModelConfig {
    pub endpoint: String,
    pub api_token: String,
    pub model: String,
    pub prompt_version: String,
    pub poll_interval: std::time::Duration,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let database_url = required("DATABASE_URL")?;
        let agent_api_token = required("AGENT_API_TOKEN")?;
        if agent_api_token.len() < 32 {
            bail!("AGENT_API_TOKEN must be at least 32 characters");
        }
        let chat_ingest_api_token = required("CHAT_INGEST_API_TOKEN")?;
        if chat_ingest_api_token.len() < 32 {
            bail!("CHAT_INGEST_API_TOKEN must be at least 32 characters");
        }
        if chat_ingest_api_token == agent_api_token {
            bail!("CHAT_INGEST_API_TOKEN must differ from AGENT_API_TOKEN");
        }
        let curator_api_token = required("CURATOR_API_TOKEN")?;
        if curator_api_token.len() < 32 {
            bail!("CURATOR_API_TOKEN must be at least 32 characters");
        }
        if curator_api_token == agent_api_token || curator_api_token == chat_ingest_api_token {
            bail!("CURATOR_API_TOKEN must differ from the agent and ingestion tokens");
        }
        let approved_slack_surfaces =
            ApprovedSlackSurfaces::parse(&required("APPROVED_SLACK_SURFACES")?)
                .map_err(anyhow::Error::msg)?;
        let embedding = embedding_config()?;
        let curator_model = curator_model_config()?;

        Ok(Self {
            database_url,
            human_addr: parse_addr("HUMAN_ADDR", "0.0.0.0:8080")?,
            agent_addr: parse_addr("AGENT_ADDR", "0.0.0.0:8081")?,
            agent_api_token,
            ingest_addr: parse_addr("INGEST_ADDR", "0.0.0.0:8082")?,
            chat_ingest_api_token,
            curator_addr: parse_addr("CURATOR_ADDR", "0.0.0.0:8083")?,
            curator_api_token,
            approved_slack_surfaces,
            interaction_inactivity: parse_duration_seconds(
                "INTERACTION_INACTIVITY_SECONDS",
                600,
                60,
            )?,
            inactivity_poll_interval: parse_duration_seconds("INACTIVITY_POLL_SECONDS", 30, 5)?,
            embedding,
            curator_model,
            static_dir: env::var("STATIC_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("web/dist")),
        })
    }
}

fn curator_model_config() -> Result<Option<CuratorModelConfig>> {
    let endpoint = optional("CURATOR_MODEL_API_URL");
    let token = optional("CURATOR_MODEL_API_TOKEN");
    let model = optional("CURATOR_MODEL");
    let prompt_version = optional("CURATOR_PROMPT_VERSION");
    let configured =
        endpoint.is_some() || token.is_some() || model.is_some() || prompt_version.is_some();
    if !configured {
        return Ok(None);
    }
    let endpoint = endpoint.context(
        "CURATOR_MODEL_API_URL is required when any curator model configuration is provided",
    )?;
    let api_token = token.context(
        "CURATOR_MODEL_API_TOKEN is required when any curator model configuration is provided",
    )?;
    let model = model
        .context("CURATOR_MODEL is required when any curator model configuration is provided")?;
    let prompt_version = prompt_version.context(
        "CURATOR_PROMPT_VERSION is required when any curator model configuration is provided",
    )?;
    if model.len() > 300 || prompt_version.len() > 300 {
        bail!("CURATOR_MODEL and CURATOR_PROMPT_VERSION must each be at most 300 characters");
    }
    Ok(Some(CuratorModelConfig {
        endpoint,
        api_token,
        model,
        prompt_version,
        poll_interval: parse_duration_seconds("CURATOR_POLL_SECONDS", 5, 1)?,
    }))
}

fn embedding_config() -> Result<Option<EmbeddingConfig>> {
    let endpoint = optional("EMBEDDING_API_URL");
    let token = optional("EMBEDDING_API_TOKEN");
    let model = optional("EMBEDDING_MODEL");
    let configured = endpoint.is_some() || token.is_some() || model.is_some();
    if !configured {
        return Ok(None);
    }
    let endpoint = endpoint
        .context("EMBEDDING_API_URL is required when any embedding configuration is provided")?;
    let api_token = token
        .context("EMBEDDING_API_TOKEN is required when any embedding configuration is provided")?;
    let model = model
        .context("EMBEDDING_MODEL is required when any embedding configuration is provided")?;
    if model.len() > 300 {
        bail!("EMBEDDING_MODEL must be at most 300 characters");
    }
    let dimensions = env::var("EMBEDDING_DIMENSIONS")
        .context("EMBEDDING_DIMENSIONS is required when embeddings are configured")?
        .parse::<i32>()
        .context("EMBEDDING_DIMENSIONS must be an integer")?;
    if !(1..=2000).contains(&dimensions) {
        bail!("EMBEDDING_DIMENSIONS must be between 1 and 2000 for pgvector HNSW indexing");
    }
    Ok(Some(EmbeddingConfig {
        endpoint,
        api_token,
        model,
        dimensions,
        poll_interval: parse_duration_seconds("EMBEDDING_POLL_SECONDS", 5, 1)?,
    }))
}

fn parse_duration_seconds(name: &str, default: u64, minimum: u64) -> Result<std::time::Duration> {
    let seconds = env::var(name)
        .unwrap_or_else(|_| default.to_string())
        .parse::<u64>()
        .with_context(|| format!("{name} must be a positive number of seconds"))?;
    if seconds < minimum {
        bail!("{name} must be at least {minimum} seconds");
    }
    Ok(std::time::Duration::from_secs(seconds))
}

fn required(name: &str) -> Result<String> {
    env::var(name)
        .with_context(|| format!("{name} is required"))
        .and_then(|value| {
            let value = value.trim().to_owned();
            if value.is_empty() {
                bail!("{name} must not be empty");
            }
            Ok(value)
        })
}

fn optional(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn parse_addr(name: &str, default: &str) -> Result<SocketAddr> {
    env::var(name)
        .unwrap_or_else(|_| default.to_owned())
        .parse()
        .with_context(|| format!("{name} must be a socket address"))
}

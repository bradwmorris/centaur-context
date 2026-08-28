use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result, bail};

use crate::ingest::ApprovedSlackSurfaces;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub human_addr: SocketAddr,
    pub agent_addr: SocketAddr,
    pub agent_api_token: String,
    pub ingest_addr: SocketAddr,
    pub chat_ingest_api_token: String,
    pub approved_slack_surfaces: ApprovedSlackSurfaces,
    pub interaction_inactivity: std::time::Duration,
    pub inactivity_poll_interval: std::time::Duration,
    pub static_dir: PathBuf,
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
        let approved_slack_surfaces =
            ApprovedSlackSurfaces::parse(&required("APPROVED_SLACK_SURFACES")?)
                .map_err(anyhow::Error::msg)?;

        Ok(Self {
            database_url,
            human_addr: parse_addr("HUMAN_ADDR", "0.0.0.0:8080")?,
            agent_addr: parse_addr("AGENT_ADDR", "0.0.0.0:8081")?,
            agent_api_token,
            ingest_addr: parse_addr("INGEST_ADDR", "0.0.0.0:8082")?,
            chat_ingest_api_token,
            approved_slack_surfaces,
            interaction_inactivity: parse_duration_seconds(
                "INTERACTION_INACTIVITY_SECONDS",
                600,
                60,
            )?,
            inactivity_poll_interval: parse_duration_seconds("INACTIVITY_POLL_SECONDS", 30, 5)?,
            static_dir: env::var("STATIC_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("web/dist")),
        })
    }
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

fn parse_addr(name: &str, default: &str) -> Result<SocketAddr> {
    env::var(name)
        .unwrap_or_else(|_| default.to_owned())
        .parse()
        .with_context(|| format!("{name} must be a socket address"))
}

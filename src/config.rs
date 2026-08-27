use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result, bail};

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub human_addr: SocketAddr,
    pub agent_addr: SocketAddr,
    pub agent_api_token: String,
    pub static_dir: PathBuf,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let database_url = required("DATABASE_URL")?;
        let agent_api_token = required("AGENT_API_TOKEN")?;
        if agent_api_token.len() < 32 {
            bail!("AGENT_API_TOKEN must be at least 32 characters");
        }

        Ok(Self {
            database_url,
            human_addr: parse_addr("HUMAN_ADDR", "0.0.0.0:8080")?,
            agent_addr: parse_addr("AGENT_ADDR", "0.0.0.0:8081")?,
            agent_api_token,
            static_dir: env::var("STATIC_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("web/dist")),
        })
    }
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

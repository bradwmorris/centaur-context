use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result, bail};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{error, info};

use crate::{config::EmbeddingConfig, db};

#[derive(Clone)]
pub struct EmbeddingClient {
    inner: Arc<Inner>,
}

struct Inner {
    http: Client,
    endpoint: Url,
    api_token: String,
    model: String,
    dimensions: i32,
}

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    model: &'a str,
    input: &'a str,
    dimensions: i32,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingItem>,
}

#[derive(Deserialize)]
struct EmbeddingItem {
    embedding: Vec<f32>,
}

impl EmbeddingClient {
    pub fn new(config: &EmbeddingConfig) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .context("build embedding HTTP client")?;
        Ok(Self {
            inner: Arc::new(Inner {
                http,
                endpoint: Url::parse(&config.endpoint).context("EMBEDDING_API_URL is invalid")?,
                api_token: config.api_token.clone(),
                model: config.model.clone(),
                dimensions: config.dimensions,
            }),
        })
    }

    pub fn model(&self) -> &str {
        &self.inner.model
    }

    pub fn dimensions(&self) -> i32 {
        self.inner.dimensions
    }

    pub async fn embed(&self, input: &str) -> Result<Vec<f32>> {
        let response = self
            .inner
            .http
            .post(self.inner.endpoint.clone())
            .bearer_auth(&self.inner.api_token)
            .json(&EmbeddingRequest {
                model: &self.inner.model,
                input,
                dimensions: self.inner.dimensions,
            })
            .send()
            .await
            .context("call embedding provider")?
            .error_for_status()
            .context("embedding provider rejected request")?
            .json::<EmbeddingResponse>()
            .await
            .context("decode embedding response")?;
        let embedding = response
            .data
            .into_iter()
            .next()
            .context("embedding provider returned no vector")?
            .embedding;
        if embedding.len() != self.inner.dimensions as usize {
            bail!(
                "embedding provider returned {} dimensions; expected {}",
                embedding.len(),
                self.inner.dimensions
            );
        }
        if embedding.iter().any(|value| !value.is_finite()) {
            bail!("embedding provider returned a non-finite value");
        }
        Ok(embedding)
    }
}

pub async fn run_worker(pool: PgPool, client: EmbeddingClient, poll_interval: Duration) {
    let mut interval = tokio::time::interval(poll_interval);
    loop {
        interval.tick().await;
        match db::claim_embedding_job(&pool).await {
            Ok(Some(job)) => {
                let text = format!("{}\n{}\n{}", job.kind, job.title, job.description);
                match client.embed(&text).await {
                    Ok(vector) => {
                        if let Err(error) = db::complete_embedding_job(
                            &pool,
                            &job,
                            client.model(),
                            client.dimensions(),
                            &vector,
                        )
                        .await
                        {
                            error!(object_id = %job.object_id, %error, "store Object embedding failed");
                            if let Err(store_error) =
                                db::fail_embedding_job(&pool, job.object_id, &error.to_string())
                                    .await
                            {
                                error!(object_id = %job.object_id, %store_error, "record embedding storage failure failed");
                            }
                        }
                    }
                    Err(error) => {
                        if let Err(store_error) =
                            db::fail_embedding_job(&pool, job.object_id, &error.to_string()).await
                        {
                            error!(object_id = %job.object_id, %store_error, "record embedding failure failed");
                        }
                    }
                }
            }
            Ok(None) => {}
            Err(error) => error!(%error, "claim Object embedding job failed"),
        }
    }
}

pub async fn prepare(pool: &PgPool, client: &EmbeddingClient) -> Result<()> {
    db::ensure_embedding_index(pool, client.dimensions())
        .await
        .context("create configured pgvector index")?;
    let queued = db::queue_missing_embeddings(pool, client.model())
        .await
        .context("queue missing Object embeddings")?;
    info!(
        model = client.model(),
        dimensions = client.dimensions(),
        queued,
        "Object embeddings enabled"
    );
    Ok(())
}

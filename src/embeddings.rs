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
    input_mode: crate::config::EmbeddingInputMode,
}

pub const OBJECT_EMBEDDING_FORMAT: &str = "centaur-object-v1";

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    model: &'a str,
    input: &'a str,
    dimensions: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_type: Option<&'a str>,
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
                input_mode: config.input_mode,
            }),
        })
    }

    pub fn model(&self) -> &str {
        &self.inner.model
    }

    pub fn dimensions(&self) -> i32 {
        self.inner.dimensions
    }

    pub fn document_mode(&self) -> &'static str {
        self.inner.input_mode.document_mode()
    }

    pub async fn embed_query(&self, input: &str) -> Result<Vec<f32>> {
        self.embed(input, self.inner.input_mode.query_mode()).await
    }

    async fn embed_document(&self, input: &str) -> Result<Vec<f32>> {
        let input_type = match self.inner.input_mode {
            crate::config::EmbeddingInputMode::Shared => None,
            crate::config::EmbeddingInputMode::Typed => Some("search_document"),
        };
        self.embed(input, input_type).await
    }

    async fn embed(&self, input: &str, input_type: Option<&str>) -> Result<Vec<f32>> {
        let response = self
            .inner
            .http
            .post(self.inner.endpoint.clone())
            .bearer_auth(&self.inner.api_token)
            .json(&EmbeddingRequest {
                model: &self.inner.model,
                input,
                dimensions: self.inner.dimensions,
                input_type,
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

pub fn format_object_document(kind: &str, title: &str, description: &str) -> String {
    format!("{OBJECT_EMBEDDING_FORMAT}\nkind: {kind}\ntitle: {title}\ndescription: {description}")
}

pub async fn run_worker(pool: PgPool, client: EmbeddingClient, poll_interval: Duration) {
    let mut interval = tokio::time::interval(poll_interval);
    loop {
        interval.tick().await;
        match db::claim_embedding_job(&pool).await {
            Ok(Some(job)) => {
                if job.format_version != OBJECT_EMBEDDING_FORMAT
                    || job.input_mode != client.document_mode()
                {
                    if let Err(error) = db::queue_missing_embeddings(
                        &pool,
                        client.model(),
                        client.dimensions(),
                        OBJECT_EMBEDDING_FORMAT,
                        client.document_mode(),
                    )
                    .await
                    {
                        error!(object_id = %job.object_id, %error, "refresh stale embedding job failed");
                    }
                    continue;
                }
                let text = format_object_document(&job.kind, &job.title, &job.description);
                match client.embed_document(&text).await {
                    Ok(vector) => {
                        if let Err(error) = db::complete_embedding_job(
                            &pool,
                            &job,
                            client.model(),
                            client.dimensions(),
                            OBJECT_EMBEDDING_FORMAT,
                            client.document_mode(),
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
    let queued = db::queue_missing_embeddings(
        pool,
        client.model(),
        client.dimensions(),
        OBJECT_EMBEDDING_FORMAT,
        client.document_mode(),
    )
    .await
    .context("queue missing Object embeddings")?;
    info!(
        model = client.model(),
        dimensions = client.dimensions(),
        format_version = OBJECT_EMBEDDING_FORMAT,
        input_mode = client.document_mode(),
        queued,
        "Object embeddings enabled"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{EmbeddingRequest, format_object_document};

    #[test]
    fn object_embedding_format_is_versioned_and_deterministic() {
        assert_eq!(
            format_object_document("memory", "Launch approved", "The team approved launch."),
            "centaur-object-v1\nkind: memory\ntitle: Launch approved\ndescription: The team approved launch."
        );
    }

    #[test]
    fn provider_input_modes_are_optional_and_explicit() {
        let shared = serde_json::to_value(EmbeddingRequest {
            model: "test",
            input: "query",
            dimensions: 3,
            input_type: None,
        })
        .unwrap();
        assert!(shared.get("input_type").is_none());
        let typed = serde_json::to_value(EmbeddingRequest {
            model: "test",
            input: "query",
            dimensions: 3,
            input_type: Some("search_query"),
        })
        .unwrap();
        assert_eq!(typed["input_type"], "search_query");
    }
}

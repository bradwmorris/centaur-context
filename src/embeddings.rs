use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result, bail};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
pub const ARTIFACT_EMBEDDING_FORMAT: &str = "centaur-artifact-chunk-v1";
pub const ARTIFACT_CHUNK_CHARACTERS: usize = 6_000;
pub const ARTIFACT_CHUNK_OVERLAP: usize = 600;
pub const ARTIFACT_EMBEDDING_MAX_BYTES: usize = 8_000;

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

pub fn format_artifact_document(title: &str, content: &str) -> String {
    format!("{ARTIFACT_EMBEDDING_FORMAT}\ntitle: {title}\ncontent: {content}")
}

pub fn artifact_chunks(title: &str, content: &str) -> Vec<(i32, i32)> {
    let characters = content.chars().collect::<Vec<_>>();
    if characters.is_empty() {
        return Vec::new();
    }
    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < characters.len() {
        let hard_end = (start + ARTIFACT_CHUNK_CHARACTERS).min(characters.len());
        let mut end = hard_end;
        if hard_end < characters.len() {
            let minimum_boundary = start + ARTIFACT_CHUNK_CHARACTERS / 2;
            if let Some(boundary) = (minimum_boundary..hard_end).rev().find(|index| {
                *index > 0
                    && characters[*index - 1] == '\n'
                    && characters.get(*index) == Some(&'\n')
            }) {
                end = boundary;
            }
        }
        if format_artifact_document(title, &characters[start..end].iter().collect::<String>()).len()
            > ARTIFACT_EMBEDDING_MAX_BYTES
        {
            let mut low = start + 1;
            let mut high = end;
            let mut safe_end = start;
            while low <= high {
                let middle = low + (high - low) / 2;
                let body = characters[start..middle].iter().collect::<String>();
                if format_artifact_document(title, &body).len() <= ARTIFACT_EMBEDDING_MAX_BYTES {
                    safe_end = middle;
                    low = middle + 1;
                } else {
                    high = middle - 1;
                }
            }
            end = safe_end;
        }
        if end == start {
            break;
        }
        chunks.push((start as i32, end as i32));
        if end == characters.len() {
            break;
        }
        let next = end.saturating_sub(ARTIFACT_CHUNK_OVERLAP);
        start = next.max(start + 1);
    }
    chunks
}

fn character_window(content: &str, start: i32, end: i32) -> String {
    content
        .chars()
        .skip(start as usize)
        .take((end - start) as usize)
        .collect()
}

async fn queue_all(pool: &PgPool, client: &EmbeddingClient) -> Result<u64> {
    let mut queued = db::queue_missing_embeddings(
        pool,
        client.model(),
        client.dimensions(),
        OBJECT_EMBEDDING_FORMAT,
        client.document_mode(),
    )
    .await?;
    for source in db::artifact_embedding_sources(pool).await? {
        let chunks = artifact_chunks(&source.title, &source.content)
            .into_iter()
            .enumerate()
            .map(|(index, (start_offset, end_offset))| {
                let body = character_window(&source.content, start_offset, end_offset);
                let formatted = format_artifact_document(&source.title, &body);
                db::ArtifactEmbeddingChunk {
                    chunk_index: index as i32,
                    start_offset,
                    end_offset,
                    source_hash: format!("{:x}", Sha256::digest(formatted.as_bytes())),
                }
            })
            .collect::<Vec<_>>();
        queued += db::queue_artifact_embedding_chunks(
            pool,
            &source,
            &chunks,
            client.model(),
            client.dimensions(),
            ARTIFACT_EMBEDDING_FORMAT,
            client.document_mode(),
        )
        .await?;
    }
    Ok(queued)
}

pub async fn run_worker(pool: PgPool, client: EmbeddingClient, poll_interval: Duration) {
    let mut interval = tokio::time::interval(poll_interval);
    loop {
        interval.tick().await;
        if let Err(error) = queue_all(&pool, &client).await {
            error!(%error, "reconcile embedding queue failed");
            continue;
        }
        match db::claim_embedding_job(
            &pool,
            client.model(),
            client.dimensions(),
            client.document_mode(),
        )
        .await
        {
            Ok(Some(job)) => {
                let text = match (
                    job.artifact_id,
                    job.artifact_content.as_deref(),
                    job.start_offset,
                    job.end_offset,
                    job.format_version.as_str(),
                ) {
                    (Some(_), Some(content), Some(start), Some(end), ARTIFACT_EMBEDDING_FORMAT) => {
                        format_artifact_document(&job.title, &character_window(content, start, end))
                    }
                    (None, None, None, None, OBJECT_EMBEDDING_FORMAT) => {
                        format_object_document(&job.kind, &job.title, &job.description)
                    }
                    _ => {
                        if let Err(error) = db::fail_embedding_job(
                            &pool,
                            job.id,
                            "embedding job target or formatter is inconsistent",
                        )
                        .await
                        {
                            error!(embedding_id = %job.id, %error, "record invalid embedding job failed");
                        }
                        continue;
                    }
                };
                match client.embed_document(&text).await {
                    Ok(vector) => {
                        if let Err(error) = db::complete_embedding_job(&pool, &job, &vector).await {
                            error!(object_id = %job.object_id, %error, "store embedding failed");
                            if let Err(store_error) =
                                db::fail_embedding_job(&pool, job.id, &error.to_string()).await
                            {
                                error!(object_id = %job.object_id, %store_error, "record embedding storage failure failed");
                            }
                        }
                    }
                    Err(error) => {
                        if let Err(store_error) =
                            db::fail_embedding_job(&pool, job.id, &error.to_string()).await
                        {
                            error!(object_id = %job.object_id, %store_error, "record embedding failure failed");
                        }
                    }
                }
            }
            Ok(None) => {}
            Err(error) => error!(%error, "claim embedding job failed"),
        }
    }
}

pub async fn prepare(pool: &PgPool, client: &EmbeddingClient) -> Result<()> {
    db::ensure_embedding_index(pool, client.dimensions())
        .await
        .context("create configured pgvector index")?;
    let queued = queue_all(pool, client)
        .await
        .context("queue missing Object and Artifact embeddings")?;
    info!(
        model = client.model(),
        dimensions = client.dimensions(),
        format_version = OBJECT_EMBEDDING_FORMAT,
        input_mode = client.document_mode(),
        queued,
        "Object and Artifact embeddings enabled"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ARTIFACT_EMBEDDING_MAX_BYTES, EmbeddingRequest, artifact_chunks, character_window,
        format_artifact_document, format_object_document,
    };

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

    #[test]
    fn artifact_chunks_are_bounded_overlapping_and_complete() {
        let content = "a".repeat(13_000);
        let chunks = artifact_chunks("Synthetic title", &content);
        assert_eq!(chunks, vec![(0, 6000), (5400, 11400), (10800, 13000)]);
    }

    #[test]
    fn artifact_chunks_guard_provider_input_bytes_for_unicode() {
        let content = "🦀".repeat(6_000);
        for (start, end) in artifact_chunks("Unicode source", &content) {
            let body = character_window(&content, start, end);
            assert!(
                format_artifact_document("Unicode source", &body).len()
                    <= ARTIFACT_EMBEDDING_MAX_BYTES
            );
        }
    }
}

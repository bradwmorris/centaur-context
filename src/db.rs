use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Transaction};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::domain::{ActorContext, ValidationError, theme_slug, validate_object_description};

#[derive(Debug, Error)]
pub enum DbError {
    #[error("record not found")]
    NotFound,
    #[error("revision conflict")]
    Conflict,
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Validation(#[from] ValidationError),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

#[derive(Clone, Debug, Deserialize, FromRow, Serialize)]
pub struct Object {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub description: String,
    pub protected: bool,
    pub lifecycle: String,
    pub revision: i64,
    pub created_by_type: String,
    pub created_by_id: String,
    pub updated_by_type: String,
    pub updated_by_id: String,
    pub provenance: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub archived_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct Connection {
    pub id: Uuid,
    pub source_object_id: Uuid,
    pub kind: String,
    pub target_object_id: Uuid,
    pub description: String,
    pub protected: bool,
    pub revision: i64,
    pub created_by_type: String,
    pub created_by_id: String,
    pub updated_by_type: String,
    pub updated_by_id: String,
    pub provenance: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub archived_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct ConnectionGraphNode {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct ConnectionGraphEdge {
    pub id: Uuid,
    pub source_object_id: Uuid,
    pub target_object_id: Uuid,
    pub kind: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConnectionGraphSnapshot {
    pub fingerprint: String,
    pub node_count: usize,
    pub connection_count: usize,
    pub nodes: Vec<ConnectionGraphNode>,
    pub edges: Vec<ConnectionGraphEdge>,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct Task {
    pub object_id: Uuid,
    pub title: String,
    pub description: String,
    pub lifecycle: String,
    pub revision: i64,
    pub provenance: Value,
    pub protected: bool,
    pub status: String,
    pub priority: String,
    pub owner_object_id: Option<Uuid>,
    pub agent_suitable: bool,
    pub blocked_reason: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub due_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub completed_at: Option<OffsetDateTime>,
    pub github_issue_url: Option<String>,
    pub brief_markdown: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct ObjectEvent {
    pub id: Uuid,
    pub run_id: Uuid,
    pub sequence: i32,
    pub target_type: String,
    pub target_id: Uuid,
    pub action: String,
    pub actor_type: String,
    pub actor_id: String,
    pub idempotency_key: Option<String>,
    pub from_revision: Option<i64>,
    pub to_revision: i64,
    pub before_state: Option<Value>,
    pub after_state: Value,
    pub reversible: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct ChatMessage {
    pub id: Uuid,
    pub chat_object_id: Uuid,
    pub provider_message_id: String,
    pub sender_user_object_id: Uuid,
    pub sender_title: String,
    pub sender_kind: String,
    pub content: String,
    #[serde(with = "time::serde::rfc3339")]
    pub source_created_at: OffsetDateTime,
    pub ingestion_sequence: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub ingested_at: OffsetDateTime,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct User {
    pub object_id: Uuid,
    pub title: String,
    pub description: String,
    pub lifecycle: String,
    pub revision: i64,
    pub provenance: Value,
    pub user_kind: String,
    pub identities: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct Source {
    pub object_id: Uuid,
    pub title: String,
    pub description: String,
    pub lifecycle: String,
    pub revision: i64,
    pub provenance: Value,
    pub protected: bool,
    pub source_kind: String,
    pub canonical_uri: Option<String>,
    pub byline: Option<String>,
    pub publisher: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub published_at: Option<OffsetDateTime>,
    pub published_at_precision: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_accessed_at: Option<OffsetDateTime>,
    pub original_language: Option<String>,
    pub original_media_type: Option<String>,
    pub original_artifact_reference: Option<String>,
    pub current_artifact_id: Option<Uuid>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct Artifact {
    pub id: Uuid,
    pub object_id: Uuid,
    pub kind: String,
    pub title: Option<String>,
    #[serde(skip_serializing)]
    pub content: Option<String>,
    pub uri: Option<String>,
    pub media_type: Option<String>,
    pub language: Option<String>,
    pub sha256: String,
    pub size_bytes: i64,
    pub capture_outcome: String,
    pub capture_reason: Option<String>,
    pub expected_size_bytes: Option<i64>,
    pub semantic_indexing_enabled: bool,
    pub metadata: Value,
    pub supersedes_artifact_id: Option<Uuid>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub captured_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct SourceSearchResult {
    #[sqlx(flatten)]
    #[serde(flatten)]
    pub source: Source,
    pub excerpt: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ArtifactWindow {
    #[serde(flatten)]
    pub content: Artifact,
    pub text: String,
    pub offset: i64,
    pub next_offset: Option<i64>,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct Note {
    pub object_id: Uuid,
    pub title: String,
    pub description: String,
    pub lifecycle: String,
    pub revision: i64,
    pub provenance: Value,
    pub protected: bool,
    pub content: String,
    pub content_format: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct NoteSearchResult {
    pub object_id: Uuid,
    pub title: String,
    pub description: String,
    pub lifecycle: String,
    pub revision: i64,
    pub content_format: String,
    pub excerpt: String,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct Theme {
    pub object_id: Uuid,
    pub title: String,
    pub description: String,
    pub slug: String,
    pub lifecycle: String,
    pub revision: i64,
    pub provenance: Value,
    pub protected: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExternalIdentity {
    pub id: Uuid,
    pub provider: String,
    pub workspace_id: String,
    pub provider_user_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub avatar_asset_sha256: Option<String>,
    pub avatar_asset_filename: Option<String>,
    pub avatar_provenance: Value,
    #[serde(with = "time::serde::rfc3339::option")]
    pub profile_refreshed_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ObjectVisual {
    pub object_id: Uuid,
    pub source_provider: Option<String>,
    pub users: Vec<UserAttribution>,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct UserAttribution {
    pub object_id: Uuid,
    pub user_object_id: Uuid,
    pub title: String,
    pub user_kind: String,
    pub role: String,
    pub avatar_url: Option<String>,
    pub avatar_asset_url: Option<String>,
}

#[derive(Clone, Debug, FromRow)]
struct ObjectVisualSource {
    object_id: Uuid,
    source_provider: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SearchCandidate {
    pub object: Object,
    pub relevance: f64,
    pub connection_count: i64,
    pub evidence: Option<SearchEvidence>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchEvidence {
    pub artifact_id: Uuid,
    pub start_offset: i32,
    pub end_offset: i32,
    pub excerpt: String,
    pub capture_outcome: String,
    pub match_kind: String,
}

#[derive(Clone, Debug)]
pub struct ContextAnchorCandidate {
    pub object: Object,
    pub priority: i32,
    pub rationale: String,
}

#[derive(Clone, Debug, FromRow)]
pub struct ContextChat {
    pub object_id: Uuid,
    pub lifecycle: String,
    pub provider: Option<String>,
    pub workspace_id: Option<String>,
    pub channel_id: Option<String>,
    pub thread_id: Option<String>,
}

impl ContextChat {
    pub fn thread_key(&self) -> Option<String> {
        Some(format!(
            "{}:{}:{}:{}",
            self.provider.as_deref()?,
            self.workspace_id.as_deref()?,
            self.channel_id.as_deref()?,
            self.thread_id.as_deref()?
        ))
    }
}

#[derive(Clone, Debug, FromRow)]
struct SearchCandidateRow {
    id: Uuid,
    kind: String,
    title: String,
    description: String,
    protected: bool,
    lifecycle: String,
    revision: i64,
    created_by_type: String,
    created_by_id: String,
    updated_by_type: String,
    updated_by_id: String,
    provenance: Value,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    archived_at: Option<OffsetDateTime>,
    relevance: f64,
    connection_count: i64,
}

#[derive(Clone, Debug, FromRow)]
struct ArtifactSearchCandidateRow {
    id: Uuid,
    kind: String,
    title: String,
    description: String,
    protected: bool,
    lifecycle: String,
    revision: i64,
    created_by_type: String,
    created_by_id: String,
    updated_by_type: String,
    updated_by_id: String,
    provenance: Value,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    archived_at: Option<OffsetDateTime>,
    relevance: f64,
    connection_count: i64,
    artifact_id: Uuid,
    start_offset: i32,
    end_offset: i32,
    excerpt: String,
    capture_outcome: String,
    match_kind: String,
}

#[derive(Clone, Debug, FromRow)]
struct ContextAnchorCandidateRow {
    id: Uuid,
    kind: String,
    title: String,
    description: String,
    protected: bool,
    lifecycle: String,
    revision: i64,
    created_by_type: String,
    created_by_id: String,
    updated_by_type: String,
    updated_by_id: String,
    provenance: Value,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    archived_at: Option<OffsetDateTime>,
    priority: i32,
    rationale: String,
}

impl From<ContextAnchorCandidateRow> for ContextAnchorCandidate {
    fn from(row: ContextAnchorCandidateRow) -> Self {
        Self {
            object: Object {
                id: row.id,
                kind: row.kind,
                title: row.title,
                description: row.description,
                protected: row.protected,
                lifecycle: row.lifecycle,
                revision: row.revision,
                created_by_type: row.created_by_type,
                created_by_id: row.created_by_id,
                updated_by_type: row.updated_by_type,
                updated_by_id: row.updated_by_id,
                provenance: row.provenance,
                created_at: row.created_at,
                updated_at: row.updated_at,
                archived_at: row.archived_at,
            },
            priority: row.priority,
            rationale: row.rationale,
        }
    }
}

impl From<SearchCandidateRow> for SearchCandidate {
    fn from(row: SearchCandidateRow) -> Self {
        Self {
            object: Object {
                id: row.id,
                kind: row.kind,
                title: row.title,
                description: row.description,
                protected: row.protected,
                lifecycle: row.lifecycle,
                revision: row.revision,
                created_by_type: row.created_by_type,
                created_by_id: row.created_by_id,
                updated_by_type: row.updated_by_type,
                updated_by_id: row.updated_by_id,
                provenance: row.provenance,
                created_at: row.created_at,
                updated_at: row.updated_at,
                archived_at: row.archived_at,
            },
            relevance: row.relevance,
            connection_count: row.connection_count,
            evidence: None,
        }
    }
}

impl From<ArtifactSearchCandidateRow> for SearchCandidate {
    fn from(row: ArtifactSearchCandidateRow) -> Self {
        Self {
            object: Object {
                id: row.id,
                kind: row.kind,
                title: row.title,
                description: row.description,
                protected: row.protected,
                lifecycle: row.lifecycle,
                revision: row.revision,
                created_by_type: row.created_by_type,
                created_by_id: row.created_by_id,
                updated_by_type: row.updated_by_type,
                updated_by_id: row.updated_by_id,
                provenance: row.provenance,
                created_at: row.created_at,
                updated_at: row.updated_at,
                archived_at: row.archived_at,
            },
            relevance: row.relevance,
            connection_count: row.connection_count,
            evidence: Some(SearchEvidence {
                artifact_id: row.artifact_id,
                start_offset: row.start_offset,
                end_offset: row.end_offset,
                excerpt: row.excerpt,
                capture_outcome: row.capture_outcome,
                match_kind: row.match_kind,
            }),
        }
    }
}

#[derive(Clone, Debug, FromRow)]
pub struct NeighborCandidate {
    pub seed_object_id: Uuid,
    pub connection_kind: String,
    pub connection_description: String,
    pub connection_count: i64,
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub description: String,
    pub protected: bool,
    pub lifecycle: String,
    pub revision: i64,
    pub created_by_type: String,
    pub created_by_id: String,
    pub updated_by_type: String,
    pub updated_by_id: String,
    pub provenance: Value,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub archived_at: Option<OffsetDateTime>,
}

impl NeighborCandidate {
    pub fn object(&self) -> Object {
        Object {
            id: self.id,
            kind: self.kind.clone(),
            title: self.title.clone(),
            description: self.description.clone(),
            protected: self.protected,
            lifecycle: self.lifecycle.clone(),
            revision: self.revision,
            created_by_type: self.created_by_type.clone(),
            created_by_id: self.created_by_id.clone(),
            updated_by_type: self.updated_by_type.clone(),
            updated_by_id: self.updated_by_id.clone(),
            provenance: self.provenance.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            archived_at: self.archived_at,
        }
    }
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct ContextConnection {
    pub id: Uuid,
    pub direction: String,
    pub kind: String,
    pub description: String,
    pub other_object_id: Uuid,
    pub other_object_kind: String,
    pub other_object_title: String,
}

#[derive(Clone, Debug, FromRow)]
pub struct EmbeddingJob {
    pub id: Uuid,
    pub object_id: Uuid,
    pub artifact_id: Option<Uuid>,
    pub chunk_index: Option<i32>,
    pub start_offset: Option<i32>,
    pub end_offset: Option<i32>,
    pub model: String,
    pub dimensions: i32,
    pub source_hash: String,
    pub format_version: String,
    pub input_mode: String,
    pub kind: String,
    pub title: String,
    pub description: String,
    pub artifact_content: Option<String>,
}

#[derive(Clone, Debug, FromRow)]
pub struct ArtifactEmbeddingSource {
    pub artifact_id: Uuid,
    pub object_id: Uuid,
    pub sha256: String,
    pub title: String,
    pub kind: String,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct ArtifactEmbeddingChunk {
    pub chunk_index: i32,
    pub start_offset: i32,
    pub end_offset: i32,
    pub source_hash: String,
}

#[derive(Clone, Debug)]
pub struct ObjectListFilter {
    pub query: Option<String>,
    pub kind: Option<String>,
    pub lifecycle: Option<String>,
    pub cursor: Option<Uuid>,
    pub limit: i64,
    pub text_search_config: crate::config::TextSearchConfig,
}

#[derive(Clone, Debug)]
pub struct SourceListFilter {
    pub query: Option<String>,
    pub source_kind: Option<String>,
    pub cursor: Option<Uuid>,
    pub limit: i64,
}

#[derive(Clone, Debug)]
pub struct NoteListFilter {
    pub query: Option<String>,
    pub cursor: Option<Uuid>,
    pub limit: i64,
}

#[derive(Clone, Debug)]
pub struct NewObject {
    pub kind: String,
    pub title: String,
    pub description: String,
    pub provenance: Value,
    pub entity_kind: Option<String>,
    pub happened_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug)]
pub struct NewSource {
    pub title: String,
    pub description: String,
    pub provenance: Value,
    pub source_kind: String,
    pub canonical_uri: Option<String>,
    pub byline: Option<String>,
    pub publisher: Option<String>,
    pub published_at: Option<OffsetDateTime>,
    pub published_at_precision: Option<String>,
    pub last_accessed_at: Option<OffsetDateTime>,
    pub original_language: Option<String>,
    pub original_media_type: Option<String>,
    pub original_artifact_reference: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NewNote {
    pub title: String,
    pub description: String,
    pub provenance: Value,
    pub content: String,
    pub content_format: String,
    pub originating_chat_object_id: Option<Uuid>,
    pub derived_from_source_object_ids: Vec<Uuid>,
}

#[derive(Clone, Debug)]
pub struct NewTheme {
    pub title: String,
    pub description: String,
    pub slug: String,
    pub provenance: Value,
    pub protected: bool,
}

#[derive(Clone, Debug, Default)]
pub struct NoteChanges {
    pub title: Option<String>,
    pub description: Option<String>,
    pub protected: Option<bool>,
    pub content: Option<String>,
    pub content_format: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct SourceChanges {
    pub title: Option<String>,
    pub description: Option<String>,
    pub provenance: Option<Value>,
    pub protected: Option<bool>,
    pub archive: bool,
    pub source_kind: Option<String>,
    pub canonical_uri: Option<Option<String>>,
    pub byline: Option<Option<String>>,
    pub publisher: Option<Option<String>>,
    pub published_at: Option<Option<OffsetDateTime>>,
    pub published_at_precision: Option<Option<String>>,
    pub last_accessed_at: Option<Option<OffsetDateTime>>,
    pub original_language: Option<Option<String>>,
    pub original_media_type: Option<Option<String>>,
    pub original_artifact_reference: Option<Option<String>>,
}

#[derive(Clone, Debug)]
pub struct NewArtifact {
    pub expected_revision: Option<i64>,
    pub kind: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub uri: Option<String>,
    pub media_type: Option<String>,
    pub language: Option<String>,
    pub captured_at: Option<OffsetDateTime>,
    pub capture_outcome: String,
    pub capture_reason: Option<String>,
    pub expected_size_bytes: Option<i64>,
    pub metadata: Value,
    pub supersedes_artifact_id: Option<Uuid>,
}

#[derive(Clone, Debug, Default)]
pub struct ObjectChanges {
    pub title: Option<String>,
    pub description: Option<String>,
    pub provenance: Option<Value>,
    pub protected: Option<bool>,
    pub archive: bool,
}

#[derive(Clone, Debug)]
pub struct NewConnection {
    pub source_object_id: Uuid,
    pub kind: String,
    pub target_object_id: Uuid,
    pub description: String,
    pub provenance: Value,
    pub protected: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ConnectionChanges {
    pub kind: Option<String>,
    pub description: Option<String>,
    pub provenance: Option<Value>,
    pub protected: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct TaskListFilter {
    pub status: Option<String>,
    pub agent_suitable: Option<bool>,
    pub limit: i64,
}

#[derive(Clone, Debug)]
pub struct NewTask {
    pub title: String,
    pub description: String,
    pub provenance: Value,
    pub status: String,
    pub priority: String,
    pub owner_object_id: Option<Uuid>,
    pub agent_suitable: bool,
    pub blocked_reason: Option<String>,
    pub due_at: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
    pub github_issue_url: Option<String>,
    pub brief_markdown: Option<String>,
    pub originating_chat_object_id: Option<Uuid>,
    pub derived_from_source_object_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Default)]
pub struct TaskChanges {
    pub title: Option<String>,
    pub description: Option<String>,
    pub provenance: Option<Value>,
    pub protected: Option<bool>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub owner_object_id: Option<Option<Uuid>>,
    pub agent_suitable: Option<bool>,
    pub blocked_reason: Option<Option<String>>,
    pub due_at: Option<Option<OffsetDateTime>>,
    pub completed_at: Option<Option<OffsetDateTime>>,
    pub github_issue_url: Option<Option<String>>,
    pub brief_markdown: Option<Option<String>>,
}

pub async fn migrate(pool: &PgPool) -> Result<(), DbError> {
    let database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(pool)
        .await?;
    if !allowed_database_name(&database) {
        return Err(DbError::Sqlx(sqlx::Error::Configuration(
            format!("refusing migrations against unexpected database {database:?}").into(),
        )));
    }
    sqlx::migrate!()
        .run(pool)
        .await
        .map_err(|error| sqlx::Error::Migrate(Box::new(error)))?;
    Ok(())
}

fn allowed_database_name(database: &str) -> bool {
    database == "centaur_context"
        || database == "centaur_context_enyu"
        || database.contains("centaur_context_test")
        || database == "centaur_os"
        || database.contains("centaur_os_test")
}

pub async fn ready(pool: &PgPool) -> Result<(), DbError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT to_regclass('public.objects') IS NOT NULL AND to_regclass('public.object_events') IS NOT NULL",
    )
    .fetch_one(pool)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(DbError::NotFound)
    }
}

pub async fn list_objects(pool: &PgPool, filter: ObjectListFilter) -> Result<Vec<Object>, DbError> {
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT id,kind,title,description,protected,CASE WHEN archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,revision,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance,created_at,updated_at,archived_at FROM objects WHERE true",
    );
    let kind_scoped = filter.kind.is_some();
    if let Some(kind) = filter.kind {
        query.push(" AND kind = ").push_bind(kind);
    }
    if let Some(cursor) = filter.cursor {
        if !kind_scoped {
            return Err(DbError::Validation(ValidationError::Unsupported {
                field: "cursor",
                value: "Object cursors require a kind filter".to_owned(),
            }));
        }
        query.push(" AND id > ").push_bind(cursor);
    }
    if let Some(lifecycle) = filter.lifecycle {
        if lifecycle == "active" {
            query.push(" AND archived_at IS NULL");
        } else {
            query.push(" AND archived_at IS NOT NULL");
        }
    }
    if let Some(search) = filter.query {
        query.push(" AND ");
        if filter.text_search_config == crate::config::TextSearchConfig::SIMPLE {
            query.push("search_document");
        } else {
            query
                .push("(setweight(to_tsvector(")
                .push_bind(filter.text_search_config.as_str())
                .push("::regconfig, coalesce(title,'')), 'A') || setweight(to_tsvector(")
                .push_bind(filter.text_search_config.as_str())
                .push("::regconfig, coalesce(description,'')), 'B'))");
        }
        query
            .push(" @@ websearch_to_tsquery(")
            .push_bind(filter.text_search_config.as_str())
            .push("::regconfig, ")
            .push_bind(search)
            .push(")");
    }
    if kind_scoped {
        query.push(" ORDER BY id");
    } else {
        query.push(" ORDER BY updated_at DESC, id");
    }
    query.push(" LIMIT ").push_bind(filter.limit);
    Ok(query.build_query_as().fetch_all(pool).await?)
}

pub async fn get_object(pool: &PgPool, id: Uuid) -> Result<Object, DbError> {
    sqlx::query_as(
        "SELECT id,kind,title,description,protected,CASE WHEN archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,revision,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance,created_at,updated_at,archived_at FROM objects WHERE id = $1",
    )
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
}

const SOURCE_SELECT: &str = r#"SELECT o.id AS object_id,o.title,o.description,CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,
       o.provenance,o.protected,s.source_kind,s.canonical_uri,s.byline,s.publisher,
       s.published_at,s.published_at_precision,s.last_accessed_at,s.original_language,
       s.original_media_type,s.original_artifact_reference,s.current_artifact_id,
       o.created_at,o.updated_at
FROM sources s JOIN objects o ON o.id=s.object_id"#;

pub async fn get_source(pool: &PgPool, id: Uuid) -> Result<Source, DbError> {
    sqlx::query_as(&format!("{SOURCE_SELECT} WHERE o.id=$1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn get_note(pool: &PgPool, id: Uuid) -> Result<Note, DbError> {
    sqlx::query_as(
        r#"SELECT o.id AS object_id,o.title,o.description,CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,
        o.provenance,o.protected,n.content,n.content_format,o.created_at,o.updated_at
        FROM notes n JOIN objects o ON o.id=n.object_id WHERE o.id=$1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(DbError::NotFound)
}

const THEME_SELECT: &str = r#"SELECT o.id AS object_id,o.title,o.description,t.slug,
       CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,o.provenance,o.protected,o.created_at,o.updated_at
FROM themes t JOIN objects o ON o.id=t.object_id"#;

pub async fn list_themes(pool: &PgPool) -> Result<Vec<Theme>, DbError> {
    Ok(sqlx::query_as(&format!(
        "{THEME_SELECT} WHERE o.archived_at IS NULL ORDER BY o.title,o.id"
    ))
    .fetch_all(pool)
    .await?)
}

pub async fn get_theme(pool: &PgPool, id: Uuid) -> Result<Theme, DbError> {
    sqlx::query_as(&format!("{THEME_SELECT} WHERE o.id=$1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn get_theme_by_slug(pool: &PgPool, slug: &str) -> Result<Theme, DbError> {
    sqlx::query_as(&format!("{THEME_SELECT} WHERE t.slug=$1"))
        .bind(slug)
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn create_theme(
    pool: &PgPool,
    actor: &ActorContext,
    mut input: NewTheme,
    idempotency_key: &str,
) -> Result<Theme, DbError> {
    if let Some(id) = idempotent_entity(pool, actor, idempotency_key).await? {
        return get_theme(pool, id).await;
    }
    validate_object_description(&input.title, &input.description)?;
    input.slug = theme_slug(input.slug)?;
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"INSERT INTO objects
        (id,kind,title,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance)
        VALUES ($1,'theme',$2,$3,$4,$5,$6,$5,$6,$7)"#,
    )
    .bind(id)
    .bind(&input.title)
    .bind(&input.description)
    .bind(input.protected)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(&input.provenance)
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO themes (object_id,slug) VALUES ($1,$2)")
        .bind(id)
        .bind(&input.slug)
        .execute(&mut *tx)
        .await?;
    insert_event(
        &mut tx,
        actor,
        "object",
        id,
        id,
        "created",
        Some(idempotency_key),
        None,
        1,
        json!({"kind":"theme","title":input.title,"slug":input.slug,"protected":input.protected}),
    )
    .await?;
    tx.commit().await?;
    get_theme(pool, id).await
}

pub async fn list_theme_objects(
    pool: &PgPool,
    theme_id: Uuid,
    kind: Option<&str>,
    limit: i64,
) -> Result<Vec<Object>, DbError> {
    let theme = get_theme(pool, theme_id).await?;
    if theme.lifecycle != "active" {
        return Err(DbError::NotFound);
    }
    let mut query = QueryBuilder::<Postgres>::new(
        r#"SELECT o.* FROM connections c
        JOIN objects o ON o.id=c.source_object_id
        WHERE c.kind='themed' AND c.archived_at IS NULL
          AND c.target_object_id="#,
    );
    query.push_bind(theme_id).push(" AND o.archived_at IS NULL");
    if let Some(kind) = kind {
        query.push(" AND o.kind=").push_bind(kind);
    }
    query
        .push(" ORDER BY o.updated_at DESC,o.id LIMIT ")
        .push_bind(limit);
    Ok(query.build_query_as().fetch_all(pool).await?)
}

pub async fn list_notes(
    pool: &PgPool,
    filter: NoteListFilter,
) -> Result<Vec<NoteSearchResult>, DbError> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"SELECT o.id AS object_id,o.title,o.description,
        CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,n.content_format,substring(n.content FROM 1 FOR 400) AS excerpt,o.updated_at
        FROM notes n JOIN objects o ON o.id=n.object_id WHERE o.archived_at IS NULL"#,
    );
    if let Some(cursor) = filter.cursor {
        query.push(" AND o.id>").push_bind(cursor);
    }
    if let Some(search) = filter.query {
        query.push(" AND to_tsvector('simple',concat_ws(' ',o.title,o.description,n.content)) @@ websearch_to_tsquery('simple',")
            .push_bind(search).push(")");
    }
    query.push(" ORDER BY o.id LIMIT ").push_bind(filter.limit);
    Ok(query.build_query_as().fetch_all(pool).await?)
}

pub async fn create_note(
    pool: &PgPool,
    actor: &ActorContext,
    input: NewNote,
    idempotency_key: &str,
) -> Result<Note, DbError> {
    if let Some(id) = idempotent_entity(pool, actor, idempotency_key).await? {
        reconcile_existing_note_links(pool, actor, id, &input, idempotency_key).await?;
        return get_note(pool, id).await;
    }
    validate_object_description(&input.title, &input.description)?;
    let originating_chat_object_id = validate_note_links(
        pool,
        actor,
        input.originating_chat_object_id,
        &input.derived_from_source_object_ids,
    )
    .await?;
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await?;
    sqlx::query(r#"INSERT INTO objects
        (id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance)
        VALUES ($1,'note',$2,$3,$4,$5,$4,$5,$6)"#)
        .bind(id).bind(&input.title).bind(&input.description).bind(actor.actor_type).bind(&actor.actor_id)
        .bind(&input.provenance).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO notes (object_id,content,content_format) VALUES ($1,$2,$3)")
        .bind(id)
        .bind(&input.content)
        .bind(&input.content_format)
        .execute(&mut *tx)
        .await?;
    let run_id = insert_event(&mut tx,actor,"object",id,id,"created",Some(idempotency_key),None,1,
        json!({"kind":"note","title":input.title,"content_format":input.content_format,"content_characters":input.content.chars().count()})).await?;

    let mut connection_ids = Vec::new();
    let mut sequence = 2_i64;
    if let Some(chat_object_id) = originating_chat_object_id {
        let connection_id = insert_note_connection(
            &mut tx,
            actor,
            chat_object_id,
            "about",
            id,
            "This conversation requested creation of the resulting Note.",
            &input.provenance,
        )
        .await?;
        insert_event_for_run(
            &mut tx,
            run_id,
            sequence,
            actor,
            "connection",
            connection_id,
            chat_object_id,
            "connected",
            None,
            None,
            1,
        )
        .await?;
        connection_ids.push(connection_id);
        sequence += 1;
    }
    for source_object_id in &input.derived_from_source_object_ids {
        let connection_id = insert_note_connection(
            &mut tx,
            actor,
            id,
            "derived_from",
            *source_object_id,
            "This Note records an observation derived from the linked Source.",
            &input.provenance,
        )
        .await?;
        insert_event_for_run(
            &mut tx,
            run_id,
            sequence,
            actor,
            "connection",
            connection_id,
            id,
            "connected",
            None,
            None,
            1,
        )
        .await?;
        connection_ids.push(connection_id);
        sequence += 1;
    }
    sqlx::query(
        r#"UPDATE runs
           SET chat_object_id=$2,primary_object_id=$3,
               result=result||jsonb_build_object('connection_ids',$4::uuid[]),updated_at=now()
           WHERE id=$1"#,
    )
    .bind(run_id)
    .bind(originating_chat_object_id)
    .bind(id)
    .bind(&connection_ids)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    get_note(pool, id).await
}

async fn reconcile_existing_note_links(
    pool: &PgPool,
    actor: &ActorContext,
    note_object_id: Uuid,
    input: &NewNote,
    idempotency_key: &str,
) -> Result<(), DbError> {
    let chat_object_id = validate_note_links(
        pool,
        actor,
        input.originating_chat_object_id,
        &input.derived_from_source_object_ids,
    )
    .await?;
    let mut requested = Vec::new();
    if let Some(chat_id) = chat_object_id {
        requested.push((
            chat_id,
            "about",
            note_object_id,
            "This conversation requested creation of the resulting Note.",
        ));
    }
    for source_id in &input.derived_from_source_object_ids {
        requested.push((
            note_object_id,
            "derived_from",
            *source_id,
            "This Note records an observation derived from the linked Source.",
        ));
    }
    let mut missing = Vec::new();
    for item in requested {
        let exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM connections
               WHERE source_object_id=$1 AND kind=$2 AND target_object_id=$3
                 AND archived_at IS NULL)"#,
        )
        .bind(item.0)
        .bind(item.1)
        .bind(item.2)
        .fetch_one(pool)
        .await?;
        if !exists {
            missing.push(item);
        }
    }
    if missing.is_empty() {
        return Ok(());
    }

    let mut tx = pool.begin().await?;
    let run_id = Uuid::new_v4();
    let reconciliation_key = format!("{idempotency_key}:note-links-v1");
    sqlx::query(
        r#"INSERT INTO runs
           (id,kind,status,actor_type,actor_id,chat_object_id,primary_object_id,
            idempotency_key,input,result,completed_at)
           VALUES ($1,'mutation','completed',$2,$3,$4,$5,$6,$7,$8,now())"#,
    )
    .bind(run_id)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(chat_object_id)
    .bind(note_object_id)
    .bind(format!(
        "{}:{}:{}",
        actor.actor_type, actor.actor_id, reconciliation_key
    ))
    .bind(json!({
        "centaur_thread_key":actor.centaur_thread_key,
        "centaur_execution_id":actor.centaur_execution_id,
        "target_type":"object",
        "target_id":note_object_id,
        "action":"linked"
    }))
    .bind(json!({"affected_object_ids":[note_object_id]}))
    .execute(&mut *tx)
    .await?;

    let mut connection_ids = Vec::new();
    for (index, (source_id, kind, target_id, description)) in missing.into_iter().enumerate() {
        let connection_id = insert_note_connection(
            &mut tx,
            actor,
            source_id,
            kind,
            target_id,
            description,
            &input.provenance,
        )
        .await?;
        insert_event_for_run(
            &mut tx,
            run_id,
            index as i64 + 1,
            actor,
            "connection",
            connection_id,
            source_id,
            "connected",
            None,
            None,
            1,
        )
        .await?;
        connection_ids.push(connection_id);
    }
    sqlx::query(
        "UPDATE runs SET result=result||jsonb_build_object('connection_ids',$2::uuid[]) WHERE id=$1",
    )
    .bind(run_id)
    .bind(&connection_ids)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn validate_note_links(
    pool: &PgPool,
    actor: &ActorContext,
    originating_chat_object_id: Option<Uuid>,
    source_object_ids: &[Uuid],
) -> Result<Option<Uuid>, DbError> {
    let resolved_chat_object_id = match originating_chat_object_id {
        Some(id) => Some(id),
        None => resolve_actor_chat(pool, actor).await?,
    };
    if let Some(chat_object_id) = resolved_chat_object_id {
        let valid: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(
               SELECT 1 FROM objects o JOIN chats c ON c.object_id=o.id
               WHERE o.id=$1 AND o.archived_at IS NULL
            )"#,
        )
        .bind(chat_object_id)
        .fetch_one(pool)
        .await?;
        if !valid {
            return Err(DbError::Invalid(
                "originating_chat_object_id must identify an active Chat".into(),
            ));
        }
    }
    let unique_source_ids = source_object_ids.iter().copied().collect::<HashSet<_>>();
    if unique_source_ids.len() != source_object_ids.len() {
        return Err(DbError::Invalid(
            "derived_from_source_object_ids must not contain duplicates".into(),
        ));
    }
    if !source_object_ids.is_empty() {
        let valid_count: i64 = sqlx::query_scalar(
            r#"SELECT count(*) FROM objects o JOIN sources s ON s.object_id=o.id
               WHERE o.id=ANY($1) AND o.archived_at IS NULL"#,
        )
        .bind(source_object_ids)
        .fetch_one(pool)
        .await?;
        if valid_count != source_object_ids.len() as i64 {
            return Err(DbError::Invalid(
                "derived_from_source_object_ids must identify active Sources".into(),
            ));
        }
    }
    Ok(resolved_chat_object_id)
}

async fn resolve_actor_chat(pool: &PgPool, actor: &ActorContext) -> Result<Option<Uuid>, DbError> {
    let Some(thread_key) = actor.centaur_thread_key.as_deref() else {
        return Ok(None);
    };
    let parts = thread_key.split(':').map(str::trim).collect::<Vec<_>>();
    if parts.len() < 4 || parts.iter().any(|part| part.is_empty()) {
        return Err(DbError::Invalid(
            "authenticated thread key cannot be mapped to a Chat".into(),
        ));
    }
    let provider = parts[0].to_ascii_lowercase();
    let workspace_id = parts[1];
    let channel_id = parts[parts.len() - 2];
    let thread_id = parts[parts.len() - 1];
    Ok(sqlx::query_scalar(
        r#"SELECT c.object_id FROM chats c JOIN objects o ON o.id=c.object_id
           WHERE lower(c.provider)=$1 AND c.workspace_id=$2 AND c.channel_id=$3
             AND c.thread_id=$4 AND o.archived_at IS NULL"#,
    )
    .bind(provider)
    .bind(workspace_id)
    .bind(channel_id)
    .bind(thread_id)
    .fetch_optional(pool)
    .await?)
}

async fn insert_note_connection(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    source_object_id: Uuid,
    kind: &str,
    target_object_id: Uuid,
    description: &str,
    provenance: &Value,
) -> Result<Uuid, DbError> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO connections
           (id,source_object_id,kind,target_object_id,description,
            created_by_type,created_by_id,updated_by_type,updated_by_id,provenance,protected)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$6,$7,$8,true)"#,
    )
    .bind(id)
    .bind(source_object_id)
    .bind(kind)
    .bind(target_object_id)
    .bind(description)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(provenance)
    .execute(&mut **tx)
    .await?;
    Ok(id)
}

pub async fn update_note(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    expected_revision: i64,
    changes: NoteChanges,
    idempotency_key: Option<&str>,
) -> Result<Note, DbError> {
    if let Some(key) = idempotency_key
        && let Some(existing_id) = idempotent_entity(pool, actor, key).await?
    {
        return get_note(pool, existing_id).await;
    }
    let current = get_note(pool, id).await?;
    let title = changes.title.unwrap_or_else(|| current.title.clone());
    let description = changes
        .description
        .unwrap_or_else(|| current.description.clone());
    validate_object_description(&title, &description)?;
    let protected = changes.protected.unwrap_or(current.protected);
    let content = changes.content.unwrap_or_else(|| current.content.clone());
    let content_format = changes
        .content_format
        .unwrap_or_else(|| current.content_format.clone());
    let mut tx = pool.begin().await?;
    let updated_revision: Option<i64> = sqlx::query_scalar(
        r#"UPDATE objects SET title=$3,description=$4,protected=$5,revision=revision+1,
           updated_by_type=$6,updated_by_id=$7,updated_at=now()
           WHERE id=$1 AND kind='note' AND revision=$2 AND archived_at IS NULL
           RETURNING revision"#,
    )
    .bind(id)
    .bind(expected_revision)
    .bind(&title)
    .bind(&description)
    .bind(protected)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .fetch_optional(&mut *tx)
    .await?;
    let updated_revision = updated_revision.ok_or(DbError::Conflict)?;
    sqlx::query("UPDATE notes SET content=$2,content_format=$3 WHERE object_id=$1")
        .bind(id)
        .bind(&content)
        .bind(&content_format)
        .execute(&mut *tx)
        .await?;
    insert_event(
        &mut tx,
        actor,
        "object",
        id,
        id,
        "updated",
        idempotency_key,
        Some(expected_revision),
        updated_revision,
        json!({
            "kind":"note",
            "title":title,
            "description_changed":description != current.description,
            "content_changed":content != current.content,
            "content_format":content_format,
            "content_characters":content.chars().count()
        }),
    )
    .await?;
    tx.commit().await?;
    get_note(pool, id).await
}

pub async fn list_sources(
    pool: &PgPool,
    filter: SourceListFilter,
) -> Result<Vec<SourceSearchResult>, DbError> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"SELECT o.id AS object_id,o.title,o.description,CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,
           o.provenance,o.protected,s.source_kind,s.canonical_uri,s.byline,s.publisher,
           s.published_at,s.published_at_precision,s.last_accessed_at,s.original_language,
           s.original_media_type,s.original_artifact_reference,s.current_artifact_id,
           o.created_at,o.updated_at,
           CASE WHEN sc.id IS NULL THEN NULL ELSE substring(sc.content FROM 1 FOR 400) END AS excerpt
           FROM sources s JOIN objects o ON o.id=s.object_id
           LEFT JOIN artifacts sc ON sc.id=s.current_artifact_id
           WHERE o.archived_at IS NULL"#,
    );
    if let Some(kind) = filter.source_kind {
        query.push(" AND s.source_kind=").push_bind(kind);
    }
    if let Some(cursor) = filter.cursor {
        query.push(" AND o.id>").push_bind(cursor);
    }
    if let Some(search) = filter.query {
        query.push(
            " AND to_tsvector('simple',concat_ws(' ',o.title,o.description,s.byline,s.publisher,sc.content)) @@ websearch_to_tsquery('simple',",
        )
        .push_bind(search)
        .push(")");
    }
    query.push(" ORDER BY o.id LIMIT ").push_bind(filter.limit);
    Ok(query.build_query_as().fetch_all(pool).await?)
}

pub async fn create_source(
    pool: &PgPool,
    actor: &ActorContext,
    input: NewSource,
    idempotency_key: &str,
) -> Result<Source, DbError> {
    if let Some(id) = idempotent_entity(pool, actor, idempotency_key).await? {
        return get_source(pool, id).await;
    }
    validate_object_description(&input.title, &input.description)?;
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"INSERT INTO objects
           (id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance)
           VALUES ($1,'source',$2,$3,$4,$5,$4,$5,$6)"#,
    )
    .bind(id)
    .bind(&input.title)
    .bind(&input.description)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(&input.provenance)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"INSERT INTO sources
           (object_id,source_kind,canonical_uri,byline,publisher,published_at,
            published_at_precision,last_accessed_at,original_language,
            original_media_type,original_artifact_reference)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)"#,
    )
    .bind(id)
    .bind(&input.source_kind)
    .bind(&input.canonical_uri)
    .bind(&input.byline)
    .bind(&input.publisher)
    .bind(input.published_at)
    .bind(&input.published_at_precision)
    .bind(input.last_accessed_at)
    .bind(&input.original_language)
    .bind(&input.original_media_type)
    .bind(&input.original_artifact_reference)
    .execute(&mut *tx)
    .await?;
    insert_event(
        &mut tx,
        actor,
        "object",
        id,
        id,
        "created",
        Some(idempotency_key),
        None,
        1,
        json!({"kind":"source","title":input.title,"source_kind":input.source_kind}),
    )
    .await?;
    tx.commit().await?;
    get_source(pool, id).await
}

pub async fn update_source(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    expected_revision: i64,
    changes: SourceChanges,
    idempotency_key: Option<&str>,
) -> Result<Source, DbError> {
    if let Some(key) = idempotency_key
        && let Some(existing_id) = idempotent_entity(pool, actor, key).await?
    {
        return get_source(pool, existing_id).await;
    }
    let current = get_source(pool, id).await?;
    let title = changes.title.unwrap_or_else(|| current.title.clone());
    let description = changes
        .description
        .unwrap_or_else(|| current.description.clone());
    validate_object_description(&title, &description)?;
    let provenance = changes
        .provenance
        .unwrap_or_else(|| current.provenance.clone());
    let protected = changes.protected.unwrap_or(current.protected);
    let canonical_uri = changes.canonical_uri.unwrap_or(current.canonical_uri);
    let byline = changes.byline.unwrap_or(current.byline);
    let publisher = changes.publisher.unwrap_or(current.publisher);
    let published_at = changes.published_at.unwrap_or(current.published_at);
    let published_at_precision = changes
        .published_at_precision
        .unwrap_or(current.published_at_precision);
    let last_accessed_at = changes.last_accessed_at.unwrap_or(current.last_accessed_at);
    let original_language = changes
        .original_language
        .unwrap_or(current.original_language);
    let original_media_type = changes
        .original_media_type
        .unwrap_or(current.original_media_type);
    let original_artifact_reference = changes
        .original_artifact_reference
        .unwrap_or(current.original_artifact_reference);
    let lifecycle = if changes.archive {
        "archived"
    } else {
        &current.lifecycle
    };
    let archived_at = changes.archive.then(OffsetDateTime::now_utc);
    let mut tx = pool.begin().await?;
    let updated_revision: Option<i64> = sqlx::query_scalar(
        r#"UPDATE objects SET title=$3,description=$4,provenance=$5,protected=$6,
           archived_at=CASE WHEN $7 THEN COALESCE(archived_at,$8) ELSE archived_at END,
           revision=revision+1,updated_by_type=$9,updated_by_id=$10,updated_at=now()
           WHERE id=$1 AND kind='source' AND revision=$2 RETURNING revision"#,
    )
    .bind(id)
    .bind(expected_revision)
    .bind(&title)
    .bind(&description)
    .bind(&provenance)
    .bind(protected)
    .bind(changes.archive)
    .bind(archived_at)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .fetch_optional(&mut *tx)
    .await?;
    let updated_revision = updated_revision.ok_or(DbError::Conflict)?;
    sqlx::query(
        r#"UPDATE sources SET source_kind=COALESCE($2,source_kind),canonical_uri=$3,
           byline=$4,publisher=$5,published_at=$6,published_at_precision=$7,
           last_accessed_at=$8,original_language=$9,original_media_type=$10,
           original_artifact_reference=$11
           WHERE object_id=$1"#,
    )
    .bind(id)
    .bind(&changes.source_kind)
    .bind(&canonical_uri)
    .bind(&byline)
    .bind(&publisher)
    .bind(published_at)
    .bind(&published_at_precision)
    .bind(last_accessed_at)
    .bind(&original_language)
    .bind(&original_media_type)
    .bind(&original_artifact_reference)
    .execute(&mut *tx)
    .await?;
    insert_event(
        &mut tx,
        actor,
        "object",
        id,
        id,
        if changes.archive {
            "archived"
        } else {
            "updated"
        },
        idempotency_key,
        Some(expected_revision),
        updated_revision,
        json!({"kind":"source","metadata_changed":true,"lifecycle":lifecycle}),
    )
    .await?;
    tx.commit().await?;
    get_source(pool, id).await
}

pub async fn list_artifacts(pool: &PgPool, object_id: Uuid) -> Result<Vec<Artifact>, DbError> {
    get_object(pool, object_id).await?;
    Ok(sqlx::query_as(
        "SELECT * FROM artifacts WHERE object_id=$1 ORDER BY created_at DESC,id DESC",
    )
    .bind(object_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_artifact_window(
    pool: &PgPool,
    object_id: Uuid,
    artifact_id: Option<Uuid>,
    offset: i64,
    limit: i64,
) -> Result<ArtifactWindow, DbError> {
    let artifact: Artifact = if let Some(artifact_id) = artifact_id {
        sqlx::query_as("SELECT * FROM artifacts WHERE object_id=$1 AND id=$2")
            .bind(object_id).bind(artifact_id).fetch_optional(pool).await?
    } else {
        sqlx::query_as("SELECT sc.* FROM sources s JOIN artifacts sc ON sc.id=s.current_artifact_id WHERE s.object_id=$1")
            .bind(object_id).fetch_optional(pool).await?
    }.ok_or(DbError::NotFound)?;
    artifact_window(artifact, offset, limit)
}

pub async fn get_artifact_window_by_id(
    pool: &PgPool,
    artifact_id: Uuid,
    offset: i64,
    limit: i64,
) -> Result<ArtifactWindow, DbError> {
    let artifact = sqlx::query_as("SELECT * FROM artifacts WHERE id=$1")
        .bind(artifact_id)
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)?;
    artifact_window(artifact, offset, limit)
}

fn artifact_window(artifact: Artifact, offset: i64, limit: i64) -> Result<ArtifactWindow, DbError> {
    let body = artifact.content.as_deref().ok_or(DbError::NotFound)?;
    let total = body.chars().count() as i64;
    if offset > total {
        return Err(DbError::Validation(ValidationError::Unsupported {
            field: "offset",
            value: offset.to_string(),
        }));
    }
    let text: String = body
        .chars()
        .skip(offset as usize)
        .take(limit as usize)
        .collect();
    let end = offset + text.chars().count() as i64;
    Ok(ArtifactWindow {
        content: artifact,
        text,
        offset,
        next_offset: (end < total).then_some(end),
    })
}

pub async fn append_artifact(
    pool: &PgPool,
    actor: &ActorContext,
    object_id: Uuid,
    input: NewArtifact,
    idempotency_key: &str,
) -> Result<Artifact, DbError> {
    if let Some(id) = idempotent_entity(pool, actor, idempotency_key).await? {
        return sqlx::query_as("SELECT * FROM artifacts WHERE id=$1")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or(DbError::NotFound);
    }
    if input.content.as_deref().is_none_or(str::is_empty)
        && input.uri.as_deref().is_none_or(str::is_empty)
    {
        return Err(DbError::Validation(ValidationError::Required(
            "content_or_uri",
        )));
    }
    if input.capture_outcome == "complete" {
        if input.content.is_none() || input.capture_reason.is_some() {
            return Err(DbError::Invalid(
                "a complete Artifact requires content and no capture reason".into(),
            ));
        }
    } else if input.capture_reason.is_none() {
        return Err(DbError::Invalid(
            "a non-complete Artifact requires a capture reason".into(),
        ));
    }
    if input.expected_size_bytes.is_some_and(|value| value <= 0) {
        return Err(DbError::Invalid(
            "expected Artifact size must be positive".into(),
        ));
    }
    let id = Uuid::new_v4();
    let bytes = input
        .content
        .as_deref()
        .unwrap_or_else(|| input.uri.as_deref().unwrap())
        .as_bytes();
    let sha256 = format!("{:x}", Sha256::digest(bytes));
    let size_bytes = bytes.len() as i64;
    let mut tx = pool.begin().await?;
    let current_revision: Option<i64> = sqlx::query_scalar(
        "SELECT revision FROM objects WHERE id=$1 AND archived_at IS NULL FOR UPDATE",
    )
    .bind(object_id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(existing) =
        sqlx::query_as("SELECT * FROM artifacts WHERE object_id=$1 AND sha256=$2")
            .bind(object_id)
            .bind(&sha256)
            .fetch_optional(&mut *tx)
            .await?
    {
        tx.commit().await?;
        return Ok(existing);
    }
    if current_revision.is_none()
        || input
            .expected_revision
            .is_some_and(|revision| Some(revision) != current_revision)
    {
        return Err(DbError::Conflict);
    }
    if let Some(superseded) = input.supersedes_artifact_id {
        let owns_superseded: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM artifacts WHERE id=$1 AND object_id=$2)",
        )
        .bind(superseded)
        .bind(object_id)
        .fetch_one(&mut *tx)
        .await?;
        if !owns_superseded {
            return Err(DbError::Invalid(
                "superseded Artifact belongs to another Object".into(),
            ));
        }
    }
    let artifact: Artifact = sqlx::query_as(
        r#"INSERT INTO artifacts
           (id,object_id,kind,title,content,uri,media_type,language,sha256,size_bytes,
            capture_outcome,capture_reason,expected_size_bytes,metadata,
            supersedes_artifact_id,captured_at)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16) RETURNING *"#,
    )
    .bind(id)
    .bind(object_id)
    .bind(&input.kind)
    .bind(&input.title)
    .bind(&input.content)
    .bind(&input.uri)
    .bind(&input.media_type)
    .bind(&input.language)
    .bind(&sha256)
    .bind(size_bytes)
    .bind(&input.capture_outcome)
    .bind(&input.capture_reason)
    .bind(input.expected_size_bytes)
    .bind(&input.metadata)
    .bind(input.supersedes_artifact_id)
    .bind(input.captured_at)
    .fetch_one(&mut *tx)
    .await?;
    if input.capture_outcome == "complete" {
        sqlx::query("UPDATE sources SET current_artifact_id=$2 WHERE object_id=$1")
            .bind(object_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    let revision: i64 = sqlx::query_scalar(
        "UPDATE objects SET revision=revision+1,updated_by_type=$2,updated_by_id=$3,updated_at=now() WHERE id=$1 RETURNING revision",
    ).bind(object_id).bind(actor.actor_type).bind(&actor.actor_id).fetch_one(&mut *tx).await?;
    insert_event(
        &mut tx,
        actor,
        "object",
        object_id,
        object_id,
        "artifact_attached",
        Some(idempotency_key),
        current_revision,
        revision,
        json!({"artifact_id":id,"kind":input.kind,"sha256":sha256,"size_bytes":size_bytes,
            "capture_outcome":input.capture_outcome}),
    )
    .await?;
    tx.commit().await?;
    Ok(artifact)
}

pub async fn create_object(
    pool: &PgPool,
    actor: &ActorContext,
    input: NewObject,
    idempotency_key: &str,
) -> Result<Object, DbError> {
    if let Some(id) = idempotent_entity(pool, actor, idempotency_key).await? {
        return get_object(pool, id).await;
    }
    validate_object_description(&input.title, &input.description)?;
    match input.kind.as_str() {
        "chat" if input.entity_kind.is_none() && input.happened_at.is_none() => {}
        "entity" if input.entity_kind.is_some() && input.happened_at.is_none() => {}
        "memory" if input.entity_kind.is_none() && input.happened_at.is_some() => {}
        "chat" | "entity" | "memory" => {
            return Err(DbError::Invalid(
                "typed Object creation fields do not match its kind".into(),
            ));
        }
        _ => {
            return Err(DbError::Invalid(
                "use the typed creation contract for this Object kind".into(),
            ));
        }
    }
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await?;
    let object: Object = sqlx::query_as(
        r#"INSERT INTO objects
           (id, kind, title, description, created_by_type, created_by_id, updated_by_type, updated_by_id, provenance)
           VALUES ($1,$2,$3,$4,$5,$6,$5,$6,$7) RETURNING *"#,
    )
    .bind(id)
    .bind(&input.kind)
    .bind(&input.title)
    .bind(&input.description)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(&input.provenance)
    .fetch_one(&mut *tx)
    .await?;
    insert_object_subtype(&mut tx, id, &input).await?;
    insert_event(
        &mut tx,
        actor,
        "object",
        id,
        id,
        "created",
        Some(idempotency_key),
        None,
        1,
        json!({"kind": input.kind, "title": input.title}),
    )
    .await?;
    tx.commit().await?;
    Ok(object)
}

async fn insert_object_subtype(
    tx: &mut Transaction<'_, Postgres>,
    object_id: Uuid,
    input: &NewObject,
) -> Result<(), DbError> {
    match input.kind.as_str() {
        "chat" => {
            sqlx::query("INSERT INTO chats (object_id) VALUES ($1)")
                .bind(object_id)
                .execute(&mut **tx)
                .await?;
        }
        "entity" => {
            sqlx::query("INSERT INTO entities (object_id,entity_kind) VALUES ($1,$2)")
                .bind(object_id)
                .bind(input.entity_kind.as_deref().expect("validated entity kind"))
                .execute(&mut **tx)
                .await?;
        }
        "memory" => {
            sqlx::query("INSERT INTO memories (object_id,happened_at) VALUES ($1,$2)")
                .bind(object_id)
                .bind(input.happened_at.expect("validated Memory time"))
                .execute(&mut **tx)
                .await?;
        }
        _ => unreachable!("Object kind validated before subtype insertion"),
    }
    Ok(())
}

pub async fn update_object(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    expected_revision: i64,
    changes: ObjectChanges,
    idempotency_key: Option<&str>,
) -> Result<Object, DbError> {
    if let Some(key) = idempotency_key
        && let Some(existing_id) = idempotent_entity(pool, actor, key).await?
    {
        return get_object(pool, existing_id).await;
    }
    let current = get_object(pool, id).await?;
    let title = changes.title.unwrap_or_else(|| current.title.clone());
    let description = changes
        .description
        .unwrap_or_else(|| current.description.clone());
    validate_object_description(&title, &description)?;
    let provenance = changes
        .provenance
        .unwrap_or_else(|| current.provenance.clone());
    let protected = changes.protected.unwrap_or(current.protected);
    let lifecycle = if changes.archive {
        "archived"
    } else {
        &current.lifecycle
    };
    let archived_at = if changes.archive {
        Some(OffsetDateTime::now_utc())
    } else {
        current.archived_at
    };
    let mut tx = pool.begin().await?;
    let updated: Option<Object> = sqlx::query_as(
        r#"UPDATE objects SET title=$3, description=$4, provenance=$5, protected=$6,
           archived_at=$7, revision=revision+1, updated_by_type=$8, updated_by_id=$9,
           updated_at=now() WHERE id=$1 AND revision=$2
           RETURNING id,kind,title,description,protected,
             CASE WHEN archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,
             revision,created_by_type,created_by_id,updated_by_type,updated_by_id,
             provenance,created_at,updated_at,archived_at"#,
    )
    .bind(id)
    .bind(expected_revision)
    .bind(&title)
    .bind(&description)
    .bind(&provenance)
    .bind(protected)
    .bind(archived_at)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .fetch_optional(&mut *tx)
    .await?;
    let updated = updated.ok_or(DbError::Conflict)?;
    insert_event(
        &mut tx,
        actor,
        "object",
        id,
        id,
        if changes.archive {
            "archived"
        } else {
            "updated"
        },
        idempotency_key,
        Some(expected_revision),
        updated.revision,
        json!({"title": title, "description_changed": description != current.description, "protected": protected, "lifecycle": lifecycle}),
    )
    .await?;
    tx.commit().await?;
    Ok(updated)
}

pub async fn list_connections(pool: &PgPool, object_id: Uuid) -> Result<Vec<Connection>, DbError> {
    Ok(sqlx::query_as(
        "SELECT * FROM connections WHERE archived_at IS NULL AND (source_object_id=$1 OR target_object_id=$1) ORDER BY updated_at DESC, id",
    )
    .bind(object_id)
    .fetch_all(pool)
    .await?)
}

pub async fn connection_graph(pool: &PgPool) -> Result<ConnectionGraphSnapshot, DbError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let nodes = sqlx::query_as::<_, ConnectionGraphNode>(
        "SELECT id,kind,title FROM objects WHERE archived_at IS NULL ORDER BY id",
    )
    .fetch_all(&mut *tx)
    .await?;
    let edges = sqlx::query_as::<_, ConnectionGraphEdge>(
        r#"SELECT c.id,c.source_object_id,c.target_object_id,c.kind,c.description
           FROM connections c
           JOIN objects source ON source.id=c.source_object_id AND source.archived_at IS NULL
           JOIN objects target ON target.id=c.target_object_id AND target.archived_at IS NULL
           WHERE c.archived_at IS NULL
           ORDER BY c.id"#,
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let fingerprint = connection_graph_fingerprint(&nodes, &edges);
    Ok(ConnectionGraphSnapshot {
        fingerprint,
        node_count: nodes.len(),
        connection_count: edges.len(),
        nodes,
        edges,
    })
}

fn connection_graph_fingerprint(
    nodes: &[ConnectionGraphNode],
    edges: &[ConnectionGraphEdge],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"centaur-connection-graph-v1\0");
    for node in nodes {
        hasher.update(node.id.as_bytes());
        hasher.update([0]);
        hasher.update(node.kind.as_bytes());
        hasher.update([0]);
        hasher.update(node.title.as_bytes());
        hasher.update([0xff]);
    }
    for edge in edges {
        hasher.update(edge.id.as_bytes());
        hasher.update(edge.source_object_id.as_bytes());
        hasher.update(edge.target_object_id.as_bytes());
        hasher.update([0]);
        hasher.update(edge.kind.as_bytes());
        hasher.update([0]);
        hasher.update(edge.description.as_bytes());
        hasher.update([0xff]);
    }
    format!("{:x}", hasher.finalize())
}

pub async fn get_connection(pool: &PgPool, id: Uuid) -> Result<Connection, DbError> {
    sqlx::query_as("SELECT * FROM connections WHERE id=$1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn create_connection(
    pool: &PgPool,
    actor: &ActorContext,
    input: NewConnection,
    idempotency_key: &str,
) -> Result<Connection, DbError> {
    validate_connection_endpoints(
        pool,
        input.source_object_id,
        &input.kind,
        input.target_object_id,
    )
    .await?;
    if let Some(id) = idempotent_entity(pool, actor, idempotency_key).await? {
        return sqlx::query_as("SELECT * FROM connections WHERE id=$1")
            .bind(id)
            .fetch_one(pool)
            .await
            .map_err(DbError::from);
    }
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await?;
    let connection: Connection = sqlx::query_as(
        r#"INSERT INTO connections
           (id, source_object_id, kind, target_object_id, description,
            created_by_type, created_by_id, updated_by_type, updated_by_id, provenance, protected)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$6,$7,$8,$9) RETURNING *"#,
    )
    .bind(id)
    .bind(input.source_object_id)
    .bind(&input.kind)
    .bind(input.target_object_id)
    .bind(&input.description)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(&input.provenance)
    .bind(input.protected)
    .fetch_one(&mut *tx)
    .await?;
    insert_event(
        &mut tx,
        actor,
        "connection",
        id,
        input.source_object_id,
        "connected",
        Some(idempotency_key),
        None,
        1,
        json!({"kind": input.kind, "target_object_id": input.target_object_id, "description": input.description, "protected": input.protected}),
    )
    .await?;
    tx.commit().await?;
    Ok(connection)
}

pub async fn update_connection(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    expected_revision: i64,
    changes: ConnectionChanges,
    idempotency_key: Option<&str>,
) -> Result<Connection, DbError> {
    if let Some(key) = idempotency_key
        && let Some(existing_id) = idempotent_entity(pool, actor, key).await?
    {
        return sqlx::query_as("SELECT * FROM connections WHERE id=$1")
            .bind(existing_id)
            .fetch_one(pool)
            .await
            .map_err(DbError::from);
    }
    let current: Connection =
        sqlx::query_as("SELECT * FROM connections WHERE id=$1 AND archived_at IS NULL")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or(DbError::NotFound)?;
    let kind = changes.kind.unwrap_or_else(|| current.kind.clone());
    validate_connection_endpoints(
        pool,
        current.source_object_id,
        &kind,
        current.target_object_id,
    )
    .await?;
    let description = changes
        .description
        .unwrap_or_else(|| current.description.clone());
    let provenance = changes
        .provenance
        .unwrap_or_else(|| current.provenance.clone());
    let protected = changes.protected.unwrap_or(current.protected);
    let mut tx = pool.begin().await?;
    let updated: Option<Connection> = sqlx::query_as(
        r#"UPDATE connections
           SET kind=$3,description=$4,provenance=$5,protected=$6,
               revision=revision+1,updated_by_type=$7,updated_by_id=$8,updated_at=now()
           WHERE id=$1 AND revision=$2 AND archived_at IS NULL RETURNING *"#,
    )
    .bind(id)
    .bind(expected_revision)
    .bind(&kind)
    .bind(&description)
    .bind(&provenance)
    .bind(protected)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .fetch_optional(&mut *tx)
    .await?;
    let updated = updated.ok_or(DbError::Conflict)?;
    insert_event(
        &mut tx,
        actor,
        "connection",
        id,
        current.source_object_id,
        "updated",
        idempotency_key,
        Some(expected_revision),
        updated.revision,
        json!({"kind": kind, "description": description, "protected": protected}),
    )
    .await?;
    tx.commit().await?;
    Ok(updated)
}

async fn validate_connection_endpoints(
    pool: &PgPool,
    source_object_id: Uuid,
    kind: &str,
    target_object_id: Uuid,
) -> Result<(), DbError> {
    if kind != "themed" {
        return Ok(());
    }
    let rows: Vec<(Uuid, String, Option<OffsetDateTime>)> =
        sqlx::query_as("SELECT id,kind,archived_at FROM objects WHERE id=$1 OR id=$2")
            .bind(source_object_id)
            .bind(target_object_id)
            .fetch_all(pool)
            .await?;
    let source = rows.iter().find(|row| row.0 == source_object_id);
    let target = rows.iter().find(|row| row.0 == target_object_id);
    if source.is_none_or(|row| row.2.is_some()) || target.is_none_or(|row| row.2.is_some()) {
        return Err(DbError::Invalid(
            "themed connections require active source and target Objects".into(),
        ));
    }
    if source.is_some_and(|row| row.1 == "theme") || target.is_none_or(|row| row.1 != "theme") {
        return Err(DbError::Invalid(
            "themed connections must point from a non-Theme Object to a Theme".into(),
        ));
    }
    Ok(())
}

pub async fn archive_connection(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    expected_revision: i64,
    idempotency_key: Option<&str>,
) -> Result<Connection, DbError> {
    if let Some(key) = idempotency_key
        && let Some(existing_id) = idempotent_entity(pool, actor, key).await?
    {
        return sqlx::query_as("SELECT * FROM connections WHERE id=$1")
            .bind(existing_id)
            .fetch_one(pool)
            .await
            .map_err(DbError::from);
    }
    let mut tx = pool.begin().await?;
    let updated: Option<Connection> = sqlx::query_as(
        r#"UPDATE connections SET archived_at=now(),revision=revision+1,
           updated_by_type=$3,updated_by_id=$4,updated_at=now()
           WHERE id=$1 AND revision=$2 AND archived_at IS NULL RETURNING *"#,
    )
    .bind(id)
    .bind(expected_revision)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .fetch_optional(&mut *tx)
    .await?;
    let updated = updated.ok_or(DbError::Conflict)?;
    insert_event(
        &mut tx,
        actor,
        "connection",
        id,
        updated.source_object_id,
        "archived",
        idempotency_key,
        Some(expected_revision),
        updated.revision,
        json!({"archived": true}),
    )
    .await?;
    tx.commit().await?;
    Ok(updated)
}

pub async fn list_tasks(pool: &PgPool, filter: TaskListFilter) -> Result<Vec<Task>, DbError> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"SELECT o.id AS object_id,o.title,o.description,CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,o.provenance,o.protected,
           t.status,t.priority,t.owner_object_id,t.agent_suitable,t.blocked_reason,t.due_at,
           t.completed_at,t.github_issue_url,t.brief_markdown,
           o.created_at,o.updated_at FROM tasks t JOIN objects o ON o.id=t.object_id WHERE true"#,
    );
    if let Some(status) = filter.status {
        query.push(" AND t.status=").push_bind(status);
    }
    if let Some(agent_suitable) = filter.agent_suitable {
        query
            .push(" AND t.agent_suitable=")
            .push_bind(agent_suitable);
    }
    query
        .push(" ORDER BY o.updated_at DESC,o.id LIMIT ")
        .push_bind(filter.limit);
    Ok(query.build_query_as().fetch_all(pool).await?)
}

pub async fn get_task(pool: &PgPool, id: Uuid) -> Result<Task, DbError> {
    sqlx::query_as(
        r#"SELECT o.id AS object_id,o.title,o.description,CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,o.provenance,o.protected,
           t.status,t.priority,t.owner_object_id,t.agent_suitable,t.blocked_reason,t.due_at,
           t.completed_at,t.github_issue_url,t.brief_markdown,
           o.created_at,o.updated_at FROM tasks t JOIN objects o ON o.id=t.object_id WHERE o.id=$1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(DbError::NotFound)
}

pub async fn create_task(
    pool: &PgPool,
    actor: &ActorContext,
    input: NewTask,
    idempotency_key: &str,
) -> Result<Task, DbError> {
    if let Some(id) = idempotent_entity(pool, actor, idempotency_key).await? {
        reconcile_existing_task_links(pool, actor, id, &input, idempotency_key).await?;
        return get_task(pool, id).await;
    }
    validate_object_description(&input.title, &input.description)?;
    let originating_chat_object_id = validate_note_links(
        pool,
        actor,
        input.originating_chat_object_id,
        &input.derived_from_source_object_ids,
    )
    .await?;
    if (input.status == "blocked") != input.blocked_reason.is_some() {
        return Err(DbError::Invalid(
            "blocked_reason is required exactly when status is blocked".into(),
        ));
    }
    if (input.status == "done") != input.completed_at.is_some() {
        return Err(DbError::Invalid(
            "completed_at is required exactly when status is done".into(),
        ));
    }
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"INSERT INTO objects
           (id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance)
           VALUES ($1,'task',$2,$3,$4,$5,$4,$5,$6)"#,
    )
    .bind(id)
    .bind(&input.title)
    .bind(&input.description)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(&input.provenance)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"INSERT INTO tasks
           (object_id,status,priority,owner_object_id,agent_suitable,blocked_reason,
            due_at,completed_at,github_issue_url,brief_markdown)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)"#,
    )
    .bind(id)
    .bind(&input.status)
    .bind(&input.priority)
    .bind(input.owner_object_id)
    .bind(input.agent_suitable)
    .bind(&input.blocked_reason)
    .bind(input.due_at)
    .bind(input.completed_at)
    .bind(&input.github_issue_url)
    .bind(&input.brief_markdown)
    .execute(&mut *tx)
    .await?;
    let run_id = insert_event(
        &mut tx,
        actor,
        "task",
        id,
        id,
        "created",
        Some(idempotency_key),
        None,
        1,
        json!({"title": input.title, "status": input.status}),
    )
    .await?;
    let mut connection_ids = Vec::new();
    let mut sequence = 2_i64;
    if let Some(chat_object_id) = originating_chat_object_id {
        let connection_id = insert_note_connection(
            &mut tx,
            actor,
            chat_object_id,
            "about",
            id,
            "This conversation requested creation of the resulting Task.",
            &input.provenance,
        )
        .await?;
        insert_event_for_run(
            &mut tx,
            run_id,
            sequence,
            actor,
            "connection",
            connection_id,
            chat_object_id,
            "connected",
            None,
            None,
            1,
        )
        .await?;
        connection_ids.push(connection_id);
        sequence += 1;
    }
    for source_object_id in &input.derived_from_source_object_ids {
        let connection_id = insert_note_connection(
            &mut tx,
            actor,
            id,
            "derived_from",
            *source_object_id,
            "This Task follows up on the linked Source.",
            &input.provenance,
        )
        .await?;
        insert_event_for_run(
            &mut tx,
            run_id,
            sequence,
            actor,
            "connection",
            connection_id,
            id,
            "connected",
            None,
            None,
            1,
        )
        .await?;
        connection_ids.push(connection_id);
        sequence += 1;
    }
    sqlx::query(
        r#"UPDATE runs
           SET chat_object_id=$2,primary_object_id=$3,
               result=result||jsonb_build_object('connection_ids',$4::uuid[]),updated_at=now()
           WHERE id=$1"#,
    )
    .bind(run_id)
    .bind(originating_chat_object_id)
    .bind(id)
    .bind(&connection_ids)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    get_task(pool, id).await
}

async fn reconcile_existing_task_links(
    pool: &PgPool,
    actor: &ActorContext,
    task_object_id: Uuid,
    input: &NewTask,
    idempotency_key: &str,
) -> Result<(), DbError> {
    let chat_object_id = validate_note_links(
        pool,
        actor,
        input.originating_chat_object_id,
        &input.derived_from_source_object_ids,
    )
    .await?;
    let mut requested = Vec::new();
    if let Some(chat_id) = chat_object_id {
        requested.push((
            chat_id,
            "about",
            task_object_id,
            "This conversation requested creation of the resulting Task.",
        ));
    }
    for source_id in &input.derived_from_source_object_ids {
        requested.push((
            task_object_id,
            "derived_from",
            *source_id,
            "This Task follows up on the linked Source.",
        ));
    }
    let mut missing = Vec::new();
    for item in requested {
        let exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM connections
               WHERE source_object_id=$1 AND kind=$2 AND target_object_id=$3
                 AND archived_at IS NULL)"#,
        )
        .bind(item.0)
        .bind(item.1)
        .bind(item.2)
        .fetch_one(pool)
        .await?;
        if !exists {
            missing.push(item);
        }
    }
    if missing.is_empty() {
        return Ok(());
    }

    let mut tx = pool.begin().await?;
    let run_id = Uuid::new_v4();
    let reconciliation_key = format!("{idempotency_key}:task-links-v1");
    sqlx::query(
        r#"INSERT INTO runs
           (id,kind,status,actor_type,actor_id,chat_object_id,primary_object_id,
            idempotency_key,input,result,completed_at)
           VALUES ($1,'mutation','completed',$2,$3,$4,$5,$6,$7,$8,now())"#,
    )
    .bind(run_id)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(chat_object_id)
    .bind(task_object_id)
    .bind(format!(
        "{}:{}:{}",
        actor.actor_type, actor.actor_id, reconciliation_key
    ))
    .bind(json!({
        "centaur_thread_key":actor.centaur_thread_key,
        "centaur_execution_id":actor.centaur_execution_id,
        "target_type":"object",
        "target_id":task_object_id,
        "action":"linked"
    }))
    .bind(json!({"affected_object_ids":[task_object_id]}))
    .execute(&mut *tx)
    .await?;

    let mut connection_ids = Vec::new();
    for (index, (source_id, kind, target_id, description)) in missing.into_iter().enumerate() {
        let connection_id = insert_note_connection(
            &mut tx,
            actor,
            source_id,
            kind,
            target_id,
            description,
            &input.provenance,
        )
        .await?;
        insert_event_for_run(
            &mut tx,
            run_id,
            index as i64 + 1,
            actor,
            "connection",
            connection_id,
            source_id,
            "connected",
            None,
            None,
            1,
        )
        .await?;
        connection_ids.push(connection_id);
    }
    sqlx::query(
        "UPDATE runs SET result=result||jsonb_build_object('connection_ids',$2::uuid[]) WHERE id=$1",
    )
    .bind(run_id)
    .bind(&connection_ids)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn update_task(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    expected_revision: i64,
    changes: TaskChanges,
    idempotency_key: Option<&str>,
) -> Result<Task, DbError> {
    if let Some(key) = idempotency_key
        && let Some(existing_id) = idempotent_entity(pool, actor, key).await?
    {
        return get_task(pool, existing_id).await;
    }
    let current = get_task(pool, id).await?;
    let title = changes.title.unwrap_or_else(|| current.title.clone());
    let description = changes
        .description
        .unwrap_or_else(|| current.description.clone());
    validate_object_description(&title, &description)?;
    let provenance = changes
        .provenance
        .unwrap_or_else(|| current.provenance.clone());
    let protected = changes.protected.unwrap_or(current.protected);
    let status = changes.status.unwrap_or_else(|| current.status.clone());
    let priority = changes.priority.unwrap_or_else(|| current.priority.clone());
    let owner_object_id = changes.owner_object_id.unwrap_or(current.owner_object_id);
    let agent_suitable = changes.agent_suitable.unwrap_or(current.agent_suitable);
    let blocked_reason = if status == "blocked" {
        changes.blocked_reason.unwrap_or(current.blocked_reason)
    } else {
        None
    };
    if status == "blocked" && blocked_reason.is_none() {
        return Err(DbError::Invalid(
            "blocked_reason is required when status is blocked".into(),
        ));
    }
    let due_at = changes.due_at.unwrap_or(current.due_at);
    let completed_at = if status == "done" {
        changes
            .completed_at
            .unwrap_or(current.completed_at)
            .or_else(|| Some(OffsetDateTime::now_utc()))
    } else {
        None
    };
    let github_issue_url = changes.github_issue_url.unwrap_or(current.github_issue_url);
    let brief_changed = changes.brief_markdown.is_some();
    let brief_markdown = changes.brief_markdown.unwrap_or(current.brief_markdown);
    let mut tx = pool.begin().await?;
    let updated: Option<Object> = sqlx::query_as(
        r#"UPDATE objects SET title=$3,description=$4,provenance=$5,protected=$6,revision=revision+1,
           updated_by_type=$7,updated_by_id=$8,updated_at=now()
           WHERE id=$1 AND revision=$2 AND kind='task'
           RETURNING id,kind,title,description,protected,
             CASE WHEN archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,
             revision,created_by_type,created_by_id,updated_by_type,updated_by_id,
             provenance,created_at,updated_at,archived_at"#,
    )
    .bind(id)
    .bind(expected_revision)
    .bind(&title)
    .bind(&description)
    .bind(&provenance)
    .bind(protected)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .fetch_optional(&mut *tx)
    .await?;
    let updated = updated.ok_or(DbError::Conflict)?;
    sqlx::query(
        r#"UPDATE tasks SET status=$2,priority=$3,owner_object_id=$4,
           agent_suitable=$5,blocked_reason=$6,due_at=$7,completed_at=$8,
           github_issue_url=$9,brief_markdown=$10 WHERE object_id=$1"#,
    )
    .bind(id)
    .bind(&status)
    .bind(&priority)
    .bind(owner_object_id)
    .bind(agent_suitable)
    .bind(&blocked_reason)
    .bind(due_at)
    .bind(completed_at)
    .bind(&github_issue_url)
    .bind(&brief_markdown)
    .execute(&mut *tx)
    .await?;
    insert_event(
        &mut tx,
        actor,
        "task",
        id,
        id,
        if status != current.status {
            "task_status_changed"
        } else {
            "updated"
        },
        idempotency_key,
        Some(expected_revision),
        updated.revision,
        json!({"title": title, "status": status, "priority": priority, "owner_object_id": owner_object_id, "agent_suitable": agent_suitable, "blocked_reason": blocked_reason, "completed_at": completed_at, "github_issue_url": github_issue_url, "brief_changed": brief_changed, "protected": protected}),
    )
    .await?;
    tx.commit().await?;
    get_task(pool, id).await
}

pub async fn list_events(pool: &PgPool, object_id: Uuid) -> Result<Vec<ObjectEvent>, DbError> {
    Ok(sqlx::query_as(
        "SELECT * FROM object_events WHERE target_type='object' AND target_id=$1 ORDER BY created_at DESC,id DESC LIMIT 100",
    )
    .bind(object_id)
    .fetch_all(pool)
    .await?)
}

pub async fn list_chat_messages(
    pool: &PgPool,
    chat_object_id: Uuid,
) -> Result<Vec<ChatMessage>, DbError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM chats WHERE object_id=$1)")
        .bind(chat_object_id)
        .fetch_one(pool)
        .await?;
    if !exists {
        return Err(DbError::NotFound);
    }
    Ok(sqlx::query_as(
        r#"SELECT m.id,m.chat_object_id,m.provider_message_id,m.sender_user_object_id,
                  o.title AS sender_title,u.user_kind AS sender_kind,m.content,
                  m.source_created_at,m.ingestion_sequence,m.ingested_at
           FROM chat_messages m
           JOIN users u ON u.object_id=m.sender_user_object_id
           JOIN objects o ON o.id=u.object_id
           WHERE m.chat_object_id=$1
           ORDER BY m.source_created_at,m.provider_message_id"#,
    )
    .bind(chat_object_id)
    .fetch_all(pool)
    .await?)
}

pub async fn list_users(pool: &PgPool, limit: i64) -> Result<Vec<User>, DbError> {
    Ok(sqlx::query_as(
        r#"SELECT o.id AS object_id,o.title,o.description,CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,o.provenance,
                  u.user_kind,u.identities,o.created_at,o.updated_at
           FROM users u JOIN objects o ON o.id=u.object_id
           WHERE o.archived_at IS NULL ORDER BY o.updated_at DESC,o.id LIMIT $1"#,
    )
    .bind(limit.clamp(1, 100))
    .fetch_all(pool)
    .await?)
}

pub async fn get_user(pool: &PgPool, id: Uuid) -> Result<User, DbError> {
    sqlx::query_as(
        r#"SELECT o.id AS object_id,o.title,o.description,CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,o.provenance,
                  u.user_kind,u.identities,o.created_at,o.updated_at
           FROM users u JOIN objects o ON o.id=u.object_id WHERE o.id=$1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(DbError::NotFound)
}

pub async fn list_user_identities(
    pool: &PgPool,
    user_object_id: Uuid,
) -> Result<Vec<ExternalIdentity>, DbError> {
    let user = get_user(pool, user_object_id).await?;
    serde_json::from_value(user.identities)
        .map_err(|error| DbError::Invalid(format!("invalid embedded User identities: {error}")))
}

pub async fn list_object_visuals(pool: &PgPool) -> Result<Vec<ObjectVisual>, DbError> {
    let sources: Vec<ObjectVisualSource> = sqlx::query_as(
        r#"SELECT o.id AS object_id,
                  CASE WHEN
                    lower(COALESCE(o.provenance->>'source_type',''))='slack'
                    OR EXISTS (
                      SELECT 1 FROM chats ch
                      WHERE ch.object_id=o.id AND ch.provider='slack'
                    )
                    OR EXISTS (
                      SELECT 1
                      FROM connections c
                      JOIN chats ch ON ch.object_id=CASE
                        WHEN c.source_object_id=o.id THEN c.target_object_id
                        ELSE c.source_object_id
                      END
                      WHERE c.archived_at IS NULL AND c.kind='derived_from'
                        AND (c.source_object_id=o.id OR c.target_object_id=o.id)
                        AND ch.provider='slack'
                    )
                    OR EXISTS (
                      SELECT 1
                      FROM jsonb_array_elements_text(
                        CASE WHEN jsonb_typeof(o.provenance->'supporting_message_ids')='array'
                          THEN o.provenance->'supporting_message_ids' ELSE '[]'::jsonb END
                      ) message_ref
                      JOIN chat_messages m ON m.id=message_ref.value::uuid
                      JOIN chats ch ON ch.object_id=m.chat_object_id
                      WHERE ch.provider='slack'
                    )
                  THEN 'slack'::text ELSE NULL::text END AS source_provider
           FROM objects o
           WHERE o.archived_at IS NULL
           ORDER BY o.updated_at DESC,o.id"#,
    )
    .fetch_all(pool)
    .await?;

    let attributions: Vec<UserAttribution> = sqlx::query_as(
        r#"WITH attribution AS (
             SELECT u.object_id,u.object_id AS user_object_id,'identity'::text AS role
             FROM users u
             UNION
             SELECT t.object_id,t.owner_object_id,'owner'::text
             FROM tasks t WHERE t.owner_object_id IS NOT NULL
             UNION
             SELECT c.source_object_id,u.object_id,'participant'::text
             FROM connections c JOIN users u ON u.object_id=c.target_object_id
             WHERE c.kind='involves' AND c.archived_at IS NULL
             UNION
             SELECT c.target_object_id,u.object_id,'participant'::text
             FROM connections c JOIN users u ON u.object_id=c.source_object_id
             WHERE c.kind='involves' AND c.archived_at IS NULL
             UNION
             SELECT o.id,m.sender_user_object_id,'source author'::text
             FROM objects o
             JOIN LATERAL jsonb_array_elements_text(
               CASE WHEN jsonb_typeof(o.provenance->'supporting_message_ids')='array'
                 THEN o.provenance->'supporting_message_ids' ELSE '[]'::jsonb END
             ) message_ref ON true
             JOIN chat_messages m ON m.id=message_ref.value::uuid
           )
           SELECT a.object_id,a.user_object_id,uo.title,u.user_kind,a.role,
                  avatar.avatar_url,
                  CASE WHEN avatar.avatar_asset_sha256 IS NOT NULL THEN
                    '/api/v2/identity-assets/' || avatar.avatar_asset_sha256 || '/' || avatar.avatar_asset_filename
                  ELSE NULL END AS avatar_asset_url
           FROM attribution a
           JOIN users u ON u.object_id=a.user_object_id
           JOIN objects uo ON uo.id=u.object_id
           LEFT JOIN LATERAL (
             SELECT e.value->>'avatar_url' AS avatar_url,
                    e.value->>'avatar_asset_sha256' AS avatar_asset_sha256,
                    e.value->>'avatar_asset_filename' AS avatar_asset_filename
             FROM jsonb_array_elements(u.identities) e(value)
             WHERE e.value->>'avatar_url' IS NOT NULL
                OR e.value->>'avatar_asset_sha256' IS NOT NULL
             ORDER BY (e.value->>'avatar_asset_sha256' IS NOT NULL) DESC,
                      (e.value->>'provider'='slack') DESC,
                      (e.value->>'profile_refreshed_at') DESC NULLS LAST LIMIT 1
           ) avatar ON true
           ORDER BY a.object_id,
             CASE a.role WHEN 'owner' THEN 1 WHEN 'source author' THEN 2
               WHEN 'participant' THEN 3 ELSE 4 END,
             uo.title,a.user_object_id"#,
    )
    .fetch_all(pool)
    .await?;

    let mut users_by_object = std::collections::HashMap::<Uuid, Vec<UserAttribution>>::new();
    for attribution in attributions {
        users_by_object
            .entry(attribution.object_id)
            .or_default()
            .push(attribution);
    }
    Ok(sources
        .into_iter()
        .map(|source| ObjectVisual {
            object_id: source.object_id,
            source_provider: source.source_provider,
            users: users_by_object
                .remove(&source.object_id)
                .unwrap_or_default(),
        })
        .collect())
}

pub async fn get_context_chat(pool: &PgPool, id: Uuid) -> Result<ContextChat, DbError> {
    sqlx::query_as(
        r#"SELECT o.id AS object_id,CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,ch.provider,ch.workspace_id,
                  ch.channel_id,ch.thread_id
           FROM chats ch JOIN objects o ON o.id=ch.object_id
           WHERE o.id=$1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(DbError::NotFound)
}

pub async fn context_anchor_candidates(
    pool: &PgPool,
    chat_object_id: Uuid,
) -> Result<Vec<ContextAnchorCandidate>, DbError> {
    let rows: Vec<ContextAnchorCandidateRow> = sqlx::query_as(
        r#"WITH candidates AS (
               SELECT $1::uuid AS object_id,0::integer AS priority,
                      'The authenticated Chat for the current thread.'::text AS rationale
               UNION ALL
               SELECT CASE WHEN c.source_object_id=$1 THEN c.target_object_id
                           ELSE c.source_object_id END AS object_id,
                      CASE WHEN other.kind='user' AND c.kind='involves'
                           THEN 1 ELSE 2 END AS priority,
                      CASE WHEN other.kind='user' AND c.kind='involves'
                           THEN 'A canonical participant in the current Chat.'
                           ELSE 'Directly connected to the current Chat by ' || c.kind || ': ' || c.description
                      END AS rationale
               FROM connections c
               JOIN objects other ON other.id=CASE
                   WHEN c.source_object_id=$1 THEN c.target_object_id
                   ELSE c.source_object_id END
               WHERE c.archived_at IS NULL
                 AND (c.source_object_id=$1 OR c.target_object_id=$1)
                 AND other.archived_at IS NULL
           ), chosen AS (
               SELECT DISTINCT ON (object_id) object_id,priority,rationale
               FROM candidates
               ORDER BY object_id,priority,rationale
           )
           SELECT o.id,o.kind,o.title,o.description,o.protected,CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,
                  o.created_by_type,o.created_by_id,o.updated_by_type,o.updated_by_id,
                  o.provenance,o.created_at,o.updated_at,o.archived_at,
                  chosen.priority,chosen.rationale
           FROM chosen JOIN objects o ON o.id=chosen.object_id
           WHERE o.archived_at IS NULL
           ORDER BY chosen.priority,o.id
           LIMIT 100"#,
    )
    .bind(chat_object_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn context_subtypes(
    pool: &PgPool,
    object_ids: &[Uuid],
    current_chat_id: Option<Uuid>,
) -> Result<std::collections::HashMap<Uuid, Value>, DbError> {
    if object_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    #[derive(FromRow)]
    struct Row {
        object_id: Uuid,
        subtype: Option<Value>,
    }
    let rows: Vec<Row> = sqlx::query_as(
        r#"SELECT o.id AS object_id,
                  CASE o.kind
                    WHEN 'task' THEN jsonb_strip_nulls(jsonb_build_object(
                        'kind','task','status',t.status,'priority',t.priority,
                        'owner_object_id',t.owner_object_id,'owner_title',owner.title,
                        'agent_suitable',t.agent_suitable,'blocked_reason',t.blocked_reason,
                        'due_at',t.due_at,'completed_at',t.completed_at,
                        'github_issue_url',t.github_issue_url))
                    WHEN 'chat' THEN jsonb_strip_nulls(jsonb_build_object(
                        'kind','chat','provider',ch.provider,'surface_kind',ch.surface_kind,
                        'channel_name',ch.channel_name,'current_thread',o.id=$2))
                    WHEN 'user' THEN jsonb_strip_nulls(jsonb_build_object(
                        'kind','user','user_kind',u.user_kind,
                        'display_name',identity.display_name))
                    WHEN 'entity' THEN jsonb_build_object(
                        'kind','entity','entity_kind',e.entity_kind)
                    WHEN 'memory' THEN jsonb_build_object(
                        'kind','memory','happened_at',m.happened_at)
                    WHEN 'source' THEN jsonb_strip_nulls(jsonb_build_object(
                        'kind','source','source_kind',s.source_kind,'canonical_uri',s.canonical_uri,
                        'publisher',s.publisher,'published_at',s.published_at,
                        'published_at_precision',s.published_at_precision,
                        'original_language',s.original_language,
                        'original_media_type',s.original_media_type,
                        'current_artifact_id',s.current_artifact_id))
                    WHEN 'note' THEN jsonb_build_object(
                        'kind','note','content_format',n.content_format,
                        'content_excerpt',substring(n.content FROM 1 FOR 400))
                    WHEN 'theme' THEN jsonb_build_object(
                        'kind','theme','slug',th.slug)
                  END AS subtype
           FROM objects o
           LEFT JOIN tasks t ON t.object_id=o.id
           LEFT JOIN objects owner ON owner.id=t.owner_object_id
           LEFT JOIN chats ch ON ch.object_id=o.id
           LEFT JOIN users u ON u.object_id=o.id
           LEFT JOIN entities e ON e.object_id=o.id
           LEFT JOIN memories m ON m.object_id=o.id
           LEFT JOIN sources s ON s.object_id=o.id
           LEFT JOIN notes n ON n.object_id=o.id
           LEFT JOIN themes th ON th.object_id=o.id
           LEFT JOIN LATERAL (
               SELECT identity->>'display_name' display_name
               FROM jsonb_array_elements(u.identities) identity
               WHERE identity->>'display_name' IS NOT NULL
               ORDER BY identity->>'profile_refreshed_at' DESC NULLS LAST LIMIT 1
           ) identity ON true
           WHERE o.id=ANY($1::uuid[])"#,
    )
    .bind(object_ids)
    .bind(current_chat_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| row.subtype.map(|subtype| (row.object_id, subtype)))
        .collect())
}

pub async fn full_text_candidates(
    pool: &PgPool,
    text_search_config: crate::config::TextSearchConfig,
    query_text: &str,
    kind: Option<&str>,
    limit: i64,
    with_connection_count: bool,
) -> Result<Vec<SearchCandidate>, DbError> {
    let mut query =
        QueryBuilder::<Postgres>::new("WITH search_query AS (SELECT websearch_to_tsquery(");
    query
        .push_bind(text_search_config.as_str())
        .push("::regconfig, regexp_replace(")
        .push_bind(query_text)
        .push(", '\\s+', ' OR ', 'g')) AS value) SELECT o.id, o.kind, o.title, o.description, o.protected, CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle, o.revision, o.created_by_type, o.created_by_id, o.updated_by_type, o.updated_by_id, o.provenance, o.created_at, o.updated_at, o.archived_at, ts_rank_cd(");
    if text_search_config == crate::config::TextSearchConfig::SIMPLE {
        query.push("o.search_document");
    } else {
        query
            .push("setweight(to_tsvector(")
            .push_bind(text_search_config.as_str())
            .push("::regconfig, coalesce(o.title,'')), 'A') || setweight(to_tsvector(")
            .push_bind(text_search_config.as_str())
            .push("::regconfig, coalesce(o.description,'')), 'B')");
    }
    query.push(", search_query.value)::float8 AS relevance,");
    if with_connection_count {
        query.push(
            r#"(SELECT count(*) FROM connections c
                WHERE c.archived_at IS NULL
                  AND (c.source_object_id=o.id OR c.target_object_id=o.id))::bigint
                AS connection_count"#,
        );
    } else {
        query.push("0::bigint AS connection_count");
    }
    query.push(" FROM objects o CROSS JOIN search_query WHERE o.archived_at IS NULL AND ");
    if text_search_config == crate::config::TextSearchConfig::SIMPLE {
        query.push("o.search_document");
    } else {
        query
            .push("(setweight(to_tsvector(")
            .push_bind(text_search_config.as_str())
            .push("::regconfig, coalesce(o.title,'')), 'A') || setweight(to_tsvector(")
            .push_bind(text_search_config.as_str())
            .push("::regconfig, coalesce(o.description,'')), 'B'))");
    }
    query.push(" @@ search_query.value");
    if let Some(kind) = kind {
        query.push(" AND o.kind=").push_bind(kind);
    }
    query
        .push(" ORDER BY relevance DESC, o.updated_at DESC, o.id LIMIT ")
        .push_bind(limit);
    Ok(query
        .build_query_as::<SearchCandidateRow>()
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(Into::into)
        .collect())
}

pub async fn artifact_full_text_candidates(
    pool: &PgPool,
    query_text: &str,
    kind: Option<&str>,
    limit: i64,
    with_connection_count: bool,
) -> Result<Vec<SearchCandidate>, DbError> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"WITH search_query AS (
             SELECT websearch_to_tsquery('simple',regexp_replace("#,
    );
    query.push_bind(query_text).push(", '\\s+', ' OR ', 'g')) AS value,")
        .push_bind(query_text.to_lowercase()).push("::text AS raw)
           SELECT o.id,o.kind,o.title,o.description,o.protected,
                  CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,
                  o.revision,o.created_by_type,o.created_by_id,o.updated_by_type,o.updated_by_id,
                  o.provenance,o.created_at,o.updated_at,o.archived_at,
                  ts_rank_cd(to_tsvector('simple',a.content),search_query.value)::float8 AS relevance,");
    if with_connection_count {
        query.push(
            r#"(SELECT count(*) FROM connections c WHERE c.archived_at IS NULL
                AND (c.source_object_id=o.id OR c.target_object_id=o.id))::bigint
                AS connection_count,"#,
        );
    } else {
        query.push("0::bigint AS connection_count,");
    }
    query.push(
        r#"a.id AS artifact_id,(excerpt.excerpt_start-1)::integer AS start_offset,
           LEAST(char_length(a.content),excerpt.excerpt_start+499)::integer AS end_offset,
           substring(a.content FROM excerpt.excerpt_start FOR 500) AS excerpt,a.capture_outcome,
           'artifact_lexical'::text AS match_kind
           FROM sources s JOIN objects o ON o.id=s.object_id
           JOIN artifacts a ON a.id=s.current_artifact_id
           CROSS JOIN search_query
           CROSS JOIN LATERAL (
             SELECT GREATEST(COALESCE(min(NULLIF(strpos(lower(a.content),term),0)),1)-100,1)
                    AS excerpt_start
             FROM regexp_split_to_table(search_query.raw,'[^[:alnum:]_]+') term
             WHERE term<>''
           ) excerpt
           WHERE o.archived_at IS NULL AND a.capture_outcome='complete'
             AND a.content IS NOT NULL
             AND to_tsvector('simple',a.content) @@ search_query.value"#,
    );
    if let Some(kind) = kind {
        query.push(" AND o.kind=").push_bind(kind);
    }
    query
        .push(" ORDER BY relevance DESC,o.updated_at DESC,o.id LIMIT ")
        .push_bind(limit);
    Ok(query
        .build_query_as::<ArtifactSearchCandidateRow>()
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(Into::into)
        .collect())
}

#[allow(clippy::too_many_arguments)]
pub async fn semantic_candidates(
    pool: &PgPool,
    vector: &[f32],
    model: &str,
    dimensions: i32,
    format_version: &str,
    input_mode: &str,
    kind: Option<&str>,
    limit: i64,
    with_connection_count: bool,
) -> Result<Vec<SearchCandidate>, DbError> {
    let vector = vector_literal(vector);
    let mut query = QueryBuilder::<Postgres>::new(
        r#"SELECT o.id, o.kind, o.title, o.description, o.protected, CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,
                  o.revision, o.created_by_type, o.created_by_id, o.updated_by_type,
                  o.updated_by_id, o.provenance, o.created_at, o.updated_at, o.archived_at,
                  (1 - (e.embedding::vector("#,
    );
    query
        .push(dimensions)
        .push(") <=> ")
        .push_bind(vector.clone())
        .push("::vector(")
        .push(dimensions)
        .push(r#")))::float8 AS relevance,"#);
    if with_connection_count {
        query.push(
            r#"(SELECT count(*) FROM connections c
                WHERE c.archived_at IS NULL
                  AND (c.source_object_id=o.id OR c.target_object_id=o.id))::bigint
                AS connection_count"#,
        );
    } else {
        query.push("0::bigint AS connection_count");
    }
    query
        .push(
            r#"
           FROM embeddings e
           JOIN objects o ON o.id=e.object_id
           WHERE o.archived_at IS NULL
             AND e.artifact_id IS NULL
             AND e.status='completed'
             AND e.source_hash=object_embedding_source_hash(e.format_version,o.kind,o.title,o.description)
             AND e.model="#,
        )
        .push_bind(model)
        .push(" AND e.dimensions=")
        .push_bind(dimensions)
        .push(" AND e.format_version=")
        .push_bind(format_version)
        .push(" AND e.input_mode=")
        .push_bind(input_mode);
    if let Some(kind) = kind {
        query.push(" AND o.kind=").push_bind(kind);
    }
    query
        .push(" ORDER BY e.embedding::vector(")
        .push(dimensions)
        .push(") <=> ")
        .push_bind(vector)
        .push("::vector(")
        .push(dimensions)
        .push(") LIMIT ")
        .push_bind(limit);
    Ok(query
        .build_query_as::<SearchCandidateRow>()
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(Into::into)
        .collect())
}

#[allow(clippy::too_many_arguments)]
pub async fn artifact_semantic_candidates(
    pool: &PgPool,
    vector: &[f32],
    model: &str,
    dimensions: i32,
    format_version: &str,
    input_mode: &str,
    kind: Option<&str>,
    limit: i64,
    with_connection_count: bool,
) -> Result<Vec<SearchCandidate>, DbError> {
    let vector = vector_literal(vector);
    let mut query = QueryBuilder::<Postgres>::new(
        r#"SELECT o.id,o.kind,o.title,o.description,o.protected,
                  CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,
                  o.revision,o.created_by_type,o.created_by_id,o.updated_by_type,o.updated_by_id,
                  o.provenance,o.created_at,o.updated_at,o.archived_at,
                  (1 - (e.embedding::vector("#,
    );
    query
        .push(dimensions)
        .push(") <=> ")
        .push_bind(vector.clone())
        .push("::vector(")
        .push(dimensions)
        .push(")))::float8 AS relevance,");
    if with_connection_count {
        query.push(
            r#"(SELECT count(*) FROM connections c WHERE c.archived_at IS NULL
                AND (c.source_object_id=o.id OR c.target_object_id=o.id))::bigint
                AS connection_count,"#,
        );
    } else {
        query.push("0::bigint AS connection_count,");
    }
    query.push(
        r#"a.id AS artifact_id,e.start_offset,
           LEAST(e.end_offset,e.start_offset+500)::integer AS end_offset,
           substring(a.content FROM e.start_offset+1 FOR LEAST(500,e.end_offset-e.start_offset)) AS excerpt,
           a.capture_outcome,'artifact_semantic'::text AS match_kind
           FROM embeddings e JOIN objects o ON o.id=e.object_id
           JOIN artifacts a ON a.id=e.artifact_id
           JOIN sources s ON s.object_id=o.id AND s.current_artifact_id=a.id
           WHERE o.archived_at IS NULL AND a.capture_outcome='complete'
             AND a.semantic_indexing_enabled
             AND e.status='completed' AND e.source_hash ~ '^[0-9a-f]{64}$'
             AND e.source_hash=encode(sha256(convert_to(
               e.format_version || chr(10) || 'title: ' || o.title || chr(10) ||
               'content: ' || substring(a.content FROM e.start_offset+1 FOR e.end_offset-e.start_offset),
               'UTF8'
             )),'hex')
             AND e.model="#,
    )
    .push_bind(model)
    .push(" AND e.dimensions=")
    .push_bind(dimensions)
    .push(" AND e.format_version=")
    .push_bind(format_version)
    .push(" AND e.input_mode=")
    .push_bind(input_mode);
    if let Some(kind) = kind {
        query.push(" AND o.kind=").push_bind(kind);
    }
    query
        .push(" ORDER BY e.embedding::vector(")
        .push(dimensions)
        .push(") <=> ")
        .push_bind(vector)
        .push("::vector(")
        .push(dimensions)
        .push(") LIMIT ")
        .push_bind(limit);
    Ok(query
        .build_query_as::<ArtifactSearchCandidateRow>()
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(Into::into)
        .collect())
}

pub async fn one_hop_neighbors(
    pool: &PgPool,
    seed_ids: &[Uuid],
    kind: Option<&str>,
    limit: i64,
) -> Result<Vec<NeighborCandidate>, DbError> {
    if seed_ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(sqlx::query_as(
        r#"WITH neighbor_edges AS (
               SELECT seed.id AS seed_object_id, c.kind AS connection_kind,
                      c.description AS connection_description,
                      CASE WHEN c.source_object_id=seed.id
                           THEN c.target_object_id ELSE c.source_object_id END AS neighbor_id
               FROM unnest($1::uuid[]) AS seed(id)
               JOIN connections c
                 ON c.archived_at IS NULL
                AND (c.source_object_id=seed.id OR c.target_object_id=seed.id)
           )
           SELECT n.seed_object_id, n.connection_kind, n.connection_description,
                  (SELECT count(*) FROM connections degree
                   WHERE degree.archived_at IS NULL
                     AND (degree.source_object_id=o.id OR degree.target_object_id=o.id))::bigint
                     AS connection_count,
                  o.id, o.kind, o.title, o.description, o.protected, CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,
                  o.revision, o.created_by_type, o.created_by_id, o.updated_by_type,
                  o.updated_by_id, o.provenance, o.created_at, o.updated_at, o.archived_at
           FROM neighbor_edges n
           JOIN objects o ON o.id=n.neighbor_id AND o.archived_at IS NULL
           WHERE NOT (o.id = ANY($1::uuid[]))
             AND ($3::text IS NULL OR o.kind=$3)
           ORDER BY o.updated_at DESC, o.id
           LIMIT $2"#,
    )
    .bind(seed_ids)
    .bind(limit)
    .bind(kind)
    .fetch_all(pool)
    .await?)
}

pub async fn context_connections(
    pool: &PgPool,
    object_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Vec<ContextConnection>>, DbError> {
    if object_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    #[derive(FromRow)]
    struct Row {
        owner_object_id: Uuid,
        id: Uuid,
        direction: String,
        kind: String,
        description: String,
        other_object_id: Uuid,
        other_object_kind: String,
        other_object_title: String,
    }
    let rows: Vec<Row> = sqlx::query_as(
        r#"SELECT owner.id AS owner_object_id, c.id,
                  CASE WHEN c.source_object_id=owner.id THEN 'outgoing' ELSE 'incoming' END AS direction,
                  c.kind, c.description, other.id AS other_object_id,
                  other.kind AS other_object_kind, other.title AS other_object_title
           FROM unnest($1::uuid[]) AS owner(id)
           JOIN LATERAL (
               SELECT candidate.* FROM connections candidate
               WHERE candidate.archived_at IS NULL
                 AND (candidate.source_object_id=owner.id OR candidate.target_object_id=owner.id)
               ORDER BY candidate.updated_at DESC, candidate.id
               LIMIT 5
           ) c ON true
           JOIN objects other
             ON other.id=CASE WHEN c.source_object_id=owner.id
                              THEN c.target_object_id ELSE c.source_object_id END
            AND other.archived_at IS NULL
           ORDER BY owner.id, c.updated_at DESC, c.id"#,
    )
    .bind(object_ids)
    .fetch_all(pool)
    .await?;
    let mut grouped = std::collections::HashMap::new();
    for row in rows {
        grouped
            .entry(row.owner_object_id)
            .or_insert_with(Vec::new)
            .push(ContextConnection {
                id: row.id,
                direction: row.direction,
                kind: row.kind,
                description: row.description,
                other_object_id: row.other_object_id,
                other_object_kind: row.other_object_kind,
                other_object_title: row.other_object_title,
            });
    }
    Ok(grouped)
}

pub async fn ensure_embedding_index(pool: &PgPool, dimensions: i32) -> Result<(), DbError> {
    if !(1..=2000).contains(&dimensions) {
        return Err(DbError::Sqlx(sqlx::Error::Configuration(
            "embedding dimensions must be between 1 and 2000".into(),
        )));
    }
    let statement = format!(
        "CREATE INDEX IF NOT EXISTS embeddings_hnsw_{dimensions}_idx \
         ON embeddings USING hnsw ((embedding::vector({dimensions})) vector_cosine_ops) \
         WHERE status='completed' AND dimensions={dimensions}"
    );
    sqlx::query(&statement).execute(pool).await?;
    Ok(())
}

pub async fn embedding_status(
    pool: &PgPool,
    configuration: Option<(&str, i32, &str)>,
) -> Result<Value, DbError> {
    #[derive(FromRow)]
    struct StatusRow {
        status: String,
        target: String,
        count: i64,
    }
    let (model, dimensions, input_mode) = configuration
        .map(|(model, dimensions, input_mode)| (Some(model), Some(dimensions), Some(input_mode)))
        .unwrap_or((None, None, None));
    let rows: Vec<StatusRow> = sqlx::query_as(
        r#"SELECT CASE WHEN status='failed' AND attempts>=5 THEN 'terminal' ELSE status END AS status,
                  CASE WHEN artifact_id IS NULL THEN 'object' ELSE 'artifact_chunk' END AS target,
                  count(*)::bigint AS count
           FROM embeddings
           WHERE ($1::text IS NULL OR (model=$1 AND dimensions=$2 AND input_mode=$3))
           GROUP BY 1,2 ORDER BY target,status"#,
    )
    .bind(model)
    .bind(dimensions)
    .bind(input_mode)
    .fetch_all(pool)
    .await?;
    let oldest_available_at: Option<OffsetDateTime> = sqlx::query_scalar(
        r#"SELECT min(available_at) FROM embeddings
           WHERE status IN ('pending','failed')
             AND ($1::text IS NULL OR (model=$1 AND dimensions=$2 AND input_mode=$3))"#,
    )
    .bind(model)
    .bind(dimensions)
    .bind(input_mode)
    .fetch_one(pool)
    .await?;
    let oldest_age_seconds = oldest_available_at.map(|available_at| {
        (OffsetDateTime::now_utc() - available_at)
            .whole_seconds()
            .max(0)
    });
    let coverage = if let Some((model, dimensions, input_mode)) = configuration {
        let active_objects: i64 =
            sqlx::query_scalar("SELECT count(*)::bigint FROM objects WHERE archived_at IS NULL")
                .fetch_one(pool)
                .await?;
        let current_complete_artifacts: i64 = sqlx::query_scalar(
            r#"SELECT count(*)::bigint FROM sources s
               JOIN objects o ON o.id=s.object_id AND o.archived_at IS NULL
               JOIN artifacts a ON a.id=s.current_artifact_id
               WHERE a.capture_outcome='complete' AND a.content IS NOT NULL"#,
        )
        .fetch_one(pool)
        .await?;
        let artifact_embedding_eligible: i64 = sqlx::query_scalar(
            r#"SELECT count(*)::bigint FROM sources s
               JOIN objects o ON o.id=s.object_id AND o.archived_at IS NULL
               JOIN artifacts a ON a.id=s.current_artifact_id
               WHERE a.capture_outcome='complete' AND a.content IS NOT NULL
                 AND a.semantic_indexing_enabled"#,
        )
        .fetch_one(pool)
        .await?;
        let completed_object_vectors: i64 = sqlx::query_scalar(
            r#"SELECT count(*)::bigint FROM embeddings e JOIN objects o ON o.id=e.object_id
               WHERE o.archived_at IS NULL AND e.artifact_id IS NULL AND e.status='completed'
                 AND e.model=$1 AND e.dimensions=$2 AND e.input_mode=$3
                 AND e.source_hash=object_embedding_source_hash(
                   e.format_version,o.kind,o.title,o.description
                 )"#,
        )
        .bind(model)
        .bind(dimensions)
        .bind(input_mode)
        .fetch_one(pool)
        .await?;
        let completed_artifact_chunks: i64 = sqlx::query_scalar(
            r#"SELECT count(*)::bigint FROM embeddings e
               JOIN objects o ON o.id=e.object_id AND o.archived_at IS NULL
               JOIN sources s ON s.object_id=o.id AND s.current_artifact_id=e.artifact_id
               JOIN artifacts a ON a.id=e.artifact_id AND a.capture_outcome='complete'
                 AND a.semantic_indexing_enabled
               WHERE e.status='completed' AND e.model=$1 AND e.dimensions=$2
                 AND e.input_mode=$3 AND e.format_version='centaur-artifact-chunk-v1'
                 AND e.source_hash=encode(sha256(convert_to(
                   e.format_version || chr(10) || 'title: ' || o.title || chr(10) ||
                   'content: ' || substring(a.content FROM e.start_offset+1 FOR e.end_offset-e.start_offset),
                   'UTF8'
                 )),'hex')"#,
        )
        .bind(model)
        .bind(dimensions)
        .bind(input_mode)
        .fetch_one(pool)
        .await?;
        let indexed_current_artifacts: i64 = sqlx::query_scalar(
            r#"SELECT count(DISTINCT e.artifact_id)::bigint FROM embeddings e
               JOIN objects o ON o.id=e.object_id AND o.archived_at IS NULL
               JOIN sources s ON s.object_id=o.id AND s.current_artifact_id=e.artifact_id
               JOIN artifacts a ON a.id=e.artifact_id AND a.capture_outcome='complete'
                 AND a.semantic_indexing_enabled
               WHERE e.status='completed' AND e.model=$1 AND e.dimensions=$2
                 AND e.input_mode=$3 AND e.format_version='centaur-artifact-chunk-v1'"#,
        )
        .bind(model)
        .bind(dimensions)
        .bind(input_mode)
        .fetch_one(pool)
        .await?;
        let stale_rows: i64 = sqlx::query_scalar(
            r#"SELECT count(*)::bigint FROM embeddings e
               JOIN objects o ON o.id=e.object_id
               LEFT JOIN artifacts a ON a.id=e.artifact_id
               WHERE e.status='completed' AND e.model=$1 AND e.dimensions=$2
                 AND e.input_mode=$3 AND (
                   (e.artifact_id IS NULL AND (
                     o.archived_at IS NOT NULL OR
                     e.source_hash<>object_embedding_source_hash(
                       e.format_version,o.kind,o.title,o.description
                     )
                   )) OR
                   (e.artifact_id IS NOT NULL AND (
                     o.archived_at IS NOT NULL OR a.capture_outcome<>'complete' OR
                     NOT a.semantic_indexing_enabled OR
                     NOT EXISTS (
                       SELECT 1 FROM sources s
                       WHERE s.object_id=e.object_id AND s.current_artifact_id=e.artifact_id
                     )
                   ))
                 )"#,
        )
        .bind(model)
        .bind(dimensions)
        .bind(input_mode)
        .fetch_one(pool)
        .await?;
        json!({
            "active_objects":active_objects,
            "current_complete_artifacts":current_complete_artifacts,
            "artifact_embedding_eligible":artifact_embedding_eligible,
            "completed_object_vectors":completed_object_vectors,
            "completed_artifact_chunks":completed_artifact_chunks,
            "indexed_current_artifacts":indexed_current_artifacts,
            "stale_rows":stale_rows,
        })
    } else {
        Value::Null
    };
    Ok(json!({
        "counts": rows.into_iter().map(|row| json!({
            "target":row.target,"status":row.status,"count":row.count
        })).collect::<Vec<_>>(),
        "oldest_available_at": oldest_available_at,
        "oldest_age_seconds": oldest_age_seconds,
        "coverage": coverage,
    }))
}

pub async fn queue_missing_embeddings(
    pool: &PgPool,
    model: &str,
    dimensions: i32,
    format_version: &str,
    input_mode: &str,
) -> Result<u64, DbError> {
    Ok(sqlx::query(
        r#"INSERT INTO embeddings
             (object_id,model,dimensions,source_hash,format_version,input_mode,status)
           SELECT o.id,$1,$4,object_embedding_source_hash($2,o.kind,o.title,o.description),$2,$3,'pending'
           FROM objects o
           LEFT JOIN embeddings e
             ON e.object_id=o.id AND e.artifact_id IS NULL AND e.model=$1
            AND e.dimensions=$4
            AND e.format_version=$2 AND e.input_mode=$3
            AND e.source_hash=object_embedding_source_hash($2,o.kind,o.title,o.description)
           WHERE o.archived_at IS NULL AND e.object_id IS NULL
           ON CONFLICT (object_id,model) WHERE artifact_id IS NULL DO UPDATE
           SET source_hash=EXCLUDED.source_hash,format_version=EXCLUDED.format_version,
               dimensions=EXCLUDED.dimensions,input_mode=EXCLUDED.input_mode,
               status='pending',attempts=0,available_at=now(),started_at=NULL,
               completed_at=NULL,last_error=NULL,embedding=NULL,updated_at=now()
           WHERE embeddings.source_hash IS DISTINCT FROM EXCLUDED.source_hash
              OR embeddings.dimensions IS DISTINCT FROM EXCLUDED.dimensions
              OR embeddings.format_version IS DISTINCT FROM EXCLUDED.format_version
              OR embeddings.input_mode IS DISTINCT FROM EXCLUDED.input_mode"#,
    )
    .bind(model)
    .bind(format_version)
    .bind(input_mode)
    .bind(dimensions)
    .execute(pool)
    .await?
    .rows_affected())
}

pub async fn artifact_embedding_sources(
    pool: &PgPool,
) -> Result<Vec<ArtifactEmbeddingSource>, DbError> {
    Ok(sqlx::query_as(
        r#"SELECT a.id AS artifact_id,a.object_id,a.sha256,o.title,a.kind,a.content
           FROM sources s
           JOIN objects o ON o.id=s.object_id AND o.archived_at IS NULL
           JOIN artifacts a ON a.id=s.current_artifact_id AND a.object_id=s.object_id
           WHERE a.capture_outcome='complete' AND a.content IS NOT NULL
             AND a.semantic_indexing_enabled
           ORDER BY a.object_id,a.id"#,
    )
    .fetch_all(pool)
    .await?)
}

#[allow(clippy::too_many_arguments)]
pub async fn queue_artifact_embedding_chunks(
    pool: &PgPool,
    source: &ArtifactEmbeddingSource,
    chunks: &[ArtifactEmbeddingChunk],
    model: &str,
    dimensions: i32,
    format_version: &str,
    input_mode: &str,
) -> Result<u64, DbError> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"DELETE FROM embeddings
           WHERE artifact_id=$1 AND model=$2
             AND (format_version<>$3 OR chunk_index >= $4)"#,
    )
    .bind(source.artifact_id)
    .bind(model)
    .bind(format_version)
    .bind(chunks.len() as i32)
    .execute(&mut *tx)
    .await?;
    let mut queued = 0;
    for chunk in chunks {
        queued += sqlx::query(
            r#"INSERT INTO embeddings
               (object_id,artifact_id,chunk_index,start_offset,end_offset,model,dimensions,
                source_hash,format_version,input_mode,status)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'pending')
               ON CONFLICT (artifact_id,model,chunk_index) WHERE artifact_id IS NOT NULL
               DO UPDATE SET object_id=EXCLUDED.object_id,start_offset=EXCLUDED.start_offset,
                 end_offset=EXCLUDED.end_offset,dimensions=EXCLUDED.dimensions,
                 source_hash=EXCLUDED.source_hash,format_version=EXCLUDED.format_version,
                 input_mode=EXCLUDED.input_mode,status='pending',attempts=0,
                 available_at=now(),started_at=NULL,completed_at=NULL,last_error=NULL,
                 embedding=NULL,updated_at=now()
               WHERE embeddings.source_hash IS DISTINCT FROM EXCLUDED.source_hash
                  OR embeddings.dimensions IS DISTINCT FROM EXCLUDED.dimensions
                  OR embeddings.format_version IS DISTINCT FROM EXCLUDED.format_version
                  OR embeddings.input_mode IS DISTINCT FROM EXCLUDED.input_mode
                  OR embeddings.start_offset IS DISTINCT FROM EXCLUDED.start_offset
                  OR embeddings.end_offset IS DISTINCT FROM EXCLUDED.end_offset"#,
        )
        .bind(source.object_id)
        .bind(source.artifact_id)
        .bind(chunk.chunk_index)
        .bind(chunk.start_offset)
        .bind(chunk.end_offset)
        .bind(model)
        .bind(dimensions)
        .bind(&chunk.source_hash)
        .bind(format_version)
        .bind(input_mode)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    }
    tx.commit().await?;
    Ok(queued)
}

pub async fn claim_embedding_job(
    pool: &PgPool,
    model: &str,
    dimensions: i32,
    input_mode: &str,
) -> Result<Option<EmbeddingJob>, DbError> {
    Ok(sqlx::query_as(
        r#"WITH recovered AS (
               UPDATE embeddings
               SET status='failed', started_at=NULL, available_at=now(),
                   last_error='worker lease expired', updated_at=now()
               WHERE status='running' AND started_at < now() - interval '5 minutes'
           ), claimed AS (
               UPDATE embeddings j
               SET status='running', attempts=attempts+1, started_at=now(), updated_at=now()
               WHERE j.id=(
                   SELECT e.id FROM embeddings e
                   WHERE e.status IN ('pending','failed') AND e.attempts < 5
                     AND e.model=$1 AND e.dimensions=$2 AND e.input_mode=$3
                     AND e.available_at <= now()
                     AND (e.artifact_id IS NULL OR EXISTS (
                       SELECT 1 FROM sources s JOIN artifacts a ON a.id=s.current_artifact_id
                       WHERE s.object_id=e.object_id AND a.id=e.artifact_id
                         AND a.capture_outcome='complete' AND a.content IS NOT NULL
                         AND a.semantic_indexing_enabled
                     ))
                   ORDER BY e.available_at, e.updated_at, e.id
                   LIMIT 1 FOR UPDATE SKIP LOCKED
               )
               RETURNING j.id,j.object_id,j.artifact_id,j.chunk_index,j.start_offset,j.end_offset,
                         j.model,j.dimensions,j.source_hash,j.format_version,j.input_mode
           )
           SELECT claimed.id,claimed.object_id,claimed.artifact_id,claimed.chunk_index,
                  claimed.start_offset,claimed.end_offset,claimed.model,claimed.dimensions,
                  claimed.source_hash,claimed.format_version,claimed.input_mode,
                  o.kind,o.title,o.description,a.content AS artifact_content
           FROM claimed JOIN objects o ON o.id=claimed.object_id
           LEFT JOIN artifacts a ON a.id=claimed.artifact_id"#,
    )
    .bind(model)
    .bind(dimensions)
    .bind(input_mode)
    .fetch_optional(pool)
    .await?)
}

pub async fn complete_embedding_job(
    pool: &PgPool,
    job: &EmbeddingJob,
    vector: &[f32],
) -> Result<(), DbError> {
    let updated = sqlx::query(
        r#"UPDATE embeddings SET status='completed',embedding=$7::vector,
           completed_at=now(),started_at=NULL,last_error=NULL,updated_at=now()
           WHERE id=$1 AND model=$2 AND dimensions=$3 AND format_version=$4
             AND input_mode=$5 AND source_hash=$6 AND status='running'"#,
    )
    .bind(job.id)
    .bind(&job.model)
    .bind(job.dimensions)
    .bind(&job.format_version)
    .bind(&job.input_mode)
    .bind(&job.source_hash)
    .bind(vector_literal(vector))
    .execute(pool)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(DbError::Conflict);
    }
    Ok(())
}

pub async fn fail_embedding_job(pool: &PgPool, id: Uuid, error: &str) -> Result<(), DbError> {
    sqlx::query(
        r#"UPDATE embeddings
           SET status='failed', started_at=NULL,
               available_at=now() + make_interval(secs => LEAST(3600, 30 * (2 ^ LEAST(attempts, 7)))),
               last_error=left($2,1000), updated_at=now()
           WHERE id=$1 AND status='running'"#,
    )
    .bind(id)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

fn vector_literal(vector: &[f32]) -> String {
    let values = vector
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

async fn idempotent_entity(
    pool: &PgPool,
    actor: &ActorContext,
    key: &str,
) -> Result<Option<Uuid>, DbError> {
    Ok(sqlx::query_scalar(
        "SELECT target_id FROM object_events WHERE actor_type=$1 AND actor_id=$2 AND idempotency_key=$3",
    )
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(key)
    .fetch_optional(pool)
    .await?)
}

#[allow(clippy::too_many_arguments)]
async fn insert_event(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    entity_type: &str,
    entity_id: Uuid,
    object_id: Uuid,
    action: &str,
    idempotency_key: Option<&str>,
    from_revision: Option<i64>,
    to_revision: i64,
    changes: Value,
) -> Result<Uuid, DbError> {
    let target_type = if entity_type == "connection" {
        "connection"
    } else {
        "object"
    };
    let target_id = if target_type == "connection" {
        entity_id
    } else {
        object_id
    };
    let run_id = Uuid::new_v4();
    let run_key = idempotency_key
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| run_id.to_string());
    let input = json!({
        "centaur_thread_key": actor.centaur_thread_key,
        "centaur_execution_id": actor.centaur_execution_id,
        "target_type": target_type,
        "target_id": target_id,
        "action": action
    });
    sqlx::query(
        r#"INSERT INTO runs
           (id,kind,status,actor_type,actor_id,primary_object_id,idempotency_key,input,result,completed_at)
           VALUES ($1,'mutation','completed',$2,$3,$4,$5,$6,$7,now())"#,
    )
    .bind(run_id)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(object_id)
    .bind(format!(
        "{}:{}:{}",
        actor.actor_type, actor.actor_id, run_key
    ))
    .bind(input)
    .bind(json!({"affected_object_ids":[object_id],"summary":changes.clone()}))
    .execute(&mut **tx)
    .await?;
    insert_event_for_run(
        tx,
        run_id,
        1,
        actor,
        entity_type,
        entity_id,
        object_id,
        action,
        idempotency_key,
        from_revision,
        to_revision,
    )
    .await?;
    Ok(run_id)
}

#[allow(clippy::too_many_arguments)]
async fn insert_event_for_run(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    sequence: i64,
    actor: &ActorContext,
    entity_type: &str,
    entity_id: Uuid,
    object_id: Uuid,
    action: &str,
    idempotency_key: Option<&str>,
    from_revision: Option<i64>,
    to_revision: i64,
) -> Result<(), DbError> {
    let target_type = if entity_type == "connection" {
        "connection"
    } else {
        "object"
    };
    let target_id = if target_type == "connection" {
        entity_id
    } else {
        object_id
    };
    let before_state: Option<Value> = if from_revision.is_some() {
        sqlx::query_scalar(
            "SELECT after_state FROM object_events WHERE target_type=$1 AND target_id=$2 ORDER BY created_at DESC,id DESC LIMIT 1",
        )
        .bind(target_type)
        .bind(target_id)
        .fetch_optional(&mut **tx)
        .await?
    } else {
        None
    };
    let after_state = target_snapshot(tx, target_type, target_id).await?;
    sqlx::query(
        r#"INSERT INTO object_events
           (id,run_id,sequence,target_type,target_id,action,actor_type,actor_id,
            idempotency_key,from_revision,to_revision,before_state,after_state,reversible,created_at)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,true,now())"#,
    )
    .bind(Uuid::new_v4())
    .bind(run_id)
    .bind(sequence)
    .bind(target_type)
    .bind(target_id)
    .bind(action)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(idempotency_key)
    .bind(from_revision)
    .bind(to_revision)
    .bind(before_state)
    .bind(after_state)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn target_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    target_type: &str,
    target_id: Uuid,
) -> Result<Value, DbError> {
    if target_type == "connection" {
        return Ok(
            sqlx::query_scalar("SELECT to_jsonb(c) FROM connections c WHERE id=$1")
                .bind(target_id)
                .fetch_optional(&mut **tx)
                .await?
                .unwrap_or_else(|| json!({"id":target_id,"archived":true})),
        );
    }
    Ok(sqlx::query_scalar(
        r#"SELECT to_jsonb(o)
          || jsonb_build_object('subtype', CASE o.kind
            WHEN 'task' THEN (SELECT to_jsonb(t)-'object_id' FROM tasks t WHERE t.object_id=o.id)
            WHEN 'chat' THEN (SELECT to_jsonb(c)-'object_id' FROM chats c WHERE c.object_id=o.id)
            WHEN 'user' THEN (SELECT to_jsonb(u)-'object_id' FROM users u WHERE u.object_id=o.id)
            WHEN 'entity' THEN (SELECT to_jsonb(e)-'object_id' FROM entities e WHERE e.object_id=o.id)
            WHEN 'memory' THEN (SELECT to_jsonb(m)-'object_id' FROM memories m WHERE m.object_id=o.id)
            WHEN 'source' THEN (SELECT to_jsonb(s)-'object_id' FROM sources s WHERE s.object_id=o.id)
            WHEN 'note' THEN (SELECT to_jsonb(n)-'object_id' FROM notes n WHERE n.object_id=o.id)
            WHEN 'theme' THEN (SELECT to_jsonb(t)-'object_id' FROM themes t WHERE t.object_id=o.id)
          END)
          || jsonb_build_object('artifacts',COALESCE(
            (SELECT jsonb_agg(to_jsonb(a)-'content' ORDER BY a.created_at,a.id) FROM artifacts a WHERE a.object_id=o.id),'[]'::jsonb))
          FROM objects o WHERE o.id=$1"#,
    )
    .bind(target_id)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or_else(|| json!({"id":target_id,"archived":true})))
}

#[cfg(test)]
mod rename_compatibility_tests {
    use super::allowed_database_name;

    #[test]
    fn accepts_canonical_and_legacy_database_names_only() {
        for allowed in [
            "centaur_context",
            "centaur_context_enyu",
            "centaur_context_test_issue_10",
            "centaur_os",
            "centaur_os_test_upgrade",
        ] {
            assert!(
                allowed_database_name(allowed),
                "expected {allowed} to be accepted"
            );
        }
        for rejected in ["postgres", "ai_v2", "centaur_contextual", "centaur_test"] {
            assert!(
                !allowed_database_name(rejected),
                "expected {rejected} to be rejected"
            );
        }
    }
}

//! Database-facing records, filters, and mutation inputs shared across repositories.

use super::*;

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
    pub routine: Option<Value>,
    pub created_by_type: String,
    pub created_by_id: String,
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
    pub work_kind: String,
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
    pub intent: Option<String>,
    pub source_artifact_id: Option<Uuid>,
    pub source_locator: Option<Value>,
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
    pub intent: Option<String>,
    pub excerpt: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
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
pub(super) struct ObjectVisualSource {
    pub(super) object_id: Uuid,
    pub(super) source_provider: Option<String>,
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
pub(super) struct SearchCandidateRow {
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
pub(super) struct ArtifactSearchCandidateRow {
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
pub(super) struct ContextAnchorCandidateRow {
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
pub enum ListSort {
    Recent,
    Connections,
    Oldest,
}

#[derive(Clone, Debug)]
pub struct ObjectListFilter {
    pub query: Option<String>,
    pub kind: Option<String>,
    pub lifecycle: Option<String>,
    pub cursor: Option<Uuid>,
    pub limit: i64,
    pub sort: ListSort,
    pub text_search_config: crate::config::TextSearchConfig,
}

#[derive(Clone, Debug)]
pub struct SourceListFilter {
    pub query: Option<String>,
    pub source_kind: Option<String>,
    pub created_after: Option<OffsetDateTime>,
    pub created_through: Option<OffsetDateTime>,
    pub cursor: Option<Uuid>,
    pub limit: i64,
    pub sort: ListSort,
}

#[derive(Clone, Debug)]
pub struct NoteListFilter {
    pub query: Option<String>,
    pub intent: Option<String>,
    pub cursor: Option<Uuid>,
    pub limit: i64,
    pub sort: ListSort,
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
    pub intent: String,
    pub source_artifact_id: Option<Uuid>,
    pub source_locator: Option<Value>,
    pub originating_chat_object_id: Option<Uuid>,
    pub derived_from_source_object_ids: Vec<Uuid>,
    pub derived_from_note_object_ids: Vec<Uuid>,
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

#[derive(Clone, Debug)]
pub struct ConnectionWriteResult {
    pub connection: Connection,
    pub reused: bool,
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
    pub cursor: Option<Uuid>,
    pub sort: ListSort,
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
    pub work_kind: String,
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
    pub work_kind: Option<String>,
    pub github_issue_url: Option<Option<String>>,
    pub brief_markdown: Option<Option<String>>,
}

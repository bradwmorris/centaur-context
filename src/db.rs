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
    pub agent_eligible: bool,
    #[serde(with = "time::serde::rfc3339::option")]
    pub due_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct ObjectEvent {
    pub id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub object_id: Uuid,
    pub action: String,
    pub actor_type: String,
    pub actor_id: String,
    pub centaur_thread_key: Option<String>,
    pub centaur_execution_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub from_revision: Option<i64>,
    pub to_revision: i64,
    pub changes: Value,
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
    pub ingested_sequence: i64,
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
    #[serde(with = "time::serde::rfc3339::option")]
    pub accessed_at: Option<OffsetDateTime>,
    pub language: Option<String>,
    pub media_type: Option<String>,
    pub artifact_reference: Option<String>,
    pub content_hash: Option<String>,
    pub current_content_id: Option<Uuid>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct SourceContent {
    pub id: Uuid,
    pub source_object_id: Uuid,
    pub version: i64,
    pub content_kind: String,
    #[serde(skip_serializing)]
    pub normalized_text: String,
    pub language: Option<String>,
    pub extraction_method: Option<String>,
    pub extraction_version: Option<String>,
    pub content_hash: String,
    pub size_bytes: i64,
    pub artifact_reference: Option<String>,
    pub locators: Value,
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
pub struct SourceContentWindow {
    #[serde(flatten)]
    pub content: SourceContent,
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

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct ThemeProposal {
    pub id: Uuid,
    pub title: String,
    pub slug: String,
    pub description: String,
    pub rationale: String,
    pub evidence: Value,
    pub provenance: Value,
    pub status: String,
    pub proposed_by_type: String,
    pub proposed_by_id: String,
    pub centaur_thread_key: String,
    pub centaur_execution_id: Option<String>,
    pub idempotency_key: String,
    pub decided_by_type: Option<String>,
    pub decided_by_id: Option<String>,
    pub decision_reason: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub decided_at: Option<OffsetDateTime>,
    pub resulting_theme_object_id: Option<Uuid>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct ExternalIdentity {
    pub id: Uuid,
    pub user_object_id: Uuid,
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
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
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
    pub object_id: Uuid,
    pub source_hash: String,
    pub format_version: String,
    pub input_mode: String,
    pub kind: String,
    pub title: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct ObjectListFilter {
    pub query: Option<String>,
    pub kind: Option<String>,
    pub lifecycle: Option<String>,
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
    pub accessed_at: Option<OffsetDateTime>,
    pub language: Option<String>,
    pub media_type: Option<String>,
    pub artifact_reference: Option<String>,
    pub content_hash: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NewNote {
    pub title: String,
    pub description: String,
    pub provenance: Value,
    pub content: String,
    pub content_format: String,
}

#[derive(Clone, Debug)]
pub struct NewTheme {
    pub title: String,
    pub description: String,
    pub slug: String,
    pub provenance: Value,
    pub protected: bool,
}

#[derive(Clone, Debug)]
pub struct NewThemeProposal {
    pub title: String,
    pub description: String,
    pub slug: String,
    pub rationale: String,
    pub evidence: Value,
    pub provenance: Value,
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
    pub accessed_at: Option<Option<OffsetDateTime>>,
    pub language: Option<Option<String>>,
    pub media_type: Option<Option<String>>,
    pub artifact_reference: Option<Option<String>>,
    pub content_hash: Option<Option<String>>,
}

#[derive(Clone, Debug)]
pub struct NewSourceContent {
    pub expected_revision: i64,
    pub content_kind: String,
    pub normalized_text: String,
    pub language: Option<String>,
    pub extraction_method: Option<String>,
    pub extraction_version: Option<String>,
    pub artifact_reference: Option<String>,
    pub locators: Value,
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
    pub agent_eligible: Option<bool>,
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
    pub agent_eligible: bool,
    pub due_at: Option<OffsetDateTime>,
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
    pub agent_eligible: Option<bool>,
    pub due_at: Option<Option<OffsetDateTime>>,
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
    let mut query = QueryBuilder::<Postgres>::new("SELECT * FROM objects WHERE true");
    if let Some(kind) = filter.kind {
        query.push(" AND kind = ").push_bind(kind);
    }
    if let Some(lifecycle) = filter.lifecycle {
        query.push(" AND lifecycle = ").push_bind(lifecycle);
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
    query
        .push(" ORDER BY updated_at DESC, id LIMIT ")
        .push_bind(filter.limit);
    Ok(query.build_query_as().fetch_all(pool).await?)
}

pub async fn get_object(pool: &PgPool, id: Uuid) -> Result<Object, DbError> {
    sqlx::query_as("SELECT * FROM objects WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
}

const SOURCE_SELECT: &str = r#"SELECT o.id AS object_id,o.title,o.description,o.lifecycle,o.revision,
       o.provenance,o.protected,s.source_kind,s.canonical_uri,s.byline,s.publisher,
       s.published_at,s.accessed_at,s.language,s.media_type,s.artifact_reference,
       s.content_hash,s.current_content_id,o.created_at,o.updated_at
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
        r#"SELECT o.id AS object_id,o.title,o.description,o.lifecycle,o.revision,
        o.provenance,o.protected,n.content,n.content_format,o.created_at,o.updated_at
        FROM notes n JOIN objects o ON o.id=n.object_id WHERE o.id=$1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(DbError::NotFound)
}

const THEME_SELECT: &str = r#"SELECT o.id AS object_id,o.title,o.description,t.slug,
       o.lifecycle,o.revision,o.provenance,o.protected,o.created_at,o.updated_at
FROM themes t JOIN objects o ON o.id=t.object_id"#;

pub async fn list_themes(pool: &PgPool) -> Result<Vec<Theme>, DbError> {
    Ok(sqlx::query_as(&format!(
        "{THEME_SELECT} WHERE o.lifecycle='active' ORDER BY o.title,o.id"
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
    query.push_bind(theme_id).push(" AND o.lifecycle='active'");
    if let Some(kind) = kind {
        query.push(" AND o.kind=").push_bind(kind);
    }
    query
        .push(" ORDER BY o.updated_at DESC,o.id LIMIT ")
        .push_bind(limit);
    Ok(query.build_query_as().fetch_all(pool).await?)
}

pub async fn has_permission(
    pool: &PgPool,
    actor: &ActorContext,
    permission: &str,
) -> Result<bool, DbError> {
    Ok(sqlx::query_scalar(
        r#"SELECT EXISTS(SELECT 1 FROM principal_permissions
           WHERE principal_type=$1 AND principal_id=$2 AND permission=$3)"#,
    )
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(permission)
    .fetch_one(pool)
    .await?)
}

pub async fn list_theme_proposals(
    pool: &PgPool,
    status: Option<&str>,
) -> Result<Vec<ThemeProposal>, DbError> {
    let mut query = QueryBuilder::<Postgres>::new("SELECT * FROM theme_proposals WHERE true");
    if let Some(status) = status {
        query.push(" AND status=").push_bind(status);
    }
    query.push(" ORDER BY created_at DESC,id");
    Ok(query.build_query_as().fetch_all(pool).await?)
}

pub async fn get_theme_proposal(pool: &PgPool, id: Uuid) -> Result<ThemeProposal, DbError> {
    sqlx::query_as("SELECT * FROM theme_proposals WHERE id=$1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn create_theme_proposal(
    pool: &PgPool,
    actor: &ActorContext,
    mut input: NewThemeProposal,
    idempotency_key: &str,
) -> Result<ThemeProposal, DbError> {
    if !actor.is_agent {
        return Err(DbError::Invalid(
            "Theme proposals require an authenticated agent".into(),
        ));
    }
    input.slug = theme_slug(input.slug)?;
    validate_object_description(&input.title, &input.description)?;
    if !input.evidence.is_object() || !input.provenance.is_object() {
        return Err(DbError::Invalid(
            "evidence and provenance must be JSON objects".into(),
        ));
    }
    if let Some(existing) = sqlx::query_as::<_, ThemeProposal>(
        r#"SELECT * FROM theme_proposals
           WHERE proposed_by_type=$1 AND proposed_by_id=$2 AND idempotency_key=$3"#,
    )
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await?
    {
        if existing.title == input.title
            && existing.slug == input.slug
            && existing.description == input.description
            && existing.rationale == input.rationale
            && existing.evidence == input.evidence
            && existing.provenance == input.provenance
        {
            return Ok(existing);
        }
        return Err(DbError::Conflict);
    }
    let duplicate: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
            SELECT 1 FROM themes t JOIN objects o ON o.id=t.object_id
            WHERE o.lifecycle='active' AND (t.slug=$1 OR lower(o.title)=lower($2))
        ) OR EXISTS(
            SELECT 1 FROM theme_proposals
            WHERE status='pending' AND (slug=$1 OR lower(title)=lower($2))
        )"#,
    )
    .bind(&input.slug)
    .bind(&input.title)
    .fetch_one(pool)
    .await?;
    if duplicate {
        return Err(DbError::Invalid(
            "an active Theme or pending proposal already has this slug or title".into(),
        ));
    }
    let proposal: ThemeProposal = sqlx::query_as(
        r#"INSERT INTO theme_proposals
        (id,title,slug,description,rationale,evidence,provenance,proposed_by_type,
         proposed_by_id,centaur_thread_key,centaur_execution_id,idempotency_key)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) RETURNING *"#,
    )
    .bind(Uuid::new_v4())
    .bind(input.title)
    .bind(input.slug)
    .bind(input.description)
    .bind(input.rationale)
    .bind(input.evidence)
    .bind(input.provenance)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(
        actor
            .centaur_thread_key
            .as_deref()
            .ok_or_else(|| DbError::Invalid("agent thread identity is required".into()))?,
    )
    .bind(&actor.centaur_execution_id)
    .bind(idempotency_key)
    .fetch_one(pool)
    .await?;
    Ok(proposal)
}

pub async fn approve_theme_proposal(
    pool: &PgPool,
    actor: &ActorContext,
    proposal_id: Uuid,
    decision_reason: &str,
    idempotency_key: &str,
) -> Result<Theme, DbError> {
    if actor.is_agent || !has_permission(pool, actor, "approve_themes").await? {
        return Err(DbError::Invalid(
            "approve_themes permission is required".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    let proposal: ThemeProposal =
        sqlx::query_as("SELECT * FROM theme_proposals WHERE id=$1 FOR UPDATE")
            .bind(proposal_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(DbError::NotFound)?;
    if proposal.status == "approved" {
        let theme_id = proposal
            .resulting_theme_object_id
            .ok_or_else(|| DbError::Invalid("approved proposal has no Theme".into()))?;
        tx.commit().await?;
        return get_theme(pool, theme_id).await;
    }
    if proposal.status != "pending" {
        return Err(DbError::Conflict);
    }
    let duplicate: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
            SELECT 1 FROM themes t JOIN objects o ON o.id=t.object_id
            WHERE o.lifecycle='active' AND (t.slug=$1 OR lower(o.title)=lower($2))
        )"#,
    )
    .bind(&proposal.slug)
    .bind(&proposal.title)
    .fetch_one(&mut *tx)
    .await?;
    if duplicate {
        return Err(DbError::Invalid(
            "an active Theme already has this slug or title".into(),
        ));
    }
    let theme_id = Uuid::new_v4();
    let object_provenance = json!({
        "source_type":"theme_proposal",
        "source_ref":proposal.id.to_string(),
        "note":proposal.rationale
    });
    sqlx::query(
        r#"INSERT INTO objects
        (id,kind,title,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance)
        VALUES ($1,'theme',$2,$3,true,$4,$5,$4,$5,$6)"#,
    )
    .bind(theme_id)
    .bind(&proposal.title)
    .bind(&proposal.description)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(object_provenance)
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO themes (object_id,slug) VALUES ($1,$2)")
        .bind(theme_id)
        .bind(&proposal.slug)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"UPDATE theme_proposals SET status='approved',decided_by_type=$2,
        decided_by_id=$3,decision_reason=$4,decided_at=now(),resulting_theme_object_id=$5,
        updated_at=now() WHERE id=$1"#,
    )
    .bind(proposal_id)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(decision_reason)
    .bind(theme_id)
    .execute(&mut *tx)
    .await?;
    insert_event(
        &mut tx,
        actor,
        "object",
        theme_id,
        theme_id,
        "created",
        Some(idempotency_key),
        None,
        1,
        json!({"kind":"theme","title":proposal.title,"slug":proposal.slug,"theme_proposal_id":proposal.id,"protected":true}),
    )
    .await?;
    tx.commit().await?;
    get_theme(pool, theme_id).await
}

pub async fn reject_theme_proposal(
    pool: &PgPool,
    actor: &ActorContext,
    proposal_id: Uuid,
    decision_reason: &str,
) -> Result<ThemeProposal, DbError> {
    if actor.is_agent || !has_permission(pool, actor, "approve_themes").await? {
        return Err(DbError::Invalid(
            "approve_themes permission is required".into(),
        ));
    }
    let proposal: ThemeProposal = sqlx::query_as(
        r#"UPDATE theme_proposals SET status='rejected',decided_by_type=$2,
        decided_by_id=$3,decision_reason=$4,decided_at=now(),updated_at=now()
        WHERE id=$1 AND status='pending' RETURNING *"#,
    )
    .bind(proposal_id)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(decision_reason)
    .fetch_optional(pool)
    .await?
    .ok_or(DbError::Conflict)?;
    Ok(proposal)
}

pub async fn list_notes(
    pool: &PgPool,
    filter: NoteListFilter,
) -> Result<Vec<NoteSearchResult>, DbError> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"SELECT o.id AS object_id,o.title,o.description,
        o.lifecycle,o.revision,n.content_format,substring(n.content FROM 1 FOR 400) AS excerpt,o.updated_at
        FROM notes n JOIN objects o ON o.id=n.object_id WHERE o.lifecycle='active'"#,
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
        return get_note(pool, id).await;
    }
    validate_object_description(&input.title, &input.description)?;
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
    insert_event(&mut tx,actor,"object",id,id,"created",Some(idempotency_key),None,1,
        json!({"kind":"note","title":input.title,"content_format":input.content_format,"content_characters":input.content.chars().count()})).await?;
    tx.commit().await?;
    get_note(pool, id).await
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
           WHERE id=$1 AND kind='note' AND revision=$2 AND lifecycle='active'
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
    sqlx::query(
        "UPDATE notes SET content=$2,content_format=$3,updated_at=now() WHERE object_id=$1",
    )
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
        r#"SELECT o.id AS object_id,o.title,o.description,o.lifecycle,o.revision,
           o.provenance,o.protected,s.source_kind,s.canonical_uri,s.byline,s.publisher,
           s.published_at,s.accessed_at,s.language,s.media_type,s.artifact_reference,
           s.content_hash,s.current_content_id,o.created_at,o.updated_at,
           CASE WHEN sc.id IS NULL THEN NULL ELSE substring(sc.normalized_text FROM 1 FOR 400) END AS excerpt
           FROM sources s JOIN objects o ON o.id=s.object_id
           LEFT JOIN source_contents sc ON sc.id=s.current_content_id
           WHERE o.lifecycle='active'"#,
    );
    if let Some(kind) = filter.source_kind {
        query.push(" AND s.source_kind=").push_bind(kind);
    }
    if let Some(cursor) = filter.cursor {
        query.push(" AND o.id>").push_bind(cursor);
    }
    if let Some(search) = filter.query {
        query.push(
            " AND to_tsvector('simple',concat_ws(' ',o.title,o.description,s.byline,s.publisher,sc.normalized_text)) @@ websearch_to_tsquery('simple',",
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
           (object_id,source_kind,canonical_uri,byline,publisher,published_at,accessed_at,
            language,media_type,artifact_reference,content_hash)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)"#,
    )
    .bind(id)
    .bind(&input.source_kind)
    .bind(&input.canonical_uri)
    .bind(&input.byline)
    .bind(&input.publisher)
    .bind(input.published_at)
    .bind(input.accessed_at)
    .bind(&input.language)
    .bind(&input.media_type)
    .bind(&input.artifact_reference)
    .bind(&input.content_hash)
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
    let accessed_at = changes.accessed_at.unwrap_or(current.accessed_at);
    let language = changes.language.unwrap_or(current.language);
    let media_type = changes.media_type.unwrap_or(current.media_type);
    let artifact_reference = changes
        .artifact_reference
        .unwrap_or(current.artifact_reference);
    let content_hash = changes.content_hash.unwrap_or(current.content_hash);
    let lifecycle = if changes.archive {
        "archived"
    } else {
        &current.lifecycle
    };
    let archived_at = changes.archive.then(OffsetDateTime::now_utc);
    let mut tx = pool.begin().await?;
    let updated_revision: Option<i64> = sqlx::query_scalar(
        r#"UPDATE objects SET title=$3,description=$4,provenance=$5,protected=$6,lifecycle=$7,
           archived_at=CASE WHEN $7='archived' THEN COALESCE(archived_at,$8) ELSE archived_at END,
           revision=revision+1,updated_by_type=$9,updated_by_id=$10,updated_at=now()
           WHERE id=$1 AND kind='source' AND revision=$2 RETURNING revision"#,
    )
    .bind(id)
    .bind(expected_revision)
    .bind(&title)
    .bind(&description)
    .bind(&provenance)
    .bind(protected)
    .bind(lifecycle)
    .bind(archived_at)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .fetch_optional(&mut *tx)
    .await?;
    let updated_revision = updated_revision.ok_or(DbError::Conflict)?;
    sqlx::query(
        r#"UPDATE sources SET source_kind=COALESCE($2,source_kind),canonical_uri=$3,
           byline=$4,publisher=$5,published_at=$6,
           accessed_at=$7,language=$8,media_type=$9,
           artifact_reference=$10,content_hash=$11,updated_at=now()
           WHERE object_id=$1"#,
    )
    .bind(id)
    .bind(&changes.source_kind)
    .bind(&canonical_uri)
    .bind(&byline)
    .bind(&publisher)
    .bind(published_at)
    .bind(accessed_at)
    .bind(&language)
    .bind(&media_type)
    .bind(&artifact_reference)
    .bind(&content_hash)
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

pub async fn list_source_contents(
    pool: &PgPool,
    source_id: Uuid,
) -> Result<Vec<SourceContent>, DbError> {
    get_source(pool, source_id).await?;
    Ok(sqlx::query_as(
        "SELECT * FROM source_contents WHERE source_object_id=$1 ORDER BY version DESC",
    )
    .bind(source_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_source_content_window(
    pool: &PgPool,
    source_id: Uuid,
    version: Option<i64>,
    offset: i64,
    limit: i64,
) -> Result<SourceContentWindow, DbError> {
    let content: SourceContent = if let Some(version) = version {
        sqlx::query_as("SELECT * FROM source_contents WHERE source_object_id=$1 AND version=$2")
            .bind(source_id).bind(version).fetch_optional(pool).await?
    } else {
        sqlx::query_as("SELECT sc.* FROM sources s JOIN source_contents sc ON sc.id=s.current_content_id WHERE s.object_id=$1")
            .bind(source_id).fetch_optional(pool).await?
    }.ok_or(DbError::NotFound)?;
    let total = content.normalized_text.chars().count() as i64;
    if offset > total {
        return Err(DbError::Validation(ValidationError::Unsupported {
            field: "offset",
            value: offset.to_string(),
        }));
    }
    let text: String = content
        .normalized_text
        .chars()
        .skip(offset as usize)
        .take(limit as usize)
        .collect();
    let end = offset + text.chars().count() as i64;
    Ok(SourceContentWindow {
        content,
        text,
        offset,
        next_offset: (end < total).then_some(end),
    })
}

pub async fn append_source_content(
    pool: &PgPool,
    actor: &ActorContext,
    source_id: Uuid,
    input: NewSourceContent,
    idempotency_key: &str,
) -> Result<SourceContent, DbError> {
    if let Some(id) = idempotent_entity(pool, actor, idempotency_key).await? {
        return sqlx::query_as("SELECT * FROM source_contents WHERE id=$1")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or(DbError::NotFound);
    }
    if input.normalized_text.is_empty() {
        return Err(DbError::Validation(ValidationError::Required(
            "normalized_text",
        )));
    }
    let id = Uuid::new_v4();
    let content_hash = format!("{:x}", Sha256::digest(input.normalized_text.as_bytes()));
    let size_bytes = input.normalized_text.len() as i64;
    let mut tx = pool.begin().await?;
    let current_revision: Option<i64> = sqlx::query_scalar(
        "SELECT revision FROM objects WHERE id=$1 AND kind='source' AND lifecycle='active' FOR UPDATE",
    ).bind(source_id).fetch_optional(&mut *tx).await?;
    if current_revision != Some(input.expected_revision) {
        return Err(DbError::Conflict);
    }
    let version: i64 = sqlx::query_scalar(
        "SELECT COALESCE(max(version),0)+1 FROM source_contents WHERE source_object_id=$1",
    )
    .bind(source_id)
    .fetch_one(&mut *tx)
    .await?;
    let content: SourceContent = sqlx::query_as(
        r#"INSERT INTO source_contents
           (id,source_object_id,version,content_kind,normalized_text,language,extraction_method,
            extraction_version,content_hash,size_bytes,artifact_reference,locators)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) RETURNING *"#,
    )
    .bind(id)
    .bind(source_id)
    .bind(version)
    .bind(&input.content_kind)
    .bind(&input.normalized_text)
    .bind(&input.language)
    .bind(&input.extraction_method)
    .bind(&input.extraction_version)
    .bind(&content_hash)
    .bind(size_bytes)
    .bind(&input.artifact_reference)
    .bind(&input.locators)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("UPDATE sources SET current_content_id=$2,updated_at=now() WHERE object_id=$1")
        .bind(source_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let revision: i64 = sqlx::query_scalar(
        "UPDATE objects SET revision=revision+1,updated_by_type=$2,updated_by_id=$3,updated_at=now() WHERE id=$1 RETURNING revision",
    ).bind(source_id).bind(actor.actor_type).bind(&actor.actor_id).fetch_one(&mut *tx).await?;
    insert_event(&mut tx,actor,"source_content",id,source_id,"content_version_created",
        Some(idempotency_key),Some(input.expected_revision),revision,
        json!({"version":version,"content_kind":input.content_kind,"content_hash":content_hash,"size_bytes":size_bytes})
    ).await?;
    tx.commit().await?;
    Ok(content)
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
    insert_object_subtype(&mut tx, id, &input.kind).await?;
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
    kind: &str,
) -> Result<(), DbError> {
    let statement = match kind {
        "chat" => Some("INSERT INTO chats (object_id) VALUES ($1)"),
        "entity" => Some("INSERT INTO entities (object_id) VALUES ($1)"),
        "memory" => Some("INSERT INTO memories (object_id) VALUES ($1)"),
        _ => None,
    };
    if let Some(statement) = statement {
        sqlx::query(statement)
            .bind(object_id)
            .execute(&mut **tx)
            .await?;
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
        r#"UPDATE objects SET title=$3, description=$4, provenance=$5, protected=$6, lifecycle=$7,
           archived_at=$8, revision=revision+1, updated_by_type=$9, updated_by_id=$10,
           updated_at=now() WHERE id=$1 AND revision=$2 RETURNING *"#,
    )
    .bind(id)
    .bind(expected_revision)
    .bind(&title)
    .bind(&description)
    .bind(&provenance)
    .bind(protected)
    .bind(lifecycle)
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
    let rows: Vec<(Uuid, String, String)> =
        sqlx::query_as("SELECT id,kind,lifecycle FROM objects WHERE id=$1 OR id=$2")
            .bind(source_object_id)
            .bind(target_object_id)
            .fetch_all(pool)
            .await?;
    let source = rows.iter().find(|row| row.0 == source_object_id);
    let target = rows.iter().find(|row| row.0 == target_object_id);
    if source.is_none_or(|row| row.2 != "active") || target.is_none_or(|row| row.2 != "active") {
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
        r#"SELECT o.id AS object_id,o.title,o.description,o.lifecycle,o.revision,o.provenance,o.protected,
           t.status,t.priority,t.owner_object_id,t.agent_eligible,t.due_at,
           o.created_at,o.updated_at FROM tasks t JOIN objects o ON o.id=t.object_id WHERE true"#,
    );
    if let Some(status) = filter.status {
        query.push(" AND t.status=").push_bind(status);
    }
    if let Some(agent_eligible) = filter.agent_eligible {
        query
            .push(" AND t.agent_eligible=")
            .push_bind(agent_eligible);
    }
    query
        .push(" ORDER BY o.updated_at DESC,o.id LIMIT ")
        .push_bind(filter.limit);
    Ok(query.build_query_as().fetch_all(pool).await?)
}

pub async fn get_task(pool: &PgPool, id: Uuid) -> Result<Task, DbError> {
    sqlx::query_as(
        r#"SELECT o.id AS object_id,o.title,o.description,o.lifecycle,o.revision,o.provenance,o.protected,
           t.status,t.priority,t.owner_object_id,t.agent_eligible,t.due_at,
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
        return get_task(pool, id).await;
    }
    validate_object_description(&input.title, &input.description)?;
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
        "INSERT INTO tasks (object_id,status,priority,owner_object_id,agent_eligible,due_at) VALUES ($1,$2,$3,$4,$5,$6)",
    )
    .bind(id)
    .bind(&input.status)
    .bind(&input.priority)
    .bind(input.owner_object_id)
    .bind(input.agent_eligible)
    .bind(input.due_at)
    .execute(&mut *tx)
    .await?;
    insert_event(
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
    tx.commit().await?;
    get_task(pool, id).await
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
    let agent_eligible = changes.agent_eligible.unwrap_or(current.agent_eligible);
    let due_at = changes.due_at.unwrap_or(current.due_at);
    let mut tx = pool.begin().await?;
    let updated: Option<Object> = sqlx::query_as(
        r#"UPDATE objects SET title=$3,description=$4,provenance=$5,protected=$6,revision=revision+1,
           updated_by_type=$7,updated_by_id=$8,updated_at=now()
           WHERE id=$1 AND revision=$2 AND kind='task' RETURNING *"#,
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
        "UPDATE tasks SET status=$2,priority=$3,owner_object_id=$4,agent_eligible=$5,due_at=$6,updated_at=now() WHERE object_id=$1",
    )
    .bind(id)
    .bind(&status)
    .bind(&priority)
    .bind(owner_object_id)
    .bind(agent_eligible)
    .bind(due_at)
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
        json!({"title": title, "status": status, "priority": priority, "owner_object_id": owner_object_id, "agent_eligible": agent_eligible, "protected": protected}),
    )
    .await?;
    tx.commit().await?;
    get_task(pool, id).await
}

pub async fn list_events(pool: &PgPool, object_id: Uuid) -> Result<Vec<ObjectEvent>, DbError> {
    Ok(sqlx::query_as(
        "SELECT * FROM object_events WHERE object_id=$1 ORDER BY created_at DESC,id DESC LIMIT 100",
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
                  m.source_created_at,m.ingested_sequence,m.ingested_at
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
        r#"SELECT o.id AS object_id,o.title,o.description,o.lifecycle,o.revision,o.provenance,
                  u.user_kind,o.created_at,o.updated_at
           FROM users u JOIN objects o ON o.id=u.object_id
           WHERE o.lifecycle='active' ORDER BY o.updated_at DESC,o.id LIMIT $1"#,
    )
    .bind(limit.clamp(1, 100))
    .fetch_all(pool)
    .await?)
}

pub async fn get_user(pool: &PgPool, id: Uuid) -> Result<User, DbError> {
    sqlx::query_as(
        r#"SELECT o.id AS object_id,o.title,o.description,o.lifecycle,o.revision,o.provenance,
                  u.user_kind,o.created_at,o.updated_at
           FROM users u JOIN objects o ON o.id=u.object_id WHERE o.id=$1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(DbError::NotFound)
}

pub async fn list_external_identities(
    pool: &PgPool,
    user_object_id: Uuid,
) -> Result<Vec<ExternalIdentity>, DbError> {
    get_user(pool, user_object_id).await?;
    Ok(sqlx::query_as(
        r#"SELECT id,user_object_id,provider,workspace_id,provider_user_id,display_name,avatar_url,
                  avatar_asset_sha256,avatar_asset_filename,avatar_provenance,
                  profile_refreshed_at,created_at,updated_at
           FROM external_identities WHERE user_object_id=$1
           ORDER BY provider,workspace_id,provider_user_id"#,
    )
    .bind(user_object_id)
    .fetch_all(pool)
    .await?)
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
           WHERE o.lifecycle='active'
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
                    '/api/v1/identity-assets/' || avatar.avatar_asset_sha256 || '/' || avatar.avatar_asset_filename
                  ELSE NULL END AS avatar_asset_url
           FROM attribution a
           JOIN users u ON u.object_id=a.user_object_id
           JOIN objects uo ON uo.id=u.object_id
           LEFT JOIN LATERAL (
             SELECT e.avatar_url,e.avatar_asset_sha256,e.avatar_asset_filename
             FROM external_identities e
             WHERE e.user_object_id=u.object_id
               AND (e.avatar_url IS NOT NULL OR e.avatar_asset_sha256 IS NOT NULL)
             ORDER BY (e.avatar_asset_sha256 IS NOT NULL) DESC,
                      (e.provider='slack') DESC,e.updated_at DESC LIMIT 1
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
        r#"SELECT o.id AS object_id,o.lifecycle,ch.provider,ch.workspace_id,
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
                 AND other.lifecycle='active'
           ), chosen AS (
               SELECT DISTINCT ON (object_id) object_id,priority,rationale
               FROM candidates
               ORDER BY object_id,priority,rationale
           )
           SELECT o.id,o.kind,o.title,o.description,o.protected,o.lifecycle,o.revision,
                  o.created_by_type,o.created_by_id,o.updated_by_type,o.updated_by_id,
                  o.provenance,o.created_at,o.updated_at,o.archived_at,
                  chosen.priority,chosen.rationale
           FROM chosen JOIN objects o ON o.id=chosen.object_id
           WHERE o.lifecycle='active'
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
        subtype: Value,
    }
    let rows: Vec<Row> = sqlx::query_as(
        r#"SELECT o.id AS object_id,
                  CASE o.kind
                    WHEN 'task' THEN jsonb_strip_nulls(jsonb_build_object(
                        'kind','task','status',t.status,'priority',t.priority,
                        'owner_object_id',t.owner_object_id,'owner_title',owner.title,
                        'agent_eligible',t.agent_eligible,'due_at',t.due_at))
                    WHEN 'chat' THEN jsonb_strip_nulls(jsonb_build_object(
                        'kind','chat','provider',ch.provider,'surface_kind',ch.surface_kind,
                        'channel_name',ch.channel_name,'current_thread',o.id=$2))
                    WHEN 'user' THEN jsonb_strip_nulls(jsonb_build_object(
                        'kind','user','user_kind',u.user_kind,
                        'display_name',identity.display_name))
                    WHEN 'entity' THEN jsonb_build_object(
                        'kind','entity','entity_kind','general')
                    WHEN 'memory' THEN jsonb_build_object(
                        'kind','memory','happened_at',m.happened_at)
                    WHEN 'source' THEN jsonb_strip_nulls(jsonb_build_object(
                        'kind','source','source_kind',s.source_kind,'canonical_uri',s.canonical_uri,
                        'publisher',s.publisher,'published_at',s.published_at,
                        'language',s.language,'media_type',s.media_type,
                        'current_content_id',s.current_content_id))
                    WHEN 'note' THEN jsonb_build_object(
                        'kind','note','content_format',n.content_format,
                        'content_excerpt',substring(n.content FROM 1 FOR 400))
                  END AS subtype
           FROM objects o
           LEFT JOIN tasks t ON t.object_id=o.id
           LEFT JOIN objects owner ON owner.id=t.owner_object_id
           LEFT JOIN chats ch ON ch.object_id=o.id
           LEFT JOIN users u ON u.object_id=o.id
           LEFT JOIN memories m ON m.object_id=o.id
           LEFT JOIN sources s ON s.object_id=o.id
           LEFT JOIN notes n ON n.object_id=o.id
           LEFT JOIN LATERAL (
               SELECT e.display_name FROM external_identities e
               WHERE e.user_object_id=o.id AND e.display_name IS NOT NULL
               ORDER BY e.updated_at DESC,e.id LIMIT 1
           ) identity ON true
           WHERE o.id=ANY($1::uuid[])"#,
    )
    .bind(object_ids)
    .bind(current_chat_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.object_id, row.subtype))
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
        .push(", '\\s+', ' OR ', 'g')) AS value) SELECT o.id, o.kind, o.title, o.description, o.protected, o.lifecycle, o.revision, o.created_by_type, o.created_by_id, o.updated_by_type, o.updated_by_id, o.provenance, o.created_at, o.updated_at, o.archived_at, ts_rank_cd(");
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
    query.push(" FROM objects o CROSS JOIN search_query WHERE o.lifecycle='active' AND ");
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
        r#"SELECT o.id, o.kind, o.title, o.description, o.protected, o.lifecycle,
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
           FROM object_embeddings e
           JOIN objects o ON o.id=e.object_id
           WHERE o.lifecycle='active'
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
                  o.id, o.kind, o.title, o.description, o.protected, o.lifecycle,
                  o.revision, o.created_by_type, o.created_by_id, o.updated_by_type,
                  o.updated_by_id, o.provenance, o.created_at, o.updated_at, o.archived_at
           FROM neighbor_edges n
           JOIN objects o ON o.id=n.neighbor_id AND o.lifecycle='active'
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
            AND other.lifecycle='active'
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
        "CREATE INDEX IF NOT EXISTS object_embeddings_hnsw_{dimensions}_idx \
         ON object_embeddings USING hnsw ((embedding::vector({dimensions})) vector_cosine_ops) \
         WHERE dimensions={dimensions}"
    );
    sqlx::query(&statement).execute(pool).await?;
    Ok(())
}

pub async fn queue_missing_embeddings(
    pool: &PgPool,
    model: &str,
    dimensions: i32,
    format_version: &str,
    input_mode: &str,
) -> Result<u64, DbError> {
    Ok(sqlx::query(
        r#"INSERT INTO object_embedding_jobs
             (object_id,source_hash,format_version,input_mode)
           SELECT o.id, object_embedding_source_hash($2,o.kind,o.title,o.description),$2,$3
           FROM objects o
           LEFT JOIN object_embeddings e
             ON e.object_id=o.id AND e.model=$1
            AND e.dimensions=$4
            AND e.format_version=$2 AND e.input_mode=$3
            AND e.source_hash=object_embedding_source_hash($2,o.kind,o.title,o.description)
           WHERE e.object_id IS NULL
           ON CONFLICT (object_id) DO UPDATE
           SET source_hash=EXCLUDED.source_hash,format_version=EXCLUDED.format_version,
               input_mode=EXCLUDED.input_mode,status='pending', attempts=0,
               available_at=now(), started_at=NULL, last_error=NULL, updated_at=now()"#,
    )
    .bind(model)
    .bind(format_version)
    .bind(input_mode)
    .bind(dimensions)
    .execute(pool)
    .await?
    .rows_affected())
}

pub async fn claim_embedding_job(pool: &PgPool) -> Result<Option<EmbeddingJob>, DbError> {
    Ok(sqlx::query_as(
        r#"WITH recovered AS (
               UPDATE object_embedding_jobs
               SET status='failed', started_at=NULL, available_at=now(),
                   last_error='worker lease expired', updated_at=now()
               WHERE status='running' AND started_at < now() - interval '5 minutes'
           ), claimed AS (
               UPDATE object_embedding_jobs j
               SET status='running', attempts=attempts+1, started_at=now(), updated_at=now()
               WHERE j.object_id=(
                   SELECT object_id FROM object_embedding_jobs
                   WHERE status IN ('pending','failed') AND attempts < 5 AND available_at <= now()
                   ORDER BY available_at, updated_at, object_id
                   LIMIT 1 FOR UPDATE SKIP LOCKED
               )
               RETURNING j.object_id,j.source_hash,j.format_version,j.input_mode
           )
           SELECT claimed.object_id,claimed.source_hash,claimed.format_version,
                  claimed.input_mode,o.kind,o.title,o.description
           FROM claimed JOIN objects o ON o.id=claimed.object_id"#,
    )
    .fetch_optional(pool)
    .await?)
}

pub async fn complete_embedding_job(
    pool: &PgPool,
    job: &EmbeddingJob,
    model: &str,
    dimensions: i32,
    format_version: &str,
    input_mode: &str,
    vector: &[f32],
) -> Result<(), DbError> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"INSERT INTO object_embeddings
           (object_id,model,dimensions,format_version,input_mode,source_hash,embedding,embedded_at)
           VALUES ($1,$2,$3,$4,$5,$6,$7::vector,now())
           ON CONFLICT (object_id,model) DO UPDATE
           SET dimensions=EXCLUDED.dimensions,format_version=EXCLUDED.format_version,
               input_mode=EXCLUDED.input_mode,source_hash=EXCLUDED.source_hash,
               embedding=EXCLUDED.embedding,embedded_at=now()"#,
    )
    .bind(job.object_id)
    .bind(model)
    .bind(dimensions)
    .bind(format_version)
    .bind(input_mode)
    .bind(&job.source_hash)
    .bind(vector_literal(vector))
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM object_embedding_jobs WHERE object_id=$1 AND source_hash=$2 AND status='running'",
    )
    .bind(job.object_id)
    .bind(&job.source_hash)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn fail_embedding_job(
    pool: &PgPool,
    object_id: Uuid,
    error: &str,
) -> Result<(), DbError> {
    sqlx::query(
        r#"UPDATE object_embedding_jobs
           SET status='failed', started_at=NULL,
               available_at=now() + make_interval(secs => LEAST(3600, 30 * (2 ^ LEAST(attempts, 7)))),
               last_error=left($2,1000), updated_at=now()
           WHERE object_id=$1 AND status='running'"#,
    )
    .bind(object_id)
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
        "SELECT entity_id FROM object_events WHERE actor_type=$1 AND actor_id=$2 AND idempotency_key=$3",
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
) -> Result<(), DbError> {
    sqlx::query(
        r#"INSERT INTO object_events
           (id,entity_type,entity_id,object_id,action,actor_type,actor_id,
            centaur_thread_key,centaur_execution_id,idempotency_key,from_revision,to_revision,changes)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)"#,
    )
    .bind(Uuid::new_v4())
    .bind(entity_type)
    .bind(entity_id)
    .bind(object_id)
    .bind(action)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(&actor.centaur_thread_key)
    .bind(&actor.centaur_execution_id)
    .bind(idempotency_key)
    .bind(from_revision)
    .bind(to_revision)
    .bind(changes)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod rename_compatibility_tests {
    use super::allowed_database_name;

    #[test]
    fn accepts_canonical_and_legacy_database_names_only() {
        for allowed in [
            "centaur_context",
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

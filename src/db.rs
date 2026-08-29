use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Transaction};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::domain::{ActorContext, ValidationError};

#[derive(Debug, Error)]
pub enum DbError {
    #[error("record not found")]
    NotFound,
    #[error("revision conflict")]
    Conflict,
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
pub struct ExternalIdentity {
    pub id: Uuid,
    pub user_object_id: Uuid,
    pub provider: String,
    pub workspace_id: String,
    pub provider_user_id: String,
    pub display_name: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug)]
pub struct SearchCandidate {
    pub object: Object,
    pub relevance: f64,
    pub connection_count: i64,
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
}

#[derive(Clone, Debug)]
pub struct NewObject {
    pub kind: String,
    pub title: String,
    pub description: String,
    pub provenance: Value,
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
    if database != "centaur_os" && !database.contains("centaur_os_test") {
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
        query
            .push(" AND search_document @@ websearch_to_tsquery('english', ")
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

pub async fn create_object(
    pool: &PgPool,
    actor: &ActorContext,
    input: NewObject,
    idempotency_key: &str,
) -> Result<Object, DbError> {
    if let Some(id) = idempotent_entity(pool, actor, idempotency_key).await? {
        return get_object(pool, id).await;
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
        r#"SELECT id,user_object_id,provider,workspace_id,provider_user_id,display_name,
                  created_at,updated_at
           FROM external_identities WHERE user_object_id=$1
           ORDER BY provider,workspace_id,provider_user_id"#,
    )
    .bind(user_object_id)
    .fetch_all(pool)
    .await?)
}

pub async fn full_text_candidates(
    pool: &PgPool,
    query_text: &str,
    kind: Option<&str>,
    limit: i64,
    with_connection_count: bool,
) -> Result<Vec<SearchCandidate>, DbError> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"WITH search_query AS (
               SELECT websearch_to_tsquery('english', regexp_replace("#,
    );
    query.push_bind(query_text).push(
        r#", '\s+', ' OR ', 'g')) AS value
           )
           SELECT o.id, o.kind, o.title, o.description, o.protected, o.lifecycle,
                  o.revision, o.created_by_type, o.created_by_id, o.updated_by_type,
                  o.updated_by_id, o.provenance, o.created_at, o.updated_at, o.archived_at,
                  ts_rank_cd(o.search_document, search_query.value)::float8 AS relevance,"#,
    );
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
    query.push(
        r#"
           FROM objects o CROSS JOIN search_query
           WHERE o.lifecycle='active' AND o.search_document @@ search_query.value"#,
    );
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

pub async fn semantic_candidates(
    pool: &PgPool,
    vector: &[f32],
    model: &str,
    dimensions: i32,
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
             AND e.source_hash=object_embedding_source_hash(o.kind,o.title,o.description)
             AND e.model="#,
        )
        .push_bind(model)
        .push(" AND e.dimensions=")
        .push_bind(dimensions);
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

pub async fn queue_missing_embeddings(pool: &PgPool, model: &str) -> Result<u64, DbError> {
    Ok(sqlx::query(
        r#"INSERT INTO object_embedding_jobs (object_id,source_hash)
           SELECT o.id, object_embedding_source_hash(o.kind,o.title,o.description)
           FROM objects o
           LEFT JOIN object_embeddings e
             ON e.object_id=o.id AND e.model=$1
            AND e.source_hash=object_embedding_source_hash(o.kind,o.title,o.description)
           WHERE e.object_id IS NULL
           ON CONFLICT (object_id) DO UPDATE
           SET source_hash=EXCLUDED.source_hash, status='pending', attempts=0,
               available_at=now(), started_at=NULL, last_error=NULL, updated_at=now()"#,
    )
    .bind(model)
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
               RETURNING j.object_id, j.source_hash
           )
           SELECT claimed.object_id, claimed.source_hash, o.kind, o.title, o.description
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
    vector: &[f32],
) -> Result<(), DbError> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"INSERT INTO object_embeddings
           (object_id, model, dimensions, source_hash, embedding, embedded_at)
           VALUES ($1,$2,$3,$4,$5::vector,now())
           ON CONFLICT (object_id,model) DO UPDATE
           SET dimensions=EXCLUDED.dimensions, source_hash=EXCLUDED.source_hash,
               embedding=EXCLUDED.embedding, embedded_at=now()"#,
    )
    .bind(job.object_id)
    .bind(model)
    .bind(dimensions)
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

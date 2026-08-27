use serde::Serialize;
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

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct Object {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub body: String,
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
    pub reason: String,
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
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub lifecycle: String,
    pub revision: i64,
    pub provenance: Value,
    pub status: String,
    pub owner_type: Option<String>,
    pub owner_id: Option<String>,
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
    pub body: String,
    pub provenance: Value,
}

#[derive(Clone, Debug, Default)]
pub struct ObjectChanges {
    pub title: Option<String>,
    pub body: Option<String>,
    pub provenance: Option<Value>,
    pub archive: bool,
}

#[derive(Clone, Debug)]
pub struct NewConnection {
    pub source_object_id: Uuid,
    pub kind: String,
    pub target_object_id: Uuid,
    pub reason: String,
    pub provenance: Value,
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
    pub body: String,
    pub provenance: Value,
    pub status: String,
    pub owner_type: Option<String>,
    pub owner_id: Option<String>,
    pub agent_eligible: bool,
    pub due_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, Default)]
pub struct TaskChanges {
    pub title: Option<String>,
    pub body: Option<String>,
    pub provenance: Option<Value>,
    pub status: Option<String>,
    pub owner_type: Option<Option<String>>,
    pub owner_id: Option<Option<String>>,
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
            .push(" AND (strpos(lower(title), lower(")
            .push_bind(search.clone())
            .push(")) > 0 OR strpos(lower(body), lower(")
            .push_bind(search)
            .push(")) > 0)");
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
           (id, kind, title, body, created_by_type, created_by_id, updated_by_type, updated_by_id, provenance)
           VALUES ($1,$2,$3,$4,$5,$6,$5,$6,$7) RETURNING *"#,
    )
    .bind(id)
    .bind(&input.kind)
    .bind(&input.title)
    .bind(&input.body)
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
    let body = changes.body.unwrap_or_else(|| current.body.clone());
    let provenance = changes
        .provenance
        .unwrap_or_else(|| current.provenance.clone());
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
        r#"UPDATE objects SET title=$3, body=$4, provenance=$5, lifecycle=$6,
           archived_at=$7, revision=revision+1, updated_by_type=$8, updated_by_id=$9,
           updated_at=now() WHERE id=$1 AND revision=$2 RETURNING *"#,
    )
    .bind(id)
    .bind(expected_revision)
    .bind(&title)
    .bind(&body)
    .bind(&provenance)
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
        json!({"title": title, "body_changed": body != current.body, "lifecycle": lifecycle}),
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
           (id, source_object_id, kind, target_object_id, reason,
            created_by_type, created_by_id, updated_by_type, updated_by_id, provenance)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$6,$7,$8) RETURNING *"#,
    )
    .bind(id)
    .bind(input.source_object_id)
    .bind(&input.kind)
    .bind(input.target_object_id)
    .bind(&input.reason)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(&input.provenance)
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
        json!({"kind": input.kind, "target_object_id": input.target_object_id, "reason": input.reason}),
    )
    .await?;
    tx.commit().await?;
    Ok(connection)
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
        r#"SELECT o.id,o.title,o.body,o.lifecycle,o.revision,o.provenance,
           t.status,t.owner_type,t.owner_id,t.agent_eligible,t.due_at,
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
        r#"SELECT o.id,o.title,o.body,o.lifecycle,o.revision,o.provenance,
           t.status,t.owner_type,t.owner_id,t.agent_eligible,t.due_at,
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
           (id,kind,title,body,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance)
           VALUES ($1,'task',$2,$3,$4,$5,$4,$5,$6)"#,
    )
    .bind(id)
    .bind(&input.title)
    .bind(&input.body)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(&input.provenance)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO tasks (object_id,status,owner_type,owner_id,agent_eligible,due_at) VALUES ($1,$2,$3,$4,$5,$6)",
    )
    .bind(id)
    .bind(&input.status)
    .bind(&input.owner_type)
    .bind(&input.owner_id)
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
    let body = changes.body.unwrap_or_else(|| current.body.clone());
    let provenance = changes
        .provenance
        .unwrap_or_else(|| current.provenance.clone());
    let status = changes.status.unwrap_or_else(|| current.status.clone());
    let owner_type = changes
        .owner_type
        .unwrap_or_else(|| current.owner_type.clone());
    let owner_id = changes.owner_id.unwrap_or_else(|| current.owner_id.clone());
    let agent_eligible = changes.agent_eligible.unwrap_or(current.agent_eligible);
    let due_at = changes.due_at.unwrap_or(current.due_at);
    let mut tx = pool.begin().await?;
    let updated: Option<Object> = sqlx::query_as(
        r#"UPDATE objects SET title=$3,body=$4,provenance=$5,revision=revision+1,
           updated_by_type=$6,updated_by_id=$7,updated_at=now()
           WHERE id=$1 AND revision=$2 AND kind='task' RETURNING *"#,
    )
    .bind(id)
    .bind(expected_revision)
    .bind(&title)
    .bind(&body)
    .bind(&provenance)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .fetch_optional(&mut *tx)
    .await?;
    let updated = updated.ok_or(DbError::Conflict)?;
    sqlx::query(
        "UPDATE tasks SET status=$2,owner_type=$3,owner_id=$4,agent_eligible=$5,due_at=$6,updated_at=now() WHERE object_id=$1",
    )
    .bind(id)
    .bind(&status)
    .bind(&owner_type)
    .bind(&owner_id)
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
        json!({"title": title, "status": status, "agent_eligible": agent_eligible}),
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

//! Explained Connection reads, writes, graph snapshots, and endpoint validation.

use super::*;

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
    Ok(
        create_or_reuse_connection(pool, actor, input, idempotency_key)
            .await?
            .connection,
    )
}

pub async fn create_or_reuse_connection(
    pool: &PgPool,
    actor: &ActorContext,
    input: NewConnection,
    idempotency_key: &str,
) -> Result<ConnectionWriteResult, DbError> {
    validate_connection_endpoints(
        pool,
        input.source_object_id,
        &input.kind,
        input.target_object_id,
    )
    .await?;
    if let Some(id) = idempotent_entity(pool, actor, idempotency_key).await? {
        let connection = sqlx::query_as("SELECT * FROM connections WHERE id=$1")
            .bind(id)
            .fetch_one(pool)
            .await?;
        return Ok(ConnectionWriteResult {
            connection,
            reused: true,
        });
    }
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await?;
    let inserted: Option<Connection> = sqlx::query_as(
        r#"INSERT INTO connections
           (id, source_object_id, kind, target_object_id, description,
            created_by_type, created_by_id, updated_by_type, updated_by_id, provenance, protected)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$6,$7,$8,$9)
           ON CONFLICT (source_object_id,kind,target_object_id)
             WHERE archived_at IS NULL DO NOTHING
           RETURNING *"#,
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
    .fetch_optional(&mut *tx)
    .await?;
    let (connection, reused) = if let Some(connection) = inserted {
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
        (connection, false)
    } else {
        let current: Connection = sqlx::query_as(
            r#"SELECT * FROM connections
               WHERE source_object_id=$1 AND kind=$2 AND target_object_id=$3
                 AND archived_at IS NULL FOR UPDATE"#,
        )
        .bind(input.source_object_id)
        .bind(&input.kind)
        .bind(input.target_object_id)
        .fetch_one(&mut *tx)
        .await?;
        let merged = merge_connection_assertion(
            &current.provenance,
            &current.description,
            &input.provenance,
            &input.description,
        );
        if merged == current.provenance {
            (current, true)
        } else {
            let updated: Connection = sqlx::query_as(
                r#"UPDATE connections SET provenance=$2,revision=revision+1,
                     updated_by_type=$3,updated_by_id=$4,updated_at=now()
                   WHERE id=$1 RETURNING *"#,
            )
            .bind(current.id)
            .bind(&merged)
            .bind(actor.actor_type)
            .bind(&actor.actor_id)
            .fetch_one(&mut *tx)
            .await?;
            insert_event(
                &mut tx,
                actor,
                "connection",
                current.id,
                current.source_object_id,
                "updated",
                Some(idempotency_key),
                Some(current.revision),
                updated.revision,
                json!({"assertion_added":true,"kind":current.kind,"target_object_id":current.target_object_id}),
            )
            .await?;
            (updated, true)
        }
    };
    tx.commit().await?;
    Ok(ConnectionWriteResult { connection, reused })
}

fn merge_connection_assertion(
    existing: &Value,
    existing_description: &str,
    incoming: &Value,
    incoming_description: &str,
) -> Value {
    let mut root = existing.as_object().cloned().unwrap_or_default();
    let mut assertions = root
        .remove("assertions")
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    if assertions.is_empty() && !root.is_empty() {
        assertions.push(json!({
            "description": existing_description,
            "provenance": Value::Object(root.clone()),
        }));
    }
    let assertion = json!({
        "description": incoming_description,
        "provenance": incoming,
    });
    if !assertions.contains(&assertion) {
        assertions.push(assertion);
    }
    root.insert("assertions".into(), Value::Array(assertions));
    Value::Object(root)
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

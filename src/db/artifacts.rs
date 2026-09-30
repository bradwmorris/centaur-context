//! Immutable Artifact reads, windows, and append transactions.

use super::*;

pub struct NewVisual {
    pub expected_revision: i64,
    pub title: String,
    pub description: String,
    pub document_key: String,
    pub media_type: String,
    pub width: i32,
    pub height: i32,
    pub bytes: Vec<u8>,
    pub supersedes_artifact_id: Option<Uuid>,
    pub method: Option<String>,
}

pub async fn read_visual_bytes(
    pool: &PgPool,
    artifact_id: Uuid,
) -> Result<(String, Vec<u8>), DbError> {
    sqlx::query_as(
        "SELECT p.media_type,p.bytes FROM artifact_binary_payloads p JOIN artifacts a ON a.id=p.artifact_id WHERE a.id=$1 AND a.kind='research_visual'",
    )
    .bind(artifact_id)
    .fetch_optional(pool)
    .await?
    .ok_or(DbError::NotFound)
}

pub async fn append_visual(
    pool: &PgPool,
    actor: &ActorContext,
    source_id: Uuid,
    input: NewVisual,
    idempotency_key: &str,
) -> Result<Artifact, DbError> {
    let sha256 = format!("{:x}", Sha256::digest(&input.bytes));
    let request = json!({
        "source_id": source_id, "expected_revision": input.expected_revision,
        "title": input.title, "description": input.description,
        "document_key": input.document_key, "media_type": input.media_type,
        "width": input.width, "height": input.height, "sha256": sha256,
        "supersedes_artifact_id": input.supersedes_artifact_id, "method": input.method,
    });
    let request_hash = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&request).map_err(|e| DbError::Invalid(e.to_string()))?)
    );
    let mut tx = pool.begin().await?;
    crate::runs::assert_thread_not_fenced(&mut tx, actor.centaur_thread_key.as_deref()).await?;
    let lock_key = format!("{}:{}:{idempotency_key}", actor.actor_type, actor.actor_id);
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(lock_key)
        .execute(&mut *tx)
        .await?;
    let prior: Option<(String, Uuid)> = sqlx::query_as(
        "SELECT request_hash,artifact_id FROM visual_upload_requests WHERE actor_type=$1 AND actor_id=$2 AND idempotency_key=$3",
    ).bind(actor.actor_type).bind(&actor.actor_id).bind(idempotency_key).fetch_optional(&mut *tx).await?;
    if let Some((old_hash, artifact_id)) = prior {
        if old_hash != request_hash {
            return Err(DbError::Invalid(
                "idempotency key was already used with a different request".into(),
            ));
        }
        let artifact = sqlx::query_as("SELECT * FROM artifacts WHERE id=$1")
            .bind(artifact_id)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        return Ok(artifact);
    }
    let source: Option<(i64, String, String, Option<Uuid>)> = sqlx::query_as(
        "SELECT o.revision,o.title,o.description,s.current_artifact_id FROM objects o JOIN sources s ON s.object_id=o.id WHERE o.id=$1 AND o.kind='source' AND o.archived_at IS NULL FOR UPDATE OF o",
    ).bind(source_id).fetch_optional(&mut *tx).await?;
    let Some((revision, title, description, canonical_id)) = source else {
        return Err(DbError::NotFound);
    };
    validate_object_description(&title, &description)?;
    if revision != input.expected_revision {
        return Err(DbError::Conflict);
    }
    if let Some(predecessor) = input.supersedes_artifact_id {
        let prior: Option<(String, Value)> =
            sqlx::query_as("SELECT kind,metadata FROM artifacts WHERE id=$1 AND object_id=$2")
                .bind(predecessor)
                .bind(source_id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some((kind, metadata)) = prior else {
            return Err(DbError::Invalid(
                "visual predecessor belongs to another Source".into(),
            ));
        };
        if kind != "research_visual"
            || metadata.get("document_key").and_then(Value::as_str)
                != Some(input.document_key.as_str())
            || canonical_id == Some(predecessor)
        {
            return Err(DbError::Invalid(
                "visual may supersede only a visual with the same document key".into(),
            ));
        }
        let has_successor: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM artifacts WHERE supersedes_artifact_id=$1)",
        )
        .bind(predecessor)
        .fetch_one(&mut *tx)
        .await?;
        if has_successor {
            return Err(DbError::Conflict);
        }
    } else {
        let key_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM artifacts WHERE object_id=$1 AND kind='research_visual' AND metadata->>'document_key'=$2)",
        ).bind(source_id).bind(&input.document_key).fetch_one(&mut *tx).await?;
        if key_exists {
            return Err(DbError::Conflict);
        }
    }
    let artifact_id = Uuid::new_v4();
    let metadata = json!({
        "document_key": input.document_key,
        "creation_key": idempotency_key,
        "description": input.description,
        "width": input.width,
        "height": input.height,
        "method": input.method,
        "predecessor_artifact_id": input.supersedes_artifact_id,
    });
    let artifact: Artifact = sqlx::query_as(
        "INSERT INTO artifacts (id,object_id,kind,title,media_type,sha256,size_bytes,capture_outcome,metadata,supersedes_artifact_id,semantic_indexing_enabled) VALUES ($1,$2,'research_visual',$3,$4,$5,$6,'complete',$7,$8,false) RETURNING *",
    ).bind(artifact_id).bind(source_id).bind(&input.title).bind(&input.media_type)
     .bind(&sha256).bind(input.bytes.len() as i64).bind(metadata).bind(input.supersedes_artifact_id)
     .fetch_one(&mut *tx).await?;
    sqlx::query("INSERT INTO artifact_binary_payloads (artifact_id,media_type,width,height,bytes) VALUES ($1,$2,$3,$4,$5)")
        .bind(artifact_id).bind(&input.media_type).bind(input.width).bind(input.height).bind(&input.bytes)
        .execute(&mut *tx).await?;
    let next_revision: i64 = sqlx::query_scalar(
        "UPDATE objects SET revision=revision+1,updated_by_type=$2,updated_by_id=$3,updated_at=now() WHERE id=$1 RETURNING revision",
    ).bind(source_id).bind(actor.actor_type).bind(&actor.actor_id).fetch_one(&mut *tx).await?;
    insert_event(&mut tx, actor, "object", source_id, source_id, "artifact_attached",
        Some(idempotency_key), Some(revision), next_revision,
        json!({"artifact_id": artifact_id, "kind": "research_visual", "sha256": sha256, "size_bytes": input.bytes.len()})).await?;
    sqlx::query("INSERT INTO visual_upload_requests (actor_type,actor_id,idempotency_key,request_hash,artifact_id) VALUES ($1,$2,$3,$4,$5)")
        .bind(actor.actor_type).bind(&actor.actor_id).bind(idempotency_key).bind(request_hash).bind(artifact_id)
        .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(artifact)
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
    if idempotent_entity(pool, actor, idempotency_key)
        .await?
        .is_some()
    {
        let id: Option<Uuid> = sqlx::query_scalar(
            "SELECT (r.result->'summary'->>'artifact_id')::uuid FROM object_events e JOIN runs r ON r.id=e.run_id WHERE e.actor_type=$1 AND e.actor_id=$2 AND e.idempotency_key=$3 AND e.action='artifact_attached' AND e.target_id=$4",
        )
        .bind(actor.actor_type)
        .bind(&actor.actor_id)
        .bind(idempotency_key)
        .bind(object_id)
        .fetch_optional(pool)
        .await?;
        let id = id.ok_or(DbError::Conflict)?;
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
    let current: Option<(i64, String, String)> = sqlx::query_as(
        "SELECT revision,title,description FROM objects WHERE id=$1 AND archived_at IS NULL FOR UPDATE",
    )
    .bind(object_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((current_revision, current_title, current_description)) = current else {
        return Err(DbError::Conflict);
    };
    validate_object_description(&current_title, &current_description)?;
    // A working document is an immutable revision, including when its text
    // reverts to an older revision or matches another document on this Object.
    // Keep the historical Object-wide hash deduplication for other kinds.
    if input.kind != "research_notes"
        && let Some(existing) =
            sqlx::query_as("SELECT * FROM artifacts WHERE object_id=$1 AND sha256=$2")
                .bind(object_id)
                .bind(&sha256)
                .fetch_optional(&mut *tx)
                .await?
    {
        tx.commit().await?;
        return Ok(existing);
    }
    if input
        .expected_revision
        .is_some_and(|revision| revision != current_revision)
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
    // Generic artifacts are supporting material. Canonical Source artifact
    // promotion belongs exclusively to the dedicated Source intake path.
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
        Some(current_revision),
        revision,
        json!({"artifact_id":id,"kind":input.kind,"sha256":sha256,"size_bytes":size_bytes,
            "capture_outcome":input.capture_outcome}),
    )
    .await?;
    tx.commit().await?;
    Ok(artifact)
}

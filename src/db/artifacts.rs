//! Immutable Artifact reads, windows, and append transactions.

use super::*;

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
    let Some((current_revision, current_title, current_description)) = current else {
        return Err(DbError::Conflict);
    };
    validate_object_description(&current_title, &current_description)?;
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

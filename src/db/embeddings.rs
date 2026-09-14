//! Embedding index status and durable embedding-job persistence.

use super::*;

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

pub(super) fn vector_literal(vector: &[f32]) -> String {
    let values = vector
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

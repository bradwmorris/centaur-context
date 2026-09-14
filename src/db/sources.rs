//! Source reads and atomic canonical Source mutations.

use super::*;

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
    if let Some(created_after) = filter.created_after {
        query.push(" AND o.created_at>").push_bind(created_after);
    }
    if let Some(created_through) = filter.created_through {
        query.push(" AND o.created_at<=").push_bind(created_through);
    }
    if let Some(cursor) = filter.cursor {
        push_object_list_cursor(&mut query, cursor, &filter.sort);
    }
    if let Some(search) = filter.query {
        query.push(
            " AND to_tsvector('simple',concat_ws(' ',o.title,o.description,s.byline,s.publisher,sc.content)) @@ websearch_to_tsquery('simple',",
        )
        .push_bind(search)
        .push(")");
    }
    push_object_list_order(&mut query, &filter.sort);
    query.push(" LIMIT ").push_bind(filter.limit);
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

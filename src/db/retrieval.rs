//! Context Builder lexical, semantic, anchor, subtype, graph, and Connection queries.

use super::*;

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

pub async fn top_connected_candidates(
    pool: &PgPool,
    excluded_object_ids: &[Uuid],
    limit: i64,
) -> Result<Vec<SearchCandidate>, DbError> {
    let rows: Vec<SearchCandidateRow> = sqlx::query_as(
        r#"SELECT o.id,o.kind,o.title,o.description,o.protected,
                  'active'::text AS lifecycle,o.revision,o.created_by_type,o.created_by_id,
                  o.updated_by_type,o.updated_by_id,o.provenance,o.created_at,o.updated_at,
                  o.archived_at,0::float8 AS relevance,count(c.id)::bigint AS connection_count
           FROM objects o
           JOIN connections c
             ON c.archived_at IS NULL
            AND (c.source_object_id=o.id OR c.target_object_id=o.id)
           JOIN objects other
             ON other.id=CASE WHEN c.source_object_id=o.id
                              THEN c.target_object_id ELSE c.source_object_id END
            AND other.archived_at IS NULL
           WHERE o.archived_at IS NULL
             AND NOT (o.id=ANY($1::uuid[]))
           GROUP BY o.id
           ORDER BY connection_count DESC,o.updated_at DESC,o.id
           LIMIT $2"#,
    )
    .bind(excluded_object_ids)
    .bind(limit.clamp(1, 100))
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
                        'github_issue_url',t.github_issue_url,'work_kind',t.work_kind,
                        'brief_markdown',t.brief_markdown,'execution_actor_id',t.execution_actor_id))
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

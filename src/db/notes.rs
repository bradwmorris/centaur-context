//! Note reads, writes, provenance links, and their atomic mutation history.

use super::*;

pub(super) struct NoteLinkInput<'a> {
    pub source_object_ids: &'a [Uuid],
    pub note_object_ids: &'a [Uuid],
    pub intent: &'a str,
    pub content: &'a str,
    pub source_artifact_id: Option<Uuid>,
    pub source_locator: Option<&'a Value>,
}

pub async fn list_notes(
    pool: &PgPool,
    filter: NoteListFilter,
) -> Result<Vec<NoteSearchResult>, DbError> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"SELECT o.id AS object_id,o.title,o.description,
        CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,n.content_format,n.intent,substring(n.content FROM 1 FOR 400) AS excerpt,o.created_at,o.updated_at
        FROM notes n JOIN objects o ON o.id=n.object_id WHERE o.archived_at IS NULL"#,
    );
    if let Some(cursor) = filter.cursor {
        push_object_list_cursor(&mut query, cursor, &filter.sort);
    }
    if let Some(search) = filter.query {
        query.push(" AND to_tsvector('simple',concat_ws(' ',o.title,o.description,n.content)) @@ websearch_to_tsquery('simple',")
            .push_bind(search).push(")");
    }
    if let Some(intent) = filter.intent {
        query.push(" AND n.intent=").push_bind(intent);
    }
    push_object_list_order(&mut query, &filter.sort);
    query.push(" LIMIT ").push_bind(filter.limit);
    Ok(query.build_query_as().fetch_all(pool).await?)
}

pub async fn create_note(
    pool: &PgPool,
    actor: &ActorContext,
    input: NewNote,
    idempotency_key: &str,
) -> Result<Note, DbError> {
    if let Some(id) = idempotent_entity(pool, actor, idempotency_key).await? {
        reconcile_existing_note_links(pool, actor, id, &input, idempotency_key).await?;
        return get_note(pool, id).await;
    }
    validate_object_description(&input.title, &input.description)?;
    let originating_chat_object_id = validate_note_links(
        pool,
        actor,
        input.originating_chat_object_id,
        NoteLinkInput {
            source_object_ids: &input.derived_from_source_object_ids,
            note_object_ids: &input.derived_from_note_object_ids,
            intent: &input.intent,
            content: &input.content,
            source_artifact_id: input.source_artifact_id,
            source_locator: input.source_locator.as_ref(),
        },
    )
    .await?;
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await?;
    sqlx::query(r#"INSERT INTO objects
        (id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance)
        VALUES ($1,'note',$2,$3,$4,$5,$4,$5,$6)"#)
        .bind(id).bind(&input.title).bind(&input.description).bind(actor.actor_type).bind(&actor.actor_id)
        .bind(&input.provenance).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO notes (object_id,content,content_format,intent,source_artifact_id,source_locator) VALUES ($1,$2,$3,$4,$5,$6)")
        .bind(id)
        .bind(&input.content)
        .bind(&input.content_format)
        .bind(&input.intent)
        .bind(input.source_artifact_id)
        .bind(&input.source_locator)
        .execute(&mut *tx)
        .await?;
    let run_id = insert_event(&mut tx,actor,"object",id,id,"created",Some(idempotency_key),None,1,
        json!({"kind":"note","title":input.title,"intent":input.intent,"content_format":input.content_format,"content_characters":input.content.chars().count(),"source_artifact_id":input.source_artifact_id,"source_locator":input.source_locator})).await?;

    let mut connection_ids = Vec::new();
    let mut sequence = 2_i64;
    if let Some(chat_object_id) = originating_chat_object_id {
        let connection_id = insert_note_connection(
            &mut tx,
            actor,
            chat_object_id,
            "about",
            id,
            "This conversation requested creation of the resulting Note.",
            &input.provenance,
        )
        .await?;
        insert_event_for_run(
            &mut tx,
            run_id,
            sequence,
            actor,
            "connection",
            connection_id,
            chat_object_id,
            "connected",
            None,
            None,
            1,
        )
        .await?;
        connection_ids.push(connection_id);
        sequence += 1;
    }
    for source_object_id in &input.derived_from_source_object_ids {
        let connection_id = insert_note_connection(
            &mut tx,
            actor,
            id,
            "derived_from",
            *source_object_id,
            "This Note records an observation derived from the linked Source.",
            &input.provenance,
        )
        .await?;
        insert_event_for_run(
            &mut tx,
            run_id,
            sequence,
            actor,
            "connection",
            connection_id,
            id,
            "connected",
            None,
            None,
            1,
        )
        .await?;
        connection_ids.push(connection_id);
        sequence += 1;
    }
    for note_object_id in &input.derived_from_note_object_ids {
        let connection_id = insert_note_connection(
            &mut tx,
            actor,
            id,
            "derived_from",
            *note_object_id,
            "This atomic Note develops or questions the linked Note.",
            &input.provenance,
        )
        .await?;
        insert_event_for_run(
            &mut tx,
            run_id,
            sequence,
            actor,
            "connection",
            connection_id,
            id,
            "connected",
            None,
            None,
            1,
        )
        .await?;
        connection_ids.push(connection_id);
        sequence += 1;
    }
    sqlx::query(
        r#"UPDATE runs
           SET chat_object_id=$2,primary_object_id=$3,
               result=result||jsonb_build_object('connection_ids',$4::uuid[]),updated_at=now()
           WHERE id=$1"#,
    )
    .bind(run_id)
    .bind(originating_chat_object_id)
    .bind(id)
    .bind(&connection_ids)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    get_note(pool, id).await
}

async fn reconcile_existing_note_links(
    pool: &PgPool,
    actor: &ActorContext,
    note_object_id: Uuid,
    input: &NewNote,
    idempotency_key: &str,
) -> Result<(), DbError> {
    let chat_object_id = validate_note_links(
        pool,
        actor,
        input.originating_chat_object_id,
        NoteLinkInput {
            source_object_ids: &input.derived_from_source_object_ids,
            note_object_ids: &input.derived_from_note_object_ids,
            intent: &input.intent,
            content: &input.content,
            source_artifact_id: input.source_artifact_id,
            source_locator: input.source_locator.as_ref(),
        },
    )
    .await?;
    let mut requested = Vec::new();
    if let Some(chat_id) = chat_object_id {
        requested.push((
            chat_id,
            "about",
            note_object_id,
            "This conversation requested creation of the resulting Note.",
        ));
    }
    for source_id in &input.derived_from_source_object_ids {
        requested.push((
            note_object_id,
            "derived_from",
            *source_id,
            "This Note records an observation derived from the linked Source.",
        ));
    }
    for note_id in &input.derived_from_note_object_ids {
        requested.push((
            note_object_id,
            "derived_from",
            *note_id,
            "This atomic Note develops or questions the linked Note.",
        ));
    }
    let mut missing = Vec::new();
    for item in requested {
        let exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM connections
               WHERE source_object_id=$1 AND kind=$2 AND target_object_id=$3
                 AND archived_at IS NULL)"#,
        )
        .bind(item.0)
        .bind(item.1)
        .bind(item.2)
        .fetch_one(pool)
        .await?;
        if !exists {
            missing.push(item);
        }
    }
    if missing.is_empty() {
        return Ok(());
    }

    let mut tx = pool.begin().await?;
    let run_id = Uuid::new_v4();
    let reconciliation_key = format!("{idempotency_key}:note-links-v1");
    sqlx::query(
        r#"INSERT INTO runs
           (id,kind,status,actor_type,actor_id,chat_object_id,primary_object_id,
            idempotency_key,input,result,completed_at)
           VALUES ($1,'mutation','completed',$2,$3,$4,$5,$6,$7,$8,now())"#,
    )
    .bind(run_id)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(chat_object_id)
    .bind(note_object_id)
    .bind(format!(
        "{}:{}:{}",
        actor.actor_type, actor.actor_id, reconciliation_key
    ))
    .bind(json!({
        "centaur_thread_key":actor.centaur_thread_key,
        "centaur_execution_id":actor.centaur_execution_id,
        "target_type":"object",
        "target_id":note_object_id,
        "action":"linked"
    }))
    .bind(json!({"affected_object_ids":[note_object_id]}))
    .execute(&mut *tx)
    .await?;

    let mut connection_ids = Vec::new();
    for (index, (source_id, kind, target_id, description)) in missing.into_iter().enumerate() {
        let connection_id = insert_note_connection(
            &mut tx,
            actor,
            source_id,
            kind,
            target_id,
            description,
            &input.provenance,
        )
        .await?;
        insert_event_for_run(
            &mut tx,
            run_id,
            index as i64 + 1,
            actor,
            "connection",
            connection_id,
            source_id,
            "connected",
            None,
            None,
            1,
        )
        .await?;
        connection_ids.push(connection_id);
    }
    sqlx::query(
        "UPDATE runs SET result=result||jsonb_build_object('connection_ids',$2::uuid[]) WHERE id=$1",
    )
    .bind(run_id)
    .bind(&connection_ids)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub(super) async fn validate_note_links(
    pool: &PgPool,
    actor: &ActorContext,
    originating_chat_object_id: Option<Uuid>,
    links: NoteLinkInput<'_>,
) -> Result<Option<Uuid>, DbError> {
    let resolved_chat_object_id = match originating_chat_object_id {
        Some(id) => Some(id),
        None => resolve_actor_chat(pool, actor, links.intent).await?,
    };
    if let Some(chat_object_id) = resolved_chat_object_id {
        let valid: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(
               SELECT 1 FROM objects o JOIN chats c ON c.object_id=o.id
               WHERE o.id=$1 AND o.archived_at IS NULL
            )"#,
        )
        .bind(chat_object_id)
        .fetch_one(pool)
        .await?;
        if !valid {
            return Err(DbError::Invalid(
                "originating_chat_object_id must identify an active Chat".into(),
            ));
        }
    }
    let unique_source_ids = links
        .source_object_ids
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    if unique_source_ids.len() != links.source_object_ids.len() {
        return Err(DbError::Invalid(
            "derived_from_source_object_ids must not contain duplicates".into(),
        ));
    }
    if !links.source_object_ids.is_empty() {
        let valid_count: i64 = sqlx::query_scalar(
            r#"SELECT count(*) FROM objects o JOIN sources s ON s.object_id=o.id
               WHERE o.id=ANY($1) AND o.archived_at IS NULL"#,
        )
        .bind(links.source_object_ids)
        .fetch_one(pool)
        .await?;
        if valid_count != links.source_object_ids.len() as i64 {
            return Err(DbError::Invalid(
                "derived_from_source_object_ids must identify active Sources".into(),
            ));
        }
    }
    let unique_note_ids = links
        .note_object_ids
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    if unique_note_ids.len() != links.note_object_ids.len() {
        return Err(DbError::Invalid(
            "derived_from_note_object_ids must not contain duplicates".into(),
        ));
    }
    if !links.note_object_ids.is_empty() {
        let valid_count: i64 = sqlx::query_scalar(
            r#"SELECT count(*) FROM objects o JOIN notes n ON n.object_id=o.id
               WHERE o.id=ANY($1) AND o.archived_at IS NULL"#,
        )
        .bind(links.note_object_ids)
        .fetch_one(pool)
        .await?;
        if valid_count != links.note_object_ids.len() as i64 {
            return Err(DbError::Invalid(
                "derived_from_note_object_ids must identify active Notes".into(),
            ));
        }
    }
    if links.intent == "excerpt" {
        if links.source_object_ids.len() != 1 {
            return Err(DbError::Invalid(
                "excerpt Notes require exactly one derived Source".into(),
            ));
        }
        if !links.note_object_ids.is_empty() {
            return Err(DbError::Invalid(
                "excerpt Notes cannot derive from other Notes".into(),
            ));
        }
        let artifact_id = links
            .source_artifact_id
            .ok_or_else(|| DbError::Invalid("excerpt Notes require source_artifact_id".into()))?;
        if links.source_locator.is_none() {
            return Err(DbError::Invalid(
                "excerpt Notes require source_locator".into(),
            ));
        }
        let artifact_content: Option<Option<String>> =
            sqlx::query_scalar("SELECT content FROM artifacts WHERE id=$1 AND object_id=$2")
                .bind(artifact_id)
                .bind(links.source_object_ids[0])
                .fetch_optional(pool)
                .await?;
        match artifact_content {
            None => {
                return Err(DbError::Invalid(
                    "source_artifact_id must belong to the excerpt Source".into(),
                ));
            }
            Some(None) => {
                return Err(DbError::Invalid(
                    "excerpt source Artifact must contain captured text".into(),
                ));
            }
            Some(Some(artifact_content)) if !artifact_content.contains(links.content) => {
                return Err(DbError::Invalid(
                    "excerpt content must occur verbatim in the source Artifact".into(),
                ));
            }
            Some(Some(_)) => {}
        }
    } else if links.source_artifact_id.is_some() || links.source_locator.is_some() {
        return Err(DbError::Invalid(
            "source evidence fields are only valid for excerpt Notes".into(),
        ));
    }
    Ok(resolved_chat_object_id)
}

async fn resolve_actor_chat(
    pool: &PgPool,
    actor: &ActorContext,
    intent: &str,
) -> Result<Option<Uuid>, DbError> {
    let Some(thread_key) = actor.centaur_thread_key.as_deref() else {
        return Ok(None);
    };
    let parts = thread_key.split(':').map(str::trim).collect::<Vec<_>>();
    if parts.len() < 4 || parts.iter().any(|part| part.is_empty()) {
        if crate::contract::standalone_note_intent(intent) {
            return Ok(None);
        }
        return Err(DbError::Invalid(
            "authenticated thread key cannot be mapped to a Chat".into(),
        ));
    }
    let provider = parts[0].to_ascii_lowercase();
    let workspace_id = parts[1];
    let channel_id = parts[parts.len() - 2];
    let thread_id = parts[parts.len() - 1];
    Ok(sqlx::query_scalar(
        r#"SELECT c.object_id FROM chats c JOIN objects o ON o.id=c.object_id
           WHERE lower(c.provider)=$1 AND c.workspace_id=$2 AND c.channel_id=$3
             AND c.thread_id=$4 AND o.archived_at IS NULL"#,
    )
    .bind(provider)
    .bind(workspace_id)
    .bind(channel_id)
    .bind(thread_id)
    .fetch_optional(pool)
    .await?)
}

pub(super) async fn insert_note_connection(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    source_object_id: Uuid,
    kind: &str,
    target_object_id: Uuid,
    description: &str,
    provenance: &Value,
) -> Result<Uuid, DbError> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO connections
           (id,source_object_id,kind,target_object_id,description,
            created_by_type,created_by_id,updated_by_type,updated_by_id,provenance,protected)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$6,$7,$8,true)"#,
    )
    .bind(id)
    .bind(source_object_id)
    .bind(kind)
    .bind(target_object_id)
    .bind(description)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(provenance)
    .execute(&mut **tx)
    .await?;
    Ok(id)
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
           WHERE id=$1 AND kind='note' AND revision=$2 AND archived_at IS NULL
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
    sqlx::query("UPDATE notes SET content=$2,content_format=$3 WHERE object_id=$1")
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

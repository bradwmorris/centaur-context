//! Task reads, writes, provenance links, and their atomic mutation history.

use super::*;

pub async fn list_tasks(pool: &PgPool, filter: TaskListFilter) -> Result<Vec<Task>, DbError> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"SELECT o.id AS object_id,o.title,o.description,CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,o.provenance,o.protected,
           t.status,t.priority,t.owner_object_id,t.agent_suitable,t.blocked_reason,t.due_at,
           t.completed_at,t.work_kind,t.github_issue_url,t.brief_markdown,
           o.created_by_type,o.created_by_id,o.created_at,o.updated_at FROM tasks t JOIN objects o ON o.id=t.object_id WHERE o.archived_at IS NULL"#,
    );
    if let Some(status) = filter.status {
        query.push(" AND t.status=").push_bind(status);
    }
    if let Some(agent_suitable) = filter.agent_suitable {
        query
            .push(" AND t.agent_suitable=")
            .push_bind(agent_suitable);
    }
    if let Some(cursor) = filter.cursor {
        push_object_list_cursor(&mut query, cursor, &filter.sort);
    }
    push_object_list_order(&mut query, &filter.sort);
    query.push(" LIMIT ").push_bind(filter.limit);
    Ok(query.build_query_as().fetch_all(pool).await?)
}

pub async fn get_task(pool: &PgPool, id: Uuid) -> Result<Task, DbError> {
    sqlx::query_as(
        r#"SELECT o.id AS object_id,o.title,o.description,CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,o.provenance,o.protected,
           t.status,t.priority,t.owner_object_id,t.agent_suitable,t.blocked_reason,t.due_at,
           t.completed_at,t.work_kind,t.github_issue_url,t.brief_markdown,
           o.created_by_type,o.created_by_id,o.created_at,o.updated_at FROM tasks t JOIN objects o ON o.id=t.object_id WHERE o.id=$1"#,
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
        reconcile_existing_task_links(pool, actor, id, &input, idempotency_key).await?;
        return get_task(pool, id).await;
    }
    validate_object_description(&input.title, &input.description)?;
    let originating_chat_object_id = validate_note_links(
        pool,
        actor,
        input.originating_chat_object_id,
        NoteLinkInput {
            source_object_ids: &input.derived_from_source_object_ids,
            note_object_ids: &[],
            intent: "insight",
            content: "",
            source_artifact_id: None,
            source_locator: None,
        },
    )
    .await?;
    if (input.status == "blocked") != input.blocked_reason.is_some() {
        return Err(DbError::Invalid(
            "blocked_reason is required exactly when status is blocked".into(),
        ));
    }
    if (input.status == "done") != input.completed_at.is_some() {
        return Err(DbError::Invalid(
            "completed_at is required exactly when status is done".into(),
        ));
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
        r#"INSERT INTO tasks
           (object_id,status,priority,owner_object_id,agent_suitable,blocked_reason,
            due_at,completed_at,github_issue_url,brief_markdown,work_kind)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)"#,
    )
    .bind(id)
    .bind(&input.status)
    .bind(&input.priority)
    .bind(input.owner_object_id)
    .bind(input.agent_suitable)
    .bind(&input.blocked_reason)
    .bind(input.due_at)
    .bind(input.completed_at)
    .bind(&input.github_issue_url)
    .bind(&input.brief_markdown)
    .bind(&input.work_kind)
    .execute(&mut *tx)
    .await?;
    let run_id = insert_event(
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
    let mut connection_ids = Vec::new();
    let mut sequence = 2_i64;
    if let Some(chat_object_id) = originating_chat_object_id {
        let connection_id = insert_note_connection(
            &mut tx,
            actor,
            chat_object_id,
            "about",
            id,
            "This conversation requested creation of the resulting Task.",
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
            "This Task follows up on the linked Source.",
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
    get_task(pool, id).await
}

async fn reconcile_existing_task_links(
    pool: &PgPool,
    actor: &ActorContext,
    task_object_id: Uuid,
    input: &NewTask,
    idempotency_key: &str,
) -> Result<(), DbError> {
    let chat_object_id = validate_note_links(
        pool,
        actor,
        input.originating_chat_object_id,
        NoteLinkInput {
            source_object_ids: &input.derived_from_source_object_ids,
            note_object_ids: &[],
            intent: "insight",
            content: "",
            source_artifact_id: None,
            source_locator: None,
        },
    )
    .await?;
    let mut requested = Vec::new();
    if let Some(chat_id) = chat_object_id {
        requested.push((
            chat_id,
            "about",
            task_object_id,
            "This conversation requested creation of the resulting Task.",
        ));
    }
    for source_id in &input.derived_from_source_object_ids {
        requested.push((
            task_object_id,
            "derived_from",
            *source_id,
            "This Task follows up on the linked Source.",
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
    let reconciliation_key = format!("{idempotency_key}:task-links-v1");
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
    .bind(task_object_id)
    .bind(format!(
        "{}:{}:{}",
        actor.actor_type, actor.actor_id, reconciliation_key
    ))
    .bind(json!({
        "centaur_thread_key":actor.centaur_thread_key,
        "centaur_execution_id":actor.centaur_execution_id,
        "target_type":"object",
        "target_id":task_object_id,
        "action":"linked"
    }))
    .bind(json!({"affected_object_ids":[task_object_id]}))
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
    if changes.status.as_deref() == Some("doing") && current.status == "doing" {
        return Err(DbError::Invalid(
            "Task is already in progress; read its execution claim".into(),
        ));
    }
    let work_kind = changes.work_kind.unwrap_or(current.work_kind);
    let status = changes.status.unwrap_or_else(|| current.status.clone());
    let priority = changes.priority.unwrap_or_else(|| current.priority.clone());
    let owner_object_id = changes.owner_object_id.unwrap_or(current.owner_object_id);
    let agent_suitable = changes.agent_suitable.unwrap_or(current.agent_suitable);
    let blocked_reason = if status == "blocked" {
        changes.blocked_reason.unwrap_or(current.blocked_reason)
    } else {
        None
    };
    if status == "blocked" && blocked_reason.is_none() {
        return Err(DbError::Invalid(
            "blocked_reason is required when status is blocked".into(),
        ));
    }
    let due_at = changes.due_at.unwrap_or(current.due_at);
    let completed_at = if status == "done" {
        changes
            .completed_at
            .unwrap_or(current.completed_at)
            .or_else(|| Some(OffsetDateTime::now_utc()))
    } else {
        None
    };
    let github_issue_url = changes.github_issue_url.unwrap_or(current.github_issue_url);
    let brief_changed = changes.brief_markdown.is_some();
    let brief_markdown = changes.brief_markdown.unwrap_or(current.brief_markdown);
    let mut tx = pool.begin().await?;
    let updated: Option<Object> = sqlx::query_as(
        r#"UPDATE objects SET title=$3,description=$4,provenance=$5,protected=$6,revision=revision+1,
           updated_by_type=$7,updated_by_id=$8,updated_at=now()
           WHERE id=$1 AND revision=$2 AND kind='task'
           RETURNING id,kind,title,description,protected,
             CASE WHEN archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,
             revision,created_by_type,created_by_id,updated_by_type,updated_by_id,
             provenance,created_at,updated_at,archived_at"#,
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
        r#"UPDATE tasks SET status=$2,priority=$3,owner_object_id=$4,
           agent_suitable=$5,blocked_reason=$6,due_at=$7,completed_at=$8,
           github_issue_url=$9,brief_markdown=$10,work_kind=$11 WHERE object_id=$1"#,
    )
    .bind(id)
    .bind(&status)
    .bind(&priority)
    .bind(owner_object_id)
    .bind(agent_suitable)
    .bind(&blocked_reason)
    .bind(due_at)
    .bind(completed_at)
    .bind(&github_issue_url)
    .bind(&brief_markdown)
    .bind(&work_kind)
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
        json!({"title": title, "status": status, "priority": priority, "owner_object_id": owner_object_id, "agent_suitable": agent_suitable, "blocked_reason": blocked_reason, "completed_at": completed_at, "github_issue_url": github_issue_url, "brief_changed": brief_changed, "protected": protected}),
    )
    .await?;
    tx.commit().await?;
    get_task(pool, id).await
}

/// Bounded deterministic task queue; semantic relevance is not execution priority.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskQueueFilter {
    pub owner_object_id: Option<Uuid>,
    #[serde(default)]
    pub statuses: Vec<String>,
    pub priority: Option<String>,
    pub due_before: Option<String>,
    #[serde(default)]
    pub ready: bool,
    pub cursor: Option<Uuid>,
}

pub async fn task_queue(
    pool: &PgPool,
    query: &str,
    filter: TaskQueueFilter,
    limit: i64,
) -> Result<Value, DbError> {
    use crate::domain::{TASK_PRIORITIES, TASK_STATUSES, allowed};
    for status in &filter.statuses {
        allowed(status.clone(), "status", TASK_STATUSES)?;
    }
    if let Some(priority) = &filter.priority {
        allowed(priority.clone(), "priority", TASK_PRIORITIES)?;
    }
    let due_before = filter
        .due_before
        .map(|v| {
            OffsetDateTime::parse(&v, &time::format_description::well_known::Rfc3339)
                .map_err(|_| DbError::Invalid("due_before must be RFC3339".into()))
        })
        .transpose()?;
    let rows: Vec<Value> = sqlx::query_scalar(r#"
      WITH eligible AS (
        SELECT o.id,o.title,o.description,o.revision,o.created_by_type,o.created_by_id,
          (to_jsonb(t) - 'brief_markdown') || jsonb_build_object('has_brief', nullif(btrim(t.brief_markdown),'') IS NOT NULL) AS task, t.due_at,
          CASE t.priority WHEN 'high' THEN 0 WHEN 'medium' THEN 1 ELSE 2 END AS rank
        FROM tasks t JOIN objects o ON o.id=t.object_id
        WHERE o.archived_at IS NULL
          AND ($1::uuid IS NULL OR t.owner_object_id=$1)
          AND (cardinality($2::text[])=0 OR t.status=ANY($2))
          AND ($3::text IS NULL OR t.priority=$3)
          AND ($4::timestamptz IS NULL OR t.due_at <= $4)
          AND ($5='' OR strpos(lower(o.title || ' ' || o.description),lower($5))>0)
          AND (NOT $6 OR (t.status IN ('backlog','todo') AND t.agent_suitable
            AND t.due_at IS NOT NULL AND nullif(btrim(t.brief_markdown),'') IS NOT NULL
            AND EXISTS (SELECT 1 FROM users u JOIN objects a ON a.id=u.object_id
                        WHERE a.id=t.owner_object_id AND a.archived_at IS NULL)
            AND NOT EXISTS (SELECT 1 FROM connections c JOIN tasks dep ON dep.object_id=c.target_object_id
                            WHERE c.source_object_id=o.id AND c.kind='depends_on'
                              AND c.archived_at IS NULL AND dep.status<>'done')))
      ), numbered AS (
        SELECT *, row_number() OVER (ORDER BY rank,due_at NULLS LAST,id) AS position FROM eligible
      ) SELECT jsonb_build_object('id',id,'kind','task','title',title,'description',description,
        'revision',revision,'created_by_type',created_by_type,'created_by_id',created_by_id,'subtype',task)
        FROM numbered WHERE $7::uuid IS NULL OR position > (SELECT position FROM numbered WHERE id=$7)
        ORDER BY position LIMIT $8
    "#).bind(filter.owner_object_id).bind(filter.statuses).bind(filter.priority).bind(due_before)
       .bind(query.trim()).bind(filter.ready).bind(filter.cursor).bind(limit+1).fetch_all(pool).await?;
    let more = rows.len() > limit as usize;
    let rows: Vec<Value> = rows.into_iter().take(limit as usize).collect();
    let next = if more {
        rows.last().map(|v| v["id"].clone())
    } else {
        None
    };
    Ok(
        json!({"objects": rows, "next_cursor": next, "retrieval":"task_queue", "query":query,
      "contract_version":crate::contract::version(),"tool_version":crate::contract::tool_version()}),
    )
}

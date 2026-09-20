//! Idempotent mutation Runs and immutable Object Event snapshots.

use super::*;

pub(super) async fn idempotent_entity(
    pool: &PgPool,
    actor: &ActorContext,
    key: &str,
) -> Result<Option<Uuid>, DbError> {
    Ok(sqlx::query_scalar(
        "SELECT target_id FROM object_events WHERE actor_type=$1 AND actor_id=$2 AND idempotency_key=$3",
    )
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(key)
    .fetch_optional(pool)
    .await?)
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn insert_event(
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
) -> Result<Uuid, DbError> {
    let target_type = if entity_type == "connection" {
        "connection"
    } else {
        "object"
    };
    let target_id = if target_type == "connection" {
        entity_id
    } else {
        object_id
    };
    let run_id = Uuid::new_v4();
    let run_key = idempotency_key
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| run_id.to_string());
    let input = json!({
        "centaur_thread_key": actor.centaur_thread_key,
        "centaur_execution_id": actor.centaur_execution_id,
        "target_type": target_type,
        "target_id": target_id,
        "action": action
    });
    sqlx::query(
        r#"INSERT INTO runs
           (id,kind,status,actor_type,actor_id,primary_object_id,idempotency_key,input,result,completed_at)
           VALUES ($1,'mutation','completed',$2,$3,$4,$5,$6,$7,now())"#,
    )
    .bind(run_id)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(object_id)
    .bind(format!(
        "{}:{}:{}",
        actor.actor_type, actor.actor_id, run_key
    ))
    .bind(input)
    .bind(json!({"affected_object_ids":[object_id],"summary":changes.clone()}))
    .execute(&mut **tx)
    .await?;
    insert_event_for_run(
        tx,
        run_id,
        1,
        actor,
        entity_type,
        entity_id,
        object_id,
        action,
        idempotency_key,
        from_revision,
        to_revision,
    )
    .await?;
    Ok(run_id)
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert_event_for_run(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    sequence: i64,
    actor: &ActorContext,
    entity_type: &str,
    entity_id: Uuid,
    object_id: Uuid,
    action: &str,
    idempotency_key: Option<&str>,
    from_revision: Option<i64>,
    to_revision: i64,
) -> Result<Uuid, DbError> {
    let target_type = if entity_type == "connection" {
        "connection"
    } else {
        "object"
    };
    let target_id = if target_type == "connection" {
        entity_id
    } else {
        object_id
    };
    let before_state: Option<Value> = if from_revision.is_some() {
        sqlx::query_scalar(
            "SELECT after_state FROM object_events WHERE target_type=$1 AND target_id=$2 ORDER BY created_at DESC,id DESC LIMIT 1",
        )
        .bind(target_type)
        .bind(target_id)
        .fetch_optional(&mut **tx)
        .await?
    } else {
        None
    };
    let after_state = target_snapshot(tx, target_type, target_id).await?;
    let event_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO object_events
           (id,run_id,sequence,target_type,target_id,action,actor_type,actor_id,
            idempotency_key,from_revision,to_revision,before_state,after_state,reversible,created_at)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,true,now())"#,
    )
    .bind(event_id)
    .bind(run_id)
    .bind(sequence)
    .bind(target_type)
    .bind(target_id)
    .bind(action)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(idempotency_key)
    .bind(from_revision)
    .bind(to_revision)
    .bind(before_state)
    .bind(after_state)
    .execute(&mut **tx)
    .await?;
    Ok(event_id)
}

pub(crate) async fn target_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    target_type: &str,
    target_id: Uuid,
) -> Result<Value, DbError> {
    if target_type == "connection" {
        return Ok(
            sqlx::query_scalar("SELECT to_jsonb(c) FROM connections c WHERE id=$1")
                .bind(target_id)
                .fetch_optional(&mut **tx)
                .await?
                .unwrap_or_else(|| json!({"id":target_id,"archived":true})),
        );
    }
    Ok(sqlx::query_scalar(
        r#"SELECT to_jsonb(o)
          || jsonb_build_object('subtype', CASE o.kind
            WHEN 'task' THEN (SELECT to_jsonb(t)-'object_id' FROM tasks t WHERE t.object_id=o.id)
            WHEN 'chat' THEN (SELECT to_jsonb(c)-'object_id' FROM chats c WHERE c.object_id=o.id)
            WHEN 'user' THEN (SELECT to_jsonb(u)-'object_id' FROM users u WHERE u.object_id=o.id)
            WHEN 'entity' THEN (SELECT to_jsonb(e)-'object_id' FROM entities e WHERE e.object_id=o.id)
            WHEN 'memory' THEN (SELECT to_jsonb(m)-'object_id' FROM memories m WHERE m.object_id=o.id)
            WHEN 'source' THEN (SELECT to_jsonb(s)-'object_id' FROM sources s WHERE s.object_id=o.id)
            WHEN 'note' THEN (SELECT to_jsonb(n)-'object_id' FROM notes n WHERE n.object_id=o.id)
            WHEN 'theme' THEN (SELECT to_jsonb(t)-'object_id' FROM themes t WHERE t.object_id=o.id)
          END)
          || jsonb_build_object('artifacts',COALESCE(
            (SELECT jsonb_agg(to_jsonb(a)-'content' ORDER BY a.created_at,a.id) FROM artifacts a WHERE a.object_id=o.id),'[]'::jsonb))
          FROM objects o WHERE o.id=$1"#,
    )
    .bind(target_id)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or_else(|| json!({"id":target_id,"archived":true})))
}

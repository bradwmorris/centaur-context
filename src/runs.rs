use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::db::{DbError, ObjectEvent};

pub const VERDICTS: &[&str] = &["unreviewed", "pass", "mixed", "fail"];

#[derive(Clone, Debug, Deserialize, Default)]
pub struct RunFilter {
    pub kind: Option<String>,
    pub status: Option<String>,
    pub verdict: Option<String>,
    pub component: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub execution_type: Option<String>,
    pub auth_mode: Option<String>,
    pub billing_mode: Option<String>,
    pub object_id: Option<Uuid>,
    pub from: Option<OffsetDateTime>,
    pub to: Option<OffsetDateTime>,
    pub before: Option<OffsetDateTime>,
    pub before_id: Option<Uuid>,
    pub root_only: bool,
    pub pinned: Option<bool>,
    pub limit: i64,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct RunSummary {
    pub id: Uuid,
    pub parent_run_id: Option<Uuid>,
    pub kind: String,
    pub status: String,
    pub actor_type: String,
    pub actor_id: String,
    pub chat_object_id: Option<Uuid>,
    pub primary_object_id: Option<Uuid>,
    pub idempotency_key: String,
    pub input: Value,
    pub trace: Value,
    pub result: Value,
    pub consulted_object_ids: Vec<Uuid>,
    pub error: Option<String>,
    pub verdict: String,
    pub review_notes: Option<String>,
    pub pinned: bool,
    pub reviewed_by: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub reviewed_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub available_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub completed_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct RunObject {
    pub object_id: Uuid,
    pub role: String,
    pub kind: String,
    pub title: String,
    pub lifecycle: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RunDetail {
    pub run: RunSummary,
    pub children: Vec<RunSummary>,
    pub objects: Vec<RunObject>,
    pub events: Vec<ObjectEvent>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRunStart {
    pub run_id: Uuid,
    pub workflow_name: String,
    pub source_kind: String,
    pub source_locator: Option<String>,
    pub requested_title: Option<String>,
    pub chat_object_id: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowTraceEntry {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub entry_type: String,
    pub name: String,
    pub status: String,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub completed_at: OffsetDateTime,
    pub duration_ms: i64,
    pub component: String,
    pub provider: Option<String>,
    pub model_id: Option<String>,
    pub reasoning_effort: Option<String>,
    pub source_execution_id: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    #[serde(default = "empty_object")]
    pub facts: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRunFinish {
    pub status: String,
    pub primary_object_id: Option<Uuid>,
    pub child_run_id: Option<Uuid>,
    #[serde(default = "empty_object")]
    pub result: Value,
    pub error: Option<String>,
}

fn empty_object() -> Value {
    json!({})
}

const RUN_SELECT: &str = r#"SELECT id,parent_run_id,kind,status,actor_type,actor_id,
 chat_object_id,primary_object_id,idempotency_key,input,trace,result,consulted_object_ids,error,verdict,
 review_notes,pinned,reviewed_by,reviewed_at,available_at,started_at,completed_at,created_at,updated_at
 FROM runs"#;

pub async fn list(pool: &PgPool, filter: RunFilter) -> Result<Vec<RunSummary>, DbError> {
    let mut query = QueryBuilder::<Postgres>::new(RUN_SELECT);
    query.push(" WHERE true");
    if let Some(value) = filter.kind {
        query.push(" AND kind=").push_bind(value);
    }
    if let Some(value) = filter.status {
        query.push(" AND status=").push_bind(value);
    }
    if let Some(value) = filter.verdict {
        query.push(" AND verdict=").push_bind(value);
    }
    if filter.root_only {
        query.push(" AND parent_run_id IS NULL");
    }
    if let Some(value) = filter.pinned {
        query.push(" AND pinned=").push_bind(value);
    }
    if let Some(value) = filter.from {
        query.push(" AND created_at>=").push_bind(value);
    }
    if let Some(value) = filter.to {
        query.push(" AND created_at<=").push_bind(value);
    }
    if let Some(value) = filter.before {
        if let Some(id) = filter.before_id {
            query
                .push(" AND (created_at,id)<(")
                .push_bind(value)
                .push(",")
                .push_bind(id)
                .push(")");
        } else {
            query.push(" AND created_at<").push_bind(value);
        }
    }
    if let Some(value) = filter.object_id {
        query
            .push(" AND (")
            .push_bind(value)
            .push("=primary_object_id OR ")
            .push_bind(value)
            .push("=chat_object_id OR ")
            .push_bind(value)
            .push("=ANY(consulted_object_ids) OR EXISTS(SELECT 1 FROM object_events e WHERE e.run_id=runs.id AND e.target_type='object' AND e.target_id=")
            .push_bind(value)
            .push("))");
    }
    for (key, value) in [
        ("component", filter.component),
        ("provider", filter.provider),
        ("model_id", filter.model),
        ("execution_type", filter.execution_type),
        ("auth_mode", filter.auth_mode),
        ("billing_mode", filter.billing_mode),
    ] {
        if let Some(value) = value {
            query
                .push(" AND EXISTS(SELECT 1 FROM jsonb_array_elements(trace) entry WHERE entry->>")
                .push_bind(key)
                .push("=")
                .push_bind(value)
                .push(")");
        }
    }
    query.push(" ORDER BY created_at DESC,id DESC LIMIT ");
    query.push_bind(filter.limit.clamp(1, 100));
    Ok(query.build_query_as().fetch_all(pool).await?)
}

pub async fn detail(pool: &PgPool, id: Uuid) -> Result<RunDetail, DbError> {
    let run = sqlx::query_as::<_, RunSummary>(&format!("{RUN_SELECT} WHERE id=$1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)?;
    let children = sqlx::query_as::<_, RunSummary>(&format!(
        "{RUN_SELECT} WHERE parent_run_id=$1 ORDER BY created_at,id"
    ))
    .bind(id)
    .fetch_all(pool)
    .await?;
    let objects = sqlx::query_as(
        r#"WITH RECURSIVE run_tree AS (
             SELECT $1::uuid id
             UNION ALL SELECT child.id FROM runs child JOIN run_tree parent ON child.parent_run_id=parent.id
           ), linked AS (
             SELECT $3::uuid object_id,'primary'::text role,1 priority WHERE $3 IS NOT NULL
             UNION ALL SELECT $4::uuid,'origin_chat',2 WHERE $4 IS NOT NULL
             UNION ALL SELECT unnest($2::uuid[]),'consulted',3
             UNION ALL SELECT target_id,'changed',4 FROM object_events
               WHERE run_id IN (SELECT id FROM run_tree) AND target_type='object'
             UNION ALL SELECT c.source_object_id,'connected',5
               FROM object_events e JOIN connections c ON c.id=e.target_id
               WHERE e.run_id IN (SELECT id FROM run_tree) AND e.target_type='connection'
             UNION ALL SELECT c.target_object_id,'connected',5
               FROM object_events e JOIN connections c ON c.id=e.target_id
               WHERE e.run_id IN (SELECT id FROM run_tree) AND e.target_type='connection'
           ), ranked AS (
             SELECT DISTINCT ON (object_id) object_id,role,priority
             FROM linked WHERE object_id IS NOT NULL
             ORDER BY object_id,priority
           ) SELECT ranked.object_id,ranked.role,o.kind,o.title,
             CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END lifecycle
           FROM ranked JOIN objects o ON o.id=ranked.object_id
           ORDER BY ranked.priority,o.title,o.id"#,
    )
    .bind(id)
    .bind(&run.consulted_object_ids)
    .bind(run.primary_object_id)
    .bind(run.chat_object_id)
    .fetch_all(pool)
    .await?;
    let events = sqlx::query_as(
        r#"WITH RECURSIVE run_tree AS (
             SELECT $1::uuid id
             UNION ALL SELECT child.id FROM runs child JOIN run_tree parent ON child.parent_run_id=parent.id
           ) SELECT e.* FROM object_events e
           WHERE e.run_id IN (SELECT id FROM run_tree)
           ORDER BY e.created_at,e.run_id,e.sequence"#,
    )
        .bind(id)
        .fetch_all(pool)
        .await?;
    Ok(RunDetail {
        run,
        children,
        objects,
        events,
    })
}

impl WorkflowRunStart {
    pub fn validate(&mut self) -> Result<(), String> {
        self.workflow_name = bounded_text(&self.workflow_name, "workflow_name", 200)?;
        if self.workflow_name != "enyu_source_ingestion" {
            return Err("workflow_name is not approved for this listener".into());
        }
        self.source_kind = bounded_text(&self.source_kind, "source_kind", 50)?;
        if !["url", "file", "text"].contains(&self.source_kind.as_str()) {
            return Err("source_kind is unsupported".into());
        }
        self.source_locator = optional_bounded(self.source_locator.take(), "source_locator", 2000)?;
        self.requested_title =
            optional_bounded(self.requested_title.take(), "requested_title", 500)?;
        Ok(())
    }
}

impl WorkflowTraceEntry {
    pub fn validate(&mut self) -> Result<(), String> {
        self.entry_type = bounded_text(&self.entry_type, "entry_type", 100)?;
        if ![
            "workflow_step",
            "agent_turn",
            "model_attempt",
            "tool_call",
            "message",
            "child_run",
        ]
        .contains(&self.entry_type.as_str())
        {
            return Err("entry_type is unsupported".into());
        }
        self.name = bounded_text(&self.name, "name", 200)?;
        self.status = bounded_text(&self.status, "status", 50)?;
        if !["completed", "failed"].contains(&self.status.as_str()) {
            return Err("trace status is unsupported".into());
        }
        self.component = bounded_text(&self.component, "component", 100)?;
        self.provider = optional_bounded(self.provider.take(), "provider", 100)?;
        self.model_id = optional_bounded(self.model_id.take(), "model_id", 200)?;
        self.reasoning_effort =
            optional_bounded(self.reasoning_effort.take(), "reasoning_effort", 100)?;
        self.source_execution_id =
            optional_bounded(self.source_execution_id.take(), "source_execution_id", 300)?;
        if self.completed_at < self.started_at {
            return Err("completed_at must not precede started_at".into());
        }
        if !(0..=86_400_000).contains(&self.duration_ms) {
            return Err("duration_ms must be between zero and one day".into());
        }
        let tokens = [
            self.input_tokens,
            self.output_tokens,
            self.cache_read_tokens,
            self.reasoning_tokens,
            self.total_tokens,
        ];
        if tokens.iter().flatten().any(|value| *value < 0) {
            return Err("token counts must not be negative".into());
        }
        if let (Some(input), Some(output), Some(total)) =
            (self.input_tokens, self.output_tokens, self.total_tokens)
        {
            let minimum = input
                .checked_add(output)
                .ok_or_else(|| "token counts are too large".to_owned())?;
            if total < minimum {
                return Err("total_tokens must include input_tokens and output_tokens".into());
            }
        }
        let facts_size = serde_json::to_vec(&self.facts)
            .map_err(|error| format!("facts could not be encoded: {error}"))?
            .len();
        if !self.facts.is_object() || facts_size > 32_768 {
            return Err("facts must be an object no larger than 32 KiB".into());
        }
        Ok(())
    }
}

impl WorkflowRunFinish {
    pub fn validate(&mut self) -> Result<(), String> {
        self.status = bounded_text(&self.status, "status", 50)?;
        if !["completed", "failed"].contains(&self.status.as_str()) {
            return Err("workflow status is unsupported".into());
        }
        let result_size = serde_json::to_vec(&self.result)
            .map_err(|error| format!("result could not be encoded: {error}"))?
            .len();
        if !self.result.is_object() || result_size > 1_048_576 {
            return Err("result must be an object no larger than 1 MiB".into());
        }
        self.error = optional_bounded(self.error.take(), "error", 10_000)?;
        if self.status == "failed" && self.error.is_none() {
            return Err("failed workflows require an error".into());
        }
        if self.status == "completed" && self.error.is_some() {
            return Err("completed workflows cannot include an error".into());
        }
        Ok(())
    }
}

fn bounded_text(value: &str, field: &str, max: usize) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.len() > max {
        return Err(format!("{field} must contain 1 to {max} characters"));
    }
    Ok(value.to_owned())
}

fn optional_bounded(
    value: Option<String>,
    field: &str,
    max: usize,
) -> Result<Option<String>, String> {
    value
        .map(|value| bounded_text(&value, field, max))
        .transpose()
}

pub async fn start_workflow_run(
    pool: &PgPool,
    actor_type: &str,
    actor_id: &str,
    input: &WorkflowRunStart,
) -> Result<RunSummary, DbError> {
    let run_input = json!({
        "workflow_name": input.workflow_name,
        "source_kind": input.source_kind,
        "source_locator": input.source_locator,
        "requested_title": input.requested_title,
    });
    sqlx::query(
        r#"INSERT INTO runs (
             id,kind,status,actor_type,actor_id,chat_object_id,idempotency_key,input,
             result,available_at,started_at
           ) VALUES ($1,'workflow','running',$2,$3,$4,$5,$6,
             jsonb_build_object('summary','Source ingestion is running.'),now(),now())
           ON CONFLICT (id) DO NOTHING"#,
    )
    .bind(input.run_id)
    .bind(actor_type)
    .bind(actor_id)
    .bind(input.chat_object_id)
    .bind(format!("workflow:{}", input.run_id))
    .bind(&run_input)
    .execute(pool)
    .await?;
    let run = detail(pool, input.run_id).await?.run;
    if run.kind != "workflow"
        || run.actor_type != actor_type
        || run.actor_id != actor_id
        || run.input != run_input
        || run.chat_object_id != input.chat_object_id
    {
        return Err(DbError::Conflict);
    }
    Ok(run)
}

pub async fn append_workflow_trace(
    pool: &PgPool,
    run_id: Uuid,
    entry: &WorkflowTraceEntry,
) -> Result<Uuid, DbError> {
    let value = serde_json::to_value(entry).map_err(|error| DbError::Invalid(error.to_string()))?;
    let updated = sqlx::query_scalar::<_, Uuid>(
        r#"UPDATE runs
           SET trace=trace||jsonb_build_array($2::jsonb),updated_at=now()
           WHERE id=$1 AND kind='workflow'
             AND NOT EXISTS(
               SELECT 1 FROM jsonb_array_elements(trace) item WHERE item->>'id'=$3
             )
           RETURNING id"#,
    )
    .bind(run_id)
    .bind(value)
    .bind(entry.id.to_string())
    .fetch_optional(pool)
    .await?;
    if updated.is_some() {
        return Ok(entry.id);
    }
    let existing: Option<bool> = sqlx::query_scalar(
        r#"SELECT EXISTS(
             SELECT 1 FROM jsonb_array_elements(trace) item WHERE item->>'id'=$2
           ) FROM runs WHERE id=$1 AND kind='workflow'"#,
    )
    .bind(run_id)
    .bind(entry.id.to_string())
    .fetch_optional(pool)
    .await?;
    match existing {
        Some(true) => Ok(entry.id),
        Some(false) => Err(DbError::Conflict),
        None => Err(DbError::NotFound),
    }
}

pub async fn finish_workflow_run(
    pool: &PgPool,
    run_id: Uuid,
    input: &WorkflowRunFinish,
) -> Result<RunSummary, DbError> {
    let mut tx = pool.begin().await?;
    let current: Option<(String, String)> =
        sqlx::query_as("SELECT kind,status FROM runs WHERE id=$1 FOR UPDATE")
            .bind(run_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((kind, status)) = current else {
        return Err(DbError::NotFound);
    };
    if kind != "workflow" {
        return Err(DbError::Conflict);
    }
    if status != "running" {
        if status == input.status {
            tx.commit().await?;
            return Ok(detail(pool, run_id).await?.run);
        }
        return Err(DbError::Conflict);
    }
    if let Some(child_run_id) = input.child_run_id {
        let linked = sqlx::query(
            r#"UPDATE runs SET parent_run_id=$1,updated_at=now()
               WHERE id=$2 AND kind='intake'
                 AND input->>'batch_id'='workflow:'||$1::text
                 AND (parent_run_id IS NULL OR parent_run_id=$1)"#,
        )
        .bind(run_id)
        .bind(child_run_id)
        .execute(&mut *tx)
        .await?;
        if linked.rows_affected() != 1 {
            return Err(DbError::Conflict);
        }
    }
    let mut result = input.result.clone();
    let result_object = result.as_object_mut().expect("validated workflow result");
    if let Some(child_run_id) = input.child_run_id {
        result_object.insert("child_run_id".into(), json!(child_run_id));
    }
    sqlx::query(
        r#"UPDATE runs SET status=$2,primary_object_id=$3,result=result||$4,
             error=$5,completed_at=now(),updated_at=now()
           WHERE id=$1"#,
    )
    .bind(run_id)
    .bind(&input.status)
    .bind(input.primary_object_id)
    .bind(result)
    .bind(&input.error)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(detail(pool, run_id).await?.run)
}

pub async fn review(
    pool: &PgPool,
    id: Uuid,
    verdict: &str,
    notes: Option<&str>,
    pinned: Option<bool>,
    reviewer: &str,
    expected_revision: i64,
) -> Result<RunSummary, DbError> {
    let updated = sqlx::query_scalar::<_, Uuid>(
        r#"UPDATE runs SET verdict=$2,review_notes=$3,pinned=COALESCE($4,pinned),
           reviewed_by=$5,reviewed_at=now(),
           result=jsonb_set(result,'{review_revision}',to_jsonb($6::bigint+1),true),updated_at=now()
           WHERE id=$1 AND COALESCE((result->>'review_revision')::bigint,0)=$6 RETURNING id"#,
    )
    .bind(id)
    .bind(verdict)
    .bind(notes)
    .bind(pinned)
    .bind(reviewer)
    .bind(expected_revision)
    .fetch_optional(pool)
    .await?
    .ok_or(DbError::Conflict)?;
    Ok(detail(pool, updated).await?.run)
}

pub async fn set_context(tx: &mut Transaction<'_, Postgres>, run_id: Uuid) -> Result<(), DbError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM runs WHERE id=$1)")
        .bind(run_id)
        .fetch_one(&mut **tx)
        .await?;
    if exists {
        Ok(())
    } else {
        Err(DbError::NotFound)
    }
}

pub async fn open_slack_interaction(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: &str,
    channel_id: &str,
    thread_id: &str,
    interaction_id: &str,
    started_at: OffsetDateTime,
    title: String,
) -> Result<(Uuid, bool), DbError> {
    let key = format!("slack-turn:{workspace_id}:{channel_id}:{thread_id}:{interaction_id}");
    if let Some(id) = sqlx::query_scalar(
        "SELECT id FROM runs WHERE kind='slack_interaction' AND idempotency_key=$1 FOR UPDATE",
    )
    .bind(&key)
    .fetch_optional(&mut **tx)
    .await?
    {
        return Ok((id, false));
    }
    let id = Uuid::new_v4();
    sqlx::query(r#"INSERT INTO runs (id,kind,status,actor_type,actor_id,idempotency_key,input,result,available_at,started_at)
      VALUES ($1,'slack_interaction','running','system','chat-ingestor',$2,$3,$4,now(),$5)"#)
      .bind(id).bind(key).bind(json!({"workspace_id":workspace_id,"channel_id":channel_id,"thread_id":thread_id,"interaction_id":interaction_id,"title":title}))
      .bind(json!({"summary":"The bot interaction is running."})).bind(started_at).execute(&mut **tx).await?;
    append_external_trace(
        tx,
        id,
        &json!({
            "id":format!("{id}:interaction-started"),
            "entry_type":"interaction",
            "name":"interaction started",
            "status":"completed",
            "component":"slackbotv2",
            "facts":{"description":"The bot accepted the message."}
        }),
    )
    .await?;
    Ok((id, true))
}

pub async fn attach_slack_chat(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    chat_object_id: Uuid,
) -> Result<(), DbError> {
    sqlx::query(r#"UPDATE runs SET chat_object_id=$2,
      consulted_object_ids=CASE WHEN $2=ANY(consulted_object_ids) THEN consulted_object_ids ELSE array_append(consulted_object_ids,$2) END,
      updated_at=now() WHERE id=$1"#)
      .bind(run_id).bind(chat_object_id).execute(&mut **tx).await?;
    Ok(())
}

pub async fn link_object(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    object_id: Uuid,
    role: &str,
) -> Result<(), DbError> {
    if matches!(role, "consulted" | "participant") {
        sqlx::query("UPDATE runs SET consulted_object_ids=CASE WHEN $2=ANY(consulted_object_ids) THEN consulted_object_ids ELSE array_append(consulted_object_ids,$2) END,updated_at=now() WHERE id=$1")
            .bind(run_id).bind(object_id).execute(&mut **tx).await?;
    } else {
        sqlx::query(r#"UPDATE runs SET result=jsonb_set(result,'{affected_object_ids}',
          COALESCE(result->'affected_object_ids','[]'::jsonb) || to_jsonb($2::uuid),true),updated_at=now() WHERE id=$1"#)
          .bind(run_id).bind(object_id).execute(&mut **tx).await?;
    }
    Ok(())
}

pub async fn append_trace(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    entry_type: &str,
    facts: Value,
) -> Result<(), DbError> {
    let entry = json!({"id":Uuid::new_v4(),"entry_type":entry_type,"facts":facts,"created_at":OffsetDateTime::now_utc()});
    let updated = sqlx::query(
        "UPDATE runs SET trace=trace || jsonb_build_array($2::jsonb),updated_at=now() WHERE id=$1",
    )
    .bind(run_id)
    .bind(entry)
    .execute(&mut **tx)
    .await?;
    if updated.rows_affected() == 1 {
        Ok(())
    } else {
        Err(DbError::NotFound)
    }
}

pub async fn append_external_trace(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    entry: &Value,
) -> Result<(), DbError> {
    let id = entry
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty() && value.len() <= 500)
        .ok_or_else(|| {
            DbError::Validation(crate::domain::ValidationError::Required("run.trace.id"))
        })?;
    sqlx::query(
        r#"UPDATE runs SET trace=trace||jsonb_build_array($2::jsonb),updated_at=now()
           WHERE id=$1 AND NOT EXISTS (
             SELECT 1 FROM jsonb_array_elements(trace) item WHERE item->>'id'=$3
           )"#,
    )
    .bind(run_id)
    .bind(entry)
    .bind(id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn finish_slack_interaction(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    status: &str,
    completed_at: Option<OffsetDateTime>,
    error: Option<&str>,
    affected_object_ids: &[Uuid],
) -> Result<(), DbError> {
    if status == "running" {
        return Ok(());
    }
    let primary_object_id = affected_object_ids.first().copied();
    let summary = if status == "failed" {
        "The bot interaction failed."
    } else if affected_object_ids.is_empty() {
        "The bot replied without changing a Context Object."
    } else {
        "The bot completed the interaction and changed Context Objects."
    };
    sqlx::query(
        r#"UPDATE runs SET status=$2,primary_object_id=COALESCE(primary_object_id,$3),
           error=$4,result=result||jsonb_build_object(
             'summary',$5,'affected_object_ids',to_jsonb($6::uuid[])
           ),completed_at=COALESCE($7,now()),updated_at=now()
           WHERE id=$1 AND kind='slack_interaction' AND status='running'"#,
    )
    .bind(run_id)
    .bind(status)
    .bind(primary_object_id)
    .bind(error)
    .bind(summary)
    .bind(affected_object_ids)
    .bind(completed_at)
    .execute(&mut **tx)
    .await?;
    append_external_trace(
        tx,
        run_id,
        &json!({
            "id":format!("{run_id}:interaction-finished"),
            "entry_type":"interaction",
            "name":"interaction finished",
            "status":status,
            "component":"slackbotv2",
            "facts":{"description":summary}
        }),
    )
    .await?;
    Ok(())
}

pub async fn attach_curator_run(
    tx: &mut Transaction<'_, Postgres>,
    interaction_run_id: Uuid,
    curator_run_id: Uuid,
) -> Result<(), DbError> {
    sqlx::query("UPDATE runs SET parent_run_id=$2 WHERE id=$1")
        .bind(curator_run_id)
        .bind(interaction_run_id)
        .execute(&mut **tx)
        .await?;
    append_trace(
        tx,
        interaction_run_id,
        "curator_queued",
        json!({"curator_run_id":curator_run_id}),
    )
    .await
}

pub async fn latest_slack_run(
    tx: &mut Transaction<'_, Postgres>,
    chat_object_id: Uuid,
) -> Result<Option<Uuid>, DbError> {
    Ok(sqlx::query_scalar("SELECT id FROM runs WHERE kind='slack_interaction' AND chat_object_id=$1 AND parent_run_id IS NULL ORDER BY created_at DESC LIMIT 1 FOR UPDATE")
       .bind(chat_object_id).fetch_optional(&mut **tx).await?)
}

pub async fn finish_curator_run(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    change_count: i32,
) -> Result<(), DbError> {
    append_trace(tx, run_id, "commit", json!({"change_count":change_count})).await?;
    sqlx::query("UPDATE runs SET status='completed',error=NULL,result=result || jsonb_build_object('change_count',$2),completed_at=now(),updated_at=now() WHERE id=$1")
        .bind(run_id).bind(change_count).execute(&mut **tx).await?;
    Ok(())
}

pub async fn fail_curator_run(pool: &PgPool, run_id: Uuid, message: &str) -> Result<(), DbError> {
    let mut tx = pool.begin().await?;
    let summary = message.chars().take(4000).collect::<String>();
    append_trace(&mut tx, run_id, "failure", json!({"error":summary})).await?;
    sqlx::query(
        "UPDATE runs SET status='failed',error=$2,completed_at=now(),updated_at=now() WHERE id=$1",
    )
    .bind(run_id)
    .bind(summary)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn append_curator_trace(
    pool: &PgPool,
    run_id: Uuid,
    entry_type: &str,
    facts: Value,
) -> Result<(), DbError> {
    let mut tx = pool.begin().await?;
    append_trace(&mut tx, run_id, entry_type, facts).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn link_curator_candidates(
    pool: &PgPool,
    run_id: Uuid,
    object_ids: &[Uuid],
) -> Result<(), DbError> {
    let mut tx = pool.begin().await?;
    for object_id in object_ids {
        link_object(&mut tx, run_id, *object_id, "consulted").await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn reverse_curator_run(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    reversal_run_id: Uuid,
    reversed_change_count: usize,
) -> Result<(), DbError> {
    append_trace(
        tx,
        reversal_run_id,
        "reversal",
        json!({"reverses_run_id":run_id,"reversed_change_count":reversed_change_count}),
    )
    .await?;
    sqlx::query("UPDATE runs SET status='reversed',result=result || jsonb_build_object('reversal_run_id',$2::uuid),updated_at=now() WHERE id=$1")
        .bind(run_id).bind(reversal_run_id).execute(&mut **tx).await?;
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NormalizedUsage {
    pub run_id: Uuid,
    pub component: String,
    pub provider: String,
    pub model_id: String,
    pub display_tier: Option<String>,
    pub execution_type: String,
    pub auth_mode: String,
    pub upstream_service: String,
    pub billing_mode: String,
    pub reasoning_effort: Option<String>,
    pub service_tier: Option<String>,
    pub source_thread_id: Option<String>,
    pub source_execution_id: String,
    pub source_turn_id: Option<String>,
    pub usage_status: String,
    pub usage_missing_reason: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_creation_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub estimated_micro_usd: Option<i64>,
    pub chatgpt_credit_microunits: Option<i64>,
    pub api_equivalent_micro_usd: Option<i64>,
    pub rate_card_version: Option<String>,
    pub pricing_snapshot: Option<Value>,
}

impl NormalizedUsage {
    pub fn validate(&self) -> Result<(), String> {
        for (field, value, max) in [
            ("component", self.component.as_str(), 100),
            ("provider", self.provider.as_str(), 100),
            ("model_id", self.model_id.as_str(), 200),
            ("upstream_service", self.upstream_service.as_str(), 200),
            (
                "source_execution_id",
                self.source_execution_id.as_str(),
                300,
            ),
        ] {
            if value.trim().is_empty() || value.len() > max {
                return Err(format!("{field} must contain 1 to {max} characters"));
            }
        }
        if !["codex_harness", "direct_api", "embedding", "other"]
            .contains(&self.execution_type.as_str())
        {
            return Err("execution_type is unsupported".into());
        }
        if ![
            "chatgpt_subscription",
            "api_key",
            "not_applicable",
            "unknown",
        ]
        .contains(&self.auth_mode.as_str())
        {
            return Err("auth_mode is unsupported".into());
        }
        if ![
            "subscription_allowance",
            "chatgpt_credits",
            "metered_api",
            "not_applicable",
            "unknown",
        ]
        .contains(&self.billing_mode.as_str())
        {
            return Err("billing_mode is unsupported".into());
        }
        if !["reported", "partial", "unavailable", "not_applicable"]
            .contains(&self.usage_status.as_str())
        {
            return Err("usage_status is unsupported".into());
        }
        if self.usage_status == "reported"
            && self.input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.total_tokens.is_none()
        {
            return Err("reported usage requires at least one token count".into());
        }
        let numeric = [
            self.input_tokens,
            self.output_tokens,
            self.cache_creation_tokens,
            self.cache_read_tokens,
            self.reasoning_tokens,
            self.total_tokens,
            self.estimated_micro_usd,
            self.chatgpt_credit_microunits,
            self.api_equivalent_micro_usd,
        ];
        if numeric.iter().flatten().any(|value| *value < 0) {
            return Err("usage values must not be negative".into());
        }
        if self.usage_status == "not_applicable" && numeric.iter().any(Option::is_some) {
            return Err("not_applicable usage cannot include tokens or charges".into());
        }
        if let (Some(i), Some(o), Some(t)) =
            (self.input_tokens, self.output_tokens, self.total_tokens)
            && i.checked_add(o).is_none_or(|m| t < m)
        {
            return Err("total_tokens must include input_tokens and output_tokens".into());
        }
        if self.billing_mode == "metered_api" {
            match(&self.rate_card_version,&self.pricing_snapshot){(Some(_),Some(snapshot))if self.estimated_micro_usd==Some(metered_micro_usd(self,snapshot)?)=>{},(None,None)if self.estimated_micro_usd.is_none()=>{},_=>return Err("metered pricing provenance must be entirely present and correct or explicitly unavailable".into())}
        }
        if matches!(
            self.billing_mode.as_str(),
            "subscription_allowance" | "chatgpt_credits"
        ) && self.estimated_micro_usd.is_some()
        {
            return Err("subscription usage cannot report per-trace billed USD".into());
        }
        if matches!(self.usage_status.as_str(), "partial" | "unavailable")
            && self
                .usage_missing_reason
                .as_deref()
                .is_none_or(str::is_empty)
        {
            return Err("partial or unavailable usage requires usage_missing_reason".into());
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct MeteredRateCard {
    input_micro_usd_per_million: i64,
    output_micro_usd_per_million: i64,
    #[serde(default)]
    cache_creation_micro_usd_per_million: i64,
    #[serde(default)]
    cache_read_micro_usd_per_million: i64,
    #[serde(default)]
    reasoning_micro_usd_per_million: i64,
}
pub fn metered_micro_usd(input: &NormalizedUsage, snapshot: &Value) -> Result<i64, String> {
    let r: MeteredRateCard = serde_json::from_value(snapshot.clone())
        .map_err(|_| "pricing_snapshot has an invalid metered rate card".to_owned())?;
    let cc = input.cache_creation_tokens.unwrap_or(0);
    let cr = input.cache_read_tokens.unwrap_or(0);
    let reason = input.reasoning_tokens.unwrap_or(0);
    let uncached = input
        .input_tokens
        .unwrap_or(0)
        .checked_sub(cc)
        .and_then(|v| v.checked_sub(cr))
        .filter(|v| *v >= 0)
        .ok_or_else(|| "cache token categories exceed input_tokens".to_owned())?;
    let output = input
        .output_tokens
        .unwrap_or(0)
        .checked_sub(reason)
        .filter(|v| *v >= 0)
        .ok_or_else(|| "reasoning_tokens exceeds output_tokens".to_owned())?;
    let terms = [
        (uncached, r.input_micro_usd_per_million),
        (output, r.output_micro_usd_per_million),
        (cc, r.cache_creation_micro_usd_per_million),
        (cr, r.cache_read_micro_usd_per_million),
        (reason, r.reasoning_micro_usd_per_million),
    ];
    let n = terms.into_iter().try_fold(0_i128, |sum, (tokens, rate)| {
        if rate < 0 {
            return Err("pricing rates must not be negative".to_owned());
        }
        sum.checked_add(i128::from(tokens) * i128::from(rate))
            .ok_or_else(|| "pricing arithmetic overflow".to_owned())
    })?;
    i64::try_from((n + 500_000) / 1_000_000).map_err(|_| "pricing arithmetic overflow".to_owned())
}
pub async fn record_usage(pool: &PgPool, input: &NormalizedUsage) -> Result<Uuid, DbError> {
    let mut tx = pool.begin().await?;
    let id = record_usage_in_tx(&mut tx, input).await?;
    tx.commit().await?;
    Ok(id)
}
pub async fn record_usage_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    input: &NormalizedUsage,
) -> Result<Uuid, DbError> {
    let id = Uuid::new_v4();
    let mut value = serde_json::to_value(input).map_err(|e| DbError::Invalid(e.to_string()))?;
    let Value::Object(ref mut object) = value else {
        unreachable!()
    };
    object.insert("id".into(), json!(id));
    object.insert("entry_type".into(), json!("model_attempt"));
    object.insert("created_at".into(), json!(OffsetDateTime::now_utc()));
    let inserted=sqlx::query_scalar::<_,Uuid>(r#"UPDATE runs SET trace=trace||jsonb_build_array($2::jsonb),updated_at=now() WHERE id=$1 AND NOT EXISTS(SELECT 1 FROM jsonb_array_elements(trace)e WHERE e->>'entry_type'='model_attempt' AND e->>'component'=$3 AND e->>'source_execution_id'=$4 AND COALESCE(e->>'source_turn_id','')=COALESCE($5,'')) RETURNING id"#).bind(input.run_id).bind(value).bind(&input.component).bind(&input.source_execution_id).bind(&input.source_turn_id).fetch_optional(&mut **tx).await?;
    if inserted.is_none() {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM runs WHERE id=$1)")
            .bind(input.run_id)
            .fetch_one(&mut **tx)
            .await?;
        if !exists {
            return Err(DbError::NotFound);
        }
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn usage() -> NormalizedUsage {
        NormalizedUsage {
            run_id: Uuid::nil(),
            component: "agent".into(),
            provider: "openai".into(),
            model_id: "fixture".into(),
            display_tier: None,
            execution_type: "codex_harness".into(),
            auth_mode: "api_key".into(),
            upstream_service: "api.openai.com".into(),
            billing_mode: "metered_api".into(),
            reasoning_effort: None,
            service_tier: None,
            source_thread_id: None,
            source_execution_id: "execution-1".into(),
            source_turn_id: None,
            usage_status: "reported".into(),
            usage_missing_reason: None,
            input_tokens: Some(1000),
            output_tokens: Some(500),
            cache_creation_tokens: Some(100),
            cache_read_tokens: Some(200),
            reasoning_tokens: Some(50),
            total_tokens: Some(1500),
            estimated_micro_usd: Some(4800),
            chatgpt_credit_microunits: None,
            api_equivalent_micro_usd: None,
            rate_card_version: Some("fixture-v1".into()),
            pricing_snapshot: Some(
                json!({"input_micro_usd_per_million":2000000,"output_micro_usd_per_million":6000000,"cache_creation_micro_usd_per_million":2000000,"cache_read_micro_usd_per_million":1000000,"reasoning_micro_usd_per_million":6000000}),
            ),
        }
    }
    #[test]
    fn metered_cost_is_exact() {
        let input = usage();
        assert_eq!(
            metered_micro_usd(&input, input.pricing_snapshot.as_ref().unwrap()).unwrap(),
            4800
        );
        assert!(input.validate().is_ok())
    }
    #[test]
    fn subscription_never_claims_billed_usd() {
        let mut input = usage();
        input.billing_mode = "subscription_allowance".into();
        assert!(input.validate().unwrap_err().contains("billed USD"))
    }

    #[test]
    fn workflow_trace_contract_accepts_safe_otel_shaped_usage() {
        let now = OffsetDateTime::now_utc();
        let mut entry = WorkflowTraceEntry {
            id: Uuid::new_v4(),
            parent_id: None,
            entry_type: "model_attempt".into(),
            name: "research-source".into(),
            status: "completed".into(),
            started_at: now,
            completed_at: now,
            duration_ms: 42,
            component: "agent".into(),
            provider: Some("codex".into()),
            model_id: Some("fixture-model".into()),
            reasoning_effort: Some("medium".into()),
            source_execution_id: Some("execution-1".into()),
            input_tokens: Some(100),
            output_tokens: Some(20),
            cache_read_tokens: Some(25),
            reasoning_tokens: Some(5),
            total_tokens: Some(120),
            facts: json!({"purpose":"capture and analyze source"}),
        };
        assert!(entry.validate().is_ok());
    }

    #[test]
    fn workflow_trace_contract_rejects_inconsistent_usage() {
        let now = OffsetDateTime::now_utc();
        let mut entry = WorkflowTraceEntry {
            id: Uuid::new_v4(),
            parent_id: None,
            entry_type: "model_attempt".into(),
            name: "research-source".into(),
            status: "completed".into(),
            started_at: now,
            completed_at: now,
            duration_ms: 42,
            component: "agent".into(),
            provider: None,
            model_id: None,
            reasoning_effort: None,
            source_execution_id: None,
            input_tokens: Some(100),
            output_tokens: Some(20),
            cache_read_tokens: None,
            reasoning_tokens: None,
            total_tokens: Some(119),
            facts: json!({}),
        };
        assert!(entry.validate().unwrap_err().contains("total_tokens"));
    }
}

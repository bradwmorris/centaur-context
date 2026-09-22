use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use axum::{
    Json, Router,
    extract::{Path, Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use subtle::ConstantTimeEq;
use time::OffsetDateTime;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::{
    api::AppState,
    config::{CuratorModelConfig, CuratorModelTransport},
    domain::{allowed, required_text},
};

const MAX_OPERATIONS: usize = 100;
const CURATOR_CONNECTION_KINDS: &[&str] =
    &["involves", "about", "themed", "related_to", "derived_from"];

#[derive(Clone)]
struct CuratorAuth(Arc<String>);

#[derive(Debug, Error)]
pub enum CuratorError {
    #[error("record not found")]
    NotFound,
    #[error("{0}")]
    Invalid(String),
    #[error("revision conflict")]
    Conflict,
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

use thiserror::Error;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationPlan {
    #[serde(default)]
    pub create_objects: Vec<CreateObject>,
    #[serde(default)]
    pub update_objects: Vec<UpdateObject>,
    #[serde(default)]
    pub create_connections: Vec<CreateConnection>,
    #[serde(default)]
    pub update_connections: Vec<UpdateConnection>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CreateObject {
    pub client_id: String,
    pub kind: String,
    pub title: String,
    pub description: String,
    pub supporting_message_ids: Vec<Uuid>,
    pub entity_kind: Option<String>,
    pub task: Option<TaskFields>,
    pub memory: Option<MemoryFields>,
    #[serde(default)]
    pub source: Option<SourceFields>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateObject {
    pub object_id: Uuid,
    pub expected_revision: i64,
    pub title: Option<String>,
    pub description: Option<String>,
    pub supporting_message_ids: Vec<Uuid>,
    pub task: Option<TaskPatch>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskFields {
    pub confirmed: bool,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default = "default_priority")]
    pub priority: String,
    pub owner_object_id: Option<Uuid>,
    #[serde(default)]
    pub agent_suitable: bool,
    pub blocked_reason: Option<String>,
    #[serde(with = "time::serde::rfc3339::option", default)]
    pub due_at: Option<OffsetDateTime>,
    pub github_issue_url: Option<String>,
    pub brief_markdown: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskPatch {
    pub confirmed: bool,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub owner_object_id: Option<Uuid>,
    #[serde(default)]
    pub clear_owner: bool,
    pub agent_suitable: Option<bool>,
    pub blocked_reason: Option<String>,
    #[serde(default)]
    pub clear_blocked_reason: bool,
    #[serde(with = "time::serde::rfc3339::option", default)]
    pub due_at: Option<OffsetDateTime>,
    #[serde(default)]
    pub clear_due_at: bool,
    pub github_issue_url: Option<String>,
    #[serde(default)]
    pub clear_github_issue_url: bool,
    pub brief_markdown: Option<String>,
    #[serde(default)]
    pub clear_brief_markdown: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryFields {
    pub primary_event: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub happened_at: OffsetDateTime,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFields {
    pub source_kind: String,
    pub canonical_uri: Option<String>,
    pub byline: Option<String>,
    pub publisher: Option<String>,
    #[serde(with = "time::serde::rfc3339::option", default)]
    pub published_at: Option<OffsetDateTime>,
    pub published_at_precision: Option<String>,
    #[serde(with = "time::serde::rfc3339::option", default)]
    pub last_accessed_at: Option<OffsetDateTime>,
    pub original_language: Option<String>,
    pub original_media_type: Option<String>,
    pub original_artifact_reference: Option<String>,
    pub content: Option<ArtifactFields>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactFields {
    pub kind: String,
    pub title: Option<String>,
    pub content: String,
    pub uri: Option<String>,
    pub media_type: Option<String>,
    pub language: Option<String>,
    #[serde(with = "time::serde::rfc3339::option", default)]
    pub captured_at: Option<OffsetDateTime>,
    #[serde(default = "empty_object")]
    pub metadata: Value,
}

fn empty_object() -> Value {
    json!({})
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum ObjectRef {
    Existing { object_id: Uuid },
    Created { client_id: String },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CreateConnection {
    pub source: ObjectRef,
    pub kind: String,
    pub target: ObjectRef,
    pub description: String,
    pub supporting_message_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateConnection {
    pub connection_id: Uuid,
    pub expected_revision: i64,
    pub kind: Option<String>,
    pub description: Option<String>,
    pub supporting_message_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize)]
struct ReconcileRequest {
    model: String,
    prompt_version: String,
    plan: ReconciliationPlan,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct CuratorRun {
    pub id: Uuid,
    pub chat_object_id: Uuid,
    pub first_message_id: Uuid,
    pub last_message_id: Uuid,
    pub trigger: String,
    pub status: String,
    pub message_count: i32,
    pub idempotency_key: String,
    pub attempts: i32,
    pub worker_id: Option<String>,
    pub model: Option<String>,
    pub prompt_version: Option<String>,
    pub proposed_plan: Option<Value>,
    pub committed_plan: Option<Value>,
    pub result: Option<Value>,
    #[serde(with = "time::serde::rfc3339")]
    pub queued_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub completed_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub reversed_at: Option<OffsetDateTime>,
    pub error_message: Option<String>,
}

#[derive(Debug, FromRow)]
struct CurrentObject {
    id: Uuid,
    kind: String,
    title: String,
    description: String,
    protected: bool,
    lifecycle: String,
    revision: i64,
    provenance: Value,
    status: Option<String>,
    priority: Option<String>,
    owner_object_id: Option<Uuid>,
    agent_suitable: Option<bool>,
    blocked_reason: Option<String>,
    due_at: Option<OffsetDateTime>,
    completed_at: Option<OffsetDateTime>,
    github_issue_url: Option<String>,
    brief_markdown: Option<String>,
}

#[derive(Debug, FromRow)]
struct CurrentConnection {
    id: Uuid,
    source_object_id: Uuid,
    kind: String,
    target_object_id: Uuid,
    description: String,
    protected: bool,
    revision: i64,
    provenance: Value,
    archived_at: Option<OffsetDateTime>,
}

#[derive(Debug, FromRow, Serialize)]
pub struct CuratorRunChange {
    pub id: Uuid,
    pub sequence: i32,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub action: String,
    pub before_state: Option<Value>,
    pub after_state: Value,
    pub after_revision: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub undone_at: Option<OffsetDateTime>,
}

#[derive(Debug, Serialize)]
pub struct CuratorRunDetail {
    pub run: CuratorRun,
    pub messages: Vec<crate::db::ChatMessage>,
    pub changes: Vec<CuratorRunChange>,
}

fn default_status() -> String {
    "todo".to_owned()
}
fn default_priority() -> String {
    "medium".to_owned()
}

pub fn router(state: AppState, token: String) -> Router {
    Router::new()
        .route("/healthz", get(|| async { Json(json!({"ok": true})) }))
        .route("/readyz", get(ready))
        .nest(
            "/api/v2/curator",
            Router::new()
                .route("/runs/{id}", get(read_run))
                .route("/runs/{id}/reconcile", post(reconcile_run))
                .route("/runs/{id}/undo", post(undo_run)),
        )
        .with_state(state)
        .layer(middleware::from_fn_with_state(
            CuratorAuth(Arc::new(token)),
            authenticate,
        ))
        .layer(TraceLayer::new_for_http())
}

async fn authenticate(
    State(auth): State<CuratorAuth>,
    request: Request,
    next: Next,
) -> Result<Response, CuratorApiError> {
    let supplied = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(CuratorApiError::Unauthorized)?;
    if supplied.len() != auth.0.len()
        || supplied.as_bytes().ct_eq(auth.0.as_bytes()).unwrap_u8() != 1
    {
        return Err(CuratorApiError::Unauthorized);
    }
    Ok(next.run(request).await)
}

async fn ready(State(state): State<AppState>) -> Result<Json<Value>, CuratorApiError> {
    crate::db::ready(&state.pool)
        .await
        .map_err(|error| CuratorError::Invalid(error.to_string()))?;
    Ok(Json(json!({"ok": true, "ready": true})))
}

async fn read_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, CuratorApiError> {
    Ok(Json(json!({"data": get_run(&state.pool, id).await?})))
}

async fn reconcile_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<ReconcileRequest>,
) -> Result<Json<Value>, CuratorApiError> {
    let model = required_text(input.model, "model", 300)
        .map_err(|e| CuratorError::Invalid(e.to_string()))?;
    let prompt_version = required_text(input.prompt_version, "prompt_version", 300)
        .map_err(|e| CuratorError::Invalid(e.to_string()))?;
    match reconcile(&state.pool, id, &model, &prompt_version, input.plan).await {
        Ok(result) => Ok(Json(json!({"data": result}))),
        Err(error) => {
            if !matches!(error, CuratorError::NotFound | CuratorError::Conflict) {
                let _ = record_failure(&state.pool, id, &error.to_string()).await;
            }
            Err(error.into())
        }
    }
}

async fn undo_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, CuratorApiError> {
    Ok(Json(json!({"data": undo(&state.pool, id).await?})))
}

pub async fn get_run(pool: &PgPool, id: Uuid) -> Result<CuratorRun, CuratorError> {
    sqlx::query_as(
        r#"SELECT id,chat_object_id,(input->>'first_message_id')::uuid first_message_id,
          (input->>'last_message_id')::uuid last_message_id,input->>'trigger' trigger,status,
          (input->>'message_count')::integer message_count,idempotency_key,
          COALESCE((result->>'attempts')::integer,0) attempts,result->>'worker_id' worker_id,
          result->>'model' model,result->>'prompt_version' prompt_version,
          result->'proposed_plan' proposed_plan,result->'committed_plan' committed_plan,
          NULLIF(result,'{}'::jsonb) result,created_at queued_at,started_at,completed_at,
          CASE WHEN status='reversed' THEN completed_at END reversed_at,error error_message
          FROM runs WHERE id=$1 AND kind='curator'"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(CuratorError::NotFound)
}

pub async fn list_runs(pool: &PgPool, limit: i64) -> Result<Vec<CuratorRun>, CuratorError> {
    Ok(sqlx::query_as(
        r#"SELECT id,chat_object_id,(input->>'first_message_id')::uuid first_message_id,
          (input->>'last_message_id')::uuid last_message_id,input->>'trigger' trigger,status,
          (input->>'message_count')::integer message_count,idempotency_key,
          COALESCE((result->>'attempts')::integer,0) attempts,result->>'worker_id' worker_id,
          result->>'model' model,result->>'prompt_version' prompt_version,
          result->'proposed_plan' proposed_plan,result->'committed_plan' committed_plan,
          NULLIF(result,'{}'::jsonb) result,created_at queued_at,started_at,completed_at,
          CASE WHEN status='reversed' THEN completed_at END reversed_at,error error_message
          FROM runs WHERE kind='curator' ORDER BY created_at DESC,id DESC LIMIT $1"#,
    )
    .bind(limit.clamp(1, 100))
    .fetch_all(pool)
    .await?)
}

pub async fn run_detail(pool: &PgPool, id: Uuid) -> Result<CuratorRunDetail, CuratorError> {
    let run = get_run(pool, id).await?;
    let messages: Vec<crate::db::ChatMessage> = sqlx::query_as(
        r#"SELECT m.id,m.chat_object_id,m.provider_message_id,m.sender_user_object_id,
                  o.title AS sender_title,u.user_kind AS sender_kind,m.content,
                  m.source_created_at,m.ingestion_sequence,m.ingested_at
           FROM chat_messages m
           JOIN users u ON u.object_id=m.sender_user_object_id
           JOIN objects o ON o.id=u.object_id
           WHERE m.chat_object_id=$1 AND m.ingestion_sequence BETWEEN
             (SELECT ingestion_sequence FROM chat_messages WHERE id=$2)
             AND (SELECT ingestion_sequence FROM chat_messages WHERE id=$3)
           ORDER BY m.ingestion_sequence"#,
    )
    .bind(run.chat_object_id)
    .bind(run.first_message_id)
    .bind(run.last_message_id)
    .fetch_all(pool)
    .await?;
    let changes = sqlx::query_as(
        r#"SELECT id,sequence,target_type entity_type,target_id entity_id,action,before_state,
                  after_state,to_revision after_revision,created_at,NULL::timestamptz undone_at
           FROM object_events WHERE run_id=$1 ORDER BY sequence"#,
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    Ok(CuratorRunDetail {
        run,
        messages,
        changes,
    })
}

pub async fn reconcile(
    pool: &PgPool,
    run_id: Uuid,
    model: &str,
    prompt_version: &str,
    plan: ReconciliationPlan,
) -> Result<Value, CuratorError> {
    let result = reconcile_owned(pool, run_id, model, prompt_version, plan, None).await;
    if let Err(error) = &result {
        record_failure(pool, run_id, &error.to_string()).await?;
    }
    result
}

async fn reconcile_owned(
    pool: &PgPool,
    run_id: Uuid,
    model: &str,
    prompt_version: &str,
    mut plan: ReconciliationPlan,
    claimed_by: Option<&str>,
) -> Result<Value, CuratorError> {
    validate_plan(&mut plan)?;
    let plan_json =
        serde_json::to_value(&plan).map_err(|e| CuratorError::Invalid(e.to_string()))?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL statement_timeout = '10s'")
        .execute(&mut *tx)
        .await?;
    let run = lock_run(&mut tx, run_id).await?;
    if run.status == "completed" {
        if run.proposed_plan.as_ref() == Some(&plan_json) {
            let mut result = run
                .result
                .unwrap_or_else(|| json!({"run_id": run_id, "status": "completed"}));
            if let Some(object) = result.as_object_mut() {
                object.retain(|key, _| {
                    [
                        "run_id",
                        "status",
                        "chat_object_id",
                        "created_objects",
                        "change_count",
                        "skipped_operations",
                    ]
                    .contains(&key.as_str())
                });
            }
            return Ok(result);
        }
        return Err(CuratorError::Conflict);
    }
    if run.status == "reversed"
        || (run.status == "running" && run.worker_id.as_deref() != claimed_by)
    {
        return Err(CuratorError::Conflict);
    }
    // Revalidate the normalized plan inside the same transaction that commits it.
    // The earlier validation is model/API feedback; this is the authority boundary.
    validate_plan(&mut plan)?;
    let message_window = load_message_window(&mut tx, &run).await?;
    let message_ids = message_window.keys().copied().collect::<HashSet<_>>();
    validate_message_refs(&plan, &message_ids)?;
    validate_human_grounded_objects(&plan, &message_window)?;
    validate_commit_policy(&mut tx, &plan, &run).await?;

    if run.status != "running" {
        sqlx::query(
            r#"UPDATE runs SET status='running',started_at=COALESCE(started_at,now()),
                  completed_at=NULL,result=result || jsonb_build_object(
                    'worker_id','curator-api','attempts',COALESCE((result->>'attempts')::integer,0)+1,
                    'model',$2::text,'prompt_version',$3::text,'proposed_plan',$4::jsonb),
                  error=NULL,updated_at=now() WHERE id=$1 AND kind='curator'"#,
        ).bind(run_id).bind(model).bind(prompt_version).bind(&plan_json).execute(&mut *tx).await?;
    } else {
        sqlx::query("UPDATE runs SET result=result || jsonb_build_object('proposed_plan',$2::jsonb),error=NULL,updated_at=now() WHERE id=$1")
            .bind(run_id)
            .bind(&plan_json)
            .execute(&mut *tx)
            .await?;
    }

    let mut created = HashMap::new();
    let mut sequence = 0_i32;
    let mut skipped_operations = 0_i32;

    for item in &plan.create_objects {
        sequence += 1;
        let id = Uuid::new_v4();
        let provenance = curator_provenance(
            run_id,
            run.chat_object_id,
            &item.supporting_message_ids,
            model,
            prompt_version,
        );
        insert_object(&mut tx, id, item, &provenance).await?;
        let after = object_snapshot(&mut tx, id).await?;
        insert_change(
            &mut tx, run_id, sequence, "object", id, "created", None, &after, 1,
        )
        .await?;
        insert_event(
            &mut tx,
            run_id,
            "object",
            id,
            id,
            "created",
            None,
            1,
            json!({"kind": item.kind, "supporting_message_ids": item.supporting_message_ids}),
        )
        .await?;
        created.insert(item.client_id.clone(), id);
    }

    for item in &plan.create_connections {
        let source = resolve_ref(&item.source, &created)?;
        let target = resolve_ref(&item.target, &created)?;
        if source == target {
            return Err(CuratorError::Invalid(
                "a connection cannot link an Object to itself".into(),
            ));
        }
        ensure_active_object(&mut tx, source).await?;
        ensure_active_object(&mut tx, target).await?;
        let exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM connections
               WHERE source_object_id=$1 AND kind=$2 AND target_object_id=$3
                 AND archived_at IS NULL)"#,
        )
        .bind(source)
        .bind(&item.kind)
        .bind(target)
        .fetch_one(&mut *tx)
        .await?;
        if exists {
            skipped_operations += 1;
            continue;
        }
        sequence += 1;
        let id = Uuid::new_v4();
        let provenance = curator_provenance(
            run_id,
            run.chat_object_id,
            &item.supporting_message_ids,
            model,
            prompt_version,
        );
        sqlx::query(
            r#"INSERT INTO connections
               (id,source_object_id,kind,target_object_id,description,created_by_type,created_by_id,
                updated_by_type,updated_by_id,provenance)
               VALUES ($1,$2,$3,$4,$5,'system','context-curator','system','context-curator',$6)"#,
        )
        .bind(id)
        .bind(source)
        .bind(&item.kind)
        .bind(target)
        .bind(&item.description)
        .bind(&provenance)
        .execute(&mut *tx)
        .await?;
        let after = connection_snapshot(&mut tx, id).await?;
        insert_change(
            &mut tx,
            run_id,
            sequence,
            "connection",
            id,
            "created",
            None,
            &after,
            1,
        )
        .await?;
        insert_event(&mut tx, run_id, "connection", id, source, "connected", None, 1, json!({"kind": item.kind, "target_object_id": target, "supporting_message_ids": item.supporting_message_ids})).await?;
    }

    let result = json!({
        "run_id": run_id,
        "status": if sequence == 0 { "no_changes" } else { "completed" },
        "chat_object_id": run.chat_object_id,
        "created_objects": created,
        "change_count": sequence,
        "skipped_operations": skipped_operations,
    });
    sqlx::query(
        r#"UPDATE runs SET status='completed',completed_at=now(),
                  result=result || $3::jsonb || jsonb_build_object('committed_plan',$2::jsonb),error=NULL,updated_at=now()
           WHERE id=$1 AND kind='curator'"#,
    ).bind(run_id).bind(&plan_json).bind(&result).execute(&mut *tx).await?;
    sqlx::query("UPDATE chats SET curated_through_message_id=$2,processing_updated_at=now() WHERE object_id=$1")
        .bind(run.chat_object_id)
        .bind(run.last_message_id)
        .execute(&mut *tx)
        .await?;
    insert_event(
        &mut tx,
        run_id,
        "curator_run",
        run_id,
        run.chat_object_id,
        "curator_committed",
        None,
        1,
        json!({"change_count": sequence, "model": model, "prompt_version": prompt_version}),
    )
    .await?;
    crate::runs::finish_curator_run(&mut tx, run_id, sequence)
        .await
        .map_err(|error| CuratorError::Invalid(format!("Run completion failed: {error}")))?;
    tx.commit().await?;
    Ok(result)
}

pub async fn undo(pool: &PgPool, run_id: Uuid) -> Result<Value, CuratorError> {
    undo_as(
        pool,
        run_id,
        &crate::domain::ActorContext::system("context-curator"),
    )
    .await
}

pub async fn undo_as(
    pool: &PgPool,
    run_id: Uuid,
    actor: &crate::domain::ActorContext,
) -> Result<Value, CuratorError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL statement_timeout = '10s'")
        .execute(&mut *tx)
        .await?;
    let run = lock_run(&mut tx, run_id).await?;
    if run.status == "reversed" {
        return Ok(run
            .result
            .unwrap_or_else(|| json!({"run_id": run_id, "status": "reversed"})));
    }
    if run.status != "completed" {
        return Err(CuratorError::Conflict);
    }
    let changes: Vec<CuratorRunChange> = sqlx::query_as(
        r#"SELECT id,sequence,target_type entity_type,target_id entity_id,action,before_state,
          after_state,to_revision after_revision,created_at,NULL::timestamptz undone_at
          FROM object_events WHERE run_id=$1 AND reversible ORDER BY sequence DESC"#,
    )
    .bind(run_id)
    .fetch_all(&mut *tx)
    .await?;
    let reversal_run_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO runs
      (id,parent_run_id,kind,status,actor_type,actor_id,idempotency_key,input,result,started_at)
      VALUES($1,$2,'curator_undo','running',$3,$4,$5,$6,'{}',now())"#,
    )
    .bind(reversal_run_id)
    .bind(run_id)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(format!(
        "undo:{run_id}:{}:{}",
        actor.actor_type, actor.actor_id
    ))
    .bind(json!({"reverses_run_id":run_id}))
    .execute(&mut *tx)
    .await?;
    for (index, change) in changes.iter().enumerate() {
        match (change.entity_type.as_str(), change.action.as_str()) {
            ("connection", "created") => archive_created_connection(&mut tx, change, actor).await?,
            ("connection", "updated") => restore_connection(&mut tx, change, actor).await?,
            ("object", "created") => archive_created_object(&mut tx, change, actor).await?,
            ("object", "updated") => restore_object(&mut tx, change, actor).await?,
            _ => {
                return Err(CuratorError::Invalid(
                    "unsupported curator change journal entry".into(),
                ));
            }
        }
        let after = if change.entity_type == "connection" {
            connection_snapshot(&mut tx, change.entity_id).await?
        } else {
            object_snapshot(&mut tx, change.entity_id).await?
        };
        insert_change(
            &mut tx,
            reversal_run_id,
            index as i32 + 1,
            &change.entity_type,
            change.entity_id,
            if change.action == "created" {
                "archived"
            } else {
                "restored"
            },
            Some(&change.after_state),
            &after,
            change.after_revision + 1,
        )
        .await?;
    }
    let result =
        json!({"run_id": run_id, "status": "reversed", "reversed_change_count": changes.len()});
    sqlx::query(
        "UPDATE runs SET status='reversed',result=result || $2::jsonb,updated_at=now() WHERE id=$1",
    )
    .bind(run_id)
    .bind(&result)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE runs SET status='completed',result=$2,completed_at=now(),updated_at=now() WHERE id=$1")
      .bind(reversal_run_id).bind(&result).execute(&mut *tx).await?;
    crate::runs::reverse_curator_run(&mut tx, run_id, reversal_run_id, changes.len())
        .await
        .map_err(|error| CuratorError::Invalid(format!("Run reversal failed: {error}")))?;
    tx.commit().await?;
    Ok(result)
}

pub fn validate_plan(plan: &mut ReconciliationPlan) -> Result<(), CuratorError> {
    let count = plan.create_objects.len()
        + plan.update_objects.len()
        + plan.create_connections.len()
        + plan.update_connections.len();
    if count > MAX_OPERATIONS {
        return Err(CuratorError::Invalid(
            "a reconciliation plan must contain at most 100 operations".into(),
        ));
    }
    if !plan.update_objects.is_empty() {
        return Err(CuratorError::Invalid(
            "the curator cannot update or delete Objects, including Memories".into(),
        ));
    }
    if !plan.update_connections.is_empty() {
        return Err(CuratorError::Invalid(
            "the curator cannot update or delete connections".into(),
        ));
    }
    let mut clients = HashSet::new();
    for item in &mut plan.create_objects {
        item.client_id = required_text(std::mem::take(&mut item.client_id), "client_id", 100)
            .map_err(invalid)?;
        if !clients.insert(item.client_id.clone()) {
            return Err(CuratorError::Invalid(
                "client_id values must be unique".into(),
            ));
        }
        item.kind = allowed(std::mem::take(&mut item.kind), "kind", &["memory"]).map_err(|_| {
            CuratorError::Invalid("the curator may create only Memory Objects".into())
        })?;
        item.title =
            required_text(std::mem::take(&mut item.title), "title", 300).map_err(invalid)?;
        item.description =
            crate::domain::object_description(&item.title, std::mem::take(&mut item.description))
                .map_err(invalid)?;
        if item.entity_kind.is_some()
            || item.memory.is_none()
            || item.task.is_some()
            || item.source.is_some()
        {
            return Err(CuratorError::Invalid(
                "Memory creation requires only memory fields".into(),
            ));
        }
    }
    for item in &mut plan.create_connections {
        for reference in [&mut item.source, &mut item.target] {
            if let ObjectRef::Created { client_id } = reference {
                *client_id =
                    required_text(std::mem::take(client_id), "client_id", 100).map_err(invalid)?;
                if !clients.contains(client_id) {
                    return Err(CuratorError::Invalid(format!(
                        "unknown created Object client_id: {client_id}"
                    )));
                }
            }
        }
        item.kind = allowed(
            std::mem::take(&mut item.kind),
            "connection kind",
            CURATOR_CONNECTION_KINDS,
        )
        .map_err(invalid)?;
        item.description = required_text(
            std::mem::take(&mut item.description),
            "connection description",
            1000,
        )
        .map_err(invalid)?;
    }
    for memory in &plan.create_objects {
        let has_provenance = plan.create_connections.iter().any(|connection| {
            connection.kind == "derived_from"
                && matches!(
                    &connection.source,
                    ObjectRef::Created { client_id } if client_id == &memory.client_id
                )
                && matches!(&connection.target, ObjectRef::Existing { .. })
                && same_message_ids(
                    &connection.supporting_message_ids,
                    &memory.supporting_message_ids,
                )
        });
        if !has_provenance {
            return Err(CuratorError::Invalid(
                "every new Memory must propose a derived_from connection to its originating Chat with the same exact supporting message IDs".into(),
            ));
        }
    }
    Ok(())
}

fn invalid(error: impl std::fmt::Display) -> CuratorError {
    CuratorError::Invalid(error.to_string())
}

async fn lock_run(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<CuratorRun, CuratorError> {
    sqlx::query_as(
        r#"SELECT id,chat_object_id,(input->>'first_message_id')::uuid first_message_id,
          (input->>'last_message_id')::uuid last_message_id,input->>'trigger' trigger,status,
          (input->>'message_count')::integer message_count,idempotency_key,
          COALESCE((result->>'attempts')::integer,0) attempts,result->>'worker_id' worker_id,
          result->>'model' model,result->>'prompt_version' prompt_version,
          result->'proposed_plan' proposed_plan,result->'committed_plan' committed_plan,
          NULLIF(result,'{}'::jsonb) result,created_at queued_at,started_at,completed_at,
          CASE WHEN status='reversed' THEN completed_at END reversed_at,error error_message
          FROM runs WHERE id=$1 AND kind='curator' FOR UPDATE"#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(CuratorError::NotFound)
}

#[derive(Debug, FromRow)]
struct MessageEvidence {
    id: Uuid,
    sender_kind: String,
    content: String,
}

async fn load_message_window(
    tx: &mut Transaction<'_, Postgres>,
    run: &CuratorRun,
) -> Result<HashMap<Uuid, MessageEvidence>, CuratorError> {
    let messages: Vec<MessageEvidence> = sqlx::query_as(
        r#"SELECT m.id,u.user_kind AS sender_kind,m.content FROM chat_messages m
           JOIN users u ON u.object_id=m.sender_user_object_id
           WHERE m.chat_object_id=$1 AND m.ingestion_sequence BETWEEN
             (SELECT ingestion_sequence FROM chat_messages WHERE id=$2)
             AND (SELECT ingestion_sequence FROM chat_messages WHERE id=$3)
           ORDER BY m.ingestion_sequence"#,
    )
    .bind(run.chat_object_id)
    .bind(run.first_message_id)
    .bind(run.last_message_id)
    .fetch_all(&mut **tx)
    .await?;
    if messages.len() != run.message_count as usize {
        return Err(CuratorError::Invalid(
            "curator run message window no longer matches its recorded count".into(),
        ));
    }
    Ok(messages
        .into_iter()
        .map(|message| (message.id, message))
        .collect())
}

fn validate_human_grounded_objects(
    plan: &ReconciliationPlan,
    messages: &HashMap<Uuid, MessageEvidence>,
) -> Result<(), CuratorError> {
    for item in &plan.create_objects {
        let evidence = item
            .supporting_message_ids
            .iter()
            .filter_map(|id| messages.get(id))
            .collect::<Vec<_>>();
        if evidence.is_empty()
            || evidence
                .iter()
                .any(|message| message.sender_kind != "human")
        {
            return Err(CuratorError::Invalid(
                "curator-created Memories must be supported only by human-authored messages".into(),
            ));
        }
        if event_capture_enabled() && is_object_creation_report(&item.description) {
            return Err(CuratorError::Invalid(
                "committed Object creation is captured from its event ledger, not chat prose"
                    .into(),
            ));
        }
        if evidence
            .iter()
            .all(|message| is_memory_noise(&message.content))
        {
            return Err(CuratorError::Invalid(
                "operational or synthetic traffic is not durable memory".into(),
            ));
        }
        if evidence
            .iter()
            .all(|message| is_workflow_request(&message.content))
            && !is_request_framed_memory(&item.description)
        {
            return Err(CuratorError::Invalid(
                "request-only evidence may record what was requested but cannot assert workflow success"
                    .into(),
            ));
        }
    }
    Ok(())
}

/// Conservative exclusion of explicit operational/test traffic; ordinary mentions
/// of testing in real research are not enough to discard a human statement.
fn event_capture_enabled() -> bool {
    std::env::var("MEMORY_CAPTURE_ENABLED").is_ok_and(|value| value == "true")
}

fn is_object_creation_report(description: &str) -> bool {
    let words = description
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_owned)
        .collect::<HashSet<_>>();
    ["added", "created", "saved", "imported"]
        .iter()
        .any(|word| words.contains(*word))
        && ["source", "sources", "note", "notes", "task", "tasks"]
            .iter()
            .any(|word| words.contains(*word))
        && !is_request_framed_memory(description)
}

fn is_memory_noise(content: &str) -> bool {
    let text = content.trim().to_lowercase();
    [
        "synthetic acceptance",
        "synthetic task-creation",
        "synthetic source acceptance",
        "one-time status check",
        "check the status once",
        "requested one-time check",
        "retry acceptance check",
    ]
    .iter()
    .any(|marker| text.contains(marker))
        || matches!(
            text.as_str(),
            "thanks" | "thank you" | "ok" | "okay" | "done"
        )
}

fn is_workflow_request(content: &str) -> bool {
    let content = content.to_ascii_lowercase();
    let trimmed = content.trim_start();
    trimmed.ends_with('?')
        || [
            "please ",
            "can you ",
            "could you ",
            "would you ",
            "research ",
            "add ",
            "create ",
            "update ",
            "ingest ",
            "import ",
            "capture ",
            "find ",
            "ensure ",
        ]
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
}

fn is_request_framed_memory(description: &str) -> bool {
    let description = description.to_ascii_lowercase();
    [
        " asked ",
        " requested ",
        " request to ",
        " instructed ",
        " directed ",
    ]
    .iter()
    .any(|marker| format!(" {description} ").contains(marker))
}

fn drop_disallowed_worker_creates(
    plan: &mut ReconciliationPlan,
    messages: &[WorkerMessage],
) -> Vec<String> {
    let human_message_ids = messages
        .iter()
        .filter(|message| message.sender_kind == "human" && !is_memory_noise(&message.content))
        .map(|message| message.id)
        .collect::<HashSet<_>>();
    let dropped = plan
        .create_objects
        .iter()
        .filter(|item| {
            item.kind != "memory"
                || (event_capture_enabled() && is_object_creation_report(&item.description))
                || item.supporting_message_ids.is_empty()
                || item
                    .supporting_message_ids
                    .iter()
                    .any(|id| !human_message_ids.contains(id))
        })
        .map(|item| item.client_id.clone())
        .collect::<HashSet<_>>();
    if dropped.is_empty() {
        return Vec::new();
    }
    plan.create_objects
        .retain(|item| !dropped.contains(&item.client_id));
    plan.create_connections.retain(|connection| {
        !object_ref_uses_client(&connection.source, &dropped)
            && !object_ref_uses_client(&connection.target, &dropped)
    });
    let mut dropped = dropped.into_iter().collect::<Vec<_>>();
    dropped.sort();
    dropped
}

fn object_ref_uses_client(reference: &ObjectRef, client_ids: &HashSet<String>) -> bool {
    matches!(reference, ObjectRef::Created { client_id } if client_ids.contains(client_id))
}

fn drop_unresolved_or_ambiguous_worker_connections(
    plan: &mut ReconciliationPlan,
    candidates: &crate::search::SearchPacket,
    chat_id: Uuid,
) -> usize {
    let mut identity_counts = HashMap::new();
    for candidate in &candidates.objects {
        *identity_counts
            .entry((
                candidate.kind.as_str(),
                candidate.title.trim().to_lowercase(),
            ))
            .or_insert(0_usize) += 1;
    }
    let eligible = candidates
        .objects
        .iter()
        .filter(|candidate| {
            identity_counts[&(
                candidate.kind.as_str(),
                candidate.title.trim().to_lowercase(),
            )] == 1
        })
        .map(|candidate| candidate.id)
        .collect::<HashSet<_>>();
    let before = plan.create_connections.len();
    plan.create_connections.retain(|connection| {
        [&connection.source, &connection.target]
            .into_iter()
            .all(|reference| match reference {
                ObjectRef::Created { .. } => true,
                ObjectRef::Existing { object_id } => {
                    *object_id == chat_id || eligible.contains(object_id)
                }
            })
    });
    before - plan.create_connections.len()
}

fn validate_message_refs(
    plan: &ReconciliationPlan,
    allowed_ids: &HashSet<Uuid>,
) -> Result<(), CuratorError> {
    let sets = plan
        .create_objects
        .iter()
        .map(|i| &i.supporting_message_ids)
        .chain(
            plan.update_objects
                .iter()
                .map(|i| &i.supporting_message_ids),
        )
        .chain(
            plan.create_connections
                .iter()
                .map(|i| &i.supporting_message_ids),
        )
        .chain(
            plan.update_connections
                .iter()
                .map(|i| &i.supporting_message_ids),
        );
    for ids in sets {
        if ids.is_empty() || ids.iter().any(|id| !allowed_ids.contains(id)) {
            return Err(CuratorError::Invalid(
                "every change must cite one or more messages from the exact curator run window"
                    .into(),
            ));
        }
    }
    Ok(())
}

async fn validate_commit_policy(
    tx: &mut Transaction<'_, Postgres>,
    plan: &ReconciliationPlan,
    run: &CuratorRun,
) -> Result<(), CuratorError> {
    if !plan.update_objects.is_empty() || !plan.update_connections.is_empty() {
        return Err(CuratorError::Invalid(
            "append-only Curator plans cannot contain update or delete operations".into(),
        ));
    }
    if plan.create_objects.iter().any(|item| item.kind != "memory") {
        return Err(CuratorError::Invalid(
            "the Curator commit layer may create only Memory Objects".into(),
        ));
    }

    let created = plan
        .create_objects
        .iter()
        .map(|item| (item.client_id.as_str(), item))
        .collect::<HashMap<_, _>>();

    for connection in &plan.create_connections {
        if !CURATOR_CONNECTION_KINDS.contains(&connection.kind.as_str()) {
            return Err(CuratorError::Invalid(
                "the Curator commit layer rejected an unsupported connection kind".into(),
            ));
        }
        let source_kind = commit_endpoint_kind(tx, &connection.source, &created, run).await?;
        let target_kind = commit_endpoint_kind(tx, &connection.target, &created, run).await?;
        if source_kind != "memory" && target_kind != "memory" {
            return Err(CuratorError::Invalid(
                "every Curator-created connection must have at least one Memory endpoint".into(),
            ));
        }
    }

    for memory in &plan.create_objects {
        let has_provenance = plan.create_connections.iter().any(|connection| {
            connection.kind == "derived_from"
                && matches!(
                    &connection.source,
                    ObjectRef::Created { client_id } if client_id == &memory.client_id
                )
                && matches!(
                    &connection.target,
                    ObjectRef::Existing { object_id } if *object_id == run.chat_object_id
                )
                && same_message_ids(
                    &connection.supporting_message_ids,
                    &memory.supporting_message_ids,
                )
        });
        if !has_provenance {
            return Err(CuratorError::Invalid(
                "every new Memory must have a derived_from connection to the originating Chat with the same exact supporting message IDs".into(),
            ));
        }
    }
    Ok(())
}

async fn commit_endpoint_kind(
    tx: &mut Transaction<'_, Postgres>,
    reference: &ObjectRef,
    created: &HashMap<&str, &CreateObject>,
    run: &CuratorRun,
) -> Result<String, CuratorError> {
    match reference {
        ObjectRef::Created { client_id } => created
            .get(client_id.as_str())
            .map(|item| item.kind.clone())
            .ok_or_else(|| {
                CuratorError::Invalid(format!("unknown created Object client_id: {client_id}"))
            }),
        ObjectRef::Existing { object_id } => {
            if *object_id != run.chat_object_id {
                let consulted: bool = sqlx::query_scalar(
                    "SELECT $2 = ANY(consulted_object_ids) FROM runs WHERE id=$1",
                )
                .bind(run.id)
                .bind(object_id)
                .fetch_one(&mut **tx)
                .await?;
                if !consulted {
                    return Err(CuratorError::Invalid(
                        "existing connection endpoints must be unambiguous retrieved candidates"
                            .into(),
                    ));
                }
            }
            sqlx::query_scalar(
                "SELECT kind FROM objects WHERE id=$1 AND archived_at IS NULL FOR SHARE",
            )
            .bind(object_id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(CuratorError::NotFound)
        }
    }
}

fn same_message_ids(left: &[Uuid], right: &[Uuid]) -> bool {
    left.len() == right.len()
        && left.iter().copied().collect::<HashSet<_>>()
            == right.iter().copied().collect::<HashSet<_>>()
}

fn resolve_ref(
    reference: &ObjectRef,
    created: &HashMap<String, Uuid>,
) -> Result<Uuid, CuratorError> {
    match reference {
        ObjectRef::Existing { object_id } => Ok(*object_id),
        ObjectRef::Created { client_id } => created.get(client_id).copied().ok_or_else(|| {
            CuratorError::Invalid(format!("unknown created Object client_id: {client_id}"))
        }),
    }
}

fn curator_provenance(
    run_id: Uuid,
    chat_id: Uuid,
    message_ids: &[Uuid],
    model: &str,
    prompt_version: &str,
) -> Value {
    json!({"source_type":"context_curator","source_ref":run_id,"curator_run_id":run_id,"chat_object_id":chat_id,"supporting_message_ids":message_ids,"model":model,"prompt_version":prompt_version})
}

async fn insert_object(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    item: &CreateObject,
    provenance: &Value,
) -> Result<(), CuratorError> {
    sqlx::query(
        r#"INSERT INTO objects (id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance)
           VALUES ($1,$2,$3,$4,'system','context-curator','system','context-curator',$5)"#,
    ).bind(id).bind(&item.kind).bind(&item.title).bind(&item.description).bind(provenance).execute(&mut **tx).await?;
    debug_assert_eq!(item.kind, "memory");
    sqlx::query("INSERT INTO memories (object_id,happened_at) VALUES ($1,$2)")
        .bind(id)
        .bind(item.memory.as_ref().expect("validated").happened_at)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn current_object(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<CurrentObject, CuratorError> {
    sqlx::query_as(
        r#"SELECT o.id,o.kind,o.title,o.description,o.protected,CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,o.provenance,
                  t.status,t.priority,t.owner_object_id,t.agent_suitable,t.blocked_reason,t.due_at,
                  t.completed_at,t.github_issue_url,t.brief_markdown
           FROM objects o LEFT JOIN tasks t ON t.object_id=o.id WHERE o.id=$1 FOR UPDATE OF o"#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(CuratorError::NotFound)
}

fn current_object_json(o: &CurrentObject) -> Value {
    json!({"id":o.id,"kind":o.kind,"title":o.title,"description":o.description,"protected":o.protected,"lifecycle":o.lifecycle,"revision":o.revision,"provenance":o.provenance,"status":o.status,"priority":o.priority,"owner_object_id":o.owner_object_id,"agent_suitable":o.agent_suitable,"blocked_reason":o.blocked_reason,"due_at":o.due_at,"completed_at":o.completed_at,"github_issue_url":o.github_issue_url,"brief_markdown":o.brief_markdown})
}

async fn ensure_active_object(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<(), CuratorError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM objects WHERE id=$1 AND archived_at IS NULL)",
    )
    .bind(id)
    .fetch_one(&mut **tx)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(CuratorError::NotFound)
    }
}

async fn current_connection(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<CurrentConnection, CuratorError> {
    sqlx::query_as("SELECT id,source_object_id,kind,target_object_id,description,protected,revision,provenance,archived_at FROM connections WHERE id=$1 FOR UPDATE")
        .bind(id).fetch_optional(&mut **tx).await?.ok_or(CuratorError::NotFound)
}

fn connection_json(c: &CurrentConnection) -> Value {
    json!({"id":c.id,"source_object_id":c.source_object_id,"kind":c.kind,"target_object_id":c.target_object_id,"description":c.description,"protected":c.protected,"revision":c.revision,"provenance":c.provenance,"archived_at":c.archived_at})
}

async fn object_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Value, CuratorError> {
    Ok(current_object_json(&current_object(tx, id).await?))
}
async fn connection_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Value, CuratorError> {
    Ok(connection_json(&current_connection(tx, id).await?))
}

#[allow(clippy::too_many_arguments)]
async fn insert_change(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    sequence: i32,
    entity_type: &str,
    entity_id: Uuid,
    action: &str,
    before: Option<&Value>,
    after: &Value,
    after_revision: i64,
) -> Result<(), CuratorError> {
    sqlx::query(
        r#"INSERT INTO object_events
      (id,run_id,sequence,target_type,target_id,action,actor_type,actor_id,idempotency_key,
       from_revision,to_revision,before_state,after_state,reversible,created_at)
      VALUES ($1,$2,$3,$4,$5,$6,'system','context-curator',$7,$8,$9,$10,$11,true,now())"#,
    )
    .bind(Uuid::new_v4())
    .bind(run_id)
    .bind(sequence)
    .bind(entity_type)
    .bind(entity_id)
    .bind(action)
    .bind(format!(
        "curator:{run_id}:{sequence}:{entity_type}:{entity_id}"
    ))
    .bind(before.map(|_| after_revision - 1))
    .bind(after_revision)
    .bind(before)
    .bind(after)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_event(
    _tx: &mut Transaction<'_, Postgres>,
    _run_id: Uuid,
    _entity_type: &str,
    _entity_id: Uuid,
    _object_id: Uuid,
    _action: &str,
    _from_revision: Option<i64>,
    _to_revision: i64,
    _changes: Value,
) -> Result<(), CuratorError> {
    Ok(())
}

async fn record_failure(pool: &PgPool, run_id: Uuid, message: &str) -> Result<(), CuratorError> {
    crate::runs::fail_curator_run(pool, run_id, message)
        .await
        .map_err(|error| CuratorError::Invalid(format!("Run failure trace failed: {error}")))?;
    Ok(())
}

fn value_str<'a>(value: &'a Value, key: &str) -> Result<&'a str, CuratorError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| CuratorError::Invalid(format!("change journal is missing {key}")))
}
async fn archive_created_connection(
    tx: &mut Transaction<'_, Postgres>,
    change: &CuratorRunChange,
    actor: &crate::domain::ActorContext,
) -> Result<(), CuratorError> {
    let result = sqlx::query("UPDATE connections SET archived_at=now(),revision=revision+1,updated_by_type=$3,updated_by_id=$4,updated_at=now() WHERE id=$1 AND revision=$2 AND archived_at IS NULL")
        .bind(change.entity_id).bind(change.after_revision).bind(actor.actor_type).bind(&actor.actor_id).execute(&mut **tx).await?;
    if result.rows_affected() != 1 {
        return Err(CuratorError::Conflict);
    }
    Ok(())
}

async fn archive_created_object(
    tx: &mut Transaction<'_, Postgres>,
    change: &CuratorRunChange,
    actor: &crate::domain::ActorContext,
) -> Result<(), CuratorError> {
    let current = current_object(tx, change.entity_id).await?;
    crate::domain::validate_object_description(&current.title, &current.description)
        .map_err(invalid)?;
    let active_edges: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM connections WHERE archived_at IS NULL AND (source_object_id=$1 OR target_object_id=$1))").bind(change.entity_id).fetch_one(&mut **tx).await?;
    if active_edges {
        return Err(CuratorError::Conflict);
    }
    let result = sqlx::query("UPDATE objects SET archived_at=now(),revision=revision+1,updated_by_type=$3,updated_by_id=$4,updated_at=now() WHERE id=$1 AND revision=$2 AND archived_at IS NULL")
        .bind(change.entity_id).bind(change.after_revision).bind(actor.actor_type).bind(&actor.actor_id).execute(&mut **tx).await?;
    if result.rows_affected() != 1 {
        return Err(CuratorError::Conflict);
    }
    Ok(())
}

async fn restore_connection(
    tx: &mut Transaction<'_, Postgres>,
    change: &CuratorRunChange,
    actor: &crate::domain::ActorContext,
) -> Result<(), CuratorError> {
    let before = change
        .before_state
        .as_ref()
        .ok_or_else(|| CuratorError::Invalid("change journal lacks before state".into()))?;
    let result = sqlx::query("UPDATE connections SET kind=$3,description=$4,protected=$5,provenance=$6,revision=revision+1,updated_by_type=$7,updated_by_id=$8,updated_at=now() WHERE id=$1 AND revision=$2 AND archived_at IS NULL")
        .bind(change.entity_id).bind(change.after_revision).bind(value_str(before,"kind")?).bind(value_str(before,"description")?).bind(before.get("protected").and_then(Value::as_bool).unwrap_or(false)).bind(before.get("provenance").cloned().unwrap_or_else(||json!({}))).bind(actor.actor_type).bind(&actor.actor_id).execute(&mut **tx).await?;
    if result.rows_affected() != 1 {
        return Err(CuratorError::Conflict);
    }
    Ok(())
}

async fn restore_object(
    tx: &mut Transaction<'_, Postgres>,
    change: &CuratorRunChange,
    actor: &crate::domain::ActorContext,
) -> Result<(), CuratorError> {
    let before = change
        .before_state
        .as_ref()
        .ok_or_else(|| CuratorError::Invalid("change journal lacks before state".into()))?;
    crate::domain::validate_object_description(
        value_str(before, "title")?,
        value_str(before, "description")?,
    )
    .map_err(invalid)?;
    let result = sqlx::query("UPDATE objects SET title=$3,description=$4,protected=$5,provenance=$6,revision=revision+1,updated_by_type=$7,updated_by_id=$8,updated_at=now() WHERE id=$1 AND revision=$2 AND archived_at IS NULL")
        .bind(change.entity_id).bind(change.after_revision).bind(value_str(before,"title")?).bind(value_str(before,"description")?).bind(before.get("protected").and_then(Value::as_bool).unwrap_or(false)).bind(before.get("provenance").cloned().unwrap_or_else(||json!({}))).bind(actor.actor_type).bind(&actor.actor_id).execute(&mut **tx).await?;
    if result.rows_affected() != 1 {
        return Err(CuratorError::Conflict);
    }
    if value_str(before, "kind")? == "task" {
        let owner = before
            .get("owner_object_id")
            .and_then(Value::as_str)
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|_| CuratorError::Invalid("invalid owner in journal".into()))?;
        let due = before
            .get("due_at")
            .and_then(Value::as_str)
            .map(|v| OffsetDateTime::parse(v, &time::format_description::well_known::Rfc3339))
            .transpose()
            .map_err(|_| CuratorError::Invalid("invalid due_at in journal".into()))?;
        let completed_at = before
            .get("completed_at")
            .and_then(Value::as_str)
            .map(|v| OffsetDateTime::parse(v, &time::format_description::well_known::Rfc3339))
            .transpose()
            .map_err(|_| CuratorError::Invalid("invalid completed_at in journal".into()))?;
        sqlx::query(
            r#"UPDATE tasks SET status=$2,priority=$3,owner_object_id=$4,
            agent_suitable=$5,blocked_reason=$6,due_at=$7,completed_at=$8,
            github_issue_url=$9,brief_markdown=$10 WHERE object_id=$1"#,
        )
        .bind(change.entity_id)
        .bind(value_str(before, "status")?)
        .bind(value_str(before, "priority")?)
        .bind(owner)
        .bind(
            before
                .get("agent_suitable")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .bind(before.get("blocked_reason").and_then(Value::as_str))
        .bind(due)
        .bind(completed_at)
        .bind(before.get("github_issue_url").and_then(Value::as_str))
        .bind(before.get("brief_markdown").and_then(Value::as_str))
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

#[derive(Debug, FromRow, Serialize)]
struct WorkerMessage {
    id: Uuid,
    provider_message_id: String,
    sender_user_object_id: Uuid,
    sender_title: String,
    sender_kind: String,
    content: String,
    #[serde(with = "time::serde::rfc3339")]
    source_created_at: OffsetDateTime,
}

#[derive(Debug, Deserialize)]
struct ModelResponse {
    choices: Vec<ModelChoice>,
    usage: Option<ModelUsage>,
}

#[derive(Debug, Deserialize)]
struct ModelUsage {
    #[serde(alias = "input_tokens", alias = "inputTokens")]
    prompt_tokens: Option<i64>,
    #[serde(alias = "output_tokens", alias = "outputTokens")]
    completion_tokens: Option<i64>,
    #[serde(alias = "totalTokens")]
    total_tokens: Option<i64>,
    #[serde(alias = "cachedInputTokens", alias = "cached_input_tokens")]
    cached_input_tokens: Option<i64>,
    #[serde(alias = "reasoningOutputTokens", alias = "reasoning_output_tokens")]
    reasoning_output_tokens: Option<i64>,
    prompt_tokens_details: Option<ModelPromptTokenDetails>,
    completion_tokens_details: Option<ModelCompletionTokenDetails>,
}

#[derive(Debug, Deserialize)]
struct ModelPromptTokenDetails {
    cached_tokens: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ModelCompletionTokenDetails {
    reasoning_tokens: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ModelChoice {
    message: ModelMessage,
}

#[derive(Debug, Deserialize)]
struct ModelMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct CentaurInferenceResponse {
    request_id: String,
    execution_id: String,
    model: String,
    provider: String,
    harness: String,
    authentication_mode: String,
    billing_basis: String,
    upstream: String,
    reasoning_effort: String,
    output: Value,
    usage: Option<ModelUsage>,
}

struct UsageAttribution<'a> {
    provider: &'a str,
    execution_type: &'a str,
    auth_mode: &'a str,
    upstream_service: &'a str,
    billing_mode: &'a str,
    reasoning_effort: Option<&'a str>,
    source_execution_id: &'a str,
}

pub async fn run_worker(
    pool: PgPool,
    embeddings: Option<crate::embeddings::EmbeddingClient>,
    config: CuratorModelConfig,
    text_search_config: crate::config::TextSearchConfig,
    allowed_providers: Vec<String>,
) {
    let worker_id = format!("context-curator-{}", Uuid::new_v4());
    let client = match reqwest::Client::builder()
        .timeout(config.request_timeout)
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            tracing::error!(%error, "failed to initialize Context Curator model client");
            return;
        }
    };
    let mut interval = tokio::time::interval(config.poll_interval);
    loop {
        interval.tick().await;
        let run = match claim_run(&pool, &worker_id, &config, &allowed_providers).await {
            Ok(Some(run)) => run,
            Ok(None) => continue,
            Err(error) => {
                tracing::error!(%error, "Context Curator queue claim failed");
                continue;
            }
        };
        let outcome = async {
            let (messages, candidates) =
                worker_context(&pool, embeddings.as_ref(), text_search_config, &run).await?;
            let mut plan =
                request_plan(&pool, &client, &config, &run, &messages, &candidates, None).await?;
            let dropped = drop_disallowed_worker_creates(&mut plan, &messages);
            let dropped_connections = drop_unresolved_or_ambiguous_worker_connections(
                &mut plan,
                &candidates,
                run.chat_object_id,
            );
            if !dropped.is_empty() || dropped_connections > 0 {
                crate::runs::append_curator_trace(
                    &pool,
                    run.id,
                    "deterministic_filter",
                    json!({
                        "dropped_create_client_ids": dropped,
                        "dropped_unresolved_or_ambiguous_connections": dropped_connections,
                    }),
                )
                .await
                .map_err(|trace_error| {
                    CuratorError::Invalid(format!(
                        "curator deterministic-filter trace failed: {trace_error}"
                    ))
                })?;
            }
            if let Err(error) = validate_plan(&mut plan) {
                crate::runs::append_curator_trace(
                    &pool,
                    run.id,
                    "validation_repair",
                    json!({"error":error.to_string()}),
                )
                .await
                .map_err(|trace_error| {
                    CuratorError::Invalid(format!("eval validation trace failed: {trace_error}"))
                })?;
                plan = request_plan(
                    &pool,
                    &client,
                    &config,
                    &run,
                    &messages,
                    &candidates,
                    Some(&error.to_string()),
                )
                .await?;
                let dropped = drop_disallowed_worker_creates(&mut plan, &messages);
                let dropped_connections = drop_unresolved_or_ambiguous_worker_connections(
                    &mut plan,
                    &candidates,
                    run.chat_object_id,
                );
                if !dropped.is_empty() || dropped_connections > 0 {
                    crate::runs::append_curator_trace(
                        &pool,
                        run.id,
                        "deterministic_filter",
                        json!({
                            "repair": true,
                            "dropped_create_client_ids": dropped,
                            "dropped_unresolved_or_ambiguous_connections": dropped_connections,
                        }),
                    )
                    .await
                    .map_err(|trace_error| {
                        CuratorError::Invalid(format!(
                            "curator deterministic-filter trace failed: {trace_error}"
                        ))
                    })?;
                }
            }
            reconcile_owned(
                &pool,
                run.id,
                &config.model,
                &config.prompt_version,
                plan,
                Some(&worker_id),
            )
            .await
        }
        .await;
        match outcome {
            Ok(_) => tracing::info!(run_id=%run.id, "Context Curator run committed"),
            Err(error) => {
                tracing::warn!(run_id=%run.id, %error, "Context Curator run failed; it may be retried");
                if let Err(record_error) = record_failure(&pool, run.id, &error.to_string()).await {
                    tracing::error!(run_id=%run.id, %record_error, "failed to persist Context Curator failure");
                }
            }
        }
    }
}

async fn claim_run(
    pool: &PgPool,
    worker_id: &str,
    config: &CuratorModelConfig,
    allowed_providers: &[String],
) -> Result<Option<CuratorRun>, CuratorError> {
    let mut tx = pool.begin().await?;
    let run: Option<CuratorRun> = sqlx::query_as(
        r#"WITH candidate AS (
               SELECT id FROM runs
               WHERE kind='curator' AND COALESCE((result->>'attempts')::integer,0) < 3
                 AND EXISTS (SELECT 1 FROM chats WHERE object_id=runs.chat_object_id AND provider=ANY($4))
                 AND (
                   (status IN ('queued','failed') AND available_at <= now())
                   OR (status='running' AND started_at < now() - interval '10 minutes')
                 )
               ORDER BY available_at,created_at,id
               FOR UPDATE SKIP LOCKED
               LIMIT 1
           )
           UPDATE runs r
           SET status='running',started_at=COALESCE(r.started_at,now()),completed_at=NULL,
               result=r.result || jsonb_build_object('worker_id',$1::text,
                 'attempts',COALESCE((r.result->>'attempts')::integer,0)+1,
                 'model',$2::text,'prompt_version',$3::text),error=NULL,updated_at=now()
           FROM candidate c WHERE r.id=c.id
           RETURNING r.id,r.chat_object_id,(r.input->>'first_message_id')::uuid first_message_id,
             (r.input->>'last_message_id')::uuid last_message_id,r.input->>'trigger' trigger,r.status,
             (r.input->>'message_count')::integer message_count,r.idempotency_key,
             COALESCE((r.result->>'attempts')::integer,0) attempts,r.result->>'worker_id' worker_id,
             r.result->>'model' model,r.result->>'prompt_version' prompt_version,
             r.result->'proposed_plan' proposed_plan,r.result->'committed_plan' committed_plan,
             NULLIF(r.result,'{}'::jsonb) result,r.created_at queued_at,r.started_at,
             r.completed_at,CASE WHEN r.status='reversed' THEN r.completed_at END reversed_at,
             r.error error_message"#,
    )
    .bind(worker_id)
    .bind(&config.model)
    .bind(&config.prompt_version)
    .bind(allowed_providers)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(run)
}

async fn worker_context(
    pool: &PgPool,
    embeddings: Option<&crate::embeddings::EmbeddingClient>,
    text_search_config: crate::config::TextSearchConfig,
    run: &CuratorRun,
) -> Result<(Vec<WorkerMessage>, crate::search::SearchPacket), CuratorError> {
    let messages: Vec<WorkerMessage> = sqlx::query_as(
        r#"SELECT m.id,m.provider_message_id,m.sender_user_object_id,o.title AS sender_title,
                  u.user_kind AS sender_kind,m.content,m.source_created_at
           FROM chat_messages m
           JOIN objects o ON o.id=m.sender_user_object_id
           JOIN users u ON u.object_id=m.sender_user_object_id
           WHERE m.chat_object_id=$1 AND m.ingestion_sequence BETWEEN
             (SELECT ingestion_sequence FROM chat_messages WHERE id=$2)
             AND (SELECT ingestion_sequence FROM chat_messages WHERE id=$3)
           ORDER BY m.ingestion_sequence"#,
    )
    .bind(run.chat_object_id)
    .bind(run.first_message_id)
    .bind(run.last_message_id)
    .fetch_all(pool)
    .await?;
    if messages.len() != run.message_count as usize {
        return Err(CuratorError::Invalid(
            "curator run message window no longer matches its recorded count".into(),
        ));
    }
    let query = messages
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(1000)
        .collect::<String>();
    let mut candidates =
        crate::search::search(pool, embeddings, text_search_config, &query, None, 20)
            .await
            .map_err(|error| {
                CuratorError::Invalid(format!("candidate retrieval failed: {error}"))
            })?;
    apply_request_aware_candidate_policy(pool, &query, &mut candidates).await?;
    let candidate_ids = candidates
        .objects
        .iter()
        .map(|object| object.id)
        .collect::<Vec<_>>();
    crate::runs::link_curator_candidates(pool, run.id, &candidate_ids)
        .await
        .map_err(|error| {
            CuratorError::Invalid(format!("candidate eval linkage failed: {error}"))
        })?;
    let mut connections = crate::db::context_connections(pool, &candidate_ids)
        .await
        .map_err(|error| {
            CuratorError::Invalid(format!("candidate graph retrieval failed: {error}"))
        })?;
    for object in &mut candidates.objects {
        object.connections = connections.remove(&object.id).unwrap_or_default();
    }
    Ok((messages, candidates))
}

const WORKFLOW_MUTATION_RELEVANCE_FLOOR: f64 = 0.02;

async fn apply_request_aware_candidate_policy(
    pool: &PgPool,
    query: &str,
    candidates: &mut crate::search::SearchPacket,
) -> Result<(), CuratorError> {
    let direct_ids = referenced_object_ids(query);
    if let Some(kinds) = requested_mutation_kinds(query) {
        candidates.objects.retain(|object| {
            direct_ids.contains(&object.id)
                || (object.relevance.score >= WORKFLOW_MUTATION_RELEVANCE_FLOOR
                    && (kinds.contains(object.kind.as_str())
                        || matches!(object.kind.as_str(), "user" | "chat")))
        });
    }
    for id in direct_ids {
        if candidates.objects.iter().any(|object| object.id == id) {
            continue;
        }
        match crate::search::read_object(pool, id).await {
            Ok(object) => candidates.objects.insert(0, object),
            Err(crate::db::DbError::NotFound) => {}
            Err(error) => {
                return Err(CuratorError::Invalid(format!(
                    "direct Context Object retrieval failed: {error}"
                )));
            }
        }
    }
    Ok(())
}

fn referenced_object_ids(query: &str) -> std::collections::BTreeSet<Uuid> {
    query
        .split(|character: char| !(character.is_ascii_hexdigit() || character == '-'))
        .filter_map(|value| Uuid::parse_str(value).ok())
        .collect()
}

fn requested_mutation_kinds(query: &str) -> Option<std::collections::BTreeSet<&'static str>> {
    let lower = query.to_ascii_lowercase();
    if ![
        "create", "update", "change", "delete", "link", "connect", "record", "add",
    ]
    .iter()
    .any(|verb| {
        lower
            .split_whitespace()
            .any(|word| word.trim_matches(|c: char| !c.is_alphanumeric()) == *verb)
    }) {
        return None;
    }
    let mut kinds = std::collections::BTreeSet::new();
    for (term, kind) in [
        ("entity", "entity"),
        ("person", "entity"),
        ("company", "entity"),
        ("source", "source"),
        ("note", "note"),
        ("task", "task"),
        ("memory", "memory"),
    ] {
        if lower.contains(term) {
            kinds.insert(kind);
        }
    }
    (!kinds.is_empty()).then_some(kinds)
}

async fn request_plan(
    pool: &PgPool,
    client: &reqwest::Client,
    config: &CuratorModelConfig,
    run: &CuratorRun,
    messages: &[WorkerMessage],
    candidates: &crate::search::SearchPacket,
    validation_feedback: Option<&str>,
) -> Result<ReconciliationPlan, CuratorError> {
    let system = r#"You are the Centaur Context Curator, an append-only interaction-memory extractor. Return only one JSON object with exactly these two arrays:
{"create_objects":[],"create_connections":[]}.

Every create_objects entry MUST contain all of these fields:
{"client_id":"unique-local-name","kind":"memory","title":"...","description":"...","supporting_message_ids":["UUID"],"memory":{"primary_event":true,"happened_at":"RFC3339"}}.
client_id is a short unique name used only to reference that new Memory from create_connections. Set primary_event=true for at most one central event.
Every create_connections entry MUST contain all of these fields:
{"source":{"client_id":"created-object-client-id"},"kind":"derived_from","target":{"object_id":"existing-object-UUID"},"description":"...","supporting_message_ids":["UUID"]}.
An existing Object reference is {"object_id":"UUID"}; a newly created Memory reference is {"client_id":"unique-local-name"}.

Create zero or more Memories only for a meaningful event, explicit decision, stable preference or commitment asserted by a human and useful later. A Memory records who did what to which specific subject; research claims and quotations belong in Notes and Sources, not copied summaries. Skip routine status checks, retries, synthetic tests, acknowledgements and incidental mentions. Zero Memories is normal. Every new Memory must have a derived_from Connection from that Memory to run.chat_object_id, using the exact same supporting_message_ids. Other links may connect a new or existing Memory to one unambiguous existing candidate using only involves, about, themed, related_to, or derived_from. Never invent a missing target or choose between ambiguous candidates; leave it unlinked. Every Connection must have at least one Memory endpoint.

Never create an Entity, Source, Note, Task, Chat, User, Theme, or any other non-Memory Object. Never update or delete any Object, including a Memory. Never update or delete a Connection. A human request proves only that the request was made: without a trusted workflow-result message, describe what the human asked for and never claim the workflow succeeded, completed, created, updated, or connected anything.

Never create a Memory from an unanswered question, a failed or empty search, an authentication or authorization error, a timeout, missing tool access, agent uncertainty, or an assistant report that evidence could not be verified. Those are transient operational outcomes, not durable knowledge. Every operation cites supporting_message_ids from this run. Use only IDs from candidate_objects for existing non-Chat endpoints. A Memory description is normally ONE plain sentence of about 15–35 words, at most two short sentences when essential. There is no minimum length. Name the actor, truthful action and concrete subject. Use the actual event time in happened_at; avoid relative time such as just or today. Use a truthful verb such as asked instead of adding caveats about what was not established. Keep IDs and evidence in provenance and Connections, not narrative boilerplate. Link only direct participants and affected objects, never incidental mentions or guessed themes. A request and a completed action are different events. Do not re-create an event already supported by the same message IDs in an existing Memory. Never repeat only the title, use placeholders or vague meta text, copy transcript fragments, mention the model or generation process, or use connection counts for reconciliation."#;
    let system = if event_capture_enabled() {
        format!(
            "{system}\nActual Source, Note and Task creation outcomes are captured separately from committed Object Events. Do not duplicate those outcomes from chat prose, even if a human reports them. You may selectively record a meaningful request, decision or discussion instead."
        )
    } else {
        system.to_owned()
    };
    let input = json!({
        "run": {"id":run.id,"chat_object_id":run.chat_object_id,"trigger":run.trigger},
        "messages": messages,
        "candidate_objects": candidates,
        "validation_feedback": validation_feedback,
    });
    let input =
        serde_json::to_string(&input).map_err(|error| CuratorError::Invalid(error.to_string()))?;
    let idempotency_id = format!(
        "curator-{}-{}",
        run.id,
        if validation_feedback.is_some() {
            "repair"
        } else {
            "initial"
        }
    );
    let value = request_json_model(
        pool,
        client,
        config,
        run.id,
        Some(run.chat_object_id),
        &system,
        input,
        reconciliation_plan_schema(),
        idempotency_id,
    )
    .await?;
    serde_json::from_value(value)
        .map_err(|e| CuratorError::Invalid(format!("invalid capture plan: {e}")))
}

/// Shared inference transport and usage attribution. Callers retain separate
/// schemas and commit authorities; model output itself grants no permissions.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn request_json_model(
    pool: &PgPool,
    client: &reqwest::Client,
    config: &CuratorModelConfig,
    run_id: Uuid,
    chat_id: Option<Uuid>,
    system: &str,
    input: String,
    schema: Value,
    idempotency_id: String,
) -> Result<Value, CuratorError> {
    let attempt_id = Uuid::new_v4().to_string();
    let request_body = match config.transport {
        CuratorModelTransport::CentaurSubscription => json!({
            "request_id": idempotency_id,
            "system_prompt": system,
            "input": input,
            "output_schema": schema,
            "reasoning_effort": "low"
        }),
        CuratorModelTransport::DirectApi => json!({
            "model": config.model,
            "messages": [
                {"role":"system","content":system},
                {"role":"user","content":input}
            ],
            "response_format":{"type":"json_object"},
            "temperature":0
        }),
    };
    let response = match client
        .post(&config.endpoint)
        .bearer_auth(&config.api_token)
        .json(&request_body)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let attribution = default_usage_attribution(config, &attempt_id);
            record_curator_usage(
                pool,
                run_id,
                chat_id,
                &config.model,
                &attribution,
                None,
                Some(&error.to_string()),
            )
            .await;
            return Err(CuratorError::Invalid(format!(
                "curator model request failed: {error}"
            )));
        }
    };
    let status = response.status();
    if !status.is_success() {
        let attribution = default_usage_attribution(config, &attempt_id);
        record_curator_usage(
            pool,
            run_id,
            chat_id,
            &config.model,
            &attribution,
            None,
            Some(&format!("HTTP {status}")),
        )
        .await;
        return Err(CuratorError::Invalid(format!(
            "curator model returned HTTP {status}"
        )));
    }
    let body: Value = match response.json().await {
        Ok(body) => body,
        Err(error) => {
            let attribution = default_usage_attribution(config, &attempt_id);
            record_curator_usage(
                pool,
                run_id,
                chat_id,
                &config.model,
                &attribution,
                None,
                Some(&error.to_string()),
            )
            .await;
            return Err(CuratorError::Invalid(format!(
                "invalid curator model response: {error}"
            )));
        }
    };
    match config.transport {
        CuratorModelTransport::CentaurSubscription => {
            let response: CentaurInferenceResponse = match serde_json::from_value(body) {
                Ok(response) => response,
                Err(error) => {
                    let attribution = default_usage_attribution(config, &attempt_id);
                    record_curator_usage(
                        pool,
                        run_id,
                        chat_id,
                        &config.model,
                        &attribution,
                        None,
                        Some(&error.to_string()),
                    )
                    .await;
                    return Err(CuratorError::Invalid(format!(
                        "invalid Centaur inference response: {error}"
                    )));
                }
            };
            if response.request_id != idempotency_id
                || response.model != "gpt-5.6-luna"
                || response.provider != "openai"
                || response.harness != "codex"
                || response.authentication_mode != "chatgpt_subscription"
                || response.billing_basis != "chatgpt_subscription"
                || response.upstream != "chatgpt.com"
            {
                let attribution = default_usage_attribution(config, &attempt_id);
                record_curator_usage(
                    pool,
                    run_id,
                    chat_id,
                    &config.model,
                    &attribution,
                    response.usage.as_ref(),
                    Some("Centaur inference attribution mismatch"),
                )
                .await;
                return Err(CuratorError::Invalid(
                    "Centaur inference attribution did not match the subscription contract"
                        .to_owned(),
                ));
            }
            let attribution = UsageAttribution {
                provider: &response.provider,
                execution_type: "codex_harness",
                auth_mode: &response.authentication_mode,
                upstream_service: &response.upstream,
                billing_mode: "subscription_allowance",
                reasoning_effort: Some(&response.reasoning_effort),
                source_execution_id: &response.execution_id,
            };
            record_curator_usage(
                pool,
                run_id,
                chat_id,
                &config.model,
                &attribution,
                response.usage.as_ref(),
                response
                    .usage
                    .is_none()
                    .then_some("Codex response omitted usage"),
            )
            .await;
            let plan: Value = serde_json::from_value(response.output).map_err(|error| {
                CuratorError::Invalid(format!(
                    "curator model returned an invalid reconciliation plan: {error}"
                ))
            })?;
            Ok(plan)
        }
        CuratorModelTransport::DirectApi => {
            let response: ModelResponse = match serde_json::from_value(body) {
                Ok(response) => response,
                Err(error) => {
                    let attribution = default_usage_attribution(config, &attempt_id);
                    record_curator_usage(
                        pool,
                        run_id,
                        chat_id,
                        &config.model,
                        &attribution,
                        None,
                        Some(&error.to_string()),
                    )
                    .await;
                    return Err(CuratorError::Invalid(format!(
                        "invalid direct API response: {error}"
                    )));
                }
            };
            let attribution = default_usage_attribution(config, &attempt_id);
            record_curator_usage(
                pool,
                run_id,
                chat_id,
                &config.model,
                &attribution,
                response.usage.as_ref(),
                response
                    .usage
                    .is_none()
                    .then_some("provider response omitted usage"),
            )
            .await;
            let content = response
                .choices
                .into_iter()
                .next()
                .ok_or_else(|| CuratorError::Invalid("curator model returned no choices".into()))?
                .message
                .content;
            let plan: Value = serde_json::from_str(&content).map_err(|error| {
                CuratorError::Invalid(format!(
                    "curator model returned an invalid reconciliation plan: {error}"
                ))
            })?;
            Ok(plan)
        }
    }
}

fn reconciliation_plan_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["create_objects", "create_connections"],
        "properties": {
            "create_objects": {"type": "array", "items": {"$ref": "#/$defs/create_object"}},
            "create_connections": {"type": "array", "items": {"$ref": "#/$defs/create_connection"}}
        },
        "$defs": {
            "uuid": {"type": "string"},
            "message_ids": {"type": "array", "items": {"$ref": "#/$defs/uuid"}},
            "memory_fields": {
                "type": "object", "additionalProperties": false,
                "required": ["primary_event", "happened_at"],
                "properties": {"primary_event": {"type": "boolean"}, "happened_at": {"type": "string"}}
            },
            "object_ref": {
                "anyOf": [
                    {"type": "object", "additionalProperties": false, "required": ["object_id"], "properties": {"object_id": {"$ref": "#/$defs/uuid"}}},
                    {"type": "object", "additionalProperties": false, "required": ["client_id"], "properties": {"client_id": {"type": "string"}}}
                ]
            },
            "create_object": {
                "type": "object", "additionalProperties": false,
                "required": ["client_id", "kind", "title", "description", "supporting_message_ids", "memory"],
                "properties": {
                    "client_id": {"type": "string"}, "kind": {"type": "string", "enum": ["memory"]}, "title": {"type": "string"},
                    "description": {"type": "string"}, "supporting_message_ids": {"$ref": "#/$defs/message_ids"},
                    "memory": {"$ref": "#/$defs/memory_fields"}
                }
            },
            "create_connection": {
                "type": "object", "additionalProperties": false,
                "required": ["source", "kind", "target", "description", "supporting_message_ids"],
                "properties": {
                    "source": {"$ref": "#/$defs/object_ref"},
                    "kind": {"type": "string", "enum": ["involves", "about", "themed", "related_to", "derived_from"]},
                    "target": {"$ref": "#/$defs/object_ref"}, "description": {"type": "string"},
                    "supporting_message_ids": {"$ref": "#/$defs/message_ids"}
                }
            }
        }
    })
}

fn default_usage_attribution<'a>(
    config: &CuratorModelConfig,
    attempt_id: &'a str,
) -> UsageAttribution<'a> {
    match config.transport {
        CuratorModelTransport::CentaurSubscription => UsageAttribution {
            provider: "openai",
            execution_type: "codex_harness",
            auth_mode: "chatgpt_subscription",
            upstream_service: "chatgpt.com",
            billing_mode: "subscription_allowance",
            reasoning_effort: Some("low"),
            source_execution_id: attempt_id,
        },
        CuratorModelTransport::DirectApi => UsageAttribution {
            provider: "openai",
            execution_type: "direct_api",
            auth_mode: "api_key",
            upstream_service: "api.openai.com",
            billing_mode: "metered_api",
            reasoning_effort: None,
            source_execution_id: attempt_id,
        },
    }
}

async fn record_curator_usage(
    pool: &PgPool,
    run_id: Uuid,
    chat_id: Option<Uuid>,
    model_id: &str,
    attribution: &UsageAttribution<'_>,
    usage: Option<&ModelUsage>,
    missing_reason: Option<&str>,
) {
    let input = crate::runs::NormalizedUsage {
        run_id,
        component: if chat_id.is_some() {
            "context_curator"
        } else {
            "context_memory_dream"
        }
        .into(),
        provider: attribution.provider.into(),
        model_id: model_id.to_owned(),
        display_tier: Some(model_id.to_owned()),
        execution_type: attribution.execution_type.into(),
        auth_mode: attribution.auth_mode.into(),
        upstream_service: attribution.upstream_service.into(),
        billing_mode: attribution.billing_mode.into(),
        reasoning_effort: attribution.reasoning_effort.map(str::to_owned),
        service_tier: None,
        source_thread_id: chat_id.map(|id| id.to_string()),
        source_execution_id: attribution.source_execution_id.to_owned(),
        source_turn_id: Some(run_id.to_string()),
        call_index: Some(1),
        usage_status: if usage.is_some() {
            "reported"
        } else {
            "unavailable"
        }
        .into(),
        usage_missing_reason: missing_reason.map(str::to_owned),
        input_tokens: usage.and_then(|value| value.prompt_tokens),
        output_tokens: usage.and_then(|value| value.completion_tokens),
        cache_creation_tokens: None,
        cache_read_tokens: usage.and_then(|value| {
            value.cached_input_tokens.or_else(|| {
                value
                    .prompt_tokens_details
                    .as_ref()
                    .and_then(|details| details.cached_tokens)
            })
        }),
        reasoning_tokens: usage.and_then(|value| {
            value.reasoning_output_tokens.or_else(|| {
                value
                    .completion_tokens_details
                    .as_ref()
                    .and_then(|details| details.reasoning_tokens)
            })
        }),
        total_tokens: usage.and_then(|value| value.total_tokens),
        estimated_micro_usd: None,
        chatgpt_credit_microunits: None,
        api_equivalent_micro_usd: None,
        rate_card_version: None,
        pricing_snapshot: None,
    };
    if let Err(error) = crate::runs::record_usage(pool, &input).await {
        tracing::error!(run_id=%run_id,%error,"failed to record Curator usage");
    }
}

#[derive(Debug)]
enum CuratorApiError {
    Unauthorized,
    Curator(CuratorError),
}
impl From<CuratorError> for CuratorApiError {
    fn from(value: CuratorError) -> Self {
        Self::Curator(value)
    }
}
impl IntoResponse for CuratorApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Authentication failed.".to_owned(),
            ),
            Self::Curator(CuratorError::NotFound) => (
                StatusCode::NOT_FOUND,
                "not_found",
                "Record not found.".to_owned(),
            ),
            Self::Curator(CuratorError::Conflict) => (
                StatusCode::CONFLICT,
                "revision_conflict",
                "The curator run or a target record changed after it was read.".to_owned(),
            ),
            Self::Curator(CuratorError::Invalid(message)) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_error",
                message,
            ),
            Self::Curator(CuratorError::Sqlx(error)) => {
                tracing::error!(%error,"curator database request failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "The curator request could not be completed.".to_owned(),
                )
            }
        };
        (
            status,
            Json(json!({"error":{"code":code,"message":message}})),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worker_message(id: Uuid, sender_kind: &str) -> WorkerMessage {
        WorkerMessage {
            id,
            provider_message_id: id.to_string(),
            sender_user_object_id: Uuid::new_v4(),
            sender_title: sender_kind.into(),
            sender_kind: sender_kind.into(),
            content: "supporting message".into(),
            source_created_at: OffsetDateTime::now_utc(),
        }
    }

    fn worker_message_with_content(id: Uuid, sender_kind: &str, content: &str) -> WorkerMessage {
        WorkerMessage {
            content: content.into(),
            ..worker_message(id, sender_kind)
        }
    }

    fn created_object(
        client_id: &str,
        kind: &str,
        supporting_message_ids: Vec<Uuid>,
    ) -> CreateObject {
        CreateObject {
            client_id: client_id.into(),
            kind: kind.into(),
            title: format!("{kind} title"),
            description: format!("A concrete {kind} description with enough grounded context."),
            supporting_message_ids,
            entity_kind: None,
            task: None,
            memory: (kind == "memory").then(|| MemoryFields {
                primary_event: true,
                happened_at: OffsetDateTime::now_utc(),
            }),
            source: None,
        }
    }

    #[test]
    fn worker_filter_drops_non_memories_and_their_connections() {
        let human_id = Uuid::new_v4();
        let mut plan = ReconciliationPlan {
            create_objects: vec![created_object("duplicate-source", "source", vec![human_id])],
            update_objects: vec![],
            create_connections: vec![CreateConnection {
                source: ObjectRef::Created {
                    client_id: "duplicate-source".into(),
                },
                kind: "derived_from".into(),
                target: ObjectRef::Existing {
                    object_id: Uuid::new_v4(),
                },
                description: "Derived from the conversation.".into(),
                supporting_message_ids: vec![human_id],
            }],
            update_connections: vec![],
        };

        let dropped =
            drop_disallowed_worker_creates(&mut plan, &[worker_message(human_id, "human")]);

        assert_eq!(dropped, vec!["duplicate-source"]);
        assert!(plan.create_objects.is_empty());
        assert!(plan.create_connections.is_empty());
    }

    #[test]
    fn worker_filter_drops_assistant_derived_memories() {
        let agent_id = Uuid::new_v4();
        let mut plan = ReconciliationPlan {
            create_objects: vec![created_object("assistant-memory", "memory", vec![agent_id])],
            update_objects: vec![],
            create_connections: vec![],
            update_connections: vec![],
        };

        let dropped =
            drop_disallowed_worker_creates(&mut plan, &[worker_message(agent_id, "agent")]);

        assert_eq!(dropped, vec!["assistant-memory"]);
        assert!(plan.create_objects.is_empty());
    }

    #[test]
    fn worker_filter_drops_tasks() {
        let human_id = Uuid::new_v4();
        let mut plan = ReconciliationPlan {
            create_objects: vec![created_object("ingestion-task", "task", vec![human_id])],
            update_objects: vec![],
            create_connections: vec![],
            update_connections: vec![],
        };

        let dropped =
            drop_disallowed_worker_creates(&mut plan, &[worker_message(human_id, "human")]);

        assert_eq!(dropped, vec!["ingestion-task"]);
        assert!(plan.create_objects.is_empty());
    }

    #[test]
    fn worker_filter_keeps_human_grounded_memory() {
        let human_id = Uuid::new_v4();
        let mut plan = ReconciliationPlan {
            create_objects: vec![created_object("durable-memory", "memory", vec![human_id])],
            update_objects: vec![],
            create_connections: vec![],
            update_connections: vec![],
        };

        let dropped = drop_disallowed_worker_creates(
            &mut plan,
            &[worker_message_with_content(
                human_id,
                "human",
                "The team chose Friday for the research review",
            )],
        );

        assert!(dropped.is_empty());
        assert_eq!(plan.create_objects.len(), 1);
    }

    #[test]
    fn worker_filter_leaves_missing_and_ambiguous_targets_unlinked() {
        let message_id = Uuid::new_v4();
        let chat_id = Uuid::new_v4();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let missing = Uuid::new_v4();
        let mut plan = ReconciliationPlan {
            create_objects: vec![created_object("memory", "memory", vec![message_id])],
            update_objects: vec![],
            create_connections: [chat_id, first, missing]
                .into_iter()
                .map(|target| CreateConnection {
                    source: ObjectRef::Created {
                        client_id: "memory".into(),
                    },
                    kind: if target == chat_id {
                        "derived_from".into()
                    } else {
                        "about".into()
                    },
                    target: ObjectRef::Existing { object_id: target },
                    description: "A candidate link from the extracted Memory.".into(),
                    supporting_message_ids: vec![message_id],
                })
                .collect(),
            update_connections: vec![],
        };
        let retrieved = |id| crate::search::RetrievedObject {
            id,
            kind: "entity".into(),
            title: "Same person".into(),
            description: "An ambiguous candidate with the same canonical title.".into(),
            revision: 1,
            subtype: None,
            relevance: crate::search::Relevance {
                score: 1.0,
                rationale: "test".into(),
            },
            evidence: None,
            connections: vec![],
        };
        let candidates = crate::search::SearchPacket {
            query: "same person".into(),
            retrieval: "test".into(),
            objects: vec![retrieved(first), retrieved(second)],
            budget: None,
        };

        assert_eq!(
            drop_unresolved_or_ambiguous_worker_connections(&mut plan, &candidates, chat_id),
            2
        );
        assert_eq!(plan.create_connections.len(), 1);
        assert_eq!(plan.create_connections[0].kind, "derived_from");
    }

    #[test]
    fn deterministic_validation_rejects_every_non_memory_kind() {
        for kind in [
            "entity", "source", "note", "task", "chat", "user", "unknown",
        ] {
            let mut plan = ReconciliationPlan {
                create_objects: vec![created_object("forbidden", kind, vec![Uuid::new_v4()])],
                update_objects: vec![],
                create_connections: vec![],
                update_connections: vec![],
            };
            assert!(validate_plan(&mut plan).is_err(), "kind {kind}");
        }
    }

    #[test]
    fn deterministic_validation_rejects_object_and_connection_updates() {
        let mut object_update = ReconciliationPlan {
            create_objects: vec![],
            update_objects: vec![UpdateObject {
                object_id: Uuid::new_v4(),
                expected_revision: 1,
                title: Some("Changed".into()),
                description: None,
                supporting_message_ids: vec![Uuid::new_v4()],
                task: None,
            }],
            create_connections: vec![],
            update_connections: vec![],
        };
        assert!(validate_plan(&mut object_update).is_err());

        let mut connection_update = ReconciliationPlan {
            create_objects: vec![],
            update_objects: vec![],
            create_connections: vec![],
            update_connections: vec![UpdateConnection {
                connection_id: Uuid::new_v4(),
                expected_revision: 1,
                kind: None,
                description: Some("Changed".into()),
                supporting_message_ids: vec![Uuid::new_v4()],
            }],
        };
        assert!(validate_plan(&mut connection_update).is_err());
    }

    #[test]
    fn deterministic_validation_rejects_unsupported_relation() {
        let mut plan = ReconciliationPlan {
            create_objects: vec![],
            update_objects: vec![],
            create_connections: vec![CreateConnection {
                source: ObjectRef::Existing {
                    object_id: Uuid::new_v4(),
                },
                kind: "depends_on".into(),
                target: ObjectRef::Existing {
                    object_id: Uuid::new_v4(),
                },
                description: "Unsupported relation between records.".into(),
                supporting_message_ids: vec![Uuid::new_v4()],
            }],
            update_connections: vec![],
        };
        assert!(validate_plan(&mut plan).is_err());
    }

    #[test]
    fn deterministic_validation_rejects_missing_chat_provenance() {
        let mut plan = ReconciliationPlan {
            create_objects: vec![created_object("memory", "memory", vec![Uuid::new_v4()])],
            update_objects: vec![],
            create_connections: vec![],
            update_connections: vec![],
        };
        assert!(
            validate_plan(&mut plan)
                .unwrap_err()
                .to_string()
                .contains("originating Chat")
        );
    }

    #[test]
    fn legacy_delete_fields_are_rejected_instead_of_ignored() {
        let error = serde_json::from_value::<ReconciliationPlan>(json!({
            "create_objects": [],
            "create_connections": [],
            "delete_objects": [{"object_id": Uuid::new_v4()}]
        }))
        .unwrap_err();
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn request_only_evidence_requires_request_framing() {
        let message_id = Uuid::new_v4();
        let messages = HashMap::from([(
            message_id,
            MessageEvidence {
                id: message_id,
                sender_kind: "human".into(),
                content: "Research and add Alexandr Wang".into(),
            },
        )]);
        let mut plan = ReconciliationPlan {
            create_objects: vec![created_object("memory", "memory", vec![message_id])],
            update_objects: vec![],
            create_connections: vec![],
            update_connections: vec![],
        };
        plan.create_objects[0].description =
            "Alexandr Wang was researched and added to the knowledge graph.".into();
        assert!(validate_human_grounded_objects(&plan, &messages).is_err());
        plan.create_objects[0].description =
            "Bradley requested research on Alexandr Wang and asked for him to be added.".into();
        assert!(validate_human_grounded_objects(&plan, &messages).is_ok());
    }

    #[test]
    fn curator_prompt_rejects_operational_failures_as_memories() {
        let source = include_str!("curator.rs");
        assert!(source.contains("unanswered question"));
        assert!(source.contains("authentication or authorization error"));
        assert!(source.contains("transient operational outcomes"));
        assert!(source.contains("explicitly asserted by a human message"));
    }

    #[test]
    fn subscription_schema_exposes_only_append_only_memory_operations() {
        let schema = reconciliation_plan_schema();
        assert_eq!(schema["additionalProperties"], json!(false));
        assert!(schema["properties"].get("update_objects").is_none());
        assert!(schema["properties"].get("update_connections").is_none());
        assert_eq!(
            schema["$defs"]["create_object"]["properties"]["kind"]["enum"],
            json!(["memory"])
        );
        assert_eq!(
            schema["$defs"]["create_connection"]["properties"]["kind"]["enum"],
            json!(CURATOR_CONNECTION_KINDS)
        );
        for definition in ["memory_fields", "create_object", "create_connection"] {
            assert_eq!(
                schema["$defs"][definition]["additionalProperties"],
                json!(false),
                "{definition} must reject extra model fields"
            );
        }
    }

    #[test]
    fn direct_api_attribution_is_an_explicit_rollback() {
        let config = CuratorModelConfig {
            transport: CuratorModelTransport::DirectApi,
            endpoint: "https://api.openai.com/v1/chat/completions".to_owned(),
            api_token: "test-token".to_owned(),
            model: "gpt-4.1-mini".to_owned(),
            prompt_version: "test".to_owned(),
            poll_interval: std::time::Duration::from_secs(1),
            request_timeout: std::time::Duration::from_secs(210),
        };
        let attribution = default_usage_attribution(&config, "attempt-1");
        assert_eq!(attribution.execution_type, "direct_api");
        assert_eq!(attribution.auth_mode, "api_key");
        assert_eq!(attribution.billing_mode, "metered_api");
    }

    #[test]
    fn subscription_attribution_uses_canonical_run_values() {
        let config = CuratorModelConfig {
            transport: CuratorModelTransport::CentaurSubscription,
            endpoint: "http://centaur-api-rs/api/internal/context-curator/infer".to_owned(),
            api_token: "test-token".to_owned(),
            model: "gpt-5.6-luna".to_owned(),
            prompt_version: "test".to_owned(),
            poll_interval: std::time::Duration::from_secs(1),
            request_timeout: std::time::Duration::from_secs(210),
        };
        let attribution = default_usage_attribution(&config, "attempt-1");
        assert_eq!(attribution.execution_type, "codex_harness");
        assert_eq!(attribution.auth_mode, "chatgpt_subscription");
        assert_eq!(attribution.billing_mode, "subscription_allowance");
    }

    #[test]
    fn mutation_kind_policy_is_request_aware() {
        assert_eq!(
            requested_mutation_kinds("Create an Entity for Example Person"),
            Some(std::collections::BTreeSet::from(["entity"]))
        );
        assert_eq!(
            requested_mutation_kinds("Research companies in this market"),
            None
        );
    }

    #[test]
    fn canonical_object_ids_are_extracted_from_requests() {
        let id = Uuid::new_v4();
        assert_eq!(
            referenced_object_ids(&format!("Update object {id}, please")),
            std::collections::BTreeSet::from([id])
        );
    }
}

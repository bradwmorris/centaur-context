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
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use subtle::ConstantTimeEq;
use time::OffsetDateTime;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::{
    api::AppState,
    config::{CuratorModelConfig, CuratorModelTransport},
    domain::{
        CONNECTION_KINDS, TASK_PRIORITIES, TASK_STATUSES, allowed, optional_text, required_text,
    },
};

const MAX_OPERATIONS: usize = 100;

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
pub struct UpdateObject {
    pub object_id: Uuid,
    pub expected_revision: i64,
    pub title: Option<String>,
    pub description: Option<String>,
    pub supporting_message_ids: Vec<Uuid>,
    pub task: Option<TaskPatch>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
pub struct MemoryFields {
    pub primary_event: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub happened_at: OffsetDateTime,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
#[serde(untagged)]
pub enum ObjectRef {
    Existing { object_id: Uuid },
    Created { client_id: String },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreateConnection {
    pub source: ObjectRef,
    pub kind: String,
    pub target: ObjectRef,
    pub description: String,
    pub supporting_message_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
            return Ok(run
                .result
                .unwrap_or_else(|| json!({"run_id": run_id, "status": "completed"})));
        }
        return Err(CuratorError::Conflict);
    }
    if run.status == "reversed"
        || (run.status == "running" && run.worker_id.as_deref() != claimed_by)
    {
        return Err(CuratorError::Conflict);
    }
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

    let message_ids = load_message_window(&mut tx, &run).await?;
    validate_message_refs(&plan, &message_ids)?;
    let mut created = HashMap::new();
    let mut changed_objects = HashMap::new();
    let mut sequence = 0_i32;

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
        changed_objects.insert(
            id,
            item.supporting_message_ids
                .iter()
                .copied()
                .collect::<HashSet<_>>(),
        );
    }

    for item in &plan.update_objects {
        sequence += 1;
        let current = current_object(&mut tx, item.object_id).await?;
        if current.protected || current.lifecycle != "active" {
            return Err(CuratorError::Invalid(
                "the curator cannot update a protected or archived Object".into(),
            ));
        }
        if current.revision != item.expected_revision {
            return Err(CuratorError::Conflict);
        }
        if current.kind == "chat" || current.kind == "user" {
            return Err(CuratorError::Invalid(
                "Chat and User Objects are protected from curator reconciliation".into(),
            ));
        }
        validate_task_patch(&current.kind, item.task.as_ref())?;
        crate::domain::validate_object_description(
            item.title.as_deref().unwrap_or(&current.title),
            item.description.as_deref().unwrap_or(&current.description),
        )
        .map_err(invalid)?;
        let before = current_object_json(&current);
        let provenance = curator_provenance(
            run_id,
            run.chat_object_id,
            &item.supporting_message_ids,
            model,
            prompt_version,
        );
        update_object(&mut tx, &current, item, &provenance).await?;
        let after = object_snapshot(&mut tx, item.object_id).await?;
        insert_change(
            &mut tx,
            run_id,
            sequence,
            "object",
            item.object_id,
            "updated",
            Some(&before),
            &after,
            current.revision + 1,
        )
        .await?;
        insert_event(
            &mut tx,
            run_id,
            if current.kind == "task" {
                "task"
            } else {
                "object"
            },
            item.object_id,
            item.object_id,
            "updated",
            Some(current.revision),
            current.revision + 1,
            json!({"supporting_message_ids": item.supporting_message_ids}),
        )
        .await?;
        changed_objects.insert(
            item.object_id,
            item.supporting_message_ids
                .iter()
                .copied()
                .collect::<HashSet<_>>(),
        );
    }

    for item in &plan.create_connections {
        sequence += 1;
        let source = resolve_ref(&item.source, &created)?;
        let target = resolve_ref(&item.target, &created)?;
        if source == target {
            return Err(CuratorError::Invalid(
                "a connection cannot link an Object to itself".into(),
            ));
        }
        ensure_active_object(&mut tx, source).await?;
        ensure_active_object(&mut tx, target).await?;
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

    for item in &plan.update_connections {
        sequence += 1;
        let current = current_connection(&mut tx, item.connection_id).await?;
        if current.protected || current.archived_at.is_some() {
            return Err(CuratorError::Invalid(
                "the curator cannot update a protected or archived connection".into(),
            ));
        }
        if current.revision != item.expected_revision {
            return Err(CuratorError::Conflict);
        }
        let before = connection_json(&current);
        let provenance = curator_provenance(
            run_id,
            run.chat_object_id,
            &item.supporting_message_ids,
            model,
            prompt_version,
        );
        sqlx::query(
            r#"UPDATE connections SET kind=COALESCE($3,kind),description=COALESCE($4,description),
                  provenance=$5,revision=revision+1,updated_by_type='system',updated_by_id='context-curator',updated_at=now()
               WHERE id=$1 AND revision=$2 AND archived_at IS NULL"#,
        ).bind(item.connection_id).bind(item.expected_revision).bind(&item.kind).bind(&item.description).bind(&provenance).execute(&mut *tx).await?;
        let after = connection_snapshot(&mut tx, item.connection_id).await?;
        insert_change(
            &mut tx,
            run_id,
            sequence,
            "connection",
            item.connection_id,
            "updated",
            Some(&before),
            &after,
            current.revision + 1,
        )
        .await?;
        insert_event(
            &mut tx,
            run_id,
            "connection",
            item.connection_id,
            current.source_object_id,
            "updated",
            Some(current.revision),
            current.revision + 1,
            json!({"supporting_message_ids": item.supporting_message_ids}),
        )
        .await?;
    }

    validate_derived_connections(&plan, &created, &changed_objects, run.chat_object_id)?;
    let result = json!({
        "run_id": run_id, "status": "completed", "chat_object_id": run.chat_object_id,
        "created_objects": created, "change_count": sequence,
    });
    sqlx::query(
        r#"UPDATE runs SET status='completed',completed_at=now(),
                  result=$3 || jsonb_build_object('committed_plan',$2::jsonb),error=NULL,updated_at=now()
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
    if count == 0 || count > MAX_OPERATIONS {
        return Err(CuratorError::Invalid(
            "a reconciliation plan must contain between 1 and 100 operations".into(),
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
        item.kind = allowed(
            std::mem::take(&mut item.kind),
            "kind",
            &["task", "entity", "memory", "source"],
        )
        .map_err(invalid)?;
        item.title =
            required_text(std::mem::take(&mut item.title), "title", 300).map_err(invalid)?;
        item.description =
            crate::domain::object_description(&item.title, std::mem::take(&mut item.description))
                .map_err(invalid)?;
        match item.kind.as_str() {
            "task" => {
                let task = item.task.as_mut().ok_or_else(|| {
                    CuratorError::Invalid("Task creation requires task fields".into())
                })?;
                validate_task_fields(task)?;
                if item.entity_kind.is_some() || item.memory.is_some() {
                    return Err(CuratorError::Invalid(
                        "Task creation cannot include memory fields".into(),
                    ));
                }
                if item.source.is_some() {
                    return Err(CuratorError::Invalid(
                        "Task creation cannot include source fields".into(),
                    ));
                }
            }
            "memory" => {
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
            "source" => {
                if item.entity_kind.is_some() || item.task.is_some() || item.memory.is_some() {
                    return Err(CuratorError::Invalid(
                        "Source creation requires only source fields".into(),
                    ));
                }
                validate_source_fields(item.source.as_mut().ok_or_else(|| {
                    CuratorError::Invalid("Source creation requires source fields".into())
                })?)?;
            }
            "entity" => {
                if item.task.is_some() || item.memory.is_some() || item.source.is_some() {
                    return Err(CuratorError::Invalid(
                        "Entity creation cannot include other typed fields".into(),
                    ));
                }
                item.entity_kind = Some(
                    allowed(
                        item.entity_kind.take().ok_or_else(|| {
                            CuratorError::Invalid("Entity creation requires entity_kind".into())
                        })?,
                        "entity_kind",
                        &[
                            "person",
                            "organization",
                            "product",
                            "project",
                            "publication",
                            "place",
                            "concept",
                            "other",
                        ],
                    )
                    .map_err(invalid)?,
                );
            }
            _ => unreachable!(),
        }
    }
    let mut updated_object_ids = HashSet::new();
    for item in &mut plan.update_objects {
        if !updated_object_ids.insert(item.object_id) {
            return Err(CuratorError::Invalid(
                "an Object may be updated only once in a reconciliation plan".into(),
            ));
        }
        if item.expected_revision < 1 {
            return Err(CuratorError::Invalid(
                "expected_revision must be positive".into(),
            ));
        }
        item.title = optional_text(item.title.take(), "title", 300).map_err(invalid)?;
        item.description =
            optional_text(item.description.take(), "description", 2000).map_err(invalid)?;
        if let (Some(title), Some(description)) = (&item.title, &item.description) {
            crate::domain::validate_object_description(title, description).map_err(invalid)?;
        }
        if let Some(task) = &mut item.task {
            if !task.confirmed {
                return Err(CuratorError::Invalid(
                    "a Task update requires an explicit confirmed instruction or commitment".into(),
                ));
            }
            task.status = task
                .status
                .take()
                .map(|value| allowed(value, "status", TASK_STATUSES))
                .transpose()
                .map_err(invalid)?;
            task.priority = task
                .priority
                .take()
                .map(|value| allowed(value, "priority", TASK_PRIORITIES))
                .transpose()
                .map_err(invalid)?;
            if task.clear_owner && task.owner_object_id.is_some() {
                return Err(CuratorError::Invalid(
                    "clear_owner conflicts with owner_object_id".into(),
                ));
            }
            if task.clear_due_at && task.due_at.is_some() {
                return Err(CuratorError::Invalid(
                    "clear_due_at conflicts with due_at".into(),
                ));
            }
            task.blocked_reason = optional_text(task.blocked_reason.take(), "blocked_reason", 2000)
                .map_err(invalid)?;
            if task.clear_blocked_reason && task.blocked_reason.is_some() {
                return Err(CuratorError::Invalid(
                    "clear_blocked_reason conflicts with blocked_reason".into(),
                ));
            }
            if task.status.as_deref() == Some("blocked")
                && (task.clear_blocked_reason || task.blocked_reason.is_none())
            {
                return Err(CuratorError::Invalid(
                    "blocked Task updates require blocked_reason".into(),
                ));
            }
            task.github_issue_url =
                optional_text(task.github_issue_url.take(), "github_issue_url", 2000)
                    .map_err(invalid)?;
            validate_github_issue_url(task.github_issue_url.as_deref())?;
            if task.clear_github_issue_url && task.github_issue_url.is_some() {
                return Err(CuratorError::Invalid(
                    "clear_github_issue_url conflicts with github_issue_url".into(),
                ));
            }
            task.brief_markdown =
                optional_text(task.brief_markdown.take(), "brief_markdown", 100_000)
                    .map_err(invalid)?;
            if task.clear_brief_markdown && task.brief_markdown.is_some() {
                return Err(CuratorError::Invalid(
                    "clear_brief_markdown conflicts with brief_markdown".into(),
                ));
            }
        }
        if item.title.is_none() && item.description.is_none() && item.task.is_none() {
            return Err(CuratorError::Invalid("Object update has no changes".into()));
        }
    }
    for item in &mut plan.create_connections {
        for reference in [&mut item.source, &mut item.target] {
            if let ObjectRef::Created { client_id } = reference {
                *client_id =
                    required_text(std::mem::take(client_id), "client_id", 100).map_err(invalid)?;
            }
        }
        item.kind = allowed(
            std::mem::take(&mut item.kind),
            "connection kind",
            CONNECTION_KINDS,
        )
        .map_err(invalid)?;
        item.description = required_text(
            std::mem::take(&mut item.description),
            "connection description",
            1000,
        )
        .map_err(invalid)?;
    }
    let mut updated_connection_ids = HashSet::new();
    for item in &mut plan.update_connections {
        if !updated_connection_ids.insert(item.connection_id) {
            return Err(CuratorError::Invalid(
                "a connection may be updated only once in a reconciliation plan".into(),
            ));
        }
        if item.expected_revision < 1 {
            return Err(CuratorError::Invalid(
                "expected_revision must be positive".into(),
            ));
        }
        item.kind = item
            .kind
            .take()
            .map(|v| allowed(v, "connection kind", CONNECTION_KINDS))
            .transpose()
            .map_err(invalid)?;
        item.description = optional_text(item.description.take(), "connection description", 1000)
            .map_err(invalid)?;
        if item.kind.is_none() && item.description.is_none() {
            return Err(CuratorError::Invalid(
                "Connection update has no changes".into(),
            ));
        }
    }
    Ok(())
}

fn validate_source_fields(source: &mut SourceFields) -> Result<(), CuratorError> {
    source.source_kind = allowed(
        std::mem::take(&mut source.source_kind),
        "source_kind",
        crate::domain::SOURCE_KINDS,
    )
    .map_err(invalid)?;
    source.canonical_uri =
        optional_text(source.canonical_uri.take(), "canonical_uri", 2000).map_err(invalid)?;
    if source
        .canonical_uri
        .as_ref()
        .is_some_and(|uri| !(uri.starts_with("https://") || uri.starts_with("http://")))
    {
        return Err(CuratorError::Invalid(
            "canonical_uri must use HTTP or HTTPS".into(),
        ));
    }
    source.byline = optional_text(source.byline.take(), "byline", 500).map_err(invalid)?;
    source.publisher = optional_text(source.publisher.take(), "publisher", 300).map_err(invalid)?;
    source.published_at_precision = source
        .published_at_precision
        .take()
        .map(|value| {
            allowed(
                value,
                "published_at_precision",
                &["instant", "day", "month", "year"],
            )
        })
        .transpose()
        .map_err(invalid)?;
    if source.published_at.is_some() != source.published_at_precision.is_some() {
        return Err(CuratorError::Invalid(
            "published_at and published_at_precision must be provided together".into(),
        ));
    }
    source.original_language =
        optional_text(source.original_language.take(), "original_language", 35).map_err(invalid)?;
    source.original_media_type = optional_text(
        source.original_media_type.take(),
        "original_media_type",
        255,
    )
    .map_err(invalid)?;
    source.original_artifact_reference = optional_text(
        source.original_artifact_reference.take(),
        "original_artifact_reference",
        1000,
    )
    .map_err(invalid)?;
    if let Some(content) = &mut source.content {
        content.kind = required_text(std::mem::take(&mut content.kind), "artifact kind", 100)
            .map_err(invalid)?;
        content.title =
            optional_text(content.title.take(), "artifact title", 500).map_err(invalid)?;
        content.content = crate::domain::required_preserved_text(
            std::mem::take(&mut content.content),
            "artifact content",
            10_000_000,
        )
        .map_err(invalid)?;
        content.uri = optional_text(content.uri.take(), "artifact uri", 2000).map_err(invalid)?;
        content.media_type = optional_text(content.media_type.take(), "artifact media_type", 255)
            .map_err(invalid)?;
        content.language =
            optional_text(content.language.take(), "language", 35).map_err(invalid)?;
        if !content.metadata.is_object() {
            return Err(CuratorError::Invalid(
                "Artifact metadata must be a JSON object".into(),
            ));
        }
    }
    Ok(())
}

fn invalid(error: impl std::fmt::Display) -> CuratorError {
    CuratorError::Invalid(error.to_string())
}

fn validate_task_fields(task: &mut TaskFields) -> Result<(), CuratorError> {
    if !task.confirmed {
        return Err(CuratorError::Invalid(
            "a Task requires an explicit confirmed instruction or commitment".into(),
        ));
    }
    task.status =
        allowed(std::mem::take(&mut task.status), "status", TASK_STATUSES).map_err(invalid)?;
    task.priority = allowed(
        std::mem::take(&mut task.priority),
        "priority",
        TASK_PRIORITIES,
    )
    .map_err(invalid)?;
    task.blocked_reason =
        optional_text(task.blocked_reason.take(), "blocked_reason", 2000).map_err(invalid)?;
    if (task.status == "blocked") != task.blocked_reason.is_some() {
        return Err(CuratorError::Invalid(
            "blocked_reason is required exactly when status is blocked".into(),
        ));
    }
    task.github_issue_url =
        optional_text(task.github_issue_url.take(), "github_issue_url", 2000).map_err(invalid)?;
    validate_github_issue_url(task.github_issue_url.as_deref())?;
    task.brief_markdown =
        optional_text(task.brief_markdown.take(), "brief_markdown", 100_000).map_err(invalid)?;
    Ok(())
}

fn validate_github_issue_url(value: Option<&str>) -> Result<(), CuratorError> {
    if let Some(url) = value {
        let valid = url
            .strip_prefix("https://github.com/")
            .map(|path| path.split('/').collect::<Vec<_>>())
            .is_some_and(|parts| {
                parts.len() == 4
                    && !parts[0].is_empty()
                    && !parts[1].is_empty()
                    && parts[2] == "issues"
                    && parts[3].parse::<u64>().is_ok_and(|number| number > 0)
            });
        if !valid {
            return Err(CuratorError::Invalid(
                "github_issue_url must be a canonical HTTPS GitHub Issue URL".into(),
            ));
        }
    }
    Ok(())
}

fn validate_task_patch(kind: &str, task: Option<&TaskPatch>) -> Result<(), CuratorError> {
    if kind == "task" {
        let task = task.ok_or_else(|| {
            CuratorError::Invalid("updating a Task requires confirmed task fields".into())
        })?;
        debug_assert!(task.confirmed, "plan validation enforces Task confirmation");
    } else if task.is_some() {
        return Err(CuratorError::Invalid(
            "only Task Objects accept task fields".into(),
        ));
    }
    Ok(())
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

async fn load_message_window(
    tx: &mut Transaction<'_, Postgres>,
    run: &CuratorRun,
) -> Result<HashSet<Uuid>, CuratorError> {
    let ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT m.id FROM chat_messages m
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
    if ids.len() != run.message_count as usize {
        return Err(CuratorError::Invalid(
            "curator run message window no longer matches its recorded count".into(),
        ));
    }
    Ok(ids.into_iter().collect())
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

fn validate_derived_connections(
    plan: &ReconciliationPlan,
    created: &HashMap<String, Uuid>,
    changed: &HashMap<Uuid, HashSet<Uuid>>,
    chat_id: Uuid,
) -> Result<(), CuratorError> {
    let mut linked = HashSet::new();
    for connection in &plan.create_connections {
        if connection.kind != "derived_from" {
            continue;
        }
        let source = resolve_ref(&connection.source, created)?;
        let target = resolve_ref(&connection.target, created)?;
        let connection_messages = connection
            .supporting_message_ids
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        if target == chat_id && changed.get(&source) == Some(&connection_messages) {
            linked.insert(source);
        }
        if source == chat_id && changed.get(&target) == Some(&connection_messages) {
            linked.insert(target);
        }
    }
    if changed.keys().any(|id| !linked.contains(id)) {
        return Err(CuratorError::Invalid("every new or updated Object must have a derived_from connection to the source Chat with the same exact supporting message IDs".into()));
    }
    Ok(())
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
    match item.kind.as_str() {
        "task" => {
            let task = item.task.as_ref().expect("validated");
            if let Some(owner_id) = task.owner_object_id {
                ensure_user(tx, owner_id).await?;
            }
            sqlx::query(
                r#"INSERT INTO tasks
                (object_id,status,priority,owner_object_id,agent_suitable,blocked_reason,
                 due_at,completed_at,github_issue_url,brief_markdown)
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)"#,
            )
            .bind(id)
            .bind(&task.status)
            .bind(&task.priority)
            .bind(task.owner_object_id)
            .bind(task.agent_suitable)
            .bind(&task.blocked_reason)
            .bind(task.due_at)
            .bind((task.status == "done").then(OffsetDateTime::now_utc))
            .bind(&task.github_issue_url)
            .bind(&task.brief_markdown)
            .execute(&mut **tx)
            .await?;
        }
        "entity" => {
            sqlx::query("INSERT INTO entities (object_id,entity_kind) VALUES ($1,$2)")
                .bind(id)
                .bind(item.entity_kind.as_deref().expect("validated"))
                .execute(&mut **tx)
                .await?;
        }
        "memory" => {
            sqlx::query("INSERT INTO memories (object_id,happened_at) VALUES ($1,$2)")
                .bind(id)
                .bind(item.memory.as_ref().expect("validated").happened_at)
                .execute(&mut **tx)
                .await?;
        }
        "source" => {
            let source = item.source.as_ref().expect("validated");
            sqlx::query(
                r#"INSERT INTO sources
                (object_id,source_kind,canonical_uri,byline,publisher,published_at,
                 published_at_precision,last_accessed_at,original_language,
                 original_media_type,original_artifact_reference)
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)"#,
            )
            .bind(id)
            .bind(&source.source_kind)
            .bind(&source.canonical_uri)
            .bind(&source.byline)
            .bind(&source.publisher)
            .bind(source.published_at)
            .bind(&source.published_at_precision)
            .bind(source.last_accessed_at)
            .bind(&source.original_language)
            .bind(&source.original_media_type)
            .bind(&source.original_artifact_reference)
            .execute(&mut **tx)
            .await?;
            if let Some(content) = &source.content {
                let content_id = Uuid::new_v4();
                let hash = format!("{:x}", Sha256::digest(content.content.as_bytes()));
                sqlx::query(
                    r#"INSERT INTO artifacts
                    (id,object_id,kind,title,content,uri,media_type,language,sha256,size_bytes,
                     metadata,captured_at)
                    VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)"#,
                )
                .bind(content_id)
                .bind(id)
                .bind(&content.kind)
                .bind(&content.title)
                .bind(&content.content)
                .bind(&content.uri)
                .bind(&content.media_type)
                .bind(&content.language)
                .bind(hash)
                .bind(content.content.len() as i64)
                .bind(&content.metadata)
                .bind(content.captured_at)
                .execute(&mut **tx)
                .await?;
                sqlx::query("UPDATE sources SET current_artifact_id=$2 WHERE object_id=$1")
                    .bind(id)
                    .bind(content_id)
                    .execute(&mut **tx)
                    .await?;
            }
        }
        _ => unreachable!("validated curator Object kind"),
    }
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

async fn update_object(
    tx: &mut Transaction<'_, Postgres>,
    current: &CurrentObject,
    item: &UpdateObject,
    provenance: &Value,
) -> Result<(), CuratorError> {
    let result = sqlx::query(
        r#"UPDATE objects SET title=COALESCE($3,title),description=COALESCE($4,description),provenance=$5,
                  revision=revision+1,updated_by_type='system',updated_by_id='context-curator',updated_at=now()
           WHERE id=$1 AND revision=$2 AND archived_at IS NULL AND protected=false"#,
    ).bind(item.object_id).bind(item.expected_revision).bind(&item.title).bind(&item.description).bind(provenance).execute(&mut **tx).await?;
    if result.rows_affected() != 1 {
        return Err(CuratorError::Conflict);
    }
    if current.kind == "task" {
        let task = item.task.as_ref().expect("validated");
        let owner = if task.clear_owner {
            None
        } else {
            task.owner_object_id.or(current.owner_object_id)
        };
        if let Some(owner_id) = owner {
            ensure_user(tx, owner_id).await?;
        }
        let due = if task.clear_due_at {
            None
        } else {
            task.due_at.or(current.due_at)
        };
        let status = task
            .status
            .as_deref()
            .or(current.status.as_deref())
            .expect("Task status");
        let blocked_reason = if status == "blocked" {
            if task.clear_blocked_reason {
                return Err(CuratorError::Invalid(
                    "a blocked Task cannot clear blocked_reason".into(),
                ));
            }
            task.blocked_reason
                .as_deref()
                .or(current.blocked_reason.as_deref())
                .ok_or_else(|| {
                    CuratorError::Invalid("a blocked Task requires blocked_reason".into())
                })?
                .to_owned()
                .into()
        } else {
            None
        };
        let completed_at = if status == "done" {
            current
                .completed_at
                .or_else(|| Some(OffsetDateTime::now_utc()))
        } else {
            None
        };
        let github_issue_url = if task.clear_github_issue_url {
            None
        } else {
            task.github_issue_url
                .as_deref()
                .or(current.github_issue_url.as_deref())
                .map(str::to_owned)
        };
        let brief_markdown = if task.clear_brief_markdown {
            None
        } else {
            task.brief_markdown
                .as_deref()
                .or(current.brief_markdown.as_deref())
                .map(str::to_owned)
        };
        sqlx::query(
            r#"UPDATE tasks SET status=COALESCE($2,status),priority=COALESCE($3,priority),owner_object_id=$4,
                  agent_suitable=COALESCE($5,agent_suitable),blocked_reason=$6,due_at=$7,
                  completed_at=$8,github_issue_url=$9,brief_markdown=$10 WHERE object_id=$1"#,
        ).bind(item.object_id).bind(&task.status).bind(&task.priority).bind(owner)
            .bind(task.agent_suitable).bind(&blocked_reason).bind(due).bind(completed_at)
            .bind(&github_issue_url).bind(&brief_markdown).execute(&mut **tx).await?;
    }
    Ok(())
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

async fn ensure_user(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<(), CuratorError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM objects o JOIN users u ON u.object_id=o.id WHERE o.id=$1 AND o.archived_at IS NULL)",
    )
    .bind(id)
    .fetch_one(&mut **tx)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(CuratorError::Invalid(
            "Task owner_object_id must name an active canonical User".into(),
        ))
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
        let run = match claim_run(&pool, &worker_id, &config).await {
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
) -> Result<Option<CuratorRun>, CuratorError> {
    let mut tx = pool.begin().await?;
    let run: Option<CuratorRun> = sqlx::query_as(
        r#"WITH candidate AS (
               SELECT id FROM runs
               WHERE kind='curator' AND COALESCE((result->>'attempts')::integer,0) < 3
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

async fn request_plan(
    pool: &PgPool,
    client: &reqwest::Client,
    config: &CuratorModelConfig,
    run: &CuratorRun,
    messages: &[WorkerMessage],
    candidates: &crate::search::SearchPacket,
    validation_feedback: Option<&str>,
) -> Result<ReconciliationPlan, CuratorError> {
    let attempt_id = Uuid::new_v4().to_string();
    let system = r#"You are the Centaur Context Context Curator. Return only one JSON object with exactly these four arrays:
{"create_objects":[],"update_objects":[],"create_connections":[],"update_connections":[]}.

Every create_objects entry MUST contain all of these fields:
{"client_id":"unique-local-name","kind":"memory|task|entity|source","title":"...","description":"...","supporting_message_ids":["UUID"],"entity_kind":null,"task":null,"memory":null,"source":null}.
client_id is a short unique name used only to reference that new Object from create_connections. For an Entity, set entity_kind to person|organization|product|project|publication|place|concept|other. For a Memory, replace memory with {"primary_event":true|false,"happened_at":"RFC3339"}. For a Task, replace task with {"confirmed":true,"status":"backlog|todo|doing|review|done|blocked","priority":"low|medium|high","owner_object_id":null,"agent_suitable":false,"blocked_reason":null,"due_at":null,"github_issue_url":null,"brief_markdown":null}; blocked_reason is required exactly for blocked Tasks.
For a Source supported explicitly by the messages, replace source with {"source_kind":"article|paper|podcast_episode|video|book|report|document|dataset|web_page|social_post|other","canonical_uri":null,"byline":null,"publisher":null,"published_at":null,"published_at_precision":null,"last_accessed_at":null,"original_language":null,"original_media_type":null,"original_artifact_reference":null,"content":null}. Optional content is a generic Artifact: {"kind":"transcript","title":null,"content":"...","uri":null,"media_type":"text/plain","language":null,"captured_at":null,"metadata":{}}. Use only content explicitly present in the evidence; never fetch or invent Artifact content.

Every update_objects entry MUST contain all of these fields:
{"object_id":"UUID","expected_revision":1,"title":null,"description":null,"supporting_message_ids":["UUID"],"task":null}.
Every create_connections entry MUST contain all of these fields:
{"source":{"client_id":"created-object-client-id"},"kind":"derived_from","target":{"object_id":"existing-object-UUID"},"description":"...","supporting_message_ids":["UUID"]}.
An existing Object reference is {"object_id":"UUID"}; a newly created Object reference is {"client_id":"unique-local-name"}. Every update_connections entry MUST contain all of these fields:
{"connection_id":"UUID","expected_revision":1,"kind":null,"description":null,"supporting_message_ids":["UUID"]}.

Create zero or more Memories: only create a Memory for a concrete event or insight worth retaining, and use primary_event=true for at most one central event. Sources and Memories are distinct: a Source represents evidence, while a Memory records an event or insight. If a message explicitly asks a bot, agent, or workflow to ingest, import, or capture a URL, file, or source, do not create or update that Source; the dedicated ingestion workflow owns Source creation. Tasks require task.confirmed=true and may be created or updated only for an explicit instruction or commitment. Never create or update a Chat, User, or Theme. Every operation cites supporting_message_ids from this run. Every created or updated Object must be connected to the source Chat in create_connections with kind=derived_from and a simple, exact description. Allowed connection kinds: involves, about, related_to, depends_on, derived_from, themed. A themed Connection must point from a non-Theme Object to an existing approved Theme candidate and explain why the Object belongs in that research vertical; it never creates vocabulary. Use existing candidate object IDs and revisions when the same thing already exists. An Object description must explicitly identify the subject, what it is or was about, and its evidenced context in 50–150 direct words. Never repeat only the title, use placeholders or vague meta text, copy transcript fragments, or mention the model or generation process. Do not use connection counts for reconciliation."#;
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
    let request_body = match config.transport {
        CuratorModelTransport::CentaurSubscription => json!({
            "request_id": idempotency_id,
            "system_prompt": system,
            "input": input,
            "output_schema": reconciliation_plan_schema(),
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
                run,
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
            run,
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
                run,
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
                        run,
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
                    run,
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
                run,
                &config.model,
                &attribution,
                response.usage.as_ref(),
                response
                    .usage
                    .is_none()
                    .then_some("Codex response omitted usage"),
            )
            .await;
            let plan = serde_json::from_value(response.output).map_err(|error| {
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
                        run,
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
                run,
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
            let plan = serde_json::from_str(&content).map_err(|error| {
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
        "required": ["create_objects", "update_objects", "create_connections", "update_connections"],
        "properties": {
            "create_objects": {"type": "array", "items": {"$ref": "#/$defs/create_object"}},
            "update_objects": {"type": "array", "items": {"$ref": "#/$defs/update_object"}},
            "create_connections": {"type": "array", "items": {"$ref": "#/$defs/create_connection"}},
            "update_connections": {"type": "array", "items": {"$ref": "#/$defs/update_connection"}}
        },
        "$defs": {
            "nullable_string": {"anyOf": [{"type": "string"}, {"type": "null"}]},
            "uuid": {"type": "string"},
            "nullable_uuid": {"anyOf": [{"$ref": "#/$defs/uuid"}, {"type": "null"}]},
            "message_ids": {"type": "array", "items": {"$ref": "#/$defs/uuid"}},
            "task_fields": {
                "type": "object", "additionalProperties": false,
                "required": ["confirmed", "status", "priority", "owner_object_id", "agent_suitable", "blocked_reason", "due_at", "github_issue_url", "brief_markdown"],
                "properties": {
                    "confirmed": {"type": "boolean"}, "status": {"type": "string"},
                    "priority": {"type": "string"}, "owner_object_id": {"$ref": "#/$defs/nullable_uuid"},
                    "agent_suitable": {"type": "boolean"}, "blocked_reason": {"$ref": "#/$defs/nullable_string"},
                    "due_at": {"$ref": "#/$defs/nullable_string"}, "github_issue_url": {"$ref": "#/$defs/nullable_string"},
                    "brief_markdown": {"$ref": "#/$defs/nullable_string"}
                }
            },
            "task_patch": {
                "type": "object", "additionalProperties": false,
                "required": ["confirmed", "status", "priority", "owner_object_id", "clear_owner", "agent_suitable", "blocked_reason", "clear_blocked_reason", "due_at", "clear_due_at", "github_issue_url", "clear_github_issue_url", "brief_markdown", "clear_brief_markdown"],
                "properties": {
                    "confirmed": {"type": "boolean"}, "status": {"$ref": "#/$defs/nullable_string"},
                    "priority": {"$ref": "#/$defs/nullable_string"}, "owner_object_id": {"$ref": "#/$defs/nullable_uuid"},
                    "clear_owner": {"type": "boolean"}, "agent_suitable": {"anyOf": [{"type": "boolean"}, {"type": "null"}]},
                    "blocked_reason": {"$ref": "#/$defs/nullable_string"}, "clear_blocked_reason": {"type": "boolean"},
                    "due_at": {"$ref": "#/$defs/nullable_string"}, "clear_due_at": {"type": "boolean"},
                    "github_issue_url": {"$ref": "#/$defs/nullable_string"}, "clear_github_issue_url": {"type": "boolean"},
                    "brief_markdown": {"$ref": "#/$defs/nullable_string"}, "clear_brief_markdown": {"type": "boolean"}
                }
            },
            "memory_fields": {
                "type": "object", "additionalProperties": false,
                "required": ["primary_event", "happened_at"],
                "properties": {"primary_event": {"type": "boolean"}, "happened_at": {"type": "string"}}
            },
            "artifact": {
                "type": "object", "additionalProperties": false,
                "required": ["kind", "title", "content", "uri", "media_type", "language", "captured_at", "metadata"],
                "properties": {
                    "kind": {"type": "string"}, "title": {"$ref": "#/$defs/nullable_string"},
                    "content": {"type": "string"}, "uri": {"$ref": "#/$defs/nullable_string"},
                    "media_type": {"$ref": "#/$defs/nullable_string"},
                    "language": {"$ref": "#/$defs/nullable_string"},
                    "captured_at": {"$ref": "#/$defs/nullable_string"},
                    "metadata": {"type": "object", "additionalProperties": true}
                }
            },
            "source_fields": {
                "type": "object", "additionalProperties": false,
                "required": ["source_kind", "canonical_uri", "byline", "publisher", "published_at", "published_at_precision", "last_accessed_at", "original_language", "original_media_type", "original_artifact_reference", "content"],
                "properties": {
                    "source_kind": {"type": "string"}, "canonical_uri": {"$ref": "#/$defs/nullable_string"},
                    "byline": {"$ref": "#/$defs/nullable_string"}, "publisher": {"$ref": "#/$defs/nullable_string"},
                    "published_at": {"$ref": "#/$defs/nullable_string"}, "published_at_precision": {"$ref": "#/$defs/nullable_string"},
                    "last_accessed_at": {"$ref": "#/$defs/nullable_string"},
                    "original_language": {"$ref": "#/$defs/nullable_string"}, "original_media_type": {"$ref": "#/$defs/nullable_string"},
                    "original_artifact_reference": {"$ref": "#/$defs/nullable_string"},
                    "content": {"anyOf": [{"$ref": "#/$defs/artifact"}, {"type": "null"}]}
                }
            },
            "object_ref": {
                "anyOf": [
                    {"type": "object", "additionalProperties": false, "required": ["object_id"], "properties": {"object_id": {"$ref": "#/$defs/uuid"}}},
                    {"type": "object", "additionalProperties": false, "required": ["client_id"], "properties": {"client_id": {"type": "string"}}}
                ]
            },
            "create_object": {
                "type": "object", "additionalProperties": false,
                "required": ["client_id", "kind", "title", "description", "supporting_message_ids", "entity_kind", "task", "memory", "source"],
                "properties": {
                    "client_id": {"type": "string"}, "kind": {"type": "string"}, "title": {"type": "string"},
                    "description": {"type": "string"}, "supporting_message_ids": {"$ref": "#/$defs/message_ids"},
                    "entity_kind": {"$ref": "#/$defs/nullable_string"},
                    "task": {"anyOf": [{"$ref": "#/$defs/task_fields"}, {"type": "null"}]},
                    "memory": {"anyOf": [{"$ref": "#/$defs/memory_fields"}, {"type": "null"}]},
                    "source": {"anyOf": [{"$ref": "#/$defs/source_fields"}, {"type": "null"}]}
                }
            },
            "update_object": {
                "type": "object", "additionalProperties": false,
                "required": ["object_id", "expected_revision", "title", "description", "supporting_message_ids", "task"],
                "properties": {
                    "object_id": {"$ref": "#/$defs/uuid"}, "expected_revision": {"type": "integer"},
                    "title": {"$ref": "#/$defs/nullable_string"}, "description": {"$ref": "#/$defs/nullable_string"},
                    "supporting_message_ids": {"$ref": "#/$defs/message_ids"},
                    "task": {"anyOf": [{"$ref": "#/$defs/task_patch"}, {"type": "null"}]}
                }
            },
            "create_connection": {
                "type": "object", "additionalProperties": false,
                "required": ["source", "kind", "target", "description", "supporting_message_ids"],
                "properties": {
                    "source": {"$ref": "#/$defs/object_ref"}, "kind": {"type": "string"},
                    "target": {"$ref": "#/$defs/object_ref"}, "description": {"type": "string"},
                    "supporting_message_ids": {"$ref": "#/$defs/message_ids"}
                }
            },
            "update_connection": {
                "type": "object", "additionalProperties": false,
                "required": ["connection_id", "expected_revision", "kind", "description", "supporting_message_ids"],
                "properties": {
                    "connection_id": {"$ref": "#/$defs/uuid"}, "expected_revision": {"type": "integer"},
                    "kind": {"$ref": "#/$defs/nullable_string"}, "description": {"$ref": "#/$defs/nullable_string"},
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
    run: &CuratorRun,
    model_id: &str,
    attribution: &UsageAttribution<'_>,
    usage: Option<&ModelUsage>,
    missing_reason: Option<&str>,
) {
    let input = crate::runs::NormalizedUsage {
        run_id: run.id,
        component: "context_curator".into(),
        provider: attribution.provider.into(),
        model_id: model_id.to_owned(),
        display_tier: Some(model_id.to_owned()),
        execution_type: attribution.execution_type.into(),
        auth_mode: attribution.auth_mode.into(),
        upstream_service: attribution.upstream_service.into(),
        billing_mode: attribution.billing_mode.into(),
        reasoning_effort: attribution.reasoning_effort.map(str::to_owned),
        service_tier: None,
        source_thread_id: Some(run.chat_object_id.to_string()),
        source_execution_id: attribution.source_execution_id.to_owned(),
        source_turn_id: Some(run.id.to_string()),
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
        tracing::error!(run_id=%run.id,%error,"failed to record Curator usage");
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

    #[test]
    fn plan_allows_work_without_a_memory() {
        let mut plan = ReconciliationPlan {
            create_objects: vec![CreateObject {
                client_id: "task".into(),
                kind: "task".into(),
                title: "A confirmed task".into(),
                description:
                    "A concrete confirmed task that does not require a fabricated event Memory."
                        .into(),
                supporting_message_ids: vec![Uuid::new_v4()],
                entity_kind: None,
                task: Some(TaskFields {
                    confirmed: true,
                    status: "todo".into(),
                    priority: "medium".into(),
                    owner_object_id: None,
                    agent_suitable: false,
                    blocked_reason: None,
                    due_at: None,
                    github_issue_url: None,
                    brief_markdown: None,
                }),
                memory: None,
                source: None,
            }],
            update_objects: vec![],
            create_connections: vec![],
            update_connections: vec![],
        };
        assert!(validate_plan(&mut plan).is_ok());
    }

    #[test]
    fn unconfirmed_task_is_rejected() {
        let mut task = TaskFields {
            confirmed: false,
            status: "todo".into(),
            priority: "medium".into(),
            owner_object_id: None,
            agent_suitable: false,
            blocked_reason: None,
            due_at: None,
            github_issue_url: None,
            brief_markdown: None,
        };
        assert!(validate_task_fields(&mut task).is_err());
    }

    #[test]
    fn subscription_schema_is_strict_at_every_object_boundary() {
        let schema = reconciliation_plan_schema();
        assert_eq!(schema["additionalProperties"], json!(false));
        for definition in [
            "task_fields",
            "task_patch",
            "memory_fields",
            "artifact",
            "source_fields",
            "create_object",
            "update_object",
            "create_connection",
            "update_connection",
        ] {
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
}

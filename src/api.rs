use std::{path::PathBuf, sync::Arc};

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use subtle::ConstantTimeEq;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use uuid::Uuid;

use crate::{
    db::{
        self, ConnectionChanges, DbError, NewConnection, NewObject, NewTask, ObjectChanges,
        TaskChanges,
    },
    domain::{
        ActorContext, CONNECTION_KINDS, OBJECT_KINDS, TASK_PRIORITIES, TASK_STATUSES,
        ValidationError, allowed, optional_text, provenance, required_text,
    },
    embeddings::EmbeddingClient,
    search,
};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub embeddings: Option<EmbeddingClient>,
    pub text_search_config: crate::config::TextSearchConfig,
}

#[derive(Clone)]
struct AgentAuth {
    token: Arc<String>,
}

pub fn human_router(state: AppState, static_dir: PathBuf) -> Router {
    let index = static_dir.join("index.html");
    Router::new()
        .merge(service_router(state))
        .fallback_service(ServeDir::new(static_dir).not_found_service(ServeFile::new(index)))
        .layer(Extension(ActorContext::human()))
        .layer(TraceLayer::new_for_http())
}

pub fn agent_router(state: AppState, token: String) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .nest(
            "/api/v1",
            Router::new()
                .route("/context", get(get_context))
                .route("/search/objects", get(search_objects))
                .route("/objects/{id}", get(read_context_object)),
        )
        .with_state(state)
        .layer(middleware::from_fn_with_state(
            AgentAuth {
                token: Arc::new(token),
            },
            agent_auth,
        ))
        .layer(TraceLayer::new_for_http())
}

fn service_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .nest(
            "/api/v1",
            Router::new()
                .route("/meta", get(api_meta))
                .route("/objects", get(list_objects).post(create_object))
                .route("/object-visuals", get(list_object_visuals))
                .route("/objects/{id}", get(read_object).patch(update_object))
                .route("/context", get(get_context))
                .route("/search/objects", get(search_objects))
                .route("/objects/{id}/connections", get(list_connections))
                .route("/objects/{id}/events", get(list_events))
                .route("/connections", post(create_connection))
                .route(
                    "/connections/{id}",
                    get(read_connection).patch(update_connection),
                )
                .route("/connections/{id}/archive", post(archive_connection))
                .route("/tasks", get(list_tasks).post(create_task))
                .route("/tasks/{id}", get(read_task).patch(update_task))
                .route("/chats/{id}/messages", get(list_chat_messages))
                .route("/users", get(list_users))
                .route("/users/{id}", get(read_user))
                .route("/users/{id}/identities", get(list_user_identities))
                .route("/curator-runs", get(list_curator_runs))
                .route("/curator-runs/{id}", get(read_curator_run))
                .route("/curator-runs/{id}/undo", post(undo_curator_run))
                .route("/evals", get(list_evals))
                .route("/evals/{id}", get(read_eval))
                .route(
                    "/evals/{id}/annotation",
                    axum::routing::patch(annotate_eval),
                ),
        )
        .with_state(state)
}

async fn agent_auth(
    State(auth): State<AgentAuth>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let bearer = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(ApiError::Unauthorized)?;
    let expected = auth.token.as_bytes();
    let supplied = bearer.as_bytes();
    if expected.len() != supplied.len() || expected.ct_eq(supplied).unwrap_u8() != 1 {
        return Err(ApiError::Unauthorized);
    }
    let principal = required_header(request.headers(), "x-centaur-principal-id")?;
    let thread_key = required_header(request.headers(), "x-centaur-thread-key")?;
    let execution_id = optional_header(request.headers(), "x-centaur-execution-id")?;
    request.extensions_mut().insert(ActorContext {
        actor_type: "centaur_agent",
        actor_id: principal,
        centaur_thread_key: Some(thread_key),
        centaur_execution_id: execution_id,
        is_agent: true,
    });
    Ok(next.run(request).await)
}

fn required_header(headers: &HeaderMap, name: &'static str) -> Result<String, ApiError> {
    optional_header(headers, name)?
        .ok_or_else(|| ApiError::BadRequest(format!("{name} is required")))
}

fn optional_header(headers: &HeaderMap, name: &'static str) -> Result<Option<String>, ApiError> {
    headers
        .get(name)
        .map(|value| {
            value
                .to_str()
                .map(str::trim)
                .map(str::to_owned)
                .map_err(|_| ApiError::BadRequest(format!("{name} is invalid")))
                .and_then(|value| {
                    if value.is_empty() {
                        Err(ApiError::BadRequest(format!("{name} must not be empty")))
                    } else {
                        Ok(value)
                    }
                })
        })
        .transpose()
}

async fn health() -> Json<Value> {
    Json(json!({"ok": true}))
}

async fn api_meta() -> Json<Value> {
    Json(json!({
        "data": {
            "product": "centaur-context",
            "product_version": crate::version::PRODUCT_VERSION,
            "api_version": crate::version::API_VERSION,
            "ontology_version": crate::version::ONTOLOGY_VERSION,
            "database_schema_version": crate::version::DATABASE_SCHEMA_VERSION,
            "tool_version": crate::version::TOOL_VERSION,
            "compatibility_policy": "fail_closed",
            "compatibility": "Only documented /api/v1 routes are supported; unknown versions fail closed."
        }
    }))
}

async fn ready(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    db::ready(&state.pool).await?;
    Ok(Json(json!({"ok": true, "ready": true})))
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
    kind: Option<String>,
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ContextQuery {
    q: String,
    chat_object_id: Option<Uuid>,
    kind: Option<String>,
    limit: Option<i64>,
}

async fn get_context(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    Query(query): Query<ContextQuery>,
) -> Result<Json<Value>, ApiError> {
    let query_text = required_text(query.q, "q", 1000)?;
    let chat_object_id = query
        .chat_object_id
        .ok_or_else(|| ApiError::BadRequest("chat_object_id is required".to_owned()))?;
    let chat = db::get_context_chat(&state.pool, chat_object_id).await?;
    if chat.lifecycle != "active" {
        return Err(ApiError::BadRequest(
            "chat_object_id must reference an active Chat".to_owned(),
        ));
    }
    let expected_thread_key = chat
        .thread_key()
        .and_then(|value| normalize_thread_key(&value))
        .ok_or_else(|| {
            ApiError::BadRequest(
                "chat_object_id must reference a Chat with a provider thread identity".to_owned(),
            )
        })?;
    let supplied_thread_key = actor
        .centaur_thread_key
        .as_deref()
        .and_then(normalize_thread_key)
        .ok_or_else(|| ApiError::BadRequest("X-Centaur-Thread-Key is invalid".to_owned()))?;
    if supplied_thread_key != expected_thread_key {
        return Err(ApiError::Forbidden(
            "The requested Chat does not match the authenticated thread.".to_owned(),
        ));
    }
    let kind = query
        .kind
        .map(|value| allowed(value, "kind", OBJECT_KINDS))
        .transpose()?;
    let limit = query.limit.unwrap_or(10).clamp(1, 10);
    let packet = search::context(
        &state.pool,
        state.embeddings.as_ref(),
        state.text_search_config,
        &query_text,
        kind.as_deref(),
        chat_object_id,
        limit,
    )
    .await?;
    Ok(Json(json!({"data": packet})))
}

async fn search_objects(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Value>, ApiError> {
    let query_text = required_text(query.q, "q", 1000)?;
    let kind = query
        .kind
        .map(|value| allowed(value, "kind", OBJECT_KINDS))
        .transpose()?;
    let packet = search::search(
        &state.pool,
        state.embeddings.as_ref(),
        state.text_search_config,
        &query_text,
        kind.as_deref(),
        bounded_limit(query.limit),
    )
    .await?;
    Ok(Json(json!({"data": packet})))
}

fn normalize_thread_key(value: &str) -> Option<String> {
    if value.len() > 1_000 {
        return None;
    }
    let parts = value.split(':').map(str::trim).collect::<Vec<_>>();
    if parts.len() != 4 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    Some(format!(
        "{}:{}:{}:{}",
        parts[0].to_ascii_lowercase(),
        parts[1],
        parts[2],
        parts[3]
    ))
}

async fn read_context_object(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data": search::read_object(&state.pool, id).await?}),
    ))
}

#[derive(Debug, Deserialize)]
struct ObjectListQuery {
    q: Option<String>,
    kind: Option<String>,
    lifecycle: Option<String>,
    limit: Option<i64>,
}

async fn list_objects(
    State(state): State<AppState>,
    Query(query): Query<ObjectListQuery>,
) -> Result<Json<Value>, ApiError> {
    let kind = query
        .kind
        .map(|value| allowed(value, "kind", OBJECT_KINDS))
        .transpose()?;
    let lifecycle = query
        .lifecycle
        .map(|value| allowed(value, "lifecycle", &["active", "archived"]))
        .transpose()?;
    let data = db::list_objects(
        &state.pool,
        db::ObjectListFilter {
            query: optional_text(query.q, "q", 300)?,
            kind,
            lifecycle,
            limit: bounded_limit(query.limit),
            text_search_config: state.text_search_config,
        },
    )
    .await?;
    Ok(Json(json!({"data": data})))
}

#[derive(Debug, Deserialize)]
struct CreateObjectRequest {
    kind: String,
    title: String,
    description: String,
    provenance: Option<Value>,
}

async fn create_object(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    headers: HeaderMap,
    Json(input): Json<CreateObjectRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let key = idempotency_key(&headers, true, &actor)?.expect("required idempotency key");
    let kind = allowed(input.kind, "kind", OBJECT_KINDS)?;
    if matches!(kind.as_str(), "task" | "user") {
        return Err(ApiError::BadRequest(format!(
            "use the typed endpoint to create a {kind}"
        )));
    }
    let title = required_text(input.title, "title", 300)?;
    let description = crate::domain::object_description(&title, input.description)?;
    let object = db::create_object(
        &state.pool,
        &actor,
        NewObject {
            kind,
            title,
            description,
            provenance: provenance(input.provenance)?,
        },
        &key,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(json!({"data": object}))))
}

async fn read_object(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data": db::get_object(&state.pool, id).await?}),
    ))
}

#[derive(Debug, Deserialize)]
struct UpdateObjectRequest {
    expected_revision: i64,
    title: Option<String>,
    description: Option<String>,
    provenance: Option<Value>,
    protected: Option<bool>,
    #[serde(default)]
    archive: bool,
}

async fn update_object(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateObjectRequest>,
) -> Result<Json<Value>, ApiError> {
    let key = idempotency_key(&headers, actor.is_agent, &actor)?;
    let object = db::update_object(
        &state.pool,
        &actor,
        id,
        input.expected_revision,
        ObjectChanges {
            title: input
                .title
                .map(|value| required_text(value, "title", 300))
                .transpose()?,
            description: input
                .description
                .map(|value| required_text(value, "description", 1000))
                .transpose()?,
            provenance: input
                .provenance
                .map(|value| provenance(Some(value)))
                .transpose()?,
            protected: input.protected,
            archive: input.archive,
        },
        key.as_deref(),
    )
    .await?;
    Ok(Json(json!({"data": object})))
}

async fn list_connections(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    db::get_object(&state.pool, id).await?;
    Ok(Json(
        json!({"data": db::list_connections(&state.pool, id).await?}),
    ))
}

async fn read_connection(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data": db::get_connection(&state.pool, id).await?}),
    ))
}

#[derive(Debug, Deserialize)]
struct CreateConnectionRequest {
    source_object_id: Uuid,
    kind: String,
    target_object_id: Uuid,
    description: String,
    provenance: Option<Value>,
    #[serde(default)]
    protected: bool,
}

async fn create_connection(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    headers: HeaderMap,
    Json(input): Json<CreateConnectionRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if input.source_object_id == input.target_object_id {
        return Err(ValidationError::SelfConnection.into());
    }
    let key = idempotency_key(&headers, true, &actor)?.expect("required idempotency key");
    let connection = db::create_connection(
        &state.pool,
        &actor,
        NewConnection {
            source_object_id: input.source_object_id,
            kind: allowed(input.kind, "connection kind", CONNECTION_KINDS)?,
            target_object_id: input.target_object_id,
            description: required_text(input.description, "description", 1000)?,
            provenance: provenance(input.provenance)?,
            protected: input.protected,
        },
        &key,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(json!({"data": connection}))))
}

#[derive(Debug, Deserialize)]
struct UpdateConnectionRequest {
    expected_revision: i64,
    kind: Option<String>,
    description: Option<String>,
    provenance: Option<Value>,
    protected: Option<bool>,
}

async fn update_connection(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateConnectionRequest>,
) -> Result<Json<Value>, ApiError> {
    let key = idempotency_key(&headers, actor.is_agent, &actor)?;
    let connection = db::update_connection(
        &state.pool,
        &actor,
        id,
        input.expected_revision,
        ConnectionChanges {
            kind: input
                .kind
                .map(|value| allowed(value, "connection kind", CONNECTION_KINDS))
                .transpose()?,
            description: input
                .description
                .map(|value| required_text(value, "description", 1000))
                .transpose()?,
            provenance: input
                .provenance
                .map(|value| provenance(Some(value)))
                .transpose()?,
            protected: input.protected,
        },
        key.as_deref(),
    )
    .await?;
    Ok(Json(json!({"data": connection})))
}

#[derive(Debug, Deserialize)]
struct RevisionRequest {
    expected_revision: i64,
}

async fn archive_connection(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<RevisionRequest>,
) -> Result<Json<Value>, ApiError> {
    let key = idempotency_key(&headers, actor.is_agent, &actor)?;
    let connection = db::archive_connection(
        &state.pool,
        &actor,
        id,
        input.expected_revision,
        key.as_deref(),
    )
    .await?;
    Ok(Json(json!({"data": connection})))
}

#[derive(Debug, Deserialize)]
struct TaskListQuery {
    status: Option<String>,
    agent_eligible: Option<bool>,
    limit: Option<i64>,
}

async fn list_tasks(
    State(state): State<AppState>,
    Query(query): Query<TaskListQuery>,
) -> Result<Json<Value>, ApiError> {
    let status = query
        .status
        .map(|value| allowed(value, "status", TASK_STATUSES))
        .transpose()?;
    let data = db::list_tasks(
        &state.pool,
        db::TaskListFilter {
            status,
            agent_eligible: query.agent_eligible,
            limit: bounded_limit(query.limit),
        },
    )
    .await?;
    Ok(Json(json!({"data": data})))
}

#[derive(Debug, Deserialize)]
struct CreateTaskRequest {
    title: String,
    description: String,
    provenance: Option<Value>,
    #[serde(default = "default_task_status")]
    status: String,
    #[serde(default = "default_task_priority")]
    priority: String,
    owner_object_id: Option<Uuid>,
    #[serde(default)]
    agent_eligible: bool,
    due_at: Option<String>,
}

fn default_task_status() -> String {
    "todo".to_owned()
}

fn default_task_priority() -> String {
    "medium".to_owned()
}

async fn create_task(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    headers: HeaderMap,
    Json(input): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let key = idempotency_key(&headers, true, &actor)?.expect("required idempotency key");
    let title = required_text(input.title, "title", 300)?;
    let description = crate::domain::object_description(&title, input.description)?;
    let task = db::create_task(
        &state.pool,
        &actor,
        NewTask {
            title,
            description,
            provenance: provenance(input.provenance)?,
            status: allowed(input.status, "status", TASK_STATUSES)?,
            priority: allowed(input.priority, "priority", TASK_PRIORITIES)?,
            owner_object_id: input.owner_object_id,
            agent_eligible: input.agent_eligible,
            due_at: parse_due_at(input.due_at)?,
        },
        &key,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(json!({"data": task}))))
}

async fn read_task(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({"data": db::get_task(&state.pool, id).await?})))
}

#[derive(Debug, Deserialize)]
struct UpdateTaskRequest {
    expected_revision: i64,
    title: Option<String>,
    description: Option<String>,
    provenance: Option<Value>,
    protected: Option<bool>,
    status: Option<String>,
    priority: Option<String>,
    owner_object_id: Option<Uuid>,
    #[serde(default)]
    clear_owner: bool,
    agent_eligible: Option<bool>,
    due_at: Option<String>,
    #[serde(default)]
    clear_due_at: bool,
}

async fn update_task(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateTaskRequest>,
) -> Result<Json<Value>, ApiError> {
    if input.clear_owner && input.owner_object_id.is_some() {
        return Err(ApiError::BadRequest(
            "clear_owner cannot be combined with owner fields".to_owned(),
        ));
    }
    if input.clear_due_at && input.due_at.is_some() {
        return Err(ApiError::BadRequest(
            "clear_due_at cannot be combined with due_at".to_owned(),
        ));
    }
    let owner_object_id = if input.clear_owner {
        Some(None)
    } else {
        input.owner_object_id.map(Some)
    };
    let due_at = if input.clear_due_at {
        Some(None)
    } else {
        input
            .due_at
            .map(|value| parse_due_at(Some(value)))
            .transpose()?
            .flatten()
            .map(Some)
    };
    let key = idempotency_key(&headers, actor.is_agent, &actor)?;
    let task = db::update_task(
        &state.pool,
        &actor,
        id,
        input.expected_revision,
        TaskChanges {
            title: input
                .title
                .map(|value| required_text(value, "title", 300))
                .transpose()?,
            description: input
                .description
                .map(|value| required_text(value, "description", 1000))
                .transpose()?,
            provenance: input
                .provenance
                .map(|value| provenance(Some(value)))
                .transpose()?,
            protected: input.protected,
            status: input
                .status
                .map(|value| allowed(value, "status", TASK_STATUSES))
                .transpose()?,
            priority: input
                .priority
                .map(|value| allowed(value, "priority", TASK_PRIORITIES))
                .transpose()?,
            owner_object_id,
            agent_eligible: input.agent_eligible,
            due_at,
        },
        key.as_deref(),
    )
    .await?;
    Ok(Json(json!({"data": task})))
}

async fn list_events(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    db::get_object(&state.pool, id).await?;
    Ok(Json(
        json!({"data": db::list_events(&state.pool, id).await?}),
    ))
}

async fn list_chat_messages(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data": db::list_chat_messages(&state.pool, id).await?}),
    ))
}

async fn list_users(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data": db::list_users(&state.pool, 100).await?}),
    ))
}

async fn list_object_visuals(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data": db::list_object_visuals(&state.pool).await?}),
    ))
}

async fn read_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({"data": db::get_user(&state.pool, id).await?})))
}

async fn list_user_identities(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data": db::list_external_identities(&state.pool, id).await?}),
    ))
}

#[derive(Debug, Deserialize)]
struct CuratorRunListQuery {
    limit: Option<i64>,
}

async fn list_curator_runs(
    State(state): State<AppState>,
    Query(query): Query<CuratorRunListQuery>,
) -> Result<Json<Value>, ApiError> {
    let runs = crate::curator::list_runs(&state.pool, bounded_limit(query.limit))
        .await
        .map_err(map_curator_error)?;
    Ok(Json(json!({"data": runs})))
}

async fn read_curator_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let detail = crate::curator::run_detail(&state.pool, id)
        .await
        .map_err(map_curator_error)?;
    Ok(Json(json!({"data": detail})))
}

async fn undo_curator_run(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let result = crate::curator::undo_as(&state.pool, id, &actor)
        .await
        .map_err(map_curator_error)?;
    Ok(Json(json!({"data": result})))
}

#[derive(Debug, Deserialize)]
struct EvalListQuery {
    kind: Option<String>,
    status: Option<String>,
    verdict: Option<String>,
    component: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    execution_type: Option<String>,
    auth_mode: Option<String>,
    billing_mode: Option<String>,
    object_id: Option<Uuid>,
    from: Option<String>,
    to: Option<String>,
    before: Option<String>,
    limit: Option<i64>,
}

async fn list_evals(
    State(state): State<AppState>,
    Query(query): Query<EvalListQuery>,
) -> Result<Json<Value>, ApiError> {
    let data = crate::evals::list(
        &state.pool,
        crate::evals::EvalFilter {
            kind: query
                .kind
                .map(|value| {
                    allowed(
                        value,
                        "kind",
                        &[
                            "slack_interaction",
                            "human_mutation",
                            "system_mutation",
                            "legacy_import",
                        ],
                    )
                })
                .transpose()?,
            status: query
                .status
                .map(|value| {
                    allowed(
                        value,
                        "status",
                        &["open", "running", "completed", "failed", "reversed"],
                    )
                })
                .transpose()?,
            verdict: query
                .verdict
                .map(|value| allowed(value, "verdict", crate::evals::VERDICTS))
                .transpose()?,
            component: optional_text(query.component, "component", 100)?,
            provider: optional_text(query.provider, "provider", 100)?,
            model: optional_text(query.model, "model", 200)?,
            execution_type: query
                .execution_type
                .map(|value| {
                    allowed(
                        value,
                        "execution_type",
                        &["codex_harness", "direct_api", "embedding", "other"],
                    )
                })
                .transpose()?,
            auth_mode: query
                .auth_mode
                .map(|value| {
                    allowed(
                        value,
                        "auth_mode",
                        &[
                            "chatgpt_subscription",
                            "api_key",
                            "not_applicable",
                            "unknown",
                        ],
                    )
                })
                .transpose()?,
            billing_mode: query
                .billing_mode
                .map(|value| {
                    allowed(
                        value,
                        "billing_mode",
                        &[
                            "subscription_allowance",
                            "chatgpt_credits",
                            "metered_api",
                            "not_applicable",
                            "unknown",
                        ],
                    )
                })
                .transpose()?,
            object_id: query.object_id,
            from: query
                .from
                .map(|value| parse_timestamp(value, "from"))
                .transpose()?,
            to: query
                .to
                .map(|value| parse_timestamp(value, "to"))
                .transpose()?,
            before: query
                .before
                .map(|value| parse_timestamp(value, "before"))
                .transpose()?,
            limit: query.limit.unwrap_or(50).clamp(1, 100),
        },
    )
    .await?;
    Ok(Json(json!({"data": data})))
}

async fn read_eval(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data": crate::evals::detail(&state.pool, id).await?}),
    ))
}

#[derive(Debug, Deserialize)]
struct EvalAnnotationRequest {
    verdict: String,
    notes: Option<String>,
    expected_revision: i64,
}

async fn annotate_eval(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    Path(id): Path<Uuid>,
    Json(input): Json<EvalAnnotationRequest>,
) -> Result<Json<Value>, ApiError> {
    let verdict = allowed(input.verdict, "verdict", crate::evals::VERDICTS)?;
    let notes = optional_text(input.notes, "notes", 4000)?;
    let eval = crate::evals::annotate(
        &state.pool,
        id,
        &verdict,
        notes.as_deref(),
        &actor.actor_id,
        input.expected_revision,
    )
    .await?;
    Ok(Json(json!({"data": eval})))
}

fn parse_timestamp(value: String, field: &'static str) -> Result<OffsetDateTime, ApiError> {
    OffsetDateTime::parse(&value, &Rfc3339)
        .map_err(|_| ApiError::BadRequest(format!("{field} must be an RFC 3339 timestamp")))
}

fn map_curator_error(error: crate::curator::CuratorError) -> ApiError {
    match error {
        crate::curator::CuratorError::NotFound => ApiError::Db(DbError::NotFound),
        crate::curator::CuratorError::Conflict => ApiError::Db(DbError::Conflict),
        crate::curator::CuratorError::Invalid(message) => ApiError::BadRequest(message),
        crate::curator::CuratorError::Sqlx(error) => ApiError::Db(DbError::Sqlx(error)),
    }
}

fn parse_due_at(value: Option<String>) -> Result<Option<OffsetDateTime>, ApiError> {
    value
        .map(|value| {
            OffsetDateTime::parse(value.trim(), &Rfc3339)
                .map_err(|_| ApiError::BadRequest("due_at must be RFC 3339".to_owned()))
        })
        .transpose()
}

fn bounded_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(50).clamp(1, 100)
}

fn idempotency_key(
    headers: &HeaderMap,
    required: bool,
    _actor: &ActorContext,
) -> Result<Option<String>, ApiError> {
    let key = optional_header(headers, "idempotency-key")?;
    if required && key.is_none() {
        return Err(ApiError::BadRequest(
            "Idempotency-Key is required".to_owned(),
        ));
    }
    if key.as_ref().is_some_and(|value| value.len() > 200) {
        return Err(ApiError::BadRequest(
            "Idempotency-Key is too long".to_owned(),
        ));
    }
    Ok(key)
}

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Unauthorized,
    Forbidden(String),
    Validation(ValidationError),
    Db(DbError),
}

impl From<ValidationError> for ApiError {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value)
    }
}

impl From<DbError> for ApiError {
    fn from(value: DbError) -> Self {
        Self::Db(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "bad_request", message),
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Authentication failed.".to_owned(),
            ),
            Self::Forbidden(message) => (StatusCode::FORBIDDEN, "forbidden", message),
            Self::Validation(error) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_error",
                error.to_string(),
            ),
            Self::Db(DbError::NotFound) => (
                StatusCode::NOT_FOUND,
                "not_found",
                "Record not found.".to_owned(),
            ),
            Self::Db(DbError::Conflict) => (
                StatusCode::CONFLICT,
                "revision_conflict",
                "The record changed after it was read.".to_owned(),
            ),
            Self::Db(DbError::Validation(error)) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_error",
                error.to_string(),
            ),
            Self::Db(DbError::Sqlx(error)) if is_constraint_error(&error) => (
                StatusCode::CONFLICT,
                "constraint_conflict",
                "The requested change conflicts with existing data.".to_owned(),
            ),
            Self::Db(DbError::Sqlx(error)) => {
                tracing::error!(%error, "database request failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "The request could not be completed.".to_owned(),
                )
            }
        };
        (
            status,
            Json(json!({"error": {"code": code, "message": message}})),
        )
            .into_response()
    }
}

fn is_constraint_error(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .is_some_and(|error| matches!(error.code().as_deref(), Some("23503" | "23505" | "23514")))
}

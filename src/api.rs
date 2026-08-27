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
};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
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
    service_router(state)
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
                .route("/objects", get(list_objects).post(create_object))
                .route("/objects/{id}", get(read_object).patch(update_object))
                .route("/objects/{id}/connections", get(list_connections))
                .route("/objects/{id}/events", get(list_events))
                .route("/connections", post(create_connection))
                .route("/connections/{id}", axum::routing::patch(update_connection))
                .route("/connections/{id}/archive", post(archive_connection))
                .route("/tasks", get(list_tasks).post(create_task))
                .route("/tasks/{id}", get(read_task).patch(update_task)),
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

async fn ready(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    db::ready(&state.pool).await?;
    Ok(Json(json!({"ok": true, "ready": true})))
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
    let object = db::create_object(
        &state.pool,
        &actor,
        NewObject {
            kind,
            title: required_text(input.title, "title", 300)?,
            description: required_text(input.description, "description", 1000)?,
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
    let task = db::create_task(
        &state.pool,
        &actor,
        NewTask {
            title: required_text(input.title, "title", 300)?,
            description: required_text(input.description, "description", 1000)?,
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
                "Agent authentication failed.".to_owned(),
            ),
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

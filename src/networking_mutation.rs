use std::sync::Arc;

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use subtle::ConstantTimeEq;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::{
    api::{ApiError, AppState},
    db::{self, NewConnection, NewObject, NewTask},
    domain::{
        ActorContext, CONNECTION_KINDS, TASK_PRIORITIES, ValidationError, allowed,
        object_description, provenance, required_text,
    },
};

const ENTITY_KINDS: &[&str] = &[
    "person",
    "organization",
    "product",
    "project",
    "publication",
    "place",
    "concept",
    "other",
];

#[derive(Clone)]
struct MutationState {
    app: AppState,
    token: Arc<String>,
    allowed_principal: Arc<String>,
}

pub fn router(app: AppState, token: String, allowed_principal: String) -> Router {
    let state = MutationState {
        app,
        token: Arc::new(token),
        allowed_principal: Arc::new(allowed_principal),
    };
    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .route("/api/v2/objects", get(search_entities).post(create_entity))
        .route("/api/v2/objects/{id}", get(read_entity))
        .route("/api/v2/connections", post(create_connection))
        .route("/api/v2/tasks", post(create_task))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state, authenticate))
        .layer(TraceLayer::new_for_http())
}

async fn health() -> Json<Value> {
    Json(json!({"ok":true}))
}

async fn ready(State(state): State<MutationState>) -> Result<Json<Value>, ApiError> {
    db::ready(&state.app.pool).await?;
    Ok(Json(json!({"ok":true,"ready":true})))
}

async fn authenticate(
    State(state): State<MutationState>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let bearer = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(ApiError::Unauthorized)?;
    if state.token.len() != bearer.len()
        || state.token.as_bytes().ct_eq(bearer.as_bytes()).unwrap_u8() != 1
    {
        return Err(ApiError::Unauthorized);
    }
    let principal = exact_header(request.headers(), "x-centaur-principal-id")?;
    if principal != *state.allowed_principal {
        return Err(ApiError::Forbidden(
            "only the configured Networking-mutation principal may use this listener".into(),
        ));
    }
    let thread_key =
        canonical_thread_key(exact_header(request.headers(), "x-centaur-thread-key")?)?;
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

fn canonical_thread_key(value: String) -> Result<String, ApiError> {
    let parts = value.split(':').collect::<Vec<_>>();
    if value.len() > 1_000
        || parts.len() != 4
        || parts
            .iter()
            .any(|part| part.is_empty() || *part != part.trim())
        || parts[0]
            .chars()
            .any(|character| character.is_ascii_uppercase())
    {
        return Err(ApiError::BadRequest(
            "x-centaur-thread-key must be canonical provider:workspace:channel:thread".into(),
        ));
    }
    Ok(value)
}

fn required_header(headers: &HeaderMap, name: &'static str) -> Result<String, ApiError> {
    optional_header(headers, name)?
        .ok_or_else(|| ApiError::BadRequest(format!("{name} is required")))
}

fn exact_header(headers: &HeaderMap, name: &'static str) -> Result<String, ApiError> {
    let value = headers
        .get(name)
        .ok_or_else(|| ApiError::BadRequest(format!("{name} is required")))?
        .to_str()
        .map_err(|_| ApiError::BadRequest(format!("{name} is invalid")))?;
    if value.is_empty() || value != value.trim() {
        return Err(ApiError::BadRequest(format!("{name} is invalid")));
    }
    Ok(value.to_owned())
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

fn idempotency_key(headers: &HeaderMap) -> Result<String, ApiError> {
    let key = required_header(headers, "idempotency-key")?;
    if key.len() > 200 {
        return Err(ApiError::BadRequest(
            "Idempotency-Key must be at most 200 characters".into(),
        ));
    }
    Ok(key)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EntitySearchQuery {
    q: String,
    kind: String,
    limit: Option<i64>,
    sort: String,
}

async fn search_entities(
    State(state): State<MutationState>,
    Query(query): Query<EntitySearchQuery>,
) -> Result<Json<Value>, ApiError> {
    if query.kind != "entity" {
        return Err(ApiError::Forbidden(
            "the Networking-mutation listener searches only Entities".into(),
        ));
    }
    if query.sort != "recent" {
        return Err(ApiError::BadRequest("sort must be recent".into()));
    }
    let limit = query.limit.unwrap_or(10);
    if !(1..=10).contains(&limit) {
        return Err(ApiError::BadRequest(
            "limit must be between 1 and 10".into(),
        ));
    }
    let data = db::list_objects(
        &state.app.pool,
        db::ObjectListFilter {
            query: Some(required_text(query.q, "q", 300)?),
            kind: Some("entity".into()),
            lifecycle: Some("active".into()),
            cursor: None,
            limit,
            sort: db::ListSort::Recent,
            text_search_config: state.app.text_search_config,
        },
    )
    .await?;
    Ok(Json(json!({"data":data})))
}

async fn read_entity(
    State(state): State<MutationState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let object = db::get_object(&state.app.pool, id).await?;
    if object.kind != "entity" || object.lifecycle != "active" {
        return Err(ApiError::Db(db::DbError::NotFound));
    }
    Ok(Json(
        json!({"data":canonical_entity(&state.app.pool,object).await?}),
    ))
}

async fn canonical_entity(pool: &sqlx::PgPool, object: db::Object) -> Result<Value, ApiError> {
    let id = object.id;
    let mut record = serde_json::to_value(object)
        .map_err(|error| ApiError::BadRequest(format!("serialize Entity: {error}")))?;
    let subtype = db::context_subtypes(pool, &[id], None)
        .await?
        .remove(&id)
        .ok_or(db::DbError::NotFound)?;
    record["entity_kind"] = subtype["entity_kind"].clone();
    Ok(record)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateEntityRequest {
    kind: String,
    title: String,
    description: String,
    entity_kind: String,
    provenance: Value,
}

async fn create_entity(
    State(state): State<MutationState>,
    Extension(actor): Extension<ActorContext>,
    headers: HeaderMap,
    Json(input): Json<CreateEntityRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if input.kind != "entity" {
        return Err(ApiError::Forbidden(
            "the Networking-mutation listener creates only Entities".into(),
        ));
    }
    let key = idempotency_key(&headers)?;
    let title = required_text(input.title, "title", 300)?;
    let description = object_description(&title, input.description)?;
    let entity_kind = allowed(input.entity_kind, "entity_kind", ENTITY_KINDS)?;
    let provenance = required_provenance(input.provenance)?;
    let expected = json!({
        "title": &title,
        "description": &description,
        "entity_kind": &entity_kind,
        "provenance": &provenance,
    });
    let object = db::create_object(
        &state.app.pool,
        &actor,
        NewObject {
            kind: "entity".into(),
            title,
            description,
            provenance,
            entity_kind: Some(entity_kind),
            happened_at: None,
        },
        &key,
    )
    .await?;
    if object.kind != "entity" {
        return Err(ApiError::Db(db::DbError::Conflict));
    }
    let record = canonical_entity(&state.app.pool, object).await?;
    ensure_replay_fields(
        &record,
        &expected,
        &["title", "description", "entity_kind", "provenance"],
    )?;
    Ok((StatusCode::CREATED, Json(json!({"data":record}))))
}

fn ensure_replay_fields(record: &Value, expected: &Value, fields: &[&str]) -> Result<(), ApiError> {
    if fields
        .iter()
        .any(|field| record[*field] != expected[*field])
    {
        return Err(ApiError::Db(db::DbError::Conflict));
    }
    Ok(())
}

fn required_provenance(value: Value) -> Result<Value, ApiError> {
    let value = provenance(Some(value))?;
    if value
        .get("source_type")
        .and_then(Value::as_str)
        .is_none_or(|source_type| source_type.trim().is_empty())
    {
        return Err(ApiError::BadRequest(
            "provenance.source_type is required".into(),
        ));
    }
    Ok(value)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateConnectionRequest {
    source_object_id: Uuid,
    target_object_id: Uuid,
    kind: String,
    description: String,
    provenance: Value,
    protected: bool,
}

async fn create_connection(
    State(state): State<MutationState>,
    Extension(actor): Extension<ActorContext>,
    headers: HeaderMap,
    Json(input): Json<CreateConnectionRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if input.protected {
        return Err(ApiError::Forbidden(
            "the Networking-mutation listener cannot create protected Connections".into(),
        ));
    }
    if input.source_object_id == input.target_object_id {
        return Err(ValidationError::SelfConnection.into());
    }
    let result = db::create_or_reuse_connection(
        &state.app.pool,
        &actor,
        NewConnection {
            source_object_id: input.source_object_id,
            target_object_id: input.target_object_id,
            kind: allowed(input.kind, "connection kind", CONNECTION_KINDS)?,
            description: required_text(input.description, "description", 1000)?,
            provenance: required_provenance(input.provenance)?,
            protected: false,
        },
        &idempotency_key(&headers)?,
    )
    .await?;
    Ok((
        if result.reused {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        Json(json!({"data":{"record":result.connection,"reused":result.reused}})),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateTaskRequest {
    title: String,
    description: String,
    status: String,
    priority: String,
    agent_suitable: bool,
    provenance: Value,
    derived_from_source_object_ids: Vec<Uuid>,
}

async fn create_task(
    State(state): State<MutationState>,
    Extension(actor): Extension<ActorContext>,
    headers: HeaderMap,
    Json(input): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if input.status != "todo"
        || input.agent_suitable
        || !input.derived_from_source_object_ids.is_empty()
    {
        return Err(ApiError::Forbidden(
            "the Networking-mutation listener creates only unassigned todo Tasks without Source links"
                .into(),
        ));
    }
    let title = required_text(input.title, "title", 300)?;
    let description = object_description(&title, input.description)?;
    let priority = allowed(input.priority, "priority", TASK_PRIORITIES)?;
    let provenance = required_provenance(input.provenance)?;
    let expected = json!({
        "title":&title,
        "description":&description,
        "status":"todo",
        "priority":&priority,
        "agent_suitable":false,
        "provenance":&provenance,
    });
    let task = db::create_task(
        &state.app.pool,
        &actor,
        NewTask {
            description,
            title,
            provenance,
            status: "todo".into(),
            priority,
            owner_object_id: None,
            agent_suitable: false,
            blocked_reason: None,
            due_at: None,
            completed_at: None,
            github_issue_url: None,
            brief_markdown: None,
            originating_chat_object_id: None,
            derived_from_source_object_ids: Vec::new(),
        },
        &idempotency_key(&headers)?,
    )
    .await?;
    let record = serde_json::to_value(task)
        .map_err(|error| ApiError::BadRequest(format!("serialize Task: {error}")))?;
    ensure_replay_fields(
        &record,
        &expected,
        &[
            "title",
            "description",
            "status",
            "priority",
            "agent_suitable",
            "provenance",
        ],
    )?;
    Ok((StatusCode::CREATED, Json(json!({"data":record}))))
}

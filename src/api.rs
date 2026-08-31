use std::{path::PathBuf, sync::Arc};

use axum::{
    Extension, Json, Router,
    body::Body,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
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
        self, ConnectionChanges, DbError, NewConnection, NewNote, NewObject, NewSource,
        NewSourceContent, NewTask, NewTheme, NewThemeProposal, ObjectChanges, SourceChanges,
        TaskChanges,
    },
    domain::{
        ActorContext, CONNECTION_KINDS, NOTE_CONTENT_FORMATS, OBJECT_KINDS, SOURCE_CONTENT_KINDS,
        SOURCE_KINDS, TASK_PRIORITIES, TASK_STATUSES, ValidationError, allowed, optional_text,
        provenance, required_preserved_text, required_text,
    },
    embeddings::EmbeddingClient,
    schema, search,
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

#[derive(Clone)]
struct IdentityAssetsDir(PathBuf);

pub fn human_router(state: AppState, static_dir: PathBuf, identity_assets_dir: PathBuf) -> Router {
    let index = static_dir.join("index.html");
    Router::new()
        .route(
            "/api/v1/identity-assets/{sha256}/{filename}",
            get(identity_asset),
        )
        .merge(service_router(state))
        .route("/api/{*path}", any(api_not_found))
        .fallback_service(ServeDir::new(static_dir).fallback(ServeFile::new(index)))
        .layer(Extension(IdentityAssetsDir(identity_assets_dir)))
        .layer(Extension(ActorContext::human()))
        .layer(TraceLayer::new_for_http())
}

async fn identity_asset(
    Path((sha256, filename)): Path<(String, String)>,
    Extension(root): Extension<IdentityAssetsDir>,
) -> Result<Response, StatusCode> {
    if !is_safe_asset_digest(&sha256) || !is_safe_asset_filename(&filename) {
        return Err(StatusCode::NOT_FOUND);
    }
    let expected_mime = if filename.ends_with(".png") {
        "image/png"
    } else if filename.ends_with(".jpg") || filename.ends_with(".jpeg") {
        "image/jpeg"
    } else {
        return Err(StatusCode::NOT_FOUND);
    };
    let bytes = tokio::fs::read(root.0.join(&sha256).join(&filename))
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    if bytes.len() > 5 * 1024 * 1024
        || !asset_bytes_match_mime(&bytes, expected_mime)
        || format!("{:x}", Sha256::digest(&bytes)) != sha256
    {
        return Err(StatusCode::NOT_FOUND);
    }
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, expected_mime)
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .header(header::ETAG, format!("\"{sha256}\""))
        .header("x-content-type-options", "nosniff")
        .body(Body::from(bytes))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn is_safe_asset_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_safe_asset_filename(value: &str) -> bool {
    !value.contains("..")
        && value.len() <= 128
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && matches!(byte, b'.' | b'_' | b'-'))
        })
}

fn asset_bytes_match_mime(bytes: &[u8], mime: &str) -> bool {
    match mime {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        _ => false,
    }
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
                .route("/objects/{id}", get(read_context_object))
                .route("/search/sources", get(search_sources))
                .route("/sources/{id}", get(read_source))
                .route("/sources/{id}/content", get(read_source_content))
                .route("/search/notes", get(search_notes))
                .route("/notes/{id}", get(read_note))
                .route("/themes", get(list_themes))
                .route("/themes/{id}", get(read_theme))
                .route("/themes/{id}/objects", get(list_theme_objects)),
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

pub fn theme_proposal_router(state: AppState, token: String) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .nest(
            "/api/v1",
            Router::new()
                .route("/theme-proposals", post(create_theme_proposal))
                .route("/theme-proposals/{id}", get(read_theme_proposal))
                .route("/theme-assignments", post(create_theme_assignment))
                .route(
                    "/theme-assignments/{id}/archive",
                    post(archive_theme_assignment),
                ),
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

pub fn note_write_router(state: AppState, token: String) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .nest("/api/v1", Router::new().route("/notes", post(create_note)))
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
                .route("/sources", get(list_sources).post(create_source))
                .route("/sources/{id}", get(read_source).patch(update_source))
                .route(
                    "/sources/{id}/contents",
                    get(list_source_contents).post(create_source_content),
                )
                .route("/sources/{id}/content", get(read_source_content))
                .route("/notes", get(list_notes).post(create_note))
                .route("/notes/{id}", get(read_note).patch(update_note))
                .route("/themes", get(list_themes).post(create_theme))
                .route("/themes/{id}", get(read_theme))
                .route("/themes/{id}/objects", get(list_theme_objects))
                .route("/theme-proposals", get(list_theme_proposals))
                .route(
                    "/theme-proposals/{id}/approve",
                    post(approve_theme_proposal),
                )
                .route("/theme-proposals/{id}/reject", post(reject_theme_proposal))
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
                .route("/schema", get(read_schema))
                .route("/schema/tables/{table}/rows", get(read_schema_rows))
                .route("/schema/tables/{table}/profile", get(read_schema_profile))
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

async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
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

async fn read_schema(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let snapshot = schema::inspect_schema(&state.pool).await?;
    let etag = format!("\"{}\"", snapshot.fingerprint);
    if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        == Some(etag.as_str())
    {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }
    let mut response = Json(json!({"data": snapshot})).into_response();
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&etag).expect("schema fingerprint is an HTTP-safe ETag"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    Ok(response)
}

#[derive(Debug, Deserialize)]
struct SchemaRowsQuery {
    limit: Option<i64>,
    cursor: Option<String>,
    focus_column: Option<String>,
    focus_value: Option<String>,
}

async fn read_schema_rows(
    State(state): State<AppState>,
    Path(table): Path<String>,
    Query(query): Query<SchemaRowsQuery>,
) -> Result<Json<Value>, ApiError> {
    let focus = match (query.focus_column.as_deref(), query.focus_value.as_deref()) {
        (Some(column), Some(value)) => Some((column, value)),
        (None, None) => None,
        _ => {
            return Err(ApiError::BadRequest(
                "focus_column and focus_value must be supplied together".to_owned(),
            ));
        }
    };
    let page = schema::read_rows(
        &state.pool,
        &table,
        query.limit,
        query.cursor.as_deref(),
        focus,
    )
    .await?;
    Ok(Json(json!({"data": page})))
}

async fn read_schema_profile(
    State(state): State<AppState>,
    Path(table): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let profile = schema::profile_table(&state.pool, &table).await?;
    Ok(Json(json!({"data": profile})))
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

#[derive(Debug, Deserialize)]
struct ThemeListQuery {
    slug: Option<String>,
}

async fn list_themes(
    State(state): State<AppState>,
    Query(query): Query<ThemeListQuery>,
) -> Result<Json<Value>, ApiError> {
    let data = if let Some(slug) = query.slug {
        vec![db::get_theme_by_slug(&state.pool, &crate::domain::theme_slug(slug)?).await?]
    } else {
        db::list_themes(&state.pool).await?
    };
    Ok(Json(json!({"data":data})))
}

async fn read_theme(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({"data":db::get_theme(&state.pool,id).await?})))
}

#[derive(Debug, Deserialize)]
struct ThemeObjectsQuery {
    kind: Option<String>,
    limit: Option<i64>,
}

async fn list_theme_objects(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<ThemeObjectsQuery>,
) -> Result<Json<Value>, ApiError> {
    let kind = query
        .kind
        .map(|value| allowed(value, "kind", OBJECT_KINDS))
        .transpose()?;
    if kind.as_deref() == Some("theme") {
        return Err(ApiError::BadRequest(
            "a Theme cannot itself be assigned a Theme".into(),
        ));
    }
    let data = db::list_theme_objects(&state.pool, id, kind.as_deref(), bounded_limit(query.limit))
        .await?;
    Ok(Json(json!({"data":data})))
}

#[derive(Debug, Deserialize)]
struct CreateThemeRequest {
    title: String,
    slug: String,
    description: String,
    provenance: Option<Value>,
    #[serde(default = "default_true")]
    protected: bool,
}

fn default_true() -> bool {
    true
}

async fn require_theme_approval_permission(
    state: &AppState,
    actor: &ActorContext,
) -> Result<(), ApiError> {
    if actor.is_agent || !db::has_permission(&state.pool, actor, "approve_themes").await? {
        return Err(ApiError::Forbidden(
            "approve_themes permission is required".into(),
        ));
    }
    Ok(())
}

async fn create_theme(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    headers: HeaderMap,
    Json(input): Json<CreateThemeRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    require_theme_approval_permission(&state, &actor).await?;
    let key = idempotency_key(&headers, true, &actor)?.expect("required idempotency key");
    let title = required_text(input.title, "title", 300)?;
    let theme = db::create_theme(
        &state.pool,
        &actor,
        NewTheme {
            description: crate::domain::object_description(&title, input.description)?,
            title,
            slug: crate::domain::theme_slug(input.slug)?,
            provenance: provenance(input.provenance)?,
            protected: input.protected,
        },
        &key,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(json!({"data":theme}))))
}

#[derive(Debug, Deserialize)]
struct ThemeProposalListQuery {
    status: Option<String>,
}

async fn list_theme_proposals(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    Query(query): Query<ThemeProposalListQuery>,
) -> Result<Json<Value>, ApiError> {
    require_theme_approval_permission(&state, &actor).await?;
    let status = query
        .status
        .map(|value| allowed(value, "status", &["pending", "approved", "rejected"]))
        .transpose()?;
    Ok(Json(json!({
        "data":db::list_theme_proposals(&state.pool,status.as_deref()).await?
    })))
}

#[derive(Debug, Deserialize)]
struct CreateThemeProposalRequest {
    title: String,
    slug: String,
    description: String,
    rationale: String,
    evidence: Option<Value>,
    provenance: Option<Value>,
}

async fn create_theme_proposal(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    headers: HeaderMap,
    Json(input): Json<CreateThemeProposalRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let key = idempotency_key(&headers, true, &actor)?.expect("required idempotency key");
    let title = required_text(input.title, "title", 300)?;
    let proposal = db::create_theme_proposal(
        &state.pool,
        &actor,
        NewThemeProposal {
            description: crate::domain::object_description(&title, input.description)?,
            title,
            slug: crate::domain::theme_slug(input.slug)?,
            rationale: required_text(input.rationale, "rationale", 2000)?,
            evidence: theme_evidence(input.evidence)?,
            provenance: provenance(input.provenance)?,
        },
        &key,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(json!({"data":proposal}))))
}

fn theme_evidence(value: Option<Value>) -> Result<Value, ApiError> {
    let value = value.unwrap_or_else(|| json!({}));
    if !value.is_object() {
        return Err(ApiError::BadRequest(
            "evidence must be a JSON object".into(),
        ));
    }
    if serde_json::to_vec(&value)
        .map_err(|_| ApiError::BadRequest("evidence must be valid JSON".into()))?
        .len()
        > 32_768
    {
        return Err(ApiError::BadRequest(
            "evidence must be at most 32768 encoded bytes".into(),
        ));
    }
    Ok(value)
}

async fn read_theme_proposal(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "data":db::get_theme_proposal(&state.pool,id).await?
    })))
}

#[derive(Debug, Deserialize)]
struct CreateThemeAssignmentRequest {
    object_id: Uuid,
    theme_id: Uuid,
    description: String,
    provenance: Option<Value>,
    #[serde(default)]
    protected: bool,
}

async fn create_theme_assignment(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    headers: HeaderMap,
    Json(input): Json<CreateThemeAssignmentRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let key = idempotency_key(&headers, true, &actor)?.expect("required idempotency key");
    let data = db::create_connection(
        &state.pool,
        &actor,
        NewConnection {
            source_object_id: input.object_id,
            kind: "themed".into(),
            target_object_id: input.theme_id,
            description: required_text(input.description, "description", 1000)?,
            provenance: provenance(input.provenance)?,
            protected: input.protected,
        },
        &key,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(json!({"data":data}))))
}

#[derive(Debug, Deserialize)]
struct ArchiveThemeAssignmentRequest {
    expected_revision: i64,
}

async fn archive_theme_assignment(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ArchiveThemeAssignmentRequest>,
) -> Result<Json<Value>, ApiError> {
    let current = db::get_connection(&state.pool, id).await?;
    if current.kind != "themed" {
        return Err(ApiError::Forbidden(
            "the Theme assignment listener can archive only themed Connections".into(),
        ));
    }
    let key = idempotency_key(&headers, true, &actor)?.expect("required idempotency key");
    let data = db::archive_connection(&state.pool, &actor, id, input.expected_revision, Some(&key))
        .await?;
    Ok(Json(json!({"data":data})))
}

#[derive(Debug, Deserialize)]
struct ThemeDecisionRequest {
    decision_reason: String,
}

async fn approve_theme_proposal(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ThemeDecisionRequest>,
) -> Result<Json<Value>, ApiError> {
    require_theme_approval_permission(&state, &actor).await?;
    let key = idempotency_key(&headers, true, &actor)?.expect("required idempotency key");
    let data = db::approve_theme_proposal(
        &state.pool,
        &actor,
        id,
        &required_text(input.decision_reason, "decision_reason", 1000)?,
        &key,
    )
    .await?;
    Ok(Json(json!({"data":data})))
}

async fn reject_theme_proposal(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    Path(id): Path<Uuid>,
    Json(input): Json<ThemeDecisionRequest>,
) -> Result<Json<Value>, ApiError> {
    require_theme_approval_permission(&state, &actor).await?;
    let data = db::reject_theme_proposal(
        &state.pool,
        &actor,
        id,
        &required_text(input.decision_reason, "decision_reason", 1000)?,
    )
    .await?;
    Ok(Json(json!({"data":data})))
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
struct SourceListQuery {
    q: Option<String>,
    source_kind: Option<String>,
    cursor: Option<Uuid>,
    limit: Option<i64>,
}

async fn source_page(state: &AppState, query: SourceListQuery) -> Result<Value, ApiError> {
    let limit = bounded_limit(query.limit);
    let mut items = db::list_sources(
        &state.pool,
        db::SourceListFilter {
            query: optional_text(query.q, "q", 1000)?,
            source_kind: query
                .source_kind
                .map(|value| allowed(value, "source_kind", SOURCE_KINDS))
                .transpose()?,
            cursor: query.cursor,
            limit: limit + 1,
        },
    )
    .await?;
    let next_cursor = if items.len() as i64 > limit {
        items.pop();
        items.last().map(|item| item.source.object_id)
    } else {
        None
    };
    Ok(json!({"items":items,"next_cursor":next_cursor}))
}

async fn list_sources(
    State(state): State<AppState>,
    Query(query): Query<SourceListQuery>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({"data":source_page(&state,query).await?})))
}

async fn search_sources(
    State(state): State<AppState>,
    Query(mut query): Query<SourceListQuery>,
) -> Result<Json<Value>, ApiError> {
    query.q = Some(required_text(query.q.unwrap_or_default(), "q", 1000)?);
    Ok(Json(json!({"data":source_page(&state,query).await?})))
}

#[derive(Debug, Deserialize)]
struct CreateSourceRequest {
    title: String,
    description: String,
    source_kind: String,
    canonical_uri: Option<String>,
    byline: Option<String>,
    publisher: Option<String>,
    published_at: Option<String>,
    published_at_precision: Option<String>,
    #[serde(alias = "accessed_at")]
    last_accessed_at: Option<String>,
    #[serde(alias = "language")]
    original_language: Option<String>,
    #[serde(alias = "media_type")]
    original_media_type: Option<String>,
    #[serde(alias = "artifact_reference")]
    original_artifact_reference: Option<String>,
    provenance: Option<Value>,
}

fn source_uri(value: Option<String>) -> Result<Option<String>, ApiError> {
    let value = optional_text(value, "canonical_uri", 2000)?;
    if value
        .as_ref()
        .is_some_and(|uri| !(uri.starts_with("https://") || uri.starts_with("http://")))
    {
        return Err(ApiError::BadRequest(
            "canonical_uri must use HTTP or HTTPS".to_owned(),
        ));
    }
    Ok(value)
}

fn github_issue_url(value: Option<String>) -> Result<Option<String>, ApiError> {
    let value = optional_text(value, "github_issue_url", 2000)?;
    if let Some(url) = value.as_deref() {
        let parts = url
            .strip_prefix("https://github.com/")
            .map(|path| path.split('/').collect::<Vec<_>>());
        let valid = parts.is_some_and(|parts| {
            parts.len() == 4
                && !parts[0].is_empty()
                && !parts[1].is_empty()
                && parts[2] == "issues"
                && parts[3].parse::<u64>().is_ok_and(|number| number > 0)
        });
        if !valid {
            return Err(ApiError::BadRequest(
                "github_issue_url must be a canonical HTTPS GitHub Issue URL".into(),
            ));
        }
    }
    Ok(value)
}

async fn create_source(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    headers: HeaderMap,
    Json(input): Json<CreateSourceRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let key = idempotency_key(&headers, true, &actor)?.expect("required idempotency key");
    let title = required_text(input.title, "title", 300)?;
    let published_at = parse_optional_timestamp(input.published_at, "published_at")?;
    let published_at_precision = input
        .published_at_precision
        .map(|value| {
            allowed(
                value,
                "published_at_precision",
                &["instant", "day", "month", "year"],
            )
        })
        .transpose()?
        .or_else(|| published_at.map(inferred_publication_precision));
    if published_at.is_some() != published_at_precision.is_some() {
        return Err(ApiError::BadRequest(
            "published_at and published_at_precision must be provided together".into(),
        ));
    }
    let source = db::create_source(
        &state.pool,
        &actor,
        NewSource {
            description: crate::domain::object_description(&title, input.description)?,
            title,
            provenance: provenance(input.provenance)?,
            source_kind: allowed(input.source_kind, "source_kind", SOURCE_KINDS)?,
            canonical_uri: source_uri(input.canonical_uri)?,
            byline: optional_text(input.byline, "byline", 500)?,
            publisher: optional_text(input.publisher, "publisher", 300)?,
            published_at,
            published_at_precision,
            last_accessed_at: parse_optional_timestamp(input.last_accessed_at, "last_accessed_at")?,
            original_language: optional_text(input.original_language, "original_language", 35)?,
            original_media_type: optional_text(
                input.original_media_type,
                "original_media_type",
                255,
            )?,
            original_artifact_reference: optional_text(
                input.original_artifact_reference,
                "original_artifact_reference",
                1000,
            )?,
        },
        &key,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(json!({"data":source}))))
}

async fn read_source(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({"data":db::get_source(&state.pool,id).await?})))
}

#[derive(Debug, Deserialize)]
struct UpdateSourceRequest {
    expected_revision: i64,
    title: Option<String>,
    description: Option<String>,
    source_kind: Option<String>,
    canonical_uri: Option<String>,
    byline: Option<String>,
    publisher: Option<String>,
    published_at: Option<String>,
    published_at_precision: Option<String>,
    #[serde(alias = "accessed_at")]
    last_accessed_at: Option<String>,
    #[serde(alias = "language")]
    original_language: Option<String>,
    #[serde(alias = "media_type")]
    original_media_type: Option<String>,
    #[serde(alias = "artifact_reference")]
    original_artifact_reference: Option<String>,
    #[serde(default)]
    clear_canonical_uri: bool,
    #[serde(default)]
    clear_byline: bool,
    #[serde(default)]
    clear_publisher: bool,
    #[serde(default)]
    clear_published_at: bool,
    #[serde(default)]
    #[serde(alias = "clear_accessed_at")]
    clear_last_accessed_at: bool,
    #[serde(default)]
    #[serde(alias = "clear_language")]
    clear_original_language: bool,
    #[serde(default)]
    #[serde(alias = "clear_media_type")]
    clear_original_media_type: bool,
    #[serde(default)]
    #[serde(alias = "clear_artifact_reference")]
    clear_original_artifact_reference: bool,
    provenance: Option<Value>,
    protected: Option<bool>,
    #[serde(default)]
    archive: bool,
}

fn nullable_change<T>(
    value: Option<T>,
    clear: bool,
    field: &'static str,
) -> Result<Option<Option<T>>, ApiError> {
    if clear && value.is_some() {
        return Err(ApiError::BadRequest(format!(
            "clear_{field} cannot be combined with {field}"
        )));
    }
    Ok(if clear { Some(None) } else { value.map(Some) })
}

async fn update_source(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateSourceRequest>,
) -> Result<Json<Value>, ApiError> {
    let key = idempotency_key(&headers, actor.is_agent, &actor)?;
    let published_at = input
        .published_at
        .map(|value| parse_optional_timestamp(Some(value), "published_at"))
        .transpose()?
        .flatten();
    let published_at_precision = input
        .published_at_precision
        .map(|value| {
            allowed(
                value,
                "published_at_precision",
                &["instant", "day", "month", "year"],
            )
        })
        .transpose()?
        .or_else(|| published_at.map(inferred_publication_precision));
    if !input.clear_published_at && (published_at.is_some() != published_at_precision.is_some()) {
        return Err(ApiError::BadRequest(
            "published_at and published_at_precision must be updated together".into(),
        ));
    }
    let source = db::update_source(
        &state.pool,
        &actor,
        id,
        input.expected_revision,
        SourceChanges {
            title: input
                .title
                .map(|v| required_text(v, "title", 300))
                .transpose()?,
            description: input
                .description
                .map(|v| required_text(v, "description", 2000))
                .transpose()?,
            provenance: input.provenance.map(|v| provenance(Some(v))).transpose()?,
            protected: input.protected,
            archive: input.archive,
            source_kind: input
                .source_kind
                .map(|v| allowed(v, "source_kind", SOURCE_KINDS))
                .transpose()?,
            canonical_uri: nullable_change(
                source_uri(input.canonical_uri)?,
                input.clear_canonical_uri,
                "canonical_uri",
            )?,
            byline: nullable_change(
                optional_text(input.byline, "byline", 500)?,
                input.clear_byline,
                "byline",
            )?,
            publisher: nullable_change(
                optional_text(input.publisher, "publisher", 300)?,
                input.clear_publisher,
                "publisher",
            )?,
            published_at: nullable_change(published_at, input.clear_published_at, "published_at")?,
            published_at_precision: nullable_change(
                published_at_precision,
                input.clear_published_at,
                "published_at_precision",
            )?,
            last_accessed_at: nullable_change(
                input
                    .last_accessed_at
                    .map(|v| parse_optional_timestamp(Some(v), "last_accessed_at"))
                    .transpose()?
                    .flatten(),
                input.clear_last_accessed_at,
                "last_accessed_at",
            )?,
            original_language: nullable_change(
                optional_text(input.original_language, "original_language", 35)?,
                input.clear_original_language,
                "original_language",
            )?,
            original_media_type: nullable_change(
                optional_text(input.original_media_type, "original_media_type", 255)?,
                input.clear_original_media_type,
                "original_media_type",
            )?,
            original_artifact_reference: nullable_change(
                optional_text(
                    input.original_artifact_reference,
                    "original_artifact_reference",
                    1000,
                )?,
                input.clear_original_artifact_reference,
                "original_artifact_reference",
            )?,
        },
        key.as_deref(),
    )
    .await?;
    Ok(Json(json!({"data":source})))
}

async fn list_source_contents(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":db::list_source_contents(&state.pool,id).await?}),
    ))
}

#[derive(Debug, Deserialize)]
struct SourceContentQuery {
    version: Option<i64>,
    offset: Option<i64>,
    limit: Option<i64>,
}

async fn read_source_content(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<SourceContentQuery>,
) -> Result<Json<Value>, ApiError> {
    if query.version.is_some_and(|v| v < 1) {
        return Err(ApiError::BadRequest("version must be positive".into()));
    }
    let offset = query.offset.unwrap_or(0);
    if offset < 0 {
        return Err(ApiError::BadRequest("offset must not be negative".into()));
    }
    let limit = query.limit.unwrap_or(8000);
    if !(1..=20_000).contains(&limit) {
        return Err(ApiError::BadRequest(
            "limit must be between 1 and 20000".into(),
        ));
    }
    Ok(Json(
        json!({"data":db::get_source_content_window(&state.pool,id,query.version,offset,limit).await?}),
    ))
}

#[derive(Debug, Deserialize)]
struct CreateSourceContentRequest {
    expected_revision: i64,
    content_kind: String,
    normalized_text: String,
    language: Option<String>,
    extraction_method: Option<String>,
    extraction_version: Option<String>,
    #[serde(alias = "artifact_reference")]
    capture_artifact_reference: Option<String>,
    coverage: Option<String>,
    captured_at: Option<String>,
    locators: Option<Value>,
}

async fn create_source_content(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CreateSourceContentRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let key = idempotency_key(&headers, true, &actor)?.expect("required idempotency key");
    let locators = input.locators.unwrap_or_else(|| json!({}));
    if !locators.is_object() {
        return Err(ApiError::BadRequest(
            "locators must be a JSON object".into(),
        ));
    }
    let content = db::append_source_content(
        &state.pool,
        &actor,
        id,
        NewSourceContent {
            expected_revision: input.expected_revision,
            content_kind: allowed(input.content_kind, "content_kind", SOURCE_CONTENT_KINDS)?,
            normalized_text: required_preserved_text(
                input.normalized_text,
                "normalized_text",
                10_000_000,
            )?,
            language: optional_text(input.language, "language", 35)?,
            extraction_method: optional_text(input.extraction_method, "extraction_method", 200)?,
            extraction_version: optional_text(input.extraction_version, "extraction_version", 100)?,
            capture_artifact_reference: optional_text(
                input.capture_artifact_reference,
                "capture_artifact_reference",
                1000,
            )?,
            coverage: allowed(
                input.coverage.unwrap_or_else(|| "unknown".to_owned()),
                "coverage",
                &["complete", "partial", "unknown"],
            )?,
            captured_at: parse_optional_timestamp(input.captured_at, "captured_at")?,
            locators,
        },
        &key,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(json!({"data":content}))))
}

#[derive(Debug, Deserialize)]
struct NoteListQuery {
    q: Option<String>,
    cursor: Option<Uuid>,
    limit: Option<i64>,
}

async fn note_page(state: &AppState, query: NoteListQuery) -> Result<Value, ApiError> {
    let limit = bounded_limit(query.limit);
    let mut items = db::list_notes(
        &state.pool,
        db::NoteListFilter {
            query: optional_text(query.q, "q", 1000)?,
            cursor: query.cursor,
            limit: limit + 1,
        },
    )
    .await?;
    let next_cursor = if items.len() as i64 > limit {
        items.pop();
        items.last().map(|item| item.object_id)
    } else {
        None
    };
    Ok(json!({"items":items,"next_cursor":next_cursor}))
}

async fn list_notes(
    State(state): State<AppState>,
    Query(query): Query<NoteListQuery>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({"data":note_page(&state,query).await?})))
}

async fn search_notes(
    State(state): State<AppState>,
    Query(mut query): Query<NoteListQuery>,
) -> Result<Json<Value>, ApiError> {
    query.q = Some(required_text(query.q.unwrap_or_default(), "q", 1000)?);
    Ok(Json(json!({"data":note_page(&state,query).await?})))
}

async fn read_note(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({"data":db::get_note(&state.pool,id).await?})))
}

#[derive(Debug, Deserialize)]
struct CreateNoteRequest {
    title: String,
    description: String,
    content: String,
    #[serde(default = "default_note_format")]
    content_format: String,
    provenance: Option<Value>,
}

fn default_note_format() -> String {
    "markdown".to_owned()
}

async fn create_note(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    headers: HeaderMap,
    Json(input): Json<CreateNoteRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let key = idempotency_key(&headers, true, &actor)?.expect("required idempotency key");
    let title = required_text(input.title, "title", 300)?;
    let note = db::create_note(
        &state.pool,
        &actor,
        NewNote {
            description: crate::domain::object_description(&title, input.description)?,
            title,
            provenance: provenance(input.provenance)?,
            content: required_text(input.content, "content", 100_000)?,
            content_format: allowed(input.content_format, "content_format", NOTE_CONTENT_FORMATS)?,
        },
        &key,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(json!({"data":note}))))
}

#[derive(Debug, Deserialize)]
struct UpdateNoteRequest {
    expected_revision: i64,
    title: Option<String>,
    description: Option<String>,
    content: Option<String>,
    content_format: Option<String>,
    protected: Option<bool>,
}

async fn update_note(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<UpdateNoteRequest>,
) -> Result<Json<Value>, ApiError> {
    let title = input
        .title
        .map(|value| required_text(value, "title", 300))
        .transpose()?;
    let description = match (title.as_deref(), input.description) {
        (Some(title), Some(value)) => Some(crate::domain::object_description(title, value)?),
        (None, Some(value)) => Some(required_text(value, "description", 2000)?),
        (_, None) => None,
    };
    let note = db::update_note(
        &state.pool,
        &actor,
        id,
        input.expected_revision,
        db::NoteChanges {
            title,
            description,
            protected: input.protected,
            content: input
                .content
                .map(|value| required_text(value, "content", 100_000))
                .transpose()?,
            content_format: input
                .content_format
                .map(|value| allowed(value, "content_format", NOTE_CONTENT_FORMATS))
                .transpose()?,
        },
        idempotency_key(&headers, false, &actor)?.as_deref(),
    )
    .await?;
    Ok(Json(json!({"data":note})))
}

#[derive(Debug, Deserialize)]
struct ObjectListQuery {
    q: Option<String>,
    kind: Option<String>,
    lifecycle: Option<String>,
    cursor: Option<Uuid>,
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
            cursor: query.cursor,
            limit: bounded_object_limit(query.limit),
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
    entity_kind: Option<String>,
    happened_at: Option<String>,
}

async fn create_object(
    State(state): State<AppState>,
    Extension(actor): Extension<ActorContext>,
    headers: HeaderMap,
    Json(input): Json<CreateObjectRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let key = idempotency_key(&headers, true, &actor)?.expect("required idempotency key");
    let kind = allowed(input.kind, "kind", OBJECT_KINDS)?;
    if !matches!(kind.as_str(), "chat" | "entity" | "memory") {
        return Err(ApiError::BadRequest(format!(
            "use the typed endpoint to create a {kind}"
        )));
    }
    let entity_kind = input
        .entity_kind
        .map(|value| {
            allowed(
                value,
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
        })
        .transpose()?;
    let happened_at = parse_optional_timestamp(input.happened_at, "happened_at")?;
    if (kind == "entity") != entity_kind.is_some() || (kind == "memory") != happened_at.is_some() {
        return Err(ApiError::BadRequest(
            "entity_kind is required only for Entities and happened_at only for Memories".into(),
        ));
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
            entity_kind,
            happened_at,
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
                .map(|value| required_text(value, "description", 2000))
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
    #[serde(alias = "agent_eligible")]
    agent_suitable: Option<bool>,
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
            agent_suitable: query.agent_suitable,
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
    #[serde(alias = "agent_eligible")]
    agent_suitable: bool,
    blocked_reason: Option<String>,
    due_at: Option<String>,
    github_issue_url: Option<String>,
    brief_markdown: Option<String>,
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
    let status = allowed(input.status, "status", TASK_STATUSES)?;
    let blocked_reason = optional_text(input.blocked_reason, "blocked_reason", 2000)?;
    if (status == "blocked") != blocked_reason.is_some() {
        return Err(ApiError::BadRequest(
            "blocked_reason is required exactly when status is blocked".into(),
        ));
    }
    let task = db::create_task(
        &state.pool,
        &actor,
        NewTask {
            title,
            description,
            provenance: provenance(input.provenance)?,
            status: status.clone(),
            priority: allowed(input.priority, "priority", TASK_PRIORITIES)?,
            owner_object_id: input.owner_object_id,
            agent_suitable: input.agent_suitable,
            blocked_reason,
            due_at: parse_due_at(input.due_at)?,
            completed_at: (status == "done").then(OffsetDateTime::now_utc),
            github_issue_url: github_issue_url(input.github_issue_url)?,
            brief_markdown: optional_text(input.brief_markdown, "brief_markdown", 100_000)?,
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
    #[serde(alias = "agent_eligible")]
    agent_suitable: Option<bool>,
    blocked_reason: Option<String>,
    #[serde(default)]
    clear_blocked_reason: bool,
    due_at: Option<String>,
    #[serde(default)]
    clear_due_at: bool,
    github_issue_url: Option<String>,
    #[serde(default)]
    clear_github_issue_url: bool,
    brief_markdown: Option<String>,
    #[serde(default)]
    clear_brief_markdown: bool,
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
    let status = input
        .status
        .map(|value| allowed(value, "status", TASK_STATUSES))
        .transpose()?;
    let blocked_reason = nullable_change(
        optional_text(input.blocked_reason, "blocked_reason", 2000)?,
        input.clear_blocked_reason,
        "blocked_reason",
    )?;
    if status.as_deref() == Some("blocked") && blocked_reason.as_ref().is_none_or(Option::is_none) {
        return Err(ApiError::BadRequest(
            "blocked_reason is required when status is blocked".into(),
        ));
    }
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
                .map(|value| required_text(value, "description", 2000))
                .transpose()?,
            provenance: input
                .provenance
                .map(|value| provenance(Some(value)))
                .transpose()?,
            protected: input.protected,
            status: status.clone(),
            priority: input
                .priority
                .map(|value| allowed(value, "priority", TASK_PRIORITIES))
                .transpose()?,
            owner_object_id,
            agent_suitable: input.agent_suitable,
            blocked_reason,
            due_at,
            completed_at: status.as_deref().map(|value| {
                if value == "done" {
                    Some(OffsetDateTime::now_utc())
                } else {
                    None
                }
            }),
            github_issue_url: nullable_change(
                github_issue_url(input.github_issue_url)?,
                input.clear_github_issue_url,
                "github_issue_url",
            )?,
            brief_markdown: nullable_change(
                optional_text(input.brief_markdown, "brief_markdown", 100_000)?,
                input.clear_brief_markdown,
                "brief_markdown",
            )?,
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
    parse_optional_timestamp(value, "due_at")
}

fn parse_optional_timestamp(
    value: Option<String>,
    field: &'static str,
) -> Result<Option<OffsetDateTime>, ApiError> {
    value
        .map(|value| {
            OffsetDateTime::parse(value.trim(), &Rfc3339)
                .map_err(|_| ApiError::BadRequest(format!("{field} must be RFC 3339")))
        })
        .transpose()
}

fn inferred_publication_precision(timestamp: OffsetDateTime) -> String {
    let utc = timestamp.to_offset(time::UtcOffset::UTC);
    if utc.hour() == 0 && utc.minute() == 0 && utc.second() == 0 && utc.nanosecond() == 0 {
        "day"
    } else {
        "instant"
    }
    .to_owned()
}

fn bounded_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(50).clamp(1, 100)
}

fn bounded_object_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(50).clamp(1, 500)
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
    Schema(schema::SchemaError),
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

impl From<schema::SchemaError> for ApiError {
    fn from(value: schema::SchemaError) -> Self {
        Self::Schema(value)
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
            Self::Db(DbError::Invalid(message)) => {
                (StatusCode::UNPROCESSABLE_ENTITY, "invalid_request", message)
            }
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
            Self::Schema(schema::SchemaError::UnsafeDatabase(database)) => (
                {
                    tracing::warn!(%database, "schema inspection refused for unexpected database");
                    StatusCode::FORBIDDEN
                },
                "forbidden",
                "Schema inspection is unavailable for this database.".to_owned(),
            ),
            Self::Schema(schema::SchemaError::UnknownTable) => (
                StatusCode::NOT_FOUND,
                "not_found",
                "Table is not registered for schema inspection.".to_owned(),
            ),
            Self::Schema(schema::SchemaError::InvalidCursor) => (
                StatusCode::BAD_REQUEST,
                "invalid_cursor",
                "The row cursor is invalid.".to_owned(),
            ),
            Self::Schema(schema::SchemaError::StaleCursor) => (
                StatusCode::CONFLICT,
                "schema_changed",
                "The schema changed; reload the table before continuing.".to_owned(),
            ),
            Self::Schema(schema::SchemaError::InvalidFocus) => (
                StatusCode::BAD_REQUEST,
                "invalid_focus",
                "The focused row lookup is invalid.".to_owned(),
            ),
            Self::Schema(schema::SchemaError::Sqlx(error)) => {
                tracing::error!(%error, "schema inspection failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "The schema could not be inspected.".to_owned(),
                )
            }
            Self::Schema(schema::SchemaError::Json(error)) => {
                tracing::error!(%error, "schema serialization failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "The schema could not be inspected.".to_owned(),
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

#[cfg(test)]
mod tests {
    use super::{bounded_limit, bounded_object_limit, inferred_publication_precision};
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    #[test]
    fn object_lists_allow_the_full_local_workspace_without_widening_other_lists() {
        assert_eq!(bounded_object_limit(Some(500)), 500);
        assert_eq!(bounded_object_limit(Some(501)), 500);
        assert_eq!(bounded_limit(Some(500)), 100);
    }

    #[test]
    fn legacy_publication_timestamps_receive_deterministic_precision() {
        let day = OffsetDateTime::parse("2026-08-30T00:00:00Z", &Rfc3339).unwrap();
        let instant = OffsetDateTime::parse("2026-08-30T00:00:01Z", &Rfc3339).unwrap();
        assert_eq!(inferred_publication_precision(day), "day");
        assert_eq!(inferred_publication_precision(instant), "instant");
    }
}

use std::sync::Arc;

use axum::{
    Extension, Json, Router,
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::Response,
    routing::{patch, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use subtle::ConstantTimeEq;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::{
    api::{ApiError, AppState},
    db::{self, ConnectionChanges, NewConnection, SourceChanges},
    domain::{
        ActorContext, CONNECTION_KINDS, SOURCE_KINDS, ValidationError, allowed, optional_text,
        provenance, required_text,
    },
};

const WORKFLOW_PRINCIPAL: &str = "workflow-enyu-context-mutation";

#[derive(Clone)]
struct MutationState {
    app: AppState,
    token: Arc<String>,
}

pub fn router(app: AppState, token: String) -> Router {
    let state = MutationState {
        app,
        token: Arc::new(token),
    };
    Router::new()
        .route("/healthz", axum::routing::get(health))
        .route("/readyz", axum::routing::get(ready))
        .route("/api/v2/sources/{id}", patch(edit_source))
        .route("/api/v2/connections", post(connect))
        .route("/api/v2/connections/{id}", patch(edit_connection))
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
    let principal = required_header(request.headers(), "x-centaur-principal-id")?;
    if principal != WORKFLOW_PRINCIPAL {
        return Err(ApiError::Forbidden(
            "only the Enyu Context-mutation workflow may use this listener".into(),
        ));
    }
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

fn idempotency_key(headers: &HeaderMap) -> Result<String, ApiError> {
    let value = required_header(headers, "idempotency-key")?;
    if value.len() > 200 {
        return Err(ApiError::BadRequest(
            "Idempotency-Key must be at most 200 characters".into(),
        ));
    }
    Ok(value)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EditSourceRequest {
    expected_revision: i64,
    title: Option<String>,
    description: Option<String>,
    source_kind: Option<String>,
    canonical_uri: Option<Option<String>>,
    byline: Option<Option<String>>,
    publisher: Option<Option<String>>,
    published_at: Option<Option<String>>,
    published_at_precision: Option<Option<String>>,
    last_accessed_at: Option<Option<String>>,
    original_language: Option<Option<String>>,
    original_media_type: Option<Option<String>>,
    original_artifact_reference: Option<Option<String>>,
}

async fn edit_source(
    State(state): State<MutationState>,
    Extension(actor): Extension<ActorContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<EditSourceRequest>,
) -> Result<Json<Value>, ApiError> {
    let key = idempotency_key(&headers)?;
    let published_at = optional_timestamp(input.published_at, "published_at")?;
    let last_accessed_at = optional_timestamp(input.last_accessed_at, "last_accessed_at")?;
    if input.expected_revision < 1 {
        return Err(ApiError::BadRequest(
            "expected_revision must be positive".into(),
        ));
    }
    let changes = SourceChanges {
        title: input
            .title
            .map(|value| required_text(value, "title", 300))
            .transpose()?,
        description: input
            .description
            .map(|value| required_text(value, "description", 2000))
            .transpose()?,
        source_kind: input
            .source_kind
            .map(|value| allowed(value, "source_kind", SOURCE_KINDS))
            .transpose()?,
        canonical_uri: optional_uri(input.canonical_uri)?,
        byline: optional_nested_text(input.byline, "byline", 500)?,
        publisher: optional_nested_text(input.publisher, "publisher", 300)?,
        published_at,
        published_at_precision: input
            .published_at_precision
            .map(|value| {
                value
                    .map(|item| {
                        allowed(
                            item,
                            "published_at_precision",
                            &["instant", "day", "month", "year"],
                        )
                    })
                    .transpose()
            })
            .transpose()?,
        last_accessed_at,
        original_language: optional_nested_text(input.original_language, "original_language", 35)?,
        original_media_type: optional_nested_text(
            input.original_media_type,
            "original_media_type",
            255,
        )?,
        original_artifact_reference: optional_nested_text(
            input.original_artifact_reference,
            "original_artifact_reference",
            1000,
        )?,
        provenance: None,
        protected: None,
        archive: false,
    };
    if source_changes_empty(&changes) {
        return Err(ApiError::BadRequest("Source edit has no changes".into()));
    }
    let source = db::update_source(
        &state.app.pool,
        &actor,
        id,
        input.expected_revision,
        changes,
        Some(&key),
    )
    .await?;
    Ok(Json(json!({"data":source,"status":"updated"})))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectRequest {
    source_object_id: Uuid,
    kind: String,
    target_object_id: Uuid,
    description: String,
    #[serde(default)]
    provenance: Option<Value>,
}

async fn connect(
    State(state): State<MutationState>,
    Extension(actor): Extension<ActorContext>,
    headers: HeaderMap,
    Json(input): Json<ConnectRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if input.source_object_id == input.target_object_id {
        return Err(ValidationError::SelfConnection.into());
    }
    let key = idempotency_key(&headers)?;
    let result = db::create_or_reuse_connection(
        &state.app.pool,
        &actor,
        NewConnection {
            source_object_id: input.source_object_id,
            kind: allowed(input.kind, "connection kind", CONNECTION_KINDS)?,
            target_object_id: input.target_object_id,
            description: required_text(input.description, "description", 1000)?,
            provenance: provenance(input.provenance)?,
            protected: false,
        },
        &key,
    )
    .await?;
    let status = if result.reused {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((
        status,
        Json(json!({
            "data": {
                "record": result.connection,
                "status": if result.reused { "reused" } else { "created" },
                "reused": result.reused,
            },
        })),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EditConnectionRequest {
    expected_revision: i64,
    kind: Option<String>,
    description: Option<String>,
}

async fn edit_connection(
    State(state): State<MutationState>,
    Extension(actor): Extension<ActorContext>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<EditConnectionRequest>,
) -> Result<Json<Value>, ApiError> {
    if input.expected_revision < 1 {
        return Err(ApiError::BadRequest(
            "expected_revision must be positive".into(),
        ));
    }
    let changes = ConnectionChanges {
        kind: input
            .kind
            .map(|value| allowed(value, "connection kind", CONNECTION_KINDS))
            .transpose()?,
        description: input
            .description
            .map(|value| required_text(value, "description", 1000))
            .transpose()?,
        provenance: None,
        protected: None,
    };
    if changes.kind.is_none() && changes.description.is_none() {
        return Err(ApiError::BadRequest(
            "Connection edit has no changes".into(),
        ));
    }
    let key = idempotency_key(&headers)?;
    let connection = db::update_connection(
        &state.app.pool,
        &actor,
        id,
        input.expected_revision,
        changes,
        Some(&key),
    )
    .await?;
    Ok(Json(json!({"data":connection,"status":"updated"})))
}

fn optional_nested_text(
    value: Option<Option<String>>,
    field: &'static str,
    max: usize,
) -> Result<Option<Option<String>>, ApiError> {
    value
        .map(|item| optional_text(item, field, max))
        .transpose()
        .map_err(Into::into)
}

fn optional_uri(value: Option<Option<String>>) -> Result<Option<Option<String>>, ApiError> {
    value
        .map(|item| {
            item.map(|raw| {
                let mut url = reqwest::Url::parse(raw.trim()).map_err(|_| {
                    ApiError::BadRequest("canonical_uri must be a valid HTTP URL".into())
                })?;
                if !matches!(url.scheme(), "http" | "https") {
                    return Err(ApiError::BadRequest(
                        "canonical_uri must use HTTP or HTTPS".into(),
                    ));
                }
                url.set_fragment(None);
                Ok(url.to_string())
            })
            .transpose()
        })
        .transpose()
}

fn optional_timestamp(
    value: Option<Option<String>>,
    field: &'static str,
) -> Result<Option<Option<OffsetDateTime>>, ApiError> {
    value
        .map(|item| {
            item.map(|raw| {
                OffsetDateTime::parse(raw.trim(), &Rfc3339)
                    .map_err(|_| ApiError::BadRequest(format!("{field} must be RFC 3339")))
            })
            .transpose()
        })
        .transpose()
}

fn source_changes_empty(changes: &SourceChanges) -> bool {
    changes.title.is_none()
        && changes.description.is_none()
        && changes.source_kind.is_none()
        && changes.canonical_uri.is_none()
        && changes.byline.is_none()
        && changes.publisher.is_none()
        && changes.published_at.is_none()
        && changes.published_at_precision.is_none()
        && changes.last_accessed_at.is_none()
        && changes.original_language.is_none()
        && changes.original_media_type.is_none()
        && changes.original_artifact_reference.is_none()
}

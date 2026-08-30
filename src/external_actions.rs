use std::{collections::HashSet, sync::Arc};

use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, Request, State},
    http::{HeaderMap, header},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use subtle::ConstantTimeEq;
use time::OffsetDateTime;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::{api::AppState, domain::ActorContext, intake::IntakeError};

const CONTRACT_VERSION: &str = "centaur-context-external-action-v1";
const MAX_BODY_BYTES: usize = 32 * 1024;

#[derive(Clone)]
struct ExternalActionState {
    app: AppState,
    token: Arc<String>,
    allowed_principals: Arc<HashSet<String>>,
}

#[derive(Clone, Debug, sqlx::FromRow, Serialize)]
pub struct ExternalAction {
    pub object_id: Uuid,
    pub provider: String,
    pub action_kind: String,
    pub external_key: String,
    pub state: String,
    pub metadata: Value,
    pub revision: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReserveRequest {
    version: String,
    idempotency_key: String,
    provider: String,
    action_kind: String,
    external_key: String,
    title: String,
    summary: String,
    #[serde(default)]
    metadata: Map<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EventRequest {
    version: String,
    idempotency_key: String,
    event_type: String,
    expected_state: Option<String>,
    #[serde(default)]
    metadata: Map<String, Value>,
}

pub fn router(app: AppState, token: String, allowed_principals: HashSet<String>) -> Router {
    let state = ExternalActionState {
        app,
        token: Arc::new(token),
        allowed_principals: Arc::new(allowed_principals),
    };
    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .route("/api/v1/external-actions/reserve", post(reserve))
        .route("/api/v1/external-actions/{id}", get(status))
        .route("/api/v1/external-actions/{id}/events", post(append_event))
        .with_state(state.clone())
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(middleware::from_fn_with_state(state, auth))
        .layer(TraceLayer::new_for_http())
}

async fn health() -> Json<Value> {
    Json(json!({"ok":true}))
}

async fn ready(State(state): State<ExternalActionState>) -> Result<Json<Value>, IntakeError> {
    crate::db::ready(&state.app.pool).await?;
    Ok(Json(json!({"ok":true,"ready":true})))
}

async fn auth(
    State(state): State<ExternalActionState>,
    mut request: Request,
    next: Next,
) -> Result<Response, IntakeError> {
    let bearer = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(IntakeError::Unauthorized)?;
    let expected = state.token.as_bytes();
    let supplied = bearer.as_bytes();
    if expected.len() != supplied.len() || expected.ct_eq(supplied).unwrap_u8() != 1 {
        return Err(IntakeError::Unauthorized);
    }
    let principal = required_header(request.headers(), "x-centaur-principal-id")?;
    if !state.allowed_principals.contains(&principal) {
        return Err(IntakeError::Forbidden(
            "principal is not allowed to use the External-action listener".into(),
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

async fn reserve(
    State(state): State<ExternalActionState>,
    Extension(actor): Extension<ActorContext>,
    Json(request): Json<ReserveRequest>,
) -> Result<Json<Value>, IntakeError> {
    version(&request.version)?;
    let idempotency_key = bounded_token(&request.idempotency_key, "idempotency_key", 128)?;
    let provider = bounded_token(&request.provider, "provider", 80)?;
    let action_kind = bounded_token(&request.action_kind, "action_kind", 80)?;
    let external_key = bounded_token(&request.external_key, "external_key", 128)?;
    let title = bounded_text(&request.title, "title", 200)?;
    let summary = bounded_text(&request.summary, "summary", 500)?;
    let metadata = safe_metadata(request.metadata)?;

    if let Some(object_id) = idempotent_entity(&state.app, &actor, &idempotency_key).await? {
        let action = get_action(&state.app, object_id).await?;
        if action.provider != provider
            || action.action_kind != action_kind
            || action.external_key != external_key
        {
            return Err(IntakeError::Conflict(
                "idempotency key belongs to a different External action".into(),
            ));
        }
        return Ok(Json(json!({"data":action,"idempotent":true})));
    }

    let object_id = Uuid::new_v4();
    let mut tx = state.app.pool.begin().await?;
    sqlx::query(
        r#"INSERT INTO objects
           (id,kind,title,description,protected,created_by_type,created_by_id,
            updated_by_type,updated_by_id,provenance)
           VALUES ($1,'external_action',$2,$3,true,$4,$5,$4,$5,$6)"#,
    )
    .bind(object_id)
    .bind(title)
    .bind(summary)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(json!({"source_type":"external_action_listener"}))
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"INSERT INTO external_actions
           (object_id,provider,action_kind,external_key,metadata)
           VALUES ($1,$2,$3,$4,$5)"#,
    )
    .bind(object_id)
    .bind(&provider)
    .bind(&action_kind)
    .bind(&external_key)
    .bind(&metadata)
    .execute(&mut *tx)
    .await?;
    insert_event(
        &mut tx,
        &actor,
        object_id,
        "reserved",
        &idempotency_key,
        1,
        metadata,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"data":get_action(&state.app, object_id).await?,"idempotent":false}),
    ))
}

async fn append_event(
    State(state): State<ExternalActionState>,
    Extension(actor): Extension<ActorContext>,
    Path(object_id): Path<Uuid>,
    Json(request): Json<EventRequest>,
) -> Result<Json<Value>, IntakeError> {
    version(&request.version)?;
    let idempotency_key = bounded_token(&request.idempotency_key, "idempotency_key", 128)?;
    let event_type = bounded_token(&request.event_type, "event_type", 80)?;
    let metadata = safe_metadata(request.metadata)?;
    if let Some(existing_id) = idempotent_entity(&state.app, &actor, &idempotency_key).await? {
        if existing_id != object_id {
            return Err(IntakeError::Conflict(
                "idempotency key belongs to a different External action".into(),
            ));
        }
        return Ok(Json(
            json!({"data":get_action(&state.app, object_id).await?,"idempotent":true}),
        ));
    }

    let current = get_action(&state.app, object_id).await?;
    if request
        .expected_state
        .as_deref()
        .is_some_and(|expected| expected != current.state)
    {
        return Err(IntakeError::Conflict(
            "External action state changed".into(),
        ));
    }
    let next_state = transition(&current.state, &event_type)?;
    let mut tx = state.app.pool.begin().await?;
    let revision: i64 = sqlx::query_scalar(
        r#"UPDATE objects SET revision=revision+1,updated_by_type=$2,updated_by_id=$3,
           updated_at=now() WHERE id=$1 RETURNING revision"#,
    )
    .bind(object_id)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("UPDATE external_actions SET state=$2,updated_at=now() WHERE object_id=$1")
        .bind(object_id)
        .bind(next_state)
        .execute(&mut *tx)
        .await?;
    insert_event(
        &mut tx,
        &actor,
        object_id,
        &event_type,
        &idempotency_key,
        revision,
        metadata,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"data":get_action(&state.app, object_id).await?,"idempotent":false}),
    ))
}

async fn status(
    State(state): State<ExternalActionState>,
    Path(object_id): Path<Uuid>,
) -> Result<Json<Value>, IntakeError> {
    Ok(Json(
        json!({"data":get_action(&state.app, object_id).await?}),
    ))
}

async fn get_action(app: &AppState, object_id: Uuid) -> Result<ExternalAction, IntakeError> {
    sqlx::query_as(
        r#"SELECT e.object_id,e.provider,e.action_kind,e.external_key,e.state,e.metadata,
                  o.revision,e.created_at,e.updated_at
           FROM external_actions e JOIN objects o ON o.id=e.object_id
           WHERE e.object_id=$1"#,
    )
    .bind(object_id)
    .fetch_optional(&app.pool)
    .await?
    .ok_or_else(|| IntakeError::BadRequest("External action not found".into()))
}

async fn idempotent_entity(
    app: &AppState,
    actor: &ActorContext,
    key: &str,
) -> Result<Option<Uuid>, IntakeError> {
    Ok(sqlx::query_scalar(
        "SELECT entity_id FROM object_events WHERE actor_type=$1 AND actor_id=$2 AND idempotency_key=$3",
    )
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(key)
    .fetch_optional(&app.pool)
    .await?)
}

async fn insert_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    actor: &ActorContext,
    object_id: Uuid,
    event_type: &str,
    idempotency_key: &str,
    revision: i64,
    metadata: Value,
) -> Result<(), IntakeError> {
    sqlx::query(
        r#"INSERT INTO object_events
           (id,entity_type,entity_id,object_id,action,actor_type,actor_id,
            centaur_thread_key,centaur_execution_id,idempotency_key,to_revision,changes)
           VALUES ($1,'external_action',$2,$2,'external_action_event',$3,$4,$5,$6,$7,$8,$9)"#,
    )
    .bind(Uuid::new_v4())
    .bind(object_id)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(&actor.centaur_thread_key)
    .bind(&actor.centaur_execution_id)
    .bind(idempotency_key)
    .bind(revision)
    .bind(json!({"event_type":event_type,"metadata":metadata}))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn version(value: &str) -> Result<(), IntakeError> {
    if value.trim() != CONTRACT_VERSION {
        return Err(IntakeError::BadRequest(format!(
            "version must be {CONTRACT_VERSION}"
        )));
    }
    Ok(())
}

fn bounded_token(value: &str, field: &str, limit: usize) -> Result<String, IntakeError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > limit
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(IntakeError::BadRequest(format!("{field} is invalid")));
    }
    Ok(value.to_owned())
}

fn bounded_text(value: &str, field: &str, limit: usize) -> Result<String, IntakeError> {
    let value = value.trim();
    if value.is_empty() || value.len() > limit {
        return Err(IntakeError::BadRequest(format!("{field} is invalid")));
    }
    Ok(value.to_owned())
}

fn safe_metadata(value: Map<String, Value>) -> Result<Value, IntakeError> {
    let value = Value::Object(value);
    if serde_json::to_vec(&value)
        .map_err(|error| IntakeError::Internal(error.to_string()))?
        .len()
        > 16 * 1024
    {
        return Err(IntakeError::BadRequest("metadata is too large".into()));
    }
    validate_metadata(&value)?;
    Ok(value)
}

fn validate_metadata(value: &Value) -> Result<(), IntakeError> {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let key_lower = key.to_ascii_lowercase();
                let hash_only = key_lower.ends_with("_sha256")
                    || matches!(
                        key_lower.as_str(),
                        "recipient_count" | "recipient_set_hash" | "recipient_fingerprint"
                    );
                if !hash_only
                    && [
                        "recipient",
                        "email",
                        "address",
                        "body",
                        "html",
                        "text",
                        "token",
                        "secret",
                        "credential",
                    ]
                    .iter()
                    .any(|blocked| key_lower.contains(blocked))
                {
                    return Err(IntakeError::BadRequest(format!(
                        "metadata key {key} is not privacy-safe"
                    )));
                }
                validate_metadata(value)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                validate_metadata(value)?;
            }
        }
        Value::String(value) if value.len() > 500 || value.contains('@') => {
            return Err(IntakeError::BadRequest(
                "metadata string is not privacy-safe".into(),
            ));
        }
        _ => {}
    }
    Ok(())
}

fn transition(current: &str, event: &str) -> Result<&'static str, IntakeError> {
    let next = match (current, event) {
        ("reserved", "previewed") | ("previewed", "previewed") => "previewed",
        ("previewed", "approved") => "approved",
        ("approved", "attempt_started") => "attempting",
        ("attempting", "accepted") | ("reconciliation_required", "reconciled_accepted") => {
            "accepted"
        }
        ("attempting", "reconciliation_required") => "reconciliation_required",
        ("reconciliation_required", "reconciled_absent") => "approved",
        ("accepted", "delivered") | ("delivered", "delivered") => "delivered",
        ("suppressed", "suppressed") => "suppressed",
        (state, "suppressed") if !matches!(state, "delivered" | "failed") => "suppressed",
        (state, "failed") if !matches!(state, "delivered" | "suppressed") => "failed",
        _ => {
            return Err(IntakeError::Conflict(format!(
                "event {event} is invalid from state {current}"
            )));
        }
    };
    Ok(next)
}

fn required_header(headers: &HeaderMap, name: &'static str) -> Result<String, IntakeError> {
    optional_header(headers, name)?
        .ok_or_else(|| IntakeError::BadRequest(format!("{name} is required")))
}

fn optional_header(headers: &HeaderMap, name: &'static str) -> Result<Option<String>, IntakeError> {
    headers
        .get(name)
        .map(|value| {
            value
                .to_str()
                .map(str::trim)
                .map(str::to_owned)
                .map_err(|_| IntakeError::BadRequest(format!("{name} is invalid")))
                .and_then(|value| {
                    if value.is_empty() {
                        Err(IntakeError::BadRequest(format!("{name} must not be empty")))
                    } else {
                        Ok(value)
                    }
                })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::{safe_metadata, transition};
    use serde_json::{Map, json};

    #[test]
    fn transitions_fail_closed() {
        assert_eq!(transition("reserved", "previewed").unwrap(), "previewed");
        assert_eq!(transition("accepted", "delivered").unwrap(), "delivered");
        assert!(transition("reserved", "accepted").is_err());
        assert!(transition("delivered", "attempt_started").is_err());
    }

    #[test]
    fn metadata_rejects_recipient_and_body_data() {
        let mut safe = Map::new();
        safe.insert("recipient_count".into(), json!(1));
        safe.insert("rendered_html_sha256".into(), json!("a".repeat(64)));
        assert!(safe_metadata(safe).is_ok());
        let mut unsafe_value = Map::new();
        unsafe_value.insert("value".into(), json!("person@example.test"));
        assert!(safe_metadata(unsafe_value).is_err());
    }
}

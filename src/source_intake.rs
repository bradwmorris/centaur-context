use std::{collections::HashSet, sync::Arc};

use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::{
    api::AppState,
    domain::ActorContext,
    intake::{
        IntakeBatchRequest, IntakeConnection, IntakeError, IntakeObject, IntakeObjectRef,
        IntakeSource, IntakeSourceContent, PreparedBatch,
    },
};

const MAX_SOURCE_INTAKE_BODY_BYTES: usize = 1024 * 1024;
const CONTRACT_VERSION: &str = "centaur-context-source-intake-v1";

fn default_coverage() -> String {
    "unknown".to_owned()
}

#[derive(Clone)]
struct SourceIntakeState {
    app: AppState,
    token: Arc<String>,
}

pub fn router(app: AppState, token: String) -> Router {
    let state = SourceIntakeState {
        app,
        token: Arc::new(token),
    };
    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .route("/api/v1/source-intake/validate", post(validate_source))
        .route("/api/v1/source-intake/commit", post(commit_source))
        .route("/api/v1/source-intake/status", post(source_status))
        .with_state(state.clone())
        .layer(DefaultBodyLimit::max(MAX_SOURCE_INTAKE_BODY_BYTES))
        .layer(middleware::from_fn_with_state(state, source_intake_auth))
        .layer(TraceLayer::new_for_http())
}

async fn health() -> Json<Value> {
    Json(json!({"ok":true}))
}

async fn ready(State(state): State<SourceIntakeState>) -> Result<Json<Value>, IntakeError> {
    crate::db::ready(&state.app.pool).await?;
    Ok(Json(json!({"ok":true,"ready":true})))
}

async fn source_intake_auth(
    State(state): State<SourceIntakeState>,
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
    if principal != "workflow-enyu-source-ingestion" {
        return Err(IntakeError::Forbidden(
            "only the Enyu Source-ingestion workflow principal may use this listener".into(),
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceIntakeRequest {
    pub version: String,
    pub idempotency_key: String,
    pub source: SourceManifest,
    #[serde(default)]
    pub connections: Vec<SourceConnection>,
    pub originating_chat_object_id: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceManifest {
    pub title: String,
    pub description: String,
    pub source_kind: String,
    pub canonical_uri: Option<String>,
    pub byline: Option<String>,
    pub publisher: Option<String>,
    pub published_at: Option<String>,
    pub published_at_precision: Option<String>,
    #[serde(alias = "accessed_at")]
    pub last_accessed_at: Option<String>,
    #[serde(alias = "language")]
    pub original_language: Option<String>,
    #[serde(alias = "media_type")]
    pub original_media_type: Option<String>,
    #[serde(alias = "artifact_reference")]
    pub original_artifact_reference: Option<String>,
    pub capture_artifact_reference: Option<String>,
    pub content_kind: String,
    pub content: String,
    pub extraction_method: Option<String>,
    pub extraction_version: Option<String>,
    #[serde(default = "default_coverage")]
    pub coverage: String,
    pub captured_at: Option<String>,
    #[serde(default)]
    pub provenance: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceConnection {
    pub target_object_id: Uuid,
    pub kind: String,
    pub description: String,
    #[serde(default)]
    pub provenance: Option<Value>,
}

async fn prepared(
    state: &SourceIntakeState,
    request: SourceIntakeRequest,
) -> Result<PreparedBatch, IntakeError> {
    let batch = source_batch(request)?;
    let mut prepared = crate::intake::prepare_batch_for_app(&state.app, None, batch).await?;
    let source = prepared.request.objects[0]
        .source
        .as_ref()
        .expect("Source adapter");
    #[derive(sqlx::FromRow)]
    struct Conflict {
        object_id: Uuid,
        protected: bool,
        current_content_id: Option<Uuid>,
        content_sha256: Option<String>,
        provenance: Value,
        canonical_uri: Option<String>,
        same_batch: bool,
    }
    let conflicts: Vec<Conflict> = sqlx::query_as(
        r#"SELECT s.object_id,o.protected,s.current_content_id,sc.content_sha256,o.provenance,
                  s.canonical_uri,EXISTS(
                    SELECT 1 FROM object_events e
                    WHERE e.object_id=s.object_id AND e.changes->>'intake_batch_id'=$3
                  ) AS same_batch
           FROM sources s JOIN objects o ON o.id=s.object_id
           LEFT JOIN source_contents sc ON sc.id=s.current_content_id
           WHERE o.archived_at IS NULL
             AND (($1::text IS NOT NULL AND s.canonical_uri=$1) OR sc.content_sha256=$2)"#,
    )
    .bind(&source.canonical_uri)
    .bind(&prepared.request.source_contents[0].content_sha256)
    .bind(&prepared.request.batch_id)
    .fetch_all(&state.app.pool)
    .await?;
    let expected = prepared.object_ids["source"];
    let mut foreign = conflicts
        .into_iter()
        .filter(|conflict| conflict.object_id != expected);
    if let Some(conflict) = foreign.next() {
        if foreign.next().is_some() {
            return Err(IntakeError::Conflict(
                "canonical URI or exact content belongs to multiple Sources".into(),
            ));
        }
        let curator_placeholder = !conflict.protected
            && conflict.current_content_id.is_none()
            && conflict.content_sha256.is_none()
            && conflict.canonical_uri == source.canonical_uri
            && conflict
                .provenance
                .get("source_type")
                .and_then(Value::as_str)
                == Some("context_curator");
        if !conflict.same_batch && !curator_placeholder {
            return Err(IntakeError::Conflict(
                "canonical URI or exact content already belongs to a different Source".into(),
            ));
        }
        prepared.adopt_object("source", conflict.object_id)?;
    }
    Ok(prepared)
}

fn source_batch(mut request: SourceIntakeRequest) -> Result<IntakeBatchRequest, IntakeError> {
    if request.version.trim() != CONTRACT_VERSION {
        return Err(IntakeError::BadRequest(format!(
            "version must be {CONTRACT_VERSION}"
        )));
    }
    let batch_id = request.idempotency_key.trim().to_owned();
    if batch_id.is_empty() || batch_id.len() > 100 {
        return Err(IntakeError::BadRequest(
            "idempotency_key must contain between 1 and 100 characters".into(),
        ));
    }
    if request.source.content.len() > 750_000 {
        return Err(IntakeError::BadRequest(
            "Source content exceeds the 750000-byte intake limit".into(),
        ));
    }
    request.source.canonical_uri = request
        .source
        .canonical_uri
        .take()
        .map(|value| canonical_uri(&value))
        .transpose()?;
    let manifest_sha256 = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&request)
                .map_err(|error| IntakeError::Internal(error.to_string()))?
        )
    );
    let content_sha256 = format!("{:x}", Sha256::digest(request.source.content.as_bytes()));
    let mut connection_keys = HashSet::new();
    let mut connections = Vec::new();
    for (index, connection) in request.connections.into_iter().enumerate() {
        if connection.kind.trim().eq_ignore_ascii_case("related_to") {
            return Err(IntakeError::BadRequest(
                "related_to is not accepted for Source intake; provide a specific evidenced kind"
                    .into(),
            ));
        }
        let edge = (
            connection.target_object_id,
            connection.kind.trim().to_ascii_lowercase(),
        );
        if !connection_keys.insert(edge) {
            return Err(IntakeError::BadRequest(
                "Source intake contains a duplicate connection".into(),
            ));
        }
        connections.push(IntakeConnection {
            client_key: format!("source-connection-{index}"),
            source: IntakeObjectRef {
                client_key: Some("source".into()),
                object_id: None,
            },
            kind: connection.kind,
            target: IntakeObjectRef {
                client_key: None,
                object_id: Some(connection.target_object_id),
            },
            description: connection.description,
            protected: true,
            provenance: connection.provenance,
        });
    }
    if let Some(chat_id) = request.originating_chat_object_id {
        connections.push(IntakeConnection {
            client_key: "originating-chat".into(),
            source: IntakeObjectRef {
                client_key: None,
                object_id: Some(chat_id),
            },
            kind: "about".into(),
            target: IntakeObjectRef {
                client_key: Some("source".into()),
                object_id: None,
            },
            description: "This Slack conversation requested ingestion of the resulting Source."
                .into(),
            protected: true,
            provenance: Some(json!({
                "source_type":"enyu_workflow",
                "source_ref":batch_id,
            })),
        });
    }
    let source = request.source;
    let canonical_uri = source.canonical_uri.clone();
    Ok(IntakeBatchRequest {
        batch_id,
        manifest_sha256,
        objects: vec![IntakeObject {
            client_key: "source".into(),
            kind: "source".into(),
            title: source.title,
            description: source.description,
            protected: true,
            provenance: source.provenance.clone(),
            user_kind: None,
            entity_kind: None,
            source: Some(IntakeSource {
                source_kind: source.source_kind,
                canonical_uri: source.canonical_uri,
                byline: source.byline,
                publisher: source.publisher,
                published_at: source.published_at,
                published_at_precision: source.published_at_precision,
                last_accessed_at: source.last_accessed_at,
                original_language: source.original_language.clone(),
                original_media_type: source.original_media_type,
                original_artifact_reference: source.original_artifact_reference,
            }),
            note: None,
            theme: None,
        }],
        external_identities: Vec::new(),
        source_contents: vec![IntakeSourceContent {
            client_key: "source-content".into(),
            source: IntakeObjectRef {
                client_key: Some("source".into()),
                object_id: None,
            },
            content_kind: source.content_kind,
            normalized_text: source.content,
            content_sha256,
            language: source.original_language,
            extraction_method: source.extraction_method,
            extraction_version: source.extraction_version,
            capture_artifact_reference: source.capture_artifact_reference,
            coverage: source.coverage,
            captured_at: source.captured_at,
            locators: Some(json!({"canonical_uri":canonical_uri})),
        }],
        connections,
    })
}

fn canonical_uri(value: &str) -> Result<String, IntakeError> {
    let mut url = reqwest::Url::parse(value.trim())
        .map_err(|_| IntakeError::BadRequest("canonical_uri must be a valid HTTP URL".into()))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(IntakeError::BadRequest(
            "canonical_uri must use HTTP or HTTPS".into(),
        ));
    }
    url.set_fragment(None);
    Ok(url.to_string())
}

async fn validate_source(
    State(state): State<SourceIntakeState>,
    Json(request): Json<SourceIntakeRequest>,
) -> Result<Json<Value>, IntakeError> {
    let prepared = prepared(&state, request).await?;
    if crate::intake::status(&state.app.pool, &prepared.request.batch_id)
        .await?
        .is_some()
    {
        return Err(IntakeError::Conflict(
            "idempotency_key already has committed Source events; use status or retry commit"
                .into(),
        ));
    }
    Ok(Json(json!({"data":{
        "valid":true,
        "status":"validated",
        "idempotency_key":prepared.request.batch_id,
        "object_id":prepared.object_ids["source"],
        "payload_sha256":prepared.payload_sha256,
        "counts":prepared.counts(),
        "writes":0,
    }})))
}

async fn commit_source(
    State(state): State<SourceIntakeState>,
    Extension(actor): Extension<ActorContext>,
    Json(request): Json<SourceIntakeRequest>,
) -> Result<(StatusCode, Json<Value>), IntakeError> {
    let prepared = prepared(&state, request).await?;
    let existing = crate::intake::status(&state.app.pool, &prepared.request.batch_id).await?;
    if let Some(existing) = existing {
        if existing.manifest_sha256 != prepared.request.manifest_sha256
            || existing.payload_sha256 != prepared.payload_sha256
            || existing.event_count != prepared.event_count() as i64
        {
            return Err(IntakeError::Conflict(
                "idempotency_key is already committed with a different Source manifest".into(),
            ));
        }
        return Ok((
            StatusCode::OK,
            Json(source_response(&prepared, "replayed", true)),
        ));
    }
    crate::intake::write_batch(&state.app.pool, &actor, &prepared).await?;
    Ok((
        StatusCode::CREATED,
        Json(source_response(&prepared, "committed", false)),
    ))
}

fn source_response(prepared: &PreparedBatch, status: &str, replayed: bool) -> Value {
    json!({"data":{
        "status":status,
        "replayed":replayed,
        "idempotency_key":prepared.request.batch_id,
        "object_id":prepared.object_ids["source"],
        "payload_sha256":prepared.payload_sha256,
        "counts":prepared.counts(),
    }})
}

async fn source_status(
    State(state): State<SourceIntakeState>,
    Json(request): Json<SourceIntakeRequest>,
) -> Result<Json<Value>, IntakeError> {
    let prepared = prepared(&state, request).await?;
    let object_id = prepared.object_ids["source"];
    let Some(existing) = crate::intake::status(&state.app.pool, &prepared.request.batch_id).await?
    else {
        return Ok(Json(json!({"data":{
            "status":"not_committed",
            "ready":false,
            "idempotency_key":prepared.request.batch_id,
            "object_id":object_id,
        }})));
    };
    if existing.manifest_sha256 != prepared.request.manifest_sha256
        || existing.payload_sha256 != prepared.payload_sha256
    {
        return Err(IntakeError::Conflict(
            "idempotency_key is committed with a different Source manifest".into(),
        ));
    }
    let lexical_ready: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
            SELECT 1 FROM sources s
            JOIN source_contents c ON c.source_object_id=s.object_id AND c.id=s.current_content_id
            WHERE s.object_id=$1 AND c.normalized_text<>''
        )"#,
    )
    .bind(object_id)
    .fetch_one(&state.app.pool)
    .await?;
    let semantic_ready = if state.app.embeddings.is_none() {
        true
    } else {
        sqlx::query_scalar(
            r#"SELECT EXISTS(
                SELECT 1 FROM object_embeddings e JOIN objects o ON o.id=e.object_id
                WHERE o.id=$1 AND e.source_hash=object_embedding_source_hash(
                    e.format_version,o.kind,o.title,o.description
                )
            )"#,
        )
        .bind(object_id)
        .fetch_one(&state.app.pool)
        .await?
    };
    Ok(Json(json!({"data":{
        "status":if lexical_ready && semantic_ready {"ready"} else {"indexing"},
        "ready":lexical_ready && semantic_ready,
        "lexical_ready":lexical_ready,
        "semantic_ready":semantic_ready,
        "idempotency_key":prepared.request.batch_id,
        "object_id":object_id,
    }})))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> SourceIntakeRequest {
        SourceIntakeRequest {
            version: CONTRACT_VERSION.into(),
            idempotency_key: "workflow:run-1".into(),
            source: SourceManifest {
                title: "Example".into(),
                description: "A concrete Source used to verify permanent workflow intake.".into(),
                source_kind: "article".into(),
                canonical_uri: Some("https://example.test/source".into()),
                byline: None,
                publisher: None,
                published_at: None,
                published_at_precision: None,
                last_accessed_at: None,
                original_language: Some("en".into()),
                original_media_type: Some("text/plain".into()),
                original_artifact_reference: None,
                capture_artifact_reference: None,
                content_kind: "article_text".into(),
                content: "Captured source text".into(),
                extraction_method: Some("enyu-researcher".into()),
                extraction_version: Some("1".into()),
                coverage: "complete".into(),
                captured_at: None,
                provenance: Some(json!({"source_type":"enyu_workflow"})),
            },
            connections: Vec::new(),
            originating_chat_object_id: None,
        }
    }

    #[test]
    fn adapter_produces_one_protected_source_and_content() {
        let batch = source_batch(request()).unwrap();
        assert_eq!(batch.objects.len(), 1);
        assert_eq!(batch.objects[0].kind, "source");
        assert!(batch.objects[0].protected);
        assert_eq!(batch.source_contents.len(), 1);
        assert_eq!(batch.source_contents[0].content_sha256.len(), 64);
    }

    #[test]
    fn adapter_rejects_ambiguous_related_to_edges() {
        let mut input = request();
        input.connections.push(SourceConnection {
            target_object_id: Uuid::new_v4(),
            kind: "related_to".into(),
            description: "Ambiguous".into(),
            provenance: None,
        });
        assert!(source_batch(input).is_err());
    }

    #[test]
    fn adapter_normalizes_the_canonical_uri_before_hashing() {
        let mut input = request();
        input.source.canonical_uri = Some(" HTTPS://EXAMPLE.TEST:443/source#fragment ".into());
        let batch = source_batch(input).unwrap();
        assert_eq!(
            batch.objects[0]
                .source
                .as_ref()
                .unwrap()
                .canonical_uri
                .as_deref(),
            Some("https://example.test/source")
        );
    }

    #[test]
    fn v1_manifest_accepts_explicit_legacy_field_aliases() {
        let parsed: SourceIntakeRequest = serde_json::from_value(json!({
            "version": CONTRACT_VERSION,
            "idempotency_key": "legacy-run",
            "source": {
                "title": "Legacy fixture",
                "description": "A legacy v1 Source manifest retained to verify rollout compatibility.",
                "source_kind": "article",
                "canonical_uri": "https://example.test/legacy",
                "byline": null,
                "publisher": null,
                "published_at": null,
                "accessed_at": "2026-08-30T00:00:00Z",
                "language": "en",
                "media_type": "text/plain",
                "artifact_reference": "artifact:legacy",
                "capture_artifact_reference": null,
                "content_kind": "article_text",
                "content": "Legacy content",
                "extraction_method": "legacy-client",
                "extraction_version": "1",
                "captured_at": null,
                "provenance": {}
            },
            "connections": [],
            "originating_chat_object_id": null
        }))
        .unwrap();
        assert_eq!(
            parsed.source.last_accessed_at.as_deref(),
            Some("2026-08-30T00:00:00Z")
        );
        assert_eq!(parsed.source.original_language.as_deref(), Some("en"));
        assert_eq!(
            parsed.source.original_media_type.as_deref(),
            Some("text/plain")
        );
        assert_eq!(
            parsed.source.original_artifact_reference.as_deref(),
            Some("artifact:legacy")
        );
        assert_eq!(parsed.source.coverage, "unknown");
    }
}

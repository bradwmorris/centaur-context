use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use subtle::ConstantTimeEq;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::{
    api::AppState,
    domain::{
        ARTIFACT_CAPTURE_OUTCOMES, ActorContext, CONNECTION_KINDS, NOTE_CONTENT_FORMATS,
        SOURCE_KINDS, USER_KINDS, allowed, object_description, optional_text, provenance,
        required_text,
    },
};

const MAX_BATCH_RESOURCES: usize = 500;
const MAX_BATCH_BODY_BYTES: usize = 12 * 1024 * 1024;
const INTAKE_NAMESPACE: Uuid = Uuid::from_u128(0x5b18f36b_699f_4da2_8b3e_9e744a7b941d);

#[derive(Clone)]
struct IntakeState {
    app: AppState,
    token: Arc<String>,
    approved_manifest_sha256: Option<String>,
}

pub fn router(app: AppState, token: String, approved_manifest_sha256: Option<String>) -> Router {
    let state = IntakeState {
        app,
        token: Arc::new(token),
        approved_manifest_sha256,
    };
    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .route("/api/v2/intake/batches/validate", post(validate_batch))
        .route("/api/v2/intake/batches/commit", post(commit_batch))
        .route("/api/v2/intake/batches/{batch_id}", get(read_batch_status))
        .with_state(state.clone())
        .layer(DefaultBodyLimit::max(MAX_BATCH_BODY_BYTES))
        .layer(middleware::from_fn_with_state(state, intake_auth))
        .layer(TraceLayer::new_for_http())
}

async fn health() -> Json<Value> {
    Json(json!({"ok":true}))
}

async fn ready(State(state): State<IntakeState>) -> Result<Json<Value>, IntakeError> {
    crate::db::ready(&state.app.pool).await?;
    Ok(Json(json!({"ok":true,"ready":true})))
}

async fn intake_auth(
    State(state): State<IntakeState>,
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
pub struct IntakeBatchRequest {
    pub batch_id: String,
    pub manifest_sha256: String,
    #[serde(default)]
    pub objects: Vec<IntakeObject>,
    #[serde(default)]
    pub artifacts: Vec<IntakeArtifact>,
    #[serde(default)]
    pub connections: Vec<IntakeConnection>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeObject {
    pub client_key: String,
    pub kind: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub protected: bool,
    #[serde(default)]
    pub provenance: Option<Value>,
    #[serde(default)]
    pub user_kind: Option<String>,
    #[serde(default)]
    pub identities: Vec<IntakeIdentity>,
    #[serde(default)]
    pub entity_kind: Option<String>,
    #[serde(default)]
    pub source: Option<IntakeSource>,
    #[serde(default)]
    pub note: Option<IntakeNote>,
    #[serde(default)]
    pub theme: Option<IntakeTheme>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeTheme {
    pub slug: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeSource {
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
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeNote {
    pub content: String,
    pub content_format: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeIdentity {
    pub id: Option<Uuid>,
    pub provider: String,
    #[serde(default)]
    pub workspace_id: String,
    pub provider_user_id: String,
    pub display_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeArtifact {
    pub client_key: String,
    pub object: IntakeObjectRef,
    pub kind: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub uri: Option<String>,
    pub media_type: Option<String>,
    pub sha256: String,
    pub size_bytes: i64,
    pub language: Option<String>,
    pub captured_at: Option<String>,
    pub capture_outcome: String,
    pub capture_reason: Option<String>,
    pub expected_size_bytes: Option<i64>,
    #[serde(default)]
    pub metadata: Option<Value>,
    pub supersedes_artifact_id: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeConnection {
    pub client_key: String,
    pub source: IntakeObjectRef,
    pub kind: String,
    pub target: IntakeObjectRef,
    pub description: String,
    #[serde(default)]
    pub protected: bool,
    #[serde(default)]
    pub provenance: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeObjectRef {
    pub client_key: Option<String>,
    pub object_id: Option<Uuid>,
}

#[derive(Clone)]
pub(crate) struct PreparedBatch {
    pub(crate) request: IntakeBatchRequest,
    pub(crate) payload_sha256: String,
    pub(crate) object_ids: HashMap<String, Uuid>,
    content_ids: HashMap<String, Uuid>,
    connection_ids: HashMap<String, Uuid>,
    adopted_object_ids: HashSet<Uuid>,
}

impl PreparedBatch {
    pub(crate) fn event_count(&self) -> usize {
        self.request.objects.len() + self.request.artifacts.len() + self.request.connections.len()
    }

    pub(crate) fn counts(&self) -> Value {
        json!({
            "objects":self.request.objects.len(),
            "artifacts":self.request.artifacts.len(),
            "connections":self.request.connections.len(),
            "events":self.event_count(),
        })
    }

    fn id_map(&self) -> Value {
        json!({
            "objects":self.object_ids,
            "artifacts":self.content_ids,
            "connections":self.connection_ids,
        })
    }

    pub(crate) fn adopt_object(
        &mut self,
        client_key: &str,
        object_id: Uuid,
    ) -> Result<(), IntakeError> {
        let previous_id = self
            .object_ids
            .insert(client_key.to_owned(), object_id)
            .ok_or_else(|| {
                IntakeError::Internal(format!("unknown object client key {client_key}"))
            })?;
        self.adopted_object_ids.insert(object_id);
        for content in &mut self.request.artifacts {
            if content.object.object_id == Some(previous_id) {
                content.object.object_id = Some(object_id);
            }
        }
        for connection in &mut self.request.connections {
            if connection.source.object_id == Some(previous_id) {
                connection.source.object_id = Some(object_id);
            }
            if connection.target.object_id == Some(previous_id) {
                connection.target.object_id = Some(object_id);
            }
        }
        self.payload_sha256 = hex_sha256(
            &serde_json::to_vec(&self.request)
                .map_err(|error| IntakeError::Internal(error.to_string()))?,
        );
        Ok(())
    }

    pub(crate) fn reuse_object(
        &mut self,
        client_key: &str,
        object_id: Uuid,
    ) -> Result<(), IntakeError> {
        let previous_id = self
            .object_ids
            .insert(client_key.to_owned(), object_id)
            .ok_or_else(|| {
                IntakeError::Internal(format!("unknown object client key {client_key}"))
            })?;
        for connection in &mut self.request.connections {
            if connection.source.object_id == Some(previous_id) {
                connection.source.object_id = Some(object_id);
            }
            if connection.target.object_id == Some(previous_id) {
                connection.target.object_id = Some(object_id);
            }
        }
        self.request
            .objects
            .retain(|object| object.client_key != client_key);
        let removed_artifact_keys = self
            .request
            .artifacts
            .iter()
            .filter(|artifact| artifact.object.object_id == Some(previous_id))
            .map(|artifact| artifact.client_key.clone())
            .collect::<HashSet<_>>();
        self.request
            .artifacts
            .retain(|artifact| artifact.object.object_id != Some(previous_id));
        self.content_ids
            .retain(|key, _| !removed_artifact_keys.contains(key));
        self.refresh_payload_sha256()?;
        Ok(())
    }

    pub(crate) fn reuse_object_with_artifacts(
        &mut self,
        client_key: &str,
        object_id: Uuid,
        supersedes_artifact_id: Option<Uuid>,
    ) -> Result<(), IntakeError> {
        let previous_id = self
            .object_ids
            .insert(client_key.to_owned(), object_id)
            .ok_or_else(|| {
                IntakeError::Internal(format!("unknown object client key {client_key}"))
            })?;
        for artifact in &mut self.request.artifacts {
            if artifact.object.object_id == Some(previous_id) {
                artifact.object.object_id = Some(object_id);
                artifact.supersedes_artifact_id = supersedes_artifact_id;
            }
        }
        for connection in &mut self.request.connections {
            if connection.source.object_id == Some(previous_id) {
                connection.source.object_id = Some(object_id);
            }
            if connection.target.object_id == Some(previous_id) {
                connection.target.object_id = Some(object_id);
            }
        }
        self.request
            .objects
            .retain(|object| object.client_key != client_key);
        self.refresh_payload_sha256()?;
        Ok(())
    }

    pub(crate) fn artifact_id(&self, client_key: &str) -> Option<Uuid> {
        self.content_ids.get(client_key).copied()
    }

    pub(crate) fn discard_connections(
        &mut self,
        existing: &HashMap<(Uuid, String, Uuid), Uuid>,
    ) -> Result<(), IntakeError> {
        let discarded = self
            .request
            .connections
            .iter()
            .filter(|connection| {
                let edge = (
                    connection
                        .source
                        .object_id
                        .expect("resolved connection source"),
                    connection.kind.clone(),
                    connection
                        .target
                        .object_id
                        .expect("resolved connection target"),
                );
                existing.get(&edge).is_some_and(|existing_id| {
                    self.connection_ids.get(&connection.client_key) != Some(existing_id)
                })
            })
            .map(|connection| connection.client_key.clone())
            .collect::<HashSet<_>>();
        self.request
            .connections
            .retain(|connection| !discarded.contains(&connection.client_key));
        self.connection_ids
            .retain(|key, _| !discarded.contains(key));
        self.refresh_payload_sha256()?;
        Ok(())
    }

    fn refresh_payload_sha256(&mut self) -> Result<(), IntakeError> {
        self.payload_sha256 = hex_sha256(
            &serde_json::to_vec(&self.request)
                .map_err(|error| IntakeError::Internal(error.to_string()))?,
        );
        Ok(())
    }
}

async fn validate_batch(
    State(state): State<IntakeState>,
    Json(request): Json<IntakeBatchRequest>,
) -> Result<Json<Value>, IntakeError> {
    let prepared = prepare_batch(&state, request).await?;
    let existing = status(&state.app.pool, &prepared.request.batch_id).await?;
    if existing.is_some() {
        return Err(IntakeError::Conflict(
            "batch_id already has committed events; use status or retry commit".into(),
        ));
    }
    Ok(Json(json!({"data":{
        "batch_id":prepared.request.batch_id,
        "status":"validated",
        "manifest_sha256":prepared.request.manifest_sha256,
        "payload_sha256":prepared.payload_sha256,
        "counts":prepared.counts(),
        "id_map":prepared.id_map(),
        "writes":0,
    }})))
}

async fn commit_batch(
    State(state): State<IntakeState>,
    Extension(actor): Extension<ActorContext>,
    Json(request): Json<IntakeBatchRequest>,
) -> Result<(StatusCode, Json<Value>), IntakeError> {
    let prepared = prepare_batch(&state, request).await?;
    if let Some(existing) = status(&state.app.pool, &prepared.request.batch_id).await? {
        if existing.manifest_sha256 != prepared.request.manifest_sha256
            || existing.payload_sha256 != prepared.payload_sha256
            || existing.event_count != prepared.event_count() as i64
        {
            return Err(IntakeError::Conflict(
                "batch_id is already committed with a different manifest or payload".into(),
            ));
        }
        return Ok((
            StatusCode::OK,
            Json(batch_response(&prepared, "replayed", true)),
        ));
    }
    write_batch(&state.app.pool, &actor, &prepared).await?;
    let committed = status(&state.app.pool, &prepared.request.batch_id)
        .await?
        .ok_or_else(|| IntakeError::Internal("committed batch status is missing".into()))?;
    if committed.event_count != prepared.event_count() as i64 {
        return Err(IntakeError::Internal(
            "committed batch event count did not reconcile".into(),
        ));
    }
    Ok((
        StatusCode::CREATED,
        Json(batch_response(&prepared, "committed", false)),
    ))
}

fn batch_response(prepared: &PreparedBatch, status: &str, replayed: bool) -> Value {
    json!({"data":{
        "batch_id":prepared.request.batch_id,
        "status":status,
        "replayed":replayed,
        "manifest_sha256":prepared.request.manifest_sha256,
        "payload_sha256":prepared.payload_sha256,
        "counts":prepared.counts(),
        "id_map":prepared.id_map(),
    }})
}

async fn read_batch_status(
    State(state): State<IntakeState>,
    Path(batch_id): Path<String>,
) -> Result<Json<Value>, IntakeError> {
    validate_key(&batch_id, "batch_id", 100)?;
    let value = status(&state.app.pool, &batch_id).await?;
    Ok(Json(json!({"data":value.map(|status| json!({
        "batch_id":batch_id,
        "status":"committed",
        "manifest_sha256":status.manifest_sha256,
        "payload_sha256":status.payload_sha256,
        "event_count":status.event_count,
        "object_ids":status.object_ids,
    }))})))
}

async fn prepare_batch(
    state: &IntakeState,
    request: IntakeBatchRequest,
) -> Result<PreparedBatch, IntakeError> {
    prepare_batch_for_app(
        &state.app,
        state.approved_manifest_sha256.as_deref(),
        request,
    )
    .await
}

pub(crate) async fn prepare_batch_for_app(
    app: &AppState,
    approved_manifest_sha256: Option<&str>,
    mut request: IntakeBatchRequest,
) -> Result<PreparedBatch, IntakeError> {
    request.batch_id = validate_key(&request.batch_id, "batch_id", 100)?;
    request.manifest_sha256 = validate_hash(&request.manifest_sha256, "manifest_sha256")?;
    if approved_manifest_sha256.is_some_and(|approved| approved != request.manifest_sha256) {
        return Err(IntakeError::Forbidden(
            "manifest is not approved for this intake listener".into(),
        ));
    }
    let resources = request.objects.len() + request.artifacts.len() + request.connections.len();
    if resources == 0 || resources > MAX_BATCH_RESOURCES {
        return Err(IntakeError::BadRequest(format!(
            "batch must contain between 1 and {MAX_BATCH_RESOURCES} resources"
        )));
    }

    let mut all_keys = HashSet::new();
    let mut object_ids = HashMap::new();
    for object in &mut request.objects {
        object.client_key = unique_key(&mut all_keys, &object.client_key, "object.client_key")?;
        object.kind = allowed(
            object.kind.clone(),
            "kind",
            &["user", "entity", "source", "note", "theme"],
        )?;
        object.title = required_text(object.title.clone(), "title", 300)?;
        object.description = object_description(&object.title, object.description.clone())?;
        object.provenance = Some(provenance(object.provenance.take())?);
        match object.kind.as_str() {
            "user" => {
                object.user_kind = Some(allowed(
                    object.user_kind.clone().ok_or_else(|| {
                        IntakeError::BadRequest("user_kind is required for user Objects".into())
                    })?,
                    "user_kind",
                    USER_KINDS,
                )?);
                for (index, identity) in object.identities.iter_mut().enumerate() {
                    identity.id = Some(identity.id.unwrap_or_else(|| {
                        stable_id(
                            &request.batch_id,
                            "identity",
                            &format!("{}:{index}", object.client_key),
                        )
                    }));
                    identity.provider =
                        required_text(identity.provider.clone(), "identity.provider", 100)?;
                    identity.workspace_id = identity.workspace_id.trim().to_owned();
                    identity.provider_user_id = required_text(
                        identity.provider_user_id.clone(),
                        "identity.provider_user_id",
                        300,
                    )?;
                    identity.display_name =
                        optional_text(identity.display_name.take(), "identity.display_name", 500)?;
                }
                if object.entity_kind.is_some()
                    || object.source.is_some()
                    || object.note.is_some()
                    || object.theme.is_some()
                {
                    return Err(IntakeError::BadRequest(
                        "user Objects cannot contain source or note data".into(),
                    ));
                }
            }
            "entity" => {
                if !object.identities.is_empty() {
                    return Err(IntakeError::BadRequest(
                        "only User Objects accept identities".into(),
                    ));
                }
                object.entity_kind = Some(allowed(
                    object.entity_kind.clone().ok_or_else(|| {
                        IntakeError::BadRequest("entity_kind is required for entity Objects".into())
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
                )?);
                if object.user_kind.is_some()
                    || object.source.is_some()
                    || object.note.is_some()
                    || object.theme.is_some()
                {
                    return Err(IntakeError::BadRequest(
                        "entity Objects cannot contain subtype data".into(),
                    ));
                }
            }
            "source" => {
                if !object.identities.is_empty() {
                    return Err(IntakeError::BadRequest(
                        "only User Objects accept identities".into(),
                    ));
                }
                if object.user_kind.is_some()
                    || object.entity_kind.is_some()
                    || object.note.is_some()
                    || object.theme.is_some()
                {
                    return Err(IntakeError::BadRequest(
                        "source Objects contain only source subtype data".into(),
                    ));
                }
                validate_source(object.source.as_mut().ok_or_else(|| {
                    IntakeError::BadRequest("source data is required for source Objects".into())
                })?)?;
            }
            "note" => {
                if !object.identities.is_empty() {
                    return Err(IntakeError::BadRequest(
                        "only User Objects accept identities".into(),
                    ));
                }
                if object.user_kind.is_some()
                    || object.entity_kind.is_some()
                    || object.source.is_some()
                    || object.theme.is_some()
                {
                    return Err(IntakeError::BadRequest(
                        "note Objects contain only note subtype data".into(),
                    ));
                }
                validate_note(object.note.as_mut().ok_or_else(|| {
                    IntakeError::BadRequest("note data is required for note Objects".into())
                })?)?;
            }
            "theme" => {
                if !object.identities.is_empty() {
                    return Err(IntakeError::BadRequest(
                        "only User Objects accept identities".into(),
                    ));
                }
                if object.user_kind.is_some()
                    || object.entity_kind.is_some()
                    || object.source.is_some()
                    || object.note.is_some()
                {
                    return Err(IntakeError::BadRequest(
                        "theme Objects may contain only theme subtype data".into(),
                    ));
                }
                let theme = object.theme.as_mut().ok_or_else(|| {
                    IntakeError::BadRequest("theme data is required for theme Objects".into())
                })?;
                theme.slug = crate::domain::theme_slug(theme.slug.clone())?;
            }
            _ => unreachable!(),
        }
        object_ids.insert(
            object.client_key.clone(),
            stable_id(&request.batch_id, "object", &object.client_key),
        );
    }

    let mut content_ids = HashMap::new();
    for content in &mut request.artifacts {
        content.client_key = unique_key(&mut all_keys, &content.client_key, "artifact.client_key")?;
        content.kind = required_text(content.kind.clone(), "artifact.kind", 100)?;
        content.title = optional_text(content.title.take(), "artifact.title", 500)?;
        content.content = content
            .content
            .take()
            .map(|value| {
                crate::domain::required_preserved_text(value, "artifact.content", 10_000_000)
            })
            .transpose()?;
        content.uri = optional_text(content.uri.take(), "artifact.uri", 2000)?;
        content.media_type = optional_text(content.media_type.take(), "artifact.media_type", 255)?;
        if content.content.is_none() && content.uri.is_none() {
            return Err(IntakeError::BadRequest(
                "Artifact requires content or uri".into(),
            ));
        }
        content.sha256 = validate_hash(&content.sha256, "artifact.sha256")?;
        let actual = hex_sha256(
            content
                .content
                .as_deref()
                .unwrap_or_else(|| content.uri.as_deref().unwrap())
                .as_bytes(),
        );
        if actual != content.sha256 {
            return Err(IntakeError::BadRequest(format!(
                "Artifact {} hash mismatch",
                content.client_key
            )));
        }
        let actual_size = content
            .content
            .as_deref()
            .unwrap_or_else(|| content.uri.as_deref().unwrap())
            .len() as i64;
        if content.size_bytes != actual_size {
            return Err(IntakeError::BadRequest(format!(
                "Artifact {} byte-size mismatch",
                content.client_key
            )));
        }
        content.capture_outcome = allowed(
            content.capture_outcome.clone(),
            "artifact.capture_outcome",
            ARTIFACT_CAPTURE_OUTCOMES,
        )?;
        content.capture_reason = optional_text(
            content.capture_reason.take(),
            "artifact.capture_reason",
            1000,
        )?;
        if content.capture_outcome == "complete" {
            if content.content.is_none() {
                return Err(IntakeError::BadRequest(
                    "a complete Artifact requires verbatim content".into(),
                ));
            }
            if content.capture_reason.is_some() {
                return Err(IntakeError::BadRequest(
                    "a complete Artifact must not include capture_reason".into(),
                ));
            }
        } else if content.capture_reason.is_none() {
            return Err(IntakeError::BadRequest(
                "a non-complete Artifact requires capture_reason".into(),
            ));
        }
        if content.expected_size_bytes.is_some_and(|value| value <= 0) {
            return Err(IntakeError::BadRequest(
                "expected_size_bytes must be positive".into(),
            ));
        }
        content.language = optional_text(content.language.take(), "language", 35)?;
        parse_time(content.captured_at.as_deref(), "captured_at")?;
        let metadata = content.metadata.get_or_insert_with(|| json!({}));
        if !metadata.is_object() {
            return Err(IntakeError::BadRequest(
                "Artifact metadata must be a JSON object".into(),
            ));
        }
        let (id, _kind) =
            resolve_ref(&app.pool, &object_ids, &request.objects, &content.object).await?;
        content.object = IntakeObjectRef {
            client_key: content.object.client_key.clone(),
            object_id: Some(id),
        };
        content_ids.insert(
            content.client_key.clone(),
            stable_id(&request.batch_id, "artifact", &content.client_key),
        );
    }

    let mut connection_ids = HashMap::new();
    let mut active_edges = HashSet::new();
    for connection in &mut request.connections {
        connection.client_key = unique_key(
            &mut all_keys,
            &connection.client_key,
            "connection.client_key",
        )?;
        connection.kind = allowed(connection.kind.clone(), "connection.kind", CONNECTION_KINDS)?;
        connection.description = required_text(
            connection.description.clone(),
            "connection.description",
            1000,
        )?;
        connection.provenance = Some(provenance(connection.provenance.take())?);
        let (source_id, source_kind) =
            resolve_ref(&app.pool, &object_ids, &request.objects, &connection.source).await?;
        let (target_id, target_kind) =
            resolve_ref(&app.pool, &object_ids, &request.objects, &connection.target).await?;
        if source_id == target_id {
            return Err(IntakeError::BadRequest(
                "connection endpoints must differ".into(),
            ));
        }
        if !active_edges.insert((source_id, connection.kind.clone(), target_id)) {
            return Err(IntakeError::BadRequest(
                "batch contains a duplicate active connection".into(),
            ));
        }
        if connection.kind == "themed" && (source_kind == "theme" || target_kind != "theme") {
            return Err(IntakeError::BadRequest(
                "themed connections must point from a non-Theme Object to a Theme".into(),
            ));
        }
        connection.source = IntakeObjectRef {
            client_key: connection.source.client_key.clone(),
            object_id: Some(source_id),
        };
        connection.target = IntakeObjectRef {
            client_key: connection.target.client_key.clone(),
            object_id: Some(target_id),
        };
        connection_ids.insert(
            connection.client_key.clone(),
            stable_id(&request.batch_id, "connection", &connection.client_key),
        );
    }

    let payload_sha256 = hex_sha256(
        &serde_json::to_vec(&request).map_err(|error| IntakeError::Internal(error.to_string()))?,
    );
    Ok(PreparedBatch {
        request,
        payload_sha256,
        object_ids,
        content_ids,
        connection_ids,
        adopted_object_ids: HashSet::new(),
    })
}

fn validate_source(source: &mut IntakeSource) -> Result<(), IntakeError> {
    source.source_kind = allowed(source.source_kind.clone(), "source_kind", SOURCE_KINDS)?;
    source.canonical_uri = optional_text(source.canonical_uri.take(), "canonical_uri", 2000)?;
    if source
        .canonical_uri
        .as_ref()
        .is_some_and(|uri| !(uri.starts_with("https://") || uri.starts_with("http://")))
    {
        return Err(IntakeError::BadRequest(
            "canonical_uri must use HTTP or HTTPS".into(),
        ));
    }
    source.byline = optional_text(source.byline.take(), "byline", 500)?;
    source.publisher = optional_text(source.publisher.take(), "publisher", 300)?;
    source.original_language =
        optional_text(source.original_language.take(), "original_language", 35)?;
    source.original_media_type = optional_text(
        source.original_media_type.take(),
        "original_media_type",
        255,
    )?;
    source.original_artifact_reference = optional_text(
        source.original_artifact_reference.take(),
        "original_artifact_reference",
        1000,
    )?;
    let published_at = parse_time(source.published_at.as_deref(), "published_at")?;
    if source.published_at_precision.is_none()
        && let Some(timestamp) = published_at
    {
        let utc = timestamp.to_offset(time::UtcOffset::UTC);
        source.published_at_precision = Some(
            if utc.hour() == 0 && utc.minute() == 0 && utc.second() == 0 && utc.nanosecond() == 0 {
                "day"
            } else {
                "instant"
            }
            .to_owned(),
        );
    }
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
        .transpose()?;
    if published_at.is_some() != source.published_at_precision.is_some() {
        return Err(IntakeError::BadRequest(
            "published_at and published_at_precision must be provided together".into(),
        ));
    }
    parse_time(source.last_accessed_at.as_deref(), "last_accessed_at")?;
    Ok(())
}

fn validate_note(note: &mut IntakeNote) -> Result<(), IntakeError> {
    note.content = required_text(note.content.clone(), "content", 100_000)?;
    note.content_format = allowed(
        note.content_format.clone(),
        "content_format",
        NOTE_CONTENT_FORMATS,
    )?;
    Ok(())
}

fn parse_time(
    value: Option<&str>,
    field: &'static str,
) -> Result<Option<OffsetDateTime>, IntakeError> {
    value
        .map(|value| {
            OffsetDateTime::parse(value, &Rfc3339)
                .map_err(|_| IntakeError::BadRequest(format!("{field} must be RFC 3339")))
        })
        .transpose()
}

async fn resolve_ref(
    pool: &PgPool,
    object_ids: &HashMap<String, Uuid>,
    objects: &[IntakeObject],
    reference: &IntakeObjectRef,
) -> Result<(Uuid, String), IntakeError> {
    match (&reference.client_key, reference.object_id) {
        (Some(key), None) => {
            let id = object_ids.get(key).copied().ok_or_else(|| {
                IntakeError::BadRequest(format!("unknown client_key reference {key}"))
            })?;
            let kind = objects
                .iter()
                .find(|object| object.client_key == *key)
                .map(|object| object.kind.clone())
                .expect("ID map and objects are aligned");
            Ok((id, kind))
        }
        (None, Some(id)) => {
            let kind: Option<String> =
                sqlx::query_scalar("SELECT kind FROM objects WHERE id=$1 AND archived_at IS NULL")
                    .bind(id)
                    .fetch_optional(pool)
                    .await?;
            kind.map(|kind| (id, kind)).ok_or_else(|| {
                IntakeError::BadRequest(format!("object_id reference {id} is not active"))
            })
        }
        _ => Err(IntakeError::BadRequest(
            "an Object reference must contain exactly one of client_key or object_id".into(),
        )),
    }
}

fn validate_key(value: &str, field: &'static str, max: usize) -> Result<String, IntakeError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > max
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
    {
        return Err(IntakeError::BadRequest(format!(
            "{field} must contain 1-{max} ASCII letters, numbers, '-', '_', '.', or ':'"
        )));
    }
    Ok(value.to_owned())
}

fn unique_key(
    keys: &mut HashSet<String>,
    value: &str,
    field: &'static str,
) -> Result<String, IntakeError> {
    let value = validate_key(value, field, 200)?;
    if !keys.insert(value.clone()) {
        return Err(IntakeError::BadRequest(format!(
            "duplicate client key {value}"
        )));
    }
    Ok(value)
}

fn validate_hash(value: &str, field: &'static str) -> Result<String, IntakeError> {
    let value = value.trim();
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(IntakeError::BadRequest(format!(
            "{field} must be a lowercase SHA-256 hex digest"
        )));
    }
    Ok(value.to_owned())
}

fn stable_id(batch_id: &str, family: &str, client_key: &str) -> Uuid {
    Uuid::new_v5(
        &INTAKE_NAMESPACE,
        format!("{batch_id}:{family}:{client_key}").as_bytes(),
    )
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) async fn write_batch(
    pool: &PgPool,
    actor: &ActorContext,
    batch: &PreparedBatch,
) -> Result<(), IntakeError> {
    let mut tx = pool.begin().await?;
    let run_id = stable_id(&batch.request.batch_id, "run", "intake");
    sqlx::query(r#"INSERT INTO runs
      (id,kind,status,actor_type,actor_id,idempotency_key,input,result,started_at)
      VALUES($1,'intake','running',$2,$3,$4,$5,'{}',now())"#)
      .bind(run_id).bind(actor.actor_type).bind(&actor.actor_id)
      .bind(format!("intake:{}",batch.request.batch_id))
      .bind(json!({"batch_id":batch.request.batch_id,"manifest_sha256":batch.request.manifest_sha256,"payload_sha256":batch.payload_sha256}))
      .execute(&mut *tx).await?;
    for object in &batch.request.objects {
        let id = batch.object_ids[&object.client_key];
        if batch.adopted_object_ids.contains(&id) {
            let source = object.source.as_ref().expect("validated adopted Source");
            let current: Option<(i64, Value)> = sqlx::query_as(
                r#"SELECT o.revision,o.provenance FROM objects o JOIN sources s ON s.object_id=o.id
                   WHERE o.id=$1 AND o.kind='source' AND o.archived_at IS NULL AND NOT o.protected
                     AND s.current_artifact_id IS NULL
                   FOR UPDATE"#,
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            let Some((revision, prior_provenance)) = current else {
                return Err(IntakeError::Conflict(
                    "the curator Source placeholder is no longer eligible for workflow adoption"
                        .into(),
                ));
            };
            let mut provenance = object
                .provenance
                .clone()
                .expect("validated adopted Source provenance");
            provenance
                .as_object_mut()
                .expect("validated provenance object")
                .insert("adopted_curator_provenance".into(), prior_provenance);
            sqlx::query(
                r#"UPDATE objects SET title=$2,description=$3,protected=true,provenance=$4,
                   revision=revision+1,updated_by_type=$5,updated_by_id=$6,updated_at=now()
                   WHERE id=$1"#,
            )
            .bind(id)
            .bind(&object.title)
            .bind(&object.description)
            .bind(&provenance)
            .bind(actor.actor_type)
            .bind(actor.actor_id.as_str())
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"UPDATE sources SET source_kind=$2,canonical_uri=$3,byline=$4,publisher=$5,
                   published_at=$6,published_at_precision=$7,last_accessed_at=$8,
                   original_language=$9,original_media_type=$10,
                   original_artifact_reference=$11 WHERE object_id=$1"#,
            )
            .bind(id)
            .bind(&source.source_kind)
            .bind(&source.canonical_uri)
            .bind(&source.byline)
            .bind(&source.publisher)
            .bind(parse_time(source.published_at.as_deref(), "published_at")?)
            .bind(&source.published_at_precision)
            .bind(parse_time(
                source.last_accessed_at.as_deref(),
                "last_accessed_at",
            )?)
            .bind(&source.original_language)
            .bind(&source.original_media_type)
            .bind(&source.original_artifact_reference)
            .execute(&mut *tx)
            .await?;
            insert_intake_event(
                &mut tx,
                actor,
                "object",
                id,
                id,
                "updated",
                revision + 1,
                batch,
                "object",
                &object.client_key,
                json!({"kind":"source","title":object.title,"protected":true,"adopted_from":"context_curator"}),
            )
            .await?;
            continue;
        }
        sqlx::query(r#"INSERT INTO objects
            (id,kind,title,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$6,$7,$8)"#)
            .bind(id).bind(&object.kind).bind(&object.title).bind(&object.description)
            .bind(object.protected).bind(actor.actor_type).bind(&actor.actor_id)
            .bind(object.provenance.as_ref().expect("validated provenance"))
            .execute(&mut *tx).await?;
        match object.kind.as_str() {
            "user" => {
                sqlx::query("INSERT INTO users (object_id,user_kind,identities) VALUES ($1,$2,$3)")
                    .bind(id)
                    .bind(object.user_kind.as_deref())
                    .bind(
                        serde_json::to_value(&object.identities)
                            .map_err(|error| IntakeError::Internal(error.to_string()))?,
                    )
                    .execute(&mut *tx)
                    .await?;
            }
            "entity" => {
                sqlx::query("INSERT INTO entities (object_id,entity_kind) VALUES ($1,$2)")
                    .bind(id)
                    .bind(
                        object
                            .entity_kind
                            .as_deref()
                            .expect("validated entity kind"),
                    )
                    .execute(&mut *tx)
                    .await?;
            }
            "note" => {
                let note = object.note.as_ref().expect("validated note");
                sqlx::query(
                    "INSERT INTO notes (object_id,content,content_format) VALUES ($1,$2,$3)",
                )
                .bind(id)
                .bind(&note.content)
                .bind(&note.content_format)
                .execute(&mut *tx)
                .await?;
            }
            "source" => {
                let source = object.source.as_ref().expect("validated source");
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
                .bind(parse_time(source.published_at.as_deref(), "published_at")?)
                .bind(&source.published_at_precision)
                .bind(parse_time(
                    source.last_accessed_at.as_deref(),
                    "last_accessed_at",
                )?)
                .bind(&source.original_language)
                .bind(&source.original_media_type)
                .bind(&source.original_artifact_reference)
                .execute(&mut *tx)
                .await?;
            }
            "theme" => {
                let theme = object.theme.as_ref().expect("validated theme");
                sqlx::query("INSERT INTO themes (object_id,slug) VALUES ($1,$2)")
                    .bind(id)
                    .bind(&theme.slug)
                    .execute(&mut *tx)
                    .await?;
            }
            _ => unreachable!(),
        }
        insert_intake_event(
            &mut tx,
            actor,
            "object",
            id,
            id,
            "created",
            1,
            batch,
            "object",
            &object.client_key,
            json!({"kind":object.kind,"title":object.title,"protected":object.protected}),
        )
        .await?;
    }
    for content in &batch.request.artifacts {
        let source_id = content.object.object_id.expect("resolved Artifact owner");
        let content_id = batch.content_ids[&content.client_key];
        let revision: i64 =
            sqlx::query_scalar("SELECT revision FROM objects WHERE id=$1 FOR UPDATE")
                .bind(source_id)
                .fetch_one(&mut *tx)
                .await?;
        sqlx::query(
            r#"INSERT INTO artifacts
            (id,object_id,kind,title,content,uri,media_type,language,sha256,size_bytes,
             capture_outcome,capture_reason,expected_size_bytes,metadata,
             supersedes_artifact_id,captured_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)"#,
        )
        .bind(content_id)
        .bind(source_id)
        .bind(&content.kind)
        .bind(&content.title)
        .bind(&content.content)
        .bind(&content.uri)
        .bind(&content.media_type)
        .bind(&content.language)
        .bind(&content.sha256)
        .bind(content.size_bytes)
        .bind(&content.capture_outcome)
        .bind(&content.capture_reason)
        .bind(content.expected_size_bytes)
        .bind(content.metadata.as_ref().expect("validated metadata"))
        .bind(content.supersedes_artifact_id)
        .bind(parse_time(content.captured_at.as_deref(), "captured_at")?)
        .execute(&mut *tx)
        .await?;
        if content.capture_outcome == "complete" {
            sqlx::query("UPDATE sources SET current_artifact_id=$2 WHERE object_id=$1")
                .bind(source_id)
                .bind(content_id)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("UPDATE objects SET revision=revision+1,updated_by_type=$2,updated_by_id=$3,updated_at=now() WHERE id=$1")
            .bind(source_id).bind(actor.actor_type).bind(&actor.actor_id).execute(&mut *tx).await?;
        insert_intake_event(
            &mut tx,
            actor,
            "object",
            source_id,
            source_id,
            "artifact_attached",
            revision + 1,
            batch,
            "artifact",
            &content.client_key,
            json!({"artifact_id":content_id,"kind":content.kind,"sha256":content.sha256,
                "size_bytes":content.size_bytes,"capture_outcome":content.capture_outcome}),
        )
        .await?;
    }
    let object_ids = batch.object_ids.values().copied().collect::<Vec<_>>();
    let primary_object_id = (object_ids.len() == 1).then_some(object_ids[0]);
    let originating_chat_object_id = batch
        .request
        .connections
        .iter()
        .find(|connection| connection.client_key == "originating-chat")
        .and_then(|connection| connection.source.object_id);
    sqlx::query(
        r#"UPDATE runs SET status='completed',
           result=jsonb_build_object('counts',$2::jsonb,'object_ids',$3::uuid[]),
           primary_object_id=$4,
           chat_object_id=CASE WHEN EXISTS(SELECT 1 FROM chats WHERE object_id=$5)
             THEN $5 ELSE chat_object_id END,
           completed_at=now(),updated_at=now() WHERE id=$1"#,
    )
    .bind(run_id)
    .bind(batch.counts())
    .bind(object_ids)
    .bind(primary_object_id)
    .bind(originating_chat_object_id)
    .execute(&mut *tx)
    .await?;
    for connection in &batch.request.connections {
        let id = batch.connection_ids[&connection.client_key];
        let source_id = connection
            .source
            .object_id
            .expect("resolved connection source");
        let target_id = connection
            .target
            .object_id
            .expect("resolved connection target");
        sqlx::query(r#"INSERT INTO connections
            (id,source_object_id,kind,target_object_id,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$7,$8,$9)"#)
            .bind(id).bind(source_id).bind(&connection.kind).bind(target_id).bind(&connection.description)
            .bind(connection.protected).bind(actor.actor_type).bind(&actor.actor_id)
            .bind(connection.provenance.as_ref().expect("validated provenance"))
            .execute(&mut *tx).await?;
        insert_intake_event(&mut tx, actor, "connection", id, source_id, "connected", 1, batch, "connection", &connection.client_key, json!({"kind":connection.kind,"target_object_id":target_id,"description":connection.description,"protected":connection.protected})).await?;
    }
    tx.commit().await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_intake_event(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    entity_type: &str,
    entity_id: Uuid,
    object_id: Uuid,
    action: &str,
    to_revision: i64,
    batch: &PreparedBatch,
    family: &str,
    client_key: &str,
    mut changes: Value,
) -> Result<(), IntakeError> {
    let object = changes.as_object_mut().expect("event changes are objects");
    object.insert("intake_batch_id".into(), json!(batch.request.batch_id));
    object.insert(
        "intake_manifest_sha256".into(),
        json!(batch.request.manifest_sha256),
    );
    object.insert("intake_payload_sha256".into(), json!(batch.payload_sha256));
    object.insert("intake_client_key".into(), json!(client_key));
    let run_id = stable_id(&batch.request.batch_id, "run", "intake");
    let sequence: i32 =
        sqlx::query_scalar("SELECT COALESCE(max(sequence),0)+1 FROM object_events WHERE run_id=$1")
            .bind(run_id)
            .fetch_one(&mut **tx)
            .await?;
    let target_type = if entity_type == "connection" {
        "connection"
    } else {
        "object"
    };
    let target_id = if target_type == "connection" {
        entity_id
    } else {
        object_id
    };
    let after: Value = if target_type == "connection" {
        sqlx::query_scalar("SELECT to_jsonb(c) FROM connections c WHERE id=$1")
            .bind(target_id)
            .fetch_one(&mut **tx)
            .await?
    } else {
        sqlx::query_scalar("SELECT to_jsonb(o) FROM objects o WHERE id=$1")
            .bind(target_id)
            .fetch_one(&mut **tx)
            .await?
    };
    sqlx::query(
        r#"INSERT INTO object_events
        (id,run_id,sequence,target_type,target_id,action,actor_type,actor_id,idempotency_key,
         from_revision,to_revision,before_state,after_state,reversible,created_at)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,true,now())"#,
    )
    .bind(Uuid::new_v4())
    .bind(run_id)
    .bind(sequence)
    .bind(target_type)
    .bind(target_id)
    .bind(if action == "connected" {
        "created"
    } else {
        action
    })
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(format!(
        "intake:{}:{family}:{client_key}",
        batch.request.batch_id
    ))
    .bind((to_revision > 1).then_some(to_revision - 1))
    .bind(to_revision)
    .bind((to_revision > 1).then(|| json!({"revision":to_revision-1})))
    .bind(after)
    .execute(&mut **tx)
    .await?;
    crate::runs::append_trace(tx, run_id, "intake_mutation", changes).await?;
    Ok(())
}

#[derive(Debug)]
pub(crate) struct BatchStatus {
    pub(crate) manifest_sha256: String,
    pub(crate) payload_sha256: String,
    pub(crate) event_count: i64,
    pub(crate) object_ids: Vec<Uuid>,
}

pub(crate) async fn status(
    pool: &PgPool,
    batch_id: &str,
) -> Result<Option<BatchStatus>, IntakeError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        manifest_sha256: String,
        payload_sha256: String,
        event_count: i64,
        object_ids: Vec<Uuid>,
    }
    let row: Option<Row> = sqlx::query_as(
        r#"SELECT input->>'manifest_sha256' manifest_sha256,input->>'payload_sha256' payload_sha256,
        COALESCE((SELECT count(*) FROM object_events e WHERE e.run_id=r.id),0)::bigint event_count,
        COALESCE(ARRAY(SELECT jsonb_array_elements_text(result->'object_ids')::uuid),'{}'::uuid[]) object_ids
        FROM runs r WHERE kind='intake' AND input->>'batch_id'=$1"#,
    )
    .bind(batch_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| BatchStatus {
        manifest_sha256: row.manifest_sha256,
        payload_sha256: row.payload_sha256,
        event_count: row.event_count,
        object_ids: row.object_ids,
    }))
}

#[derive(Debug)]
pub(crate) enum IntakeError {
    BadRequest(String),
    Unauthorized,
    Forbidden(String),
    Conflict(String),
    Internal(String),
    Database(sqlx::Error),
}

impl From<sqlx::Error> for IntakeError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

impl From<crate::db::DbError> for IntakeError {
    fn from(value: crate::db::DbError) -> Self {
        match value {
            crate::db::DbError::NotFound => Self::BadRequest("record not found".into()),
            crate::db::DbError::Conflict => Self::Conflict("record conflict".into()),
            crate::db::DbError::Invalid(message) => Self::BadRequest(message),
            crate::db::DbError::Validation(error) => Self::BadRequest(error.to_string()),
            crate::db::DbError::Sqlx(error) => Self::Database(error),
        }
    }
}

impl From<crate::domain::ValidationError> for IntakeError {
    fn from(value: crate::domain::ValidationError) -> Self {
        Self::BadRequest(value.to_string())
    }
}

impl IntoResponse for IntakeError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "invalid_intake_batch", message),
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Authentication failed.".into(),
            ),
            Self::Forbidden(message) => (StatusCode::FORBIDDEN, "manifest_not_approved", message),
            Self::Conflict(message) => (StatusCode::CONFLICT, "intake_batch_conflict", message),
            Self::Internal(message) => {
                tracing::error!(%message, "Context intake internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "The intake operation failed.".into(),
                )
            }
            Self::Database(error) => {
                tracing::error!(%error, "Context intake database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    "The intake operation failed.".into(),
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
    fn stable_ids_are_scoped_to_batch_family_and_client_key() {
        assert_eq!(
            stable_id("batch-1", "object", "source-1"),
            stable_id("batch-1", "object", "source-1")
        );
        assert_ne!(
            stable_id("batch-1", "object", "source-1"),
            stable_id("batch-2", "object", "source-1")
        );
        assert_ne!(
            stable_id("batch-1", "object", "source-1"),
            stable_id("batch-1", "content", "source-1")
        );
    }

    #[test]
    fn keys_and_hashes_fail_closed() {
        assert!(validate_key("bad key", "batch_id", 100).is_err());
        assert!(validate_hash(&"A".repeat(64), "manifest_sha256").is_err());
        assert_eq!(
            validate_hash(&"a".repeat(64), "manifest_sha256").unwrap(),
            "a".repeat(64)
        );
    }
}

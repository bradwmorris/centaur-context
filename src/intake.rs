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
        ActorContext, CONNECTION_KINDS, NOTE_CONTENT_FORMATS, SOURCE_CONTENT_KINDS, SOURCE_KINDS,
        USER_KINDS, allowed, object_description, optional_text, provenance, required_text,
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
        .route("/api/v1/intake/batches/validate", post(validate_batch))
        .route("/api/v1/intake/batches/commit", post(commit_batch))
        .route("/api/v1/intake/batches/{batch_id}", get(read_batch_status))
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
    pub external_identities: Vec<IntakeExternalIdentity>,
    #[serde(default)]
    pub source_contents: Vec<IntakeSourceContent>,
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
    pub source: Option<IntakeSource>,
    #[serde(default)]
    pub note: Option<IntakeNote>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeSource {
    pub source_kind: String,
    pub canonical_uri: Option<String>,
    pub byline: Option<String>,
    pub publisher: Option<String>,
    pub published_at: Option<String>,
    pub accessed_at: Option<String>,
    pub language: Option<String>,
    pub media_type: Option<String>,
    pub artifact_reference: Option<String>,
    pub content_hash: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeNote {
    pub content: String,
    pub content_format: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeExternalIdentity {
    pub client_key: String,
    pub user: IntakeObjectRef,
    pub provider: String,
    #[serde(default)]
    pub workspace_id: String,
    pub provider_user_id: String,
    pub display_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntakeSourceContent {
    pub client_key: String,
    pub source: IntakeObjectRef,
    pub content_kind: String,
    pub normalized_text: String,
    pub content_hash: String,
    pub language: Option<String>,
    pub extraction_method: Option<String>,
    pub extraction_version: Option<String>,
    pub artifact_reference: Option<String>,
    #[serde(default)]
    pub locators: Option<Value>,
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
    identity_ids: HashMap<String, Uuid>,
    content_ids: HashMap<String, Uuid>,
    connection_ids: HashMap<String, Uuid>,
    adopted_object_ids: HashSet<Uuid>,
}

impl PreparedBatch {
    pub(crate) fn event_count(&self) -> usize {
        self.request.objects.len()
            + self.request.source_contents.len()
            + self.request.connections.len()
    }

    pub(crate) fn counts(&self) -> Value {
        json!({
            "objects":self.request.objects.len(),
            "external_identities":self.request.external_identities.len(),
            "source_contents":self.request.source_contents.len(),
            "connections":self.request.connections.len(),
            "events":self.event_count(),
        })
    }

    fn id_map(&self) -> Value {
        json!({
            "objects":self.object_ids,
            "external_identities":self.identity_ids,
            "source_contents":self.content_ids,
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
        for content in &mut self.request.source_contents {
            if content.source.object_id == Some(previous_id) {
                content.source.object_id = Some(object_id);
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
    let resources = request.objects.len()
        + request.external_identities.len()
        + request.source_contents.len()
        + request.connections.len();
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
            &["user", "entity", "source", "note"],
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
                if object.source.is_some() || object.note.is_some() {
                    return Err(IntakeError::BadRequest(
                        "user Objects cannot contain source or note data".into(),
                    ));
                }
            }
            "entity" => {
                if object.user_kind.is_some() || object.source.is_some() || object.note.is_some() {
                    return Err(IntakeError::BadRequest(
                        "entity Objects cannot contain subtype data".into(),
                    ));
                }
            }
            "source" => {
                if object.user_kind.is_some() || object.note.is_some() {
                    return Err(IntakeError::BadRequest(
                        "source Objects contain only source subtype data".into(),
                    ));
                }
                validate_source(object.source.as_mut().ok_or_else(|| {
                    IntakeError::BadRequest("source data is required for source Objects".into())
                })?)?;
            }
            "note" => {
                if object.user_kind.is_some() || object.source.is_some() {
                    return Err(IntakeError::BadRequest(
                        "note Objects contain only note subtype data".into(),
                    ));
                }
                validate_note(object.note.as_mut().ok_or_else(|| {
                    IntakeError::BadRequest("note data is required for note Objects".into())
                })?)?;
            }
            _ => unreachable!(),
        }
        object_ids.insert(
            object.client_key.clone(),
            stable_id(&request.batch_id, "object", &object.client_key),
        );
    }

    let mut identity_ids = HashMap::new();
    for identity in &mut request.external_identities {
        identity.client_key = unique_key(
            &mut all_keys,
            &identity.client_key,
            "external_identity.client_key",
        )?;
        identity.provider = required_text(identity.provider.clone(), "provider", 100)?;
        identity.workspace_id = identity.workspace_id.trim().to_owned();
        identity.provider_user_id =
            required_text(identity.provider_user_id.clone(), "provider_user_id", 300)?;
        identity.display_name = optional_text(identity.display_name.take(), "display_name", 500)?;
        let (id, kind) =
            resolve_ref(&app.pool, &object_ids, &request.objects, &identity.user).await?;
        if kind != "user" {
            return Err(IntakeError::BadRequest(
                "external identity must reference a user Object".into(),
            ));
        }
        identity.user = IntakeObjectRef {
            client_key: identity.user.client_key.clone(),
            object_id: Some(id),
        };
        identity_ids.insert(
            identity.client_key.clone(),
            stable_id(&request.batch_id, "identity", &identity.client_key),
        );
    }

    let mut content_ids = HashMap::new();
    for content in &mut request.source_contents {
        content.client_key = unique_key(
            &mut all_keys,
            &content.client_key,
            "source_content.client_key",
        )?;
        content.content_kind = allowed(
            content.content_kind.clone(),
            "content_kind",
            SOURCE_CONTENT_KINDS,
        )?;
        content.normalized_text = required_text(
            content.normalized_text.clone(),
            "normalized_text",
            10_000_000,
        )?;
        content.content_hash = validate_hash(&content.content_hash, "content_hash")?;
        let actual = hex_sha256(content.normalized_text.as_bytes());
        if actual != content.content_hash {
            return Err(IntakeError::BadRequest(format!(
                "source content {} hash mismatch",
                content.client_key
            )));
        }
        content.language = optional_text(content.language.take(), "language", 35)?;
        content.extraction_method =
            optional_text(content.extraction_method.take(), "extraction_method", 200)?;
        content.extraction_version =
            optional_text(content.extraction_version.take(), "extraction_version", 100)?;
        content.artifact_reference = optional_text(
            content.artifact_reference.take(),
            "artifact_reference",
            1000,
        )?;
        let locators = content.locators.get_or_insert_with(|| json!({}));
        if !locators.is_object() {
            return Err(IntakeError::BadRequest(
                "locators must be a JSON object".into(),
            ));
        }
        let (id, kind) =
            resolve_ref(&app.pool, &object_ids, &request.objects, &content.source).await?;
        if kind != "source" {
            return Err(IntakeError::BadRequest(
                "source content must reference a source Object".into(),
            ));
        }
        content.source = IntakeObjectRef {
            client_key: content.source.client_key.clone(),
            object_id: Some(id),
        };
        content_ids.insert(
            content.client_key.clone(),
            stable_id(&request.batch_id, "content", &content.client_key),
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
        let (source_id, _) =
            resolve_ref(&app.pool, &object_ids, &request.objects, &connection.source).await?;
        let (target_id, _) =
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
        identity_ids,
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
    source.language = optional_text(source.language.take(), "language", 35)?;
    source.media_type = optional_text(source.media_type.take(), "media_type", 255)?;
    source.artifact_reference =
        optional_text(source.artifact_reference.take(), "artifact_reference", 1000)?;
    source.content_hash = source
        .content_hash
        .take()
        .map(|hash| validate_hash(&hash, "content_hash"))
        .transpose()?;
    parse_time(source.published_at.as_deref(), "published_at")?;
    parse_time(source.accessed_at.as_deref(), "accessed_at")?;
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
                sqlx::query_scalar("SELECT kind FROM objects WHERE id=$1 AND lifecycle='active'")
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
    for object in &batch.request.objects {
        let id = batch.object_ids[&object.client_key];
        if batch.adopted_object_ids.contains(&id) {
            let source = object.source.as_ref().expect("validated adopted Source");
            let current: Option<(i64, Value)> = sqlx::query_as(
                r#"SELECT o.revision,o.provenance FROM objects o JOIN sources s ON s.object_id=o.id
                   WHERE o.id=$1 AND o.kind='source' AND o.lifecycle='active' AND NOT o.protected
                     AND s.current_content_id IS NULL AND s.content_hash IS NULL
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
                   published_at=$6,accessed_at=$7,language=$8,media_type=$9,
                   artifact_reference=$10,updated_at=now() WHERE object_id=$1"#,
            )
            .bind(id)
            .bind(&source.source_kind)
            .bind(&source.canonical_uri)
            .bind(&source.byline)
            .bind(&source.publisher)
            .bind(parse_time(source.published_at.as_deref(), "published_at")?)
            .bind(parse_time(source.accessed_at.as_deref(), "accessed_at")?)
            .bind(&source.language)
            .bind(&source.media_type)
            .bind(&source.artifact_reference)
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
                sqlx::query("INSERT INTO users (object_id,user_kind) VALUES ($1,$2)")
                    .bind(id)
                    .bind(object.user_kind.as_deref())
                    .execute(&mut *tx)
                    .await?;
            }
            "entity" => {
                sqlx::query("INSERT INTO entities (object_id) VALUES ($1)")
                    .bind(id)
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
                sqlx::query(r#"INSERT INTO sources
                    (object_id,source_kind,canonical_uri,byline,publisher,published_at,accessed_at,language,media_type,artifact_reference,content_hash)
                    VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)"#)
                    .bind(id).bind(&source.source_kind).bind(&source.canonical_uri).bind(&source.byline).bind(&source.publisher)
                    .bind(parse_time(source.published_at.as_deref(), "published_at")?).bind(parse_time(source.accessed_at.as_deref(), "accessed_at")?)
                    .bind(&source.language).bind(&source.media_type).bind(&source.artifact_reference).bind(&source.content_hash)
                    .execute(&mut *tx).await?;
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
    for identity in &batch.request.external_identities {
        sqlx::query(
            r#"INSERT INTO external_identities
            (id,user_object_id,provider,workspace_id,provider_user_id,display_name)
            VALUES ($1,$2,$3,$4,$5,$6)"#,
        )
        .bind(batch.identity_ids[&identity.client_key])
        .bind(identity.user.object_id.expect("resolved identity user"))
        .bind(&identity.provider)
        .bind(&identity.workspace_id)
        .bind(&identity.provider_user_id)
        .bind(&identity.display_name)
        .execute(&mut *tx)
        .await?;
    }
    for content in &batch.request.source_contents {
        let source_id = content.source.object_id.expect("resolved source");
        let content_id = batch.content_ids[&content.client_key];
        let revision: i64 =
            sqlx::query_scalar("SELECT revision FROM objects WHERE id=$1 FOR UPDATE")
                .bind(source_id)
                .fetch_one(&mut *tx)
                .await?;
        let version: i64 = sqlx::query_scalar(
            "SELECT COALESCE(max(version),0)+1 FROM source_contents WHERE source_object_id=$1",
        )
        .bind(source_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(r#"INSERT INTO source_contents
            (id,source_object_id,version,content_kind,normalized_text,language,extraction_method,extraction_version,content_hash,size_bytes,artifact_reference,locators)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)"#)
            .bind(content_id).bind(source_id).bind(version).bind(&content.content_kind).bind(&content.normalized_text)
            .bind(&content.language).bind(&content.extraction_method).bind(&content.extraction_version).bind(&content.content_hash)
            .bind(content.normalized_text.len() as i64).bind(&content.artifact_reference).bind(content.locators.as_ref().expect("validated locators"))
            .execute(&mut *tx).await?;
        sqlx::query("UPDATE sources SET current_content_id=$2,content_hash=$3,updated_at=now() WHERE object_id=$1")
            .bind(source_id).bind(content_id).bind(&content.content_hash).execute(&mut *tx).await?;
        sqlx::query("UPDATE objects SET revision=revision+1,updated_by_type=$2,updated_by_id=$3,updated_at=now() WHERE id=$1")
            .bind(source_id).bind(actor.actor_type).bind(&actor.actor_id).execute(&mut *tx).await?;
        insert_intake_event(&mut tx, actor, "source_content", content_id, source_id, "content_version_created", revision + 1, batch, "content", &content.client_key, json!({"content_kind":content.content_kind,"version":version,"content_hash":content.content_hash,"size_bytes":content.normalized_text.len()})).await?;
    }
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
    sqlx::query(r#"INSERT INTO object_events
        (id,entity_type,entity_id,object_id,action,actor_type,actor_id,centaur_thread_key,centaur_execution_id,idempotency_key,to_revision,changes)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)"#)
        .bind(Uuid::new_v4()).bind(entity_type).bind(entity_id).bind(object_id).bind(action)
        .bind(actor.actor_type).bind(&actor.actor_id).bind(&actor.centaur_thread_key).bind(&actor.centaur_execution_id)
        .bind(format!("intake:{}:{family}:{client_key}", batch.request.batch_id)).bind(to_revision).bind(changes)
        .execute(&mut **tx).await?;
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
        r#"SELECT
        min(changes->>'intake_manifest_sha256') AS manifest_sha256,
        min(changes->>'intake_payload_sha256') AS payload_sha256,
        count(*)::bigint AS event_count,
        array_agg(DISTINCT object_id ORDER BY object_id) AS object_ids
        FROM object_events WHERE changes->>'intake_batch_id'=$1
        HAVING count(*)>0"#,
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
        Self::Internal(value.to_string())
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

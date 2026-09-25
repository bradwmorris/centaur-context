//! Universal Context search/read/apply request models and atomic persistence.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

use crate::{
    contract,
    db::{self, Connection, DbError, Object},
    domain::{
        ActorContext, NOTE_CONTENT_FORMATS, SOURCE_KINDS, TASK_PRIORITIES, TASK_STATUSES, allowed,
        object_description, provenance, required_text, theme_slug,
    },
};

const MAX_OPERATIONS: usize = 20;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyRequest {
    pub contract_version: String,
    pub idempotency_key: String,
    pub chat_object_id: Option<Uuid>,
    #[serde(default)]
    pub validate_only: bool,
    pub operations: Vec<ApplyOperation>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApplyOperation {
    CreateObject {
        local_ref: String,
        kind: String,
        title: String,
        description: String,
        #[serde(default)]
        provenance: Option<Value>,
        #[serde(default)]
        fields: Map<String, Value>,
    },
    UpdateObject {
        object_id: Uuid,
        expected_revision: i64,
        #[serde(default)]
        changes: Map<String, Value>,
    },
    UpdateDescription {
        object_id: Uuid,
        expected_revision: i64,
        description: String,
    },
    PromoteSourceArtifact {
        source_id: Uuid,
        expected_revision: i64,
        artifact_id: Uuid,
        expected_sha256: String,
    },
    ArchiveObject {
        object_id: Uuid,
        expected_revision: i64,
    },
    CreateConnection {
        source: ObjectReference,
        kind: String,
        target: ObjectReference,
        description: String,
        #[serde(default)]
        provenance: Option<Value>,
    },
    UpdateConnection {
        connection_id: Uuid,
        expected_revision: i64,
        kind: Option<String>,
        description: Option<String>,
        provenance: Option<Value>,
    },
    ArchiveConnection {
        connection_id: Uuid,
        expected_revision: i64,
    },
    AppendArtifact {
        object: ObjectReference,
        expected_revision: i64,
        kind: String,
        title: Option<String>,
        content: Option<String>,
        uri: Option<String>,
        media_type: Option<String>,
        language: Option<String>,
        #[serde(default = "complete_capture")]
        capture_outcome: String,
        capture_reason: Option<String>,
        #[serde(default = "empty_object")]
        metadata: Value,
        supersedes_artifact_id: Option<Uuid>,
    },
}

fn complete_capture() -> String {
    "complete".to_owned()
}

fn empty_object() -> Value {
    json!({})
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum ObjectReference {
    Id { object_id: Uuid },
    Local { local_ref: String },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ApplyResponse {
    pub replayed: bool,
    pub validated_only: bool,
    pub request_hash: String,
    pub contract_version: String,
    pub tool_version: String,
    pub run_id: Option<Uuid>,
    pub event_ids: Vec<Uuid>,
    pub results: Vec<Value>,
}

pub fn request_hash(request: &ApplyRequest) -> Result<String, DbError> {
    let bytes = serde_json::to_vec(request)
        .map_err(|error| DbError::Invalid(format!("cannot normalize apply request: {error}")))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

#[derive(Clone, Copy, PartialEq)]
enum WriteAuthority {
    Ordinary,
    ReviewedMaintenance,
}

pub(crate) async fn apply_maintenance(
    pool: &PgPool,
    actor: &ActorContext,
    request: ApplyRequest,
) -> Result<ApplyResponse, DbError> {
    apply_with_authority(pool, actor, request, WriteAuthority::ReviewedMaintenance).await
}

pub async fn apply(
    pool: &PgPool,
    actor: &ActorContext,
    request: ApplyRequest,
) -> Result<ApplyResponse, DbError> {
    apply_with_authority(pool, actor, request, WriteAuthority::Ordinary).await
}

async fn apply_with_authority(
    pool: &PgPool,
    actor: &ActorContext,
    request: ApplyRequest,
    authority: WriteAuthority,
) -> Result<ApplyResponse, DbError> {
    validate_request(&request)?;
    validate_authority_operations(&request, authority)?;
    let request_hash = request_hash(&request)?;
    let mut tx = pool.begin().await?;
    crate::runs::assert_thread_not_fenced(&mut tx, actor.centaur_thread_key.as_deref()).await?;

    if !request.validate_only {
        let claimed: Option<bool> = sqlx::query_scalar(
            r#"INSERT INTO context_apply_requests
               (principal_id,idempotency_key,request_hash,contract_version)
               VALUES ($1,$2,$3,$4)
               ON CONFLICT DO NOTHING RETURNING true"#,
        )
        .bind(&actor.actor_id)
        .bind(&request.idempotency_key)
        .bind(&request_hash)
        .bind(&request.contract_version)
        .fetch_optional(&mut *tx)
        .await?;
        if claimed.is_none() {
            let existing: (String, Option<Value>) = sqlx::query_as(
                r#"SELECT request_hash,response FROM context_apply_requests
                   WHERE principal_id=$1 AND idempotency_key=$2 FOR UPDATE"#,
            )
            .bind(&actor.actor_id)
            .bind(&request.idempotency_key)
            .fetch_one(&mut *tx)
            .await?;
            if existing.0 != request_hash {
                return Err(DbError::Invalid(
                    "idempotency key was already used with a different request".into(),
                ));
            }
            let mut response: ApplyResponse =
                serde_json::from_value(existing.1.ok_or_else(|| {
                    DbError::Invalid("matching context_apply request is incomplete".into())
                })?)
                .map_err(|error| {
                    DbError::Invalid(format!("stored apply response is invalid: {error}"))
                })?;
            response.replayed = true;
            tx.commit().await?;
            return Ok(response);
        }
    }

    let local_ids = assign_local_ids(&request.operations)?;
    validate_new_object_connections(&request)?;
    let run_id = Uuid::new_v4();
    if !request.validate_only {
        let run_key = format!(
            "context_apply:{}:{}",
            actor.actor_id, request.idempotency_key
        );
        sqlx::query(
            r#"INSERT INTO runs
               (id,kind,status,actor_type,actor_id,chat_object_id,idempotency_key,input,result,completed_at)
               VALUES ($1,'mutation','running',$2,$3,$4,$5,$6,'{}'::jsonb,NULL)"#,
        )
        .bind(run_id)
        .bind(actor.actor_type)
        .bind(&actor.actor_id)
        .bind(request.chat_object_id)
        .bind(run_key)
        .bind(json!({
            "operation":"context_apply",
            "contract_version":contract::version(),
            "contract_hash":contract::hash(),
            "tool_version":contract::tool_version(),
            "request_hash":request_hash,
            "centaur_thread_key":actor.centaur_thread_key,
            "centaur_execution_id":actor.centaur_execution_id,
            "operation_count":request.operations.len()
        }))
        .execute(&mut *tx)
        .await?;
    }

    let mut event_ids = Vec::new();
    let mut results = Vec::new();
    let mut sequence = 1_i64;
    for operation in &request.operations {
        let before = if authority == WriteAuthority::ReviewedMaintenance {
            let target = match operation {
                ApplyOperation::UpdateObject { object_id, .. }
                | ApplyOperation::UpdateDescription { object_id, .. }
                | ApplyOperation::PromoteSourceArtifact {
                    source_id: object_id,
                    ..
                }
                | ApplyOperation::ArchiveObject { object_id, .. }
                | ApplyOperation::AppendArtifact {
                    object: ObjectReference::Id { object_id },
                    ..
                } => Some(("object", "objects", *object_id)),
                ApplyOperation::UpdateConnection { connection_id, .. }
                | ApplyOperation::ArchiveConnection { connection_id, .. } => {
                    Some(("connection", "connections", *connection_id))
                }
                _ => None,
            };
            if let Some((kind, table, id)) = target {
                // Hold the row lock before reading the recovery snapshot.
                let sql = format!("SELECT id FROM {table} WHERE id=$1 FOR UPDATE");
                sqlx::query_scalar::<_, Uuid>(&sql)
                    .bind(id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .ok_or(DbError::NotFound)?;
                Some(db::target_snapshot(&mut tx, kind, id).await?)
            } else {
                None
            }
        } else {
            None
        };
        let mut result = execute_operation(
            &mut tx,
            actor,
            operation,
            &local_ids,
            before.as_ref(),
            (!request.validate_only).then_some(run_id),
            &mut sequence,
            &mut event_ids,
            authority,
        )
        .await?;
        if let Some(before) = before {
            result["before"] = before;
        }
        results.push(result);
    }

    if let Some(chat_id) = request.chat_object_id {
        for object_id in local_ids.values() {
            let result = create_connection(
                &mut tx,
                actor,
                chat_id,
                "about",
                *object_id,
                "This verified Chat caused creation of the connected Context Object.",
                json!({"source_type":"centaur_chat","source_ref":chat_id}),
                (!request.validate_only).then_some(run_id),
                &mut sequence,
                &mut event_ids,
                authority,
                true,
            )
            .await?;
            results.push(json!({"operation":"automatic_chat_connection","data":result}));
        }
    }

    let response = ApplyResponse {
        replayed: false,
        validated_only: request.validate_only,
        request_hash: request_hash.clone(),
        contract_version: contract::version().to_owned(),
        tool_version: contract::tool_version().to_owned(),
        run_id: (!request.validate_only).then_some(run_id),
        event_ids,
        results,
    };

    if request.validate_only {
        tx.rollback().await?;
        return Ok(ApplyResponse {
            run_id: None,
            event_ids: Vec::new(),
            results: response
                .results
                .into_iter()
                .map(|value| {
                    if authority == WriteAuthority::ReviewedMaintenance {
                        value
                    } else {
                        json!({"operation":value["operation"],"valid":true})
                    }
                })
                .collect(),
            ..response
        });
    }

    let response_value = serde_json::to_value(&response)
        .map_err(|error| DbError::Invalid(format!("cannot store apply response: {error}")))?;
    sqlx::query(
        "UPDATE runs SET status='completed',result=$2,completed_at=now(),updated_at=now() WHERE id=$1",
    )
    .bind(run_id)
    .bind(&response_value)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"UPDATE context_apply_requests SET run_id=$3,response=$4,completed_at=now()
           WHERE principal_id=$1 AND idempotency_key=$2"#,
    )
    .bind(&actor.actor_id)
    .bind(&request.idempotency_key)
    .bind(run_id)
    .bind(&response_value)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(response)
}

fn validate_request(request: &ApplyRequest) -> Result<(), DbError> {
    if request.contract_version != contract::version() {
        return Err(DbError::Invalid(format!(
            "unsupported contract version {}; expected {}",
            request.contract_version,
            contract::version()
        )));
    }
    required_text(request.idempotency_key.clone(), "idempotency_key", 300)?;
    if request.operations.is_empty() || request.operations.len() > MAX_OPERATIONS {
        return Err(DbError::Invalid(format!(
            "operations must contain between 1 and {MAX_OPERATIONS} entries"
        )));
    }
    Ok(())
}

fn validate_authority_operations(
    request: &ApplyRequest,
    authority: WriteAuthority,
) -> Result<(), DbError> {
    if authority == WriteAuthority::Ordinary
        && request.operations.iter().any(|operation| {
            matches!(
                operation,
                ApplyOperation::UpdateDescription { .. }
                    | ApplyOperation::PromoteSourceArtifact { .. }
            )
        })
    {
        return Err(DbError::Invalid(
            "operation is available only through reviewed maintenance".into(),
        ));
    }
    Ok(())
}

fn assign_local_ids(operations: &[ApplyOperation]) -> Result<BTreeMap<String, Uuid>, DbError> {
    let mut ids = BTreeMap::new();
    for operation in operations {
        if let ApplyOperation::CreateObject { local_ref, .. } = operation {
            let local_ref = required_text(local_ref.clone(), "local_ref", 100)?;
            if ids.insert(local_ref.clone(), Uuid::new_v4()).is_some() {
                return Err(DbError::Invalid(format!("duplicate local_ref {local_ref}")));
            }
        }
    }
    Ok(ids)
}

fn validate_new_object_connections(request: &ApplyRequest) -> Result<(), DbError> {
    if request.chat_object_id.is_some() {
        return Ok(());
    }
    let connected = request
        .operations
        .iter()
        .filter_map(|operation| match operation {
            ApplyOperation::CreateConnection { source, target, .. } => Some([source, target]),
            _ => None,
        })
        .flatten()
        .filter_map(|reference| match reference {
            ObjectReference::Local { local_ref } => Some(local_ref.as_str()),
            ObjectReference::Id { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    for operation in &request.operations {
        let ApplyOperation::CreateObject {
            local_ref,
            kind,
            fields,
            ..
        } = operation
        else {
            continue;
        };
        let standalone_note = kind == "note"
            && contract::standalone_note_intent(&string_or(fields, "intent", "idea")?);
        if !standalone_note && !connected.contains(local_ref.as_str()) {
            return Err(DbError::Invalid(format!(
                "new Object {local_ref} requires a Connection"
            )));
        }
    }
    Ok(())
}

fn resolve_reference(
    reference: &ObjectReference,
    local_ids: &BTreeMap<String, Uuid>,
) -> Result<Uuid, DbError> {
    match reference {
        ObjectReference::Id { object_id } => Ok(*object_id),
        ObjectReference::Local { local_ref } => local_ids
            .get(local_ref)
            .copied()
            .ok_or_else(|| DbError::Invalid(format!("unknown local_ref {local_ref}"))),
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_operation(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    operation: &ApplyOperation,
    local_ids: &BTreeMap<String, Uuid>,
    before: Option<&Value>,
    run_id: Option<Uuid>,
    sequence: &mut i64,
    event_ids: &mut Vec<Uuid>,
    authority: WriteAuthority,
) -> Result<Value, DbError> {
    match operation {
        ApplyOperation::CreateObject {
            local_ref,
            kind,
            title,
            description,
            provenance: provenance_value,
            fields,
        } => {
            let id = local_ids[local_ref];
            let value = create_object(
                tx,
                actor,
                id,
                kind,
                title,
                description,
                provenance_value.clone(),
                fields,
            )
            .await?;
            record_event(
                tx, run_id, sequence, event_ids, actor, "object", id, id, "created", None, 1,
            )
            .await?;
            Ok(json!({"operation":"create_object","local_ref":local_ref,"data":value}))
        }
        ApplyOperation::UpdateObject {
            object_id,
            expected_revision,
            changes,
        } => {
            let value = update_object(
                tx,
                actor,
                *object_id,
                *expected_revision,
                changes,
                authority,
            )
            .await?;
            record_event(
                tx,
                run_id,
                sequence,
                event_ids,
                actor,
                "object",
                *object_id,
                *object_id,
                "updated",
                Some(*expected_revision),
                *expected_revision + 1,
            )
            .await?;
            Ok(json!({"operation":"update_object","data":value}))
        }
        ApplyOperation::UpdateDescription {
            object_id,
            expected_revision,
            description,
        } => {
            let value = update_description(
                tx,
                actor,
                *object_id,
                *expected_revision,
                description,
                authority,
            )
            .await?;
            record_event_with_before(
                tx,
                run_id,
                sequence,
                event_ids,
                actor,
                "object",
                *object_id,
                *object_id,
                "updated",
                Some(*expected_revision),
                *expected_revision + 1,
                before,
            )
            .await?;
            Ok(json!({"operation":"update_description","data":value}))
        }
        ApplyOperation::PromoteSourceArtifact {
            source_id,
            expected_revision,
            artifact_id,
            expected_sha256,
        } => {
            let value = promote_source_artifact(
                tx,
                actor,
                *source_id,
                *expected_revision,
                *artifact_id,
                expected_sha256,
                authority,
            )
            .await?;
            record_event_with_before(
                tx,
                run_id,
                sequence,
                event_ids,
                actor,
                "object",
                *source_id,
                *source_id,
                "updated",
                Some(*expected_revision),
                *expected_revision + 1,
                before,
            )
            .await?;
            Ok(json!({"operation":"promote_source_artifact","data":value}))
        }
        ApplyOperation::ArchiveObject {
            object_id,
            expected_revision,
        } => {
            let value =
                archive_object(tx, actor, *object_id, *expected_revision, authority).await?;
            record_event(
                tx,
                run_id,
                sequence,
                event_ids,
                actor,
                "object",
                *object_id,
                *object_id,
                "archived",
                Some(*expected_revision),
                *expected_revision + 1,
            )
            .await?;
            Ok(json!({"operation":"archive_object","data":value}))
        }
        ApplyOperation::CreateConnection {
            source,
            kind,
            target,
            description,
            provenance: provenance_value,
        } => {
            let source_id = resolve_reference(source, local_ids)?;
            let target_id = resolve_reference(target, local_ids)?;
            let value = create_connection(
                tx,
                actor,
                source_id,
                kind,
                target_id,
                description,
                provenance(provenance_value.clone())?,
                run_id,
                sequence,
                event_ids,
                authority,
                false,
            )
            .await?;
            Ok(json!({"operation":"create_connection","data":value}))
        }
        ApplyOperation::UpdateConnection {
            connection_id,
            expected_revision,
            kind,
            description,
            provenance: provenance_value,
        } => {
            let value = update_connection(
                tx,
                actor,
                *connection_id,
                *expected_revision,
                kind.as_deref(),
                description.as_deref(),
                provenance_value.clone(),
                authority,
            )
            .await?;
            record_event(
                tx,
                run_id,
                sequence,
                event_ids,
                actor,
                "connection",
                *connection_id,
                value.source_object_id,
                "updated",
                Some(*expected_revision),
                value.revision,
            )
            .await?;
            Ok(json!({"operation":"update_connection","data":value}))
        }
        ApplyOperation::ArchiveConnection {
            connection_id,
            expected_revision,
        } => {
            let value =
                archive_connection(tx, actor, *connection_id, *expected_revision, authority)
                    .await?;
            record_event(
                tx,
                run_id,
                sequence,
                event_ids,
                actor,
                "connection",
                *connection_id,
                value.source_object_id,
                "archived",
                Some(*expected_revision),
                value.revision,
            )
            .await?;
            Ok(json!({"operation":"archive_connection","data":value}))
        }
        ApplyOperation::AppendArtifact {
            object,
            expected_revision,
            kind,
            title,
            content,
            uri,
            media_type,
            language,
            capture_outcome,
            capture_reason,
            metadata,
            supersedes_artifact_id,
        } => {
            let object_id = resolve_reference(object, local_ids)?;
            let value = append_artifact(
                tx,
                actor,
                object_id,
                *expected_revision,
                kind,
                title.as_deref(),
                content.as_deref(),
                uri.as_deref(),
                media_type.as_deref(),
                language.as_deref(),
                capture_outcome,
                capture_reason.as_deref(),
                metadata,
                *supersedes_artifact_id,
                authority,
            )
            .await?;
            record_event(
                tx,
                run_id,
                sequence,
                event_ids,
                actor,
                "object",
                object_id,
                object_id,
                "artifact_attached",
                Some(*expected_revision),
                *expected_revision + 1,
            )
            .await?;
            Ok(json!({"operation":"append_artifact","data":value}))
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn record_event_with_before(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Option<Uuid>,
    sequence: &mut i64,
    event_ids: &mut Vec<Uuid>,
    actor: &ActorContext,
    entity_type: &str,
    entity_id: Uuid,
    object_id: Uuid,
    action: &str,
    from_revision: Option<i64>,
    to_revision: i64,
    before_state: Option<&Value>,
) -> Result<(), DbError> {
    if let Some(run_id) = run_id {
        let id = db::insert_event_for_run_with_before(
            tx,
            run_id,
            *sequence,
            actor,
            entity_type,
            entity_id,
            object_id,
            action,
            None,
            from_revision,
            to_revision,
            before_state.cloned(),
        )
        .await?;
        event_ids.push(id);
        *sequence += 1;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn record_event(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Option<Uuid>,
    sequence: &mut i64,
    event_ids: &mut Vec<Uuid>,
    actor: &ActorContext,
    entity_type: &str,
    entity_id: Uuid,
    object_id: Uuid,
    action: &str,
    from_revision: Option<i64>,
    to_revision: i64,
) -> Result<(), DbError> {
    if let Some(run_id) = run_id {
        let id = db::insert_event_for_run(
            tx,
            run_id,
            *sequence,
            actor,
            entity_type,
            entity_id,
            object_id,
            action,
            None,
            from_revision,
            to_revision,
        )
        .await?;
        event_ids.push(id);
        *sequence += 1;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn create_object(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    id: Uuid,
    kind: &str,
    title: &str,
    description: &str,
    provenance_value: Option<Value>,
    fields: &Map<String, Value>,
) -> Result<Value, DbError> {
    if !contract::interactive_writable(kind) {
        return Err(DbError::Invalid(format!(
            "{kind} Objects are system-managed"
        )));
    }
    validate_fields(kind, fields, &[])?;
    let title = required_text(title.to_owned(), "title", 300)?;
    let description = object_description(&title, description.to_owned())?;
    let provenance_value = provenance(provenance_value)?;
    sqlx::query(
        r#"INSERT INTO objects
           (id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance)
           VALUES ($1,$2,$3,$4,$5,$6,$5,$6,$7)"#,
    )
    .bind(id)
    .bind(kind)
    .bind(&title)
    .bind(&description)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(&provenance_value)
    .execute(&mut **tx)
    .await?;
    insert_subtype(tx, id, kind, fields).await?;
    db::target_snapshot(tx, "object", id).await
}

async fn insert_subtype(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    kind: &str,
    fields: &Map<String, Value>,
) -> Result<(), DbError> {
    match kind {
        "task" => {
            let status = string_or(fields, "status", "todo")?;
            let priority = string_or(fields, "priority", "medium")?;
            allowed(status.clone(), "status", TASK_STATUSES)?;
            allowed(priority.clone(), "priority", TASK_PRIORITIES)?;
            let blocked_reason = optional_string(fields, "blocked_reason")?;
            if (status == "blocked") != blocked_reason.is_some() {
                return Err(DbError::Invalid(
                    "blocked_reason is required exactly when status is blocked".into(),
                ));
            }
            let completed_at = optional_time(fields, "completed_at")?;
            if (status == "done") != completed_at.is_some() {
                return Err(DbError::Invalid(
                    "completed_at is required exactly when status is done".into(),
                ));
            }
            sqlx::query(
                r#"INSERT INTO tasks
                   (object_id,status,priority,owner_object_id,agent_suitable,blocked_reason,due_at,
                    completed_at,github_issue_url,brief_markdown,work_kind)
                   VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)"#,
            )
            .bind(id)
            .bind(status)
            .bind(priority)
            .bind(optional_uuid(fields, "owner_object_id")?)
            .bind(bool_or(fields, "agent_suitable", false)?)
            .bind(blocked_reason)
            .bind(optional_time(fields, "due_at")?)
            .bind(completed_at)
            .bind(optional_string(fields, "github_issue_url")?)
            .bind(optional_string(fields, "brief_markdown")?)
            .bind(string_or(fields, "work_kind", "general")?)
            .execute(&mut **tx)
            .await?;
        }
        "entity" => {
            let entity_kind = required_field_string(fields, "entity_kind")?;
            allowed(
                entity_kind.clone(),
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
            )?;
            sqlx::query("INSERT INTO entities (object_id,entity_kind) VALUES ($1,$2)")
                .bind(id)
                .bind(entity_kind)
                .execute(&mut **tx)
                .await?;
        }
        "source" => {
            let source_kind = string_or(fields, "source_kind", "other")?;
            allowed(source_kind.clone(), "source_kind", SOURCE_KINDS)?;
            let canonical_uri = optional_string(fields, "canonical_uri")?;
            validate_http_uri(canonical_uri.as_deref(), "canonical_uri")?;
            let published_at = optional_time(fields, "published_at")?;
            let published_at_precision = optional_string(fields, "published_at_precision")?;
            if let Some(value) = published_at_precision.clone() {
                allowed(
                    value,
                    "published_at_precision",
                    &["instant", "day", "month", "year"],
                )?;
            }
            if published_at.is_some() != published_at_precision.is_some() {
                return Err(DbError::Invalid(
                    "published_at and published_at_precision must be provided together".into(),
                ));
            }
            sqlx::query(
                r#"INSERT INTO sources
                   (object_id,source_kind,canonical_uri,byline,publisher,published_at,
                    published_at_precision,last_accessed_at,original_language,
                    original_media_type,original_artifact_reference)
                   VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)"#,
            )
            .bind(id)
            .bind(source_kind)
            .bind(canonical_uri)
            .bind(optional_string(fields, "byline")?)
            .bind(optional_string(fields, "publisher")?)
            .bind(published_at)
            .bind(published_at_precision)
            .bind(optional_time(fields, "last_accessed_at")?)
            .bind(optional_string(fields, "original_language")?)
            .bind(optional_string(fields, "original_media_type")?)
            .bind(optional_string(fields, "original_artifact_reference")?)
            .execute(&mut **tx)
            .await?;
        }
        "note" => {
            let content = required_field_string(fields, "content")?;
            required_text(content.clone(), "content", 100_000)?;
            let content_format = string_or(fields, "content_format", "markdown")?;
            allowed(
                content_format.clone(),
                "content_format",
                NOTE_CONTENT_FORMATS,
            )?;
            let intent = string_or(fields, "intent", "idea")?;
            allowed(intent.clone(), "intent", &["idea", "excerpt", "fact"])?;
            validate_note_evidence(tx, &content, &intent, fields).await?;
            sqlx::query(
                r#"INSERT INTO notes
                   (object_id,content,content_format,intent,source_artifact_id,source_locator)
                   VALUES ($1,$2,$3,$4,$5,$6)"#,
            )
            .bind(id)
            .bind(content)
            .bind(content_format)
            .bind(intent)
            .bind(optional_uuid(fields, "source_artifact_id")?)
            .bind(
                fields
                    .get("source_locator")
                    .cloned()
                    .filter(|value| !value.is_null()),
            )
            .execute(&mut **tx)
            .await?;
        }
        "theme" => {
            let slug = theme_slug(required_field_string(fields, "slug")?)?;
            sqlx::query("INSERT INTO themes (object_id,slug) VALUES ($1,$2)")
                .bind(id)
                .bind(slug)
                .execute(&mut **tx)
                .await?;
        }
        _ => unreachable!("write boundary checked before subtype insertion"),
    }
    Ok(())
}

fn validate_fields(
    kind: &str,
    fields: &Map<String, Value>,
    generic: &[&str],
) -> Result<(), DbError> {
    let allowed_fields = contract::object_fields(kind)
        .ok_or_else(|| DbError::Invalid(format!("unknown Object kind {kind}")))?;
    for key in fields.keys() {
        if !allowed_fields.contains(&key.as_str()) && !generic.contains(&key.as_str()) {
            return Err(DbError::Invalid(format!("unknown field {key} for {kind}")));
        }
    }
    Ok(())
}

async fn lock_writable_object(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    authority: WriteAuthority,
) -> Result<Object, DbError> {
    let object: Object = sqlx::query_as(
        r#"SELECT id,kind,title,description,protected,
           CASE WHEN archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,
           revision,created_by_type,created_by_id,updated_by_type,updated_by_id,
           provenance,created_at,updated_at,archived_at
           FROM objects WHERE id=$1 FOR UPDATE"#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(DbError::NotFound)?;
    if object.lifecycle != "active" {
        return Err(DbError::NotFound);
    }
    if object.protected && authority == WriteAuthority::Ordinary {
        return Err(DbError::Invalid(
            "protected Objects cannot be changed through context_apply".into(),
        ));
    }
    if authority == WriteAuthority::ReviewedMaintenance
        && !matches!(
            object.kind.as_str(),
            "source" | "note" | "entity" | "theme" | "task"
        )
    {
        return Err(DbError::Invalid(
            "maintenance cannot change system-managed Objects".into(),
        ));
    }
    if !contract::interactive_writable(&object.kind) {
        return Err(DbError::Invalid(format!(
            "{} Objects are system-managed",
            object.kind
        )));
    }
    if authority == WriteAuthority::Ordinary {
        crate::domain::validate_object_description(&object.title, &object.description)?;
    }
    Ok(object)
}

async fn update_object(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    id: Uuid,
    expected_revision: i64,
    changes: &Map<String, Value>,
    authority: WriteAuthority,
) -> Result<Value, DbError> {
    let current = lock_writable_object(tx, id, authority).await?;
    if current.revision != expected_revision {
        return Err(DbError::Conflict);
    }
    validate_fields(
        &current.kind,
        changes,
        &["title", "description", "provenance"],
    )?;
    let title = changes
        .get("title")
        .map(value_string)
        .transpose()?
        .unwrap_or(current.title);
    let description = changes
        .get("description")
        .map(value_string)
        .transpose()?
        .unwrap_or(current.description);
    let description = object_description(&title, description)?;
    let provenance_value = changes
        .get("provenance")
        .cloned()
        .map(|value| provenance(Some(value)))
        .transpose()?
        .unwrap_or(current.provenance);
    sqlx::query(
        r#"UPDATE objects SET title=$3,description=$4,provenance=$5,revision=revision+1,
           updated_by_type=$6,updated_by_id=$7,updated_at=now()
           WHERE id=$1 AND revision=$2"#,
    )
    .bind(id)
    .bind(expected_revision)
    .bind(title)
    .bind(description)
    .bind(provenance_value)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .execute(&mut **tx)
    .await?;
    update_subtype(tx, id, &current.kind, changes).await?;
    db::target_snapshot(tx, "object", id).await
}

async fn update_description(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    id: Uuid,
    expected_revision: i64,
    description: &str,
    authority: WriteAuthority,
) -> Result<Value, DbError> {
    if authority != WriteAuthority::ReviewedMaintenance {
        return Err(DbError::Invalid(
            "update_description is available only through reviewed maintenance".into(),
        ));
    }
    let current: Object = sqlx::query_as(
        r#"SELECT id,kind,title,description,protected,
           CASE WHEN archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,
           revision,created_by_type,created_by_id,updated_by_type,updated_by_id,
           provenance,created_at,updated_at,archived_at
           FROM objects WHERE id=$1 FOR UPDATE"#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(DbError::NotFound)?;
    if current.revision != expected_revision {
        return Err(DbError::Conflict);
    }
    let description = object_description(&current.title, description.to_owned())?;
    sqlx::query(
        r#"UPDATE objects SET description=$3,revision=revision+1,
           updated_by_type=$4,updated_by_id=$5,updated_at=now()
           WHERE id=$1 AND revision=$2"#,
    )
    .bind(id)
    .bind(expected_revision)
    .bind(description)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .execute(&mut **tx)
    .await?;
    db::target_snapshot(tx, "object", id).await
}

#[allow(clippy::too_many_arguments)]
async fn promote_source_artifact(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    source_id: Uuid,
    expected_revision: i64,
    artifact_id: Uuid,
    expected_sha256: &str,
    authority: WriteAuthority,
) -> Result<Value, DbError> {
    if authority != WriteAuthority::ReviewedMaintenance {
        return Err(DbError::Invalid(
            "promote_source_artifact is available only through reviewed maintenance".into(),
        ));
    }
    if expected_sha256.len() != 64
        || !expected_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(DbError::Invalid(
            "expected_sha256 must be 64 lowercase hexadecimal characters".into(),
        ));
    }
    let current = lock_writable_object(tx, source_id, authority).await?;
    if current.kind != "source" {
        return Err(DbError::Invalid(
            "promote_source_artifact requires a Source Object".into(),
        ));
    }
    if current.revision != expected_revision {
        return Err(DbError::Conflict);
    }
    let artifact: (Uuid, String, String, Option<String>, bool) = sqlx::query_as(
        r#"SELECT object_id,sha256,capture_outcome,content,semantic_indexing_enabled
           FROM artifacts WHERE id=$1"#,
    )
    .bind(artifact_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(DbError::NotFound)?;
    if artifact.0 != source_id {
        return Err(DbError::Invalid(
            "Artifact does not belong to the selected Source".into(),
        ));
    }
    let Some(content) = artifact
        .3
        .as_deref()
        .filter(|content| !content.trim().is_empty())
    else {
        return Err(DbError::Invalid(
            "promoted Artifact must contain complete nonempty captured text".into(),
        ));
    };
    let content_sha256 = format!("{:x}", Sha256::digest(content.as_bytes()));
    if artifact.1 != expected_sha256 || content_sha256 != expected_sha256 {
        return Err(DbError::Invalid(
            "expected_sha256 does not match the stored Artifact content".into(),
        ));
    }
    if artifact.2 != "complete" {
        return Err(DbError::Invalid(
            "promoted Artifact must contain complete nonempty captured text".into(),
        ));
    }
    if !artifact.4 {
        return Err(DbError::Invalid(
            "promoted Artifact must be eligible for semantic indexing".into(),
        ));
    }
    sqlx::query("UPDATE sources SET current_artifact_id=$2 WHERE object_id=$1")
        .bind(source_id)
        .bind(artifact_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        r#"UPDATE objects SET revision=revision+1,updated_by_type=$3,
           updated_by_id=$4,updated_at=now() WHERE id=$1 AND revision=$2"#,
    )
    .bind(source_id)
    .bind(expected_revision)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .execute(&mut **tx)
    .await?;
    db::target_snapshot(tx, "object", source_id).await
}

async fn update_subtype(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    kind: &str,
    changes: &Map<String, Value>,
) -> Result<(), DbError> {
    if changes
        .keys()
        .all(|key| ["title", "description", "provenance"].contains(&key.as_str()))
    {
        return Ok(());
    }
    let snapshot = db::target_snapshot(tx, "object", id).await?;
    if kind == "task"
        && changes.get("status").and_then(Value::as_str) == Some("doing")
        && snapshot["subtype"]["status"] == "doing"
    {
        return Err(DbError::Invalid(
            "Task is already in progress; read its execution claim".into(),
        ));
    }
    let mut fields = snapshot["subtype"].as_object().cloned().unwrap_or_default();
    for (key, value) in changes {
        if !["title", "description", "provenance"].contains(&key.as_str()) {
            fields.insert(key.clone(), value.clone());
        }
    }
    match kind {
        "task" => {
            let status = required_field_string(&fields, "status")?;
            let priority = required_field_string(&fields, "priority")?;
            allowed(status.clone(), "status", TASK_STATUSES)?;
            allowed(priority.clone(), "priority", TASK_PRIORITIES)?;
            let blocked_reason = optional_string(&fields, "blocked_reason")?;
            if (status == "blocked") != blocked_reason.is_some() {
                return Err(DbError::Invalid(
                    "blocked_reason is required exactly when status is blocked".into(),
                ));
            }
            let completed_at = optional_time(&fields, "completed_at")?;
            if (status == "done") != completed_at.is_some() {
                return Err(DbError::Invalid(
                    "completed_at is required exactly when status is done".into(),
                ));
            }
            sqlx::query(
                r#"UPDATE tasks SET status=$2,priority=$3,owner_object_id=$4,agent_suitable=$5,
                   blocked_reason=$6,due_at=$7,completed_at=$8,github_issue_url=$9,brief_markdown=$10,work_kind=$11
                   WHERE object_id=$1"#,
            )
            .bind(id).bind(status).bind(priority)
            .bind(optional_uuid(&fields, "owner_object_id")?)
            .bind(bool_or(&fields, "agent_suitable", false)?)
            .bind(blocked_reason)
            .bind(optional_time(&fields, "due_at")?)
            .bind(completed_at)
            .bind(optional_string(&fields, "github_issue_url")?)
            .bind(optional_string(&fields, "brief_markdown")?)
            .bind(string_or(&fields, "work_kind", "general")?)
            .execute(&mut **tx).await?;
        }
        "entity" => {
            let entity_kind = required_field_string(&fields, "entity_kind")?;
            allowed(
                entity_kind.clone(),
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
            )?;
            sqlx::query("UPDATE entities SET entity_kind=$2 WHERE object_id=$1")
                .bind(id)
                .bind(entity_kind)
                .execute(&mut **tx)
                .await?;
        }
        "source" => {
            let source_kind = required_field_string(&fields, "source_kind")?;
            allowed(source_kind.clone(), "source_kind", SOURCE_KINDS)?;
            let canonical_uri = optional_string(&fields, "canonical_uri")?;
            validate_http_uri(canonical_uri.as_deref(), "canonical_uri")?;
            let published_at = optional_time(&fields, "published_at")?;
            let published_at_precision = optional_string(&fields, "published_at_precision")?;
            if let Some(value) = published_at_precision.clone() {
                allowed(
                    value,
                    "published_at_precision",
                    &["instant", "day", "month", "year"],
                )?;
            }
            if published_at.is_some() != published_at_precision.is_some() {
                return Err(DbError::Invalid(
                    "published_at and published_at_precision must be provided together".into(),
                ));
            }
            sqlx::query(
                r#"UPDATE sources SET source_kind=$2,canonical_uri=$3,byline=$4,publisher=$5,
                   published_at=$6,published_at_precision=$7,last_accessed_at=$8,
                   original_language=$9,original_media_type=$10,original_artifact_reference=$11
                   WHERE object_id=$1"#,
            )
            .bind(id)
            .bind(source_kind)
            .bind(canonical_uri)
            .bind(optional_string(&fields, "byline")?)
            .bind(optional_string(&fields, "publisher")?)
            .bind(published_at)
            .bind(published_at_precision)
            .bind(optional_time(&fields, "last_accessed_at")?)
            .bind(optional_string(&fields, "original_language")?)
            .bind(optional_string(&fields, "original_media_type")?)
            .bind(optional_string(&fields, "original_artifact_reference")?)
            .execute(&mut **tx)
            .await?;
        }
        "note" => {
            let content = required_field_string(&fields, "content")?;
            let content_format = required_field_string(&fields, "content_format")?;
            let intent = optional_string(&fields, "intent")?;
            allowed(
                content_format.clone(),
                "content_format",
                NOTE_CONTENT_FORMATS,
            )?;
            if let Some(value) = intent.as_deref() {
                allowed(
                    value.to_owned(),
                    "intent",
                    &["idea", "excerpt", "fact", "insight", "question"],
                )?;
                validate_note_evidence(tx, &content, value, &fields).await?;
            } else if optional_uuid(&fields, "source_artifact_id")?.is_some()
                || fields
                    .get("source_locator")
                    .is_some_and(|value| !value.is_null())
            {
                return Err(DbError::Invalid(
                    "unclassified Notes cannot carry excerpt evidence".into(),
                ));
            }
            sqlx::query(
                "UPDATE notes SET content=$2,content_format=$3,intent=$4,source_artifact_id=$5,source_locator=$6 WHERE object_id=$1",
            )
            .bind(id).bind(content).bind(content_format).bind(intent)
            .bind(optional_uuid(&fields, "source_artifact_id")?)
            .bind(fields.get("source_locator").cloned().filter(|value| !value.is_null()))
            .execute(&mut **tx).await?;
        }
        "theme" => {
            let slug = theme_slug(required_field_string(&fields, "slug")?)?;
            sqlx::query("UPDATE themes SET slug=$2 WHERE object_id=$1")
                .bind(id)
                .bind(slug)
                .execute(&mut **tx)
                .await?;
        }
        _ => unreachable!("write boundary checked before subtype update"),
    }
    Ok(())
}

async fn archive_object(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    id: Uuid,
    expected_revision: i64,
    authority: WriteAuthority,
) -> Result<Value, DbError> {
    let current = lock_writable_object(tx, id, authority).await?;
    if current.revision != expected_revision {
        return Err(DbError::Conflict);
    }
    let active_connections: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM connections
           WHERE archived_at IS NULL AND (source_object_id=$1 OR target_object_id=$1)"#,
    )
    .bind(id)
    .fetch_one(&mut **tx)
    .await?;
    if active_connections > 0 {
        return Err(DbError::Invalid(
            "archive an Object's active Connections first, in the same batch if appropriate".into(),
        ));
    }
    sqlx::query(
        r#"UPDATE objects SET archived_at=now(),revision=revision+1,updated_by_type=$3,
           updated_by_id=$4,updated_at=now() WHERE id=$1 AND revision=$2"#,
    )
    .bind(id)
    .bind(expected_revision)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .execute(&mut **tx)
    .await?;
    db::target_snapshot(tx, "object", id).await
}

async fn validate_endpoints(
    tx: &mut Transaction<'_, Postgres>,
    source_id: Uuid,
    kind: &str,
    target_id: Uuid,
    authority: WriteAuthority,
    verified_chat_connection: bool,
) -> Result<(), DbError> {
    if source_id == target_id {
        return Err(DbError::Invalid(
            "source and target Objects must be different".into(),
        ));
    }
    if !contract::connection_kind(kind) {
        return Err(DbError::Invalid(format!(
            "unsupported Connection kind {kind}"
        )));
    }
    let rows: Vec<(Uuid, String, bool, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT id,kind,protected,archived_at FROM objects WHERE id=$1 OR id=$2 FOR UPDATE",
    )
    .bind(source_id)
    .bind(target_id)
    .fetch_all(&mut **tx)
    .await?;
    let source = rows.iter().find(|row| row.0 == source_id);
    let target = rows.iter().find(|row| row.0 == target_id);
    if source.is_none_or(|row| row.3.is_some()) || target.is_none_or(|row| row.3.is_some()) {
        return Err(DbError::Invalid(
            "Connections require active endpoint Objects".into(),
        ));
    }
    if authority == WriteAuthority::Ordinary
        && (source.is_some_and(|row| row.2) || target.is_some_and(|row| row.2))
    {
        // Connecting new research to a protected Source does not edit the Source.
        // Excerpts additionally have to cite an Artifact owned by that Source.
        let research_source_link = target.is_some_and(|row| row.1 == "source")
            && source.is_some_and(|row| !row.2)
            && ((kind == "about" && source.is_some_and(|row| row.1 == "task"))
                || (kind == "derived_from"
                    && source.is_some_and(|row| row.1 == "note")
                    && sqlx::query_scalar::<_, bool>(
                        "SELECT EXISTS (SELECT 1 FROM notes n LEFT JOIN artifacts a ON a.id=n.source_artifact_id WHERE n.object_id=$1 AND (n.intent IN ('idea','fact','insight','question') OR (n.intent='excerpt' AND a.object_id=$2)))",
                    )
                    .bind(source_id)
                    .bind(target_id)
                    .fetch_one(&mut **tx)
                    .await?));
        let chat_provenance_link = verified_chat_connection
            && kind == "about"
            && source.is_some_and(|row| row.1 == "chat")
            && target.is_some_and(|row| !row.2);
        if !research_source_link && !chat_provenance_link {
            return Err(DbError::Invalid(
                "protected Objects require separate Connection authority".into(),
            ));
        }
    }
    if kind == "themed"
        && (source.is_some_and(|row| row.1 == "theme") || target.is_none_or(|row| row.1 != "theme"))
    {
        return Err(DbError::Invalid(
            "themed Connections must point from a non-Theme Object to a Theme".into(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn create_connection(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    source_id: Uuid,
    kind: &str,
    target_id: Uuid,
    description: &str,
    provenance_value: Value,
    run_id: Option<Uuid>,
    sequence: &mut i64,
    event_ids: &mut Vec<Uuid>,
    authority: WriteAuthority,
    verified_chat_connection: bool,
) -> Result<Value, DbError> {
    validate_endpoints(
        tx,
        source_id,
        kind,
        target_id,
        authority,
        verified_chat_connection,
    )
    .await?;
    let description = required_text(description.to_owned(), "description", 1000)?;
    let inserted: Option<Connection> = sqlx::query_as(
        r#"INSERT INTO connections
           (id,source_object_id,kind,target_object_id,description,created_by_type,created_by_id,
            updated_by_type,updated_by_id,provenance,protected)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$6,$7,$8,false)
           ON CONFLICT (source_object_id,kind,target_object_id) WHERE archived_at IS NULL DO NOTHING
           RETURNING *"#,
    )
    .bind(Uuid::new_v4())
    .bind(source_id)
    .bind(kind)
    .bind(target_id)
    .bind(&description)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(&provenance_value)
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(connection) = inserted {
        record_event(
            tx,
            run_id,
            sequence,
            event_ids,
            actor,
            "connection",
            connection.id,
            source_id,
            "created",
            None,
            1,
        )
        .await?;
        return Ok(json!({"connection":connection,"reused":false}));
    }
    let current: Connection = sqlx::query_as(
        r#"SELECT * FROM connections WHERE source_object_id=$1 AND kind=$2
           AND target_object_id=$3 AND archived_at IS NULL FOR UPDATE"#,
    )
    .bind(source_id)
    .bind(kind)
    .bind(target_id)
    .fetch_one(&mut **tx)
    .await?;
    if current.description == description && current.provenance == provenance_value {
        return Ok(json!({"connection":current,"reused":true}));
    }
    if authority == WriteAuthority::ReviewedMaintenance || current.protected {
        return Err(DbError::Invalid(
            "changing an existing Connection requires its explicit ID and expected revision".into(),
        ));
    }
    let mut root = current.provenance.as_object().cloned().unwrap_or_default();
    let mut assertions = root
        .remove("assertions")
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    if assertions.is_empty() {
        assertions.push(json!({"description":current.description,"provenance":current.provenance}));
    }
    let assertion = json!({"description":description,"provenance":provenance_value});
    if !assertions.contains(&assertion) {
        assertions.push(assertion);
    }
    root.insert("assertions".into(), Value::Array(assertions));
    let updated: Connection = sqlx::query_as(
        r#"UPDATE connections SET provenance=$2,revision=revision+1,updated_by_type=$3,
           updated_by_id=$4,updated_at=now() WHERE id=$1 RETURNING *"#,
    )
    .bind(current.id)
    .bind(Value::Object(root))
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .fetch_one(&mut **tx)
    .await?;
    record_event(
        tx,
        run_id,
        sequence,
        event_ids,
        actor,
        "connection",
        updated.id,
        source_id,
        "updated",
        Some(current.revision),
        updated.revision,
    )
    .await?;
    Ok(json!({"connection":updated,"reused":true,"assertion_added":true}))
}

async fn lock_connection(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    authority: WriteAuthority,
) -> Result<Connection, DbError> {
    let connection: Connection =
        sqlx::query_as("SELECT * FROM connections WHERE id=$1 AND archived_at IS NULL FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(DbError::NotFound)?;
    if connection.protected && authority == WriteAuthority::Ordinary {
        return Err(DbError::Invalid(
            "protected Connections require separate authority".into(),
        ));
    }
    Ok(connection)
}

#[allow(clippy::too_many_arguments)]
async fn update_connection(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    id: Uuid,
    expected_revision: i64,
    kind: Option<&str>,
    description: Option<&str>,
    provenance_value: Option<Value>,
    authority: WriteAuthority,
) -> Result<Connection, DbError> {
    let current = lock_connection(tx, id, authority).await?;
    if current.revision != expected_revision {
        return Err(DbError::Conflict);
    }
    let kind = kind.unwrap_or(&current.kind);
    validate_endpoints(
        tx,
        current.source_object_id,
        kind,
        current.target_object_id,
        authority,
        false,
    )
    .await?;
    let description = required_text(
        description.unwrap_or(&current.description).to_owned(),
        "description",
        1000,
    )?;
    let provenance_value = provenance_value
        .map(|value| provenance(Some(value)))
        .transpose()?
        .unwrap_or(current.provenance);
    Ok(sqlx::query_as(
        r#"UPDATE connections SET kind=$3,description=$4,provenance=$5,revision=revision+1,
           updated_by_type=$6,updated_by_id=$7,updated_at=now()
           WHERE id=$1 AND revision=$2 RETURNING *"#,
    )
    .bind(id)
    .bind(expected_revision)
    .bind(kind)
    .bind(description)
    .bind(provenance_value)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .fetch_one(&mut **tx)
    .await?)
}

async fn archive_connection(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    id: Uuid,
    expected_revision: i64,
    authority: WriteAuthority,
) -> Result<Connection, DbError> {
    let current = lock_connection(tx, id, authority).await?;
    if current.revision != expected_revision {
        return Err(DbError::Conflict);
    }
    Ok(sqlx::query_as(
        r#"UPDATE connections SET archived_at=now(),revision=revision+1,updated_by_type=$3,
           updated_by_id=$4,updated_at=now() WHERE id=$1 AND revision=$2 RETURNING *"#,
    )
    .bind(id)
    .bind(expected_revision)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .fetch_one(&mut **tx)
    .await?)
}

#[allow(clippy::too_many_arguments)]
async fn append_artifact(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    object_id: Uuid,
    expected_revision: i64,
    kind: &str,
    title: Option<&str>,
    content: Option<&str>,
    uri: Option<&str>,
    media_type: Option<&str>,
    language: Option<&str>,
    capture_outcome: &str,
    capture_reason: Option<&str>,
    metadata: &Value,
    supersedes_artifact_id: Option<Uuid>,
    authority: WriteAuthority,
) -> Result<Value, DbError> {
    let object = lock_writable_object(tx, object_id, authority).await?;
    if object.revision != expected_revision {
        return Err(DbError::Conflict);
    }
    if content.is_none_or(str::is_empty) && uri.is_none_or(str::is_empty) {
        return Err(DbError::Invalid(
            "Artifact content or URI is required".into(),
        ));
    }
    if !metadata.is_object() {
        return Err(DbError::Invalid(
            "Artifact metadata must be an object".into(),
        ));
    }
    if let Some(predecessor) = supersedes_artifact_id {
        let same_object: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM artifacts WHERE id=$1 AND object_id=$2)",
        )
        .bind(predecessor)
        .bind(object_id)
        .fetch_one(&mut **tx)
        .await?;
        if !same_object {
            return Err(DbError::Invalid(
                "superseded Artifact belongs to another Object".into(),
            ));
        }
    }
    allowed(
        capture_outcome.to_owned(),
        "capture_outcome",
        &[
            "complete",
            "incomplete",
            "unavailable",
            "paywalled",
            "disallowed",
            "too_large",
            "unsupported",
        ],
    )?;
    if capture_outcome == "complete" && (content.is_none() || capture_reason.is_some()) {
        return Err(DbError::Invalid(
            "a complete Artifact requires content and no capture reason".into(),
        ));
    }
    if capture_outcome != "complete" && capture_reason.is_none() {
        return Err(DbError::Invalid(
            "a non-complete Artifact requires a capture reason".into(),
        ));
    }
    let bytes = content
        .unwrap_or_else(|| uri.expect("content or URI validated"))
        .as_bytes();
    let sha256 = format!("{:x}", Sha256::digest(bytes));
    let artifact_id = Uuid::new_v4();
    let artifact: Value = sqlx::query_scalar(
        r#"INSERT INTO artifacts
           (id,object_id,kind,title,content,uri,media_type,language,sha256,size_bytes,
            capture_outcome,capture_reason,metadata,supersedes_artifact_id)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
           RETURNING to_jsonb(artifacts)-'content'"#,
    )
    .bind(artifact_id)
    .bind(object_id)
    .bind(required_text(kind.to_owned(), "kind", 100)?)
    .bind(title)
    .bind(content)
    .bind(uri)
    .bind(media_type)
    .bind(language)
    .bind(sha256)
    .bind(bytes.len() as i64)
    .bind(capture_outcome)
    .bind(capture_reason)
    .bind(metadata)
    .bind(supersedes_artifact_id)
    .fetch_one(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE objects SET revision=revision+1,updated_by_type=$2,updated_by_id=$3,updated_at=now() WHERE id=$1",
    )
    .bind(object_id).bind(actor.actor_type).bind(&actor.actor_id).execute(&mut **tx).await?;
    Ok(artifact)
}

fn validate_http_uri(value: Option<&str>, field: &str) -> Result<(), DbError> {
    if value.is_some_and(|uri| !(uri.starts_with("https://") || uri.starts_with("http://"))) {
        return Err(DbError::Invalid(format!("{field} must use HTTP or HTTPS")));
    }
    Ok(())
}

async fn validate_note_evidence(
    tx: &mut Transaction<'_, Postgres>,
    content: &str,
    intent: &str,
    fields: &Map<String, Value>,
) -> Result<(), DbError> {
    let artifact_id = optional_uuid(fields, "source_artifact_id")?;
    let locator = fields
        .get("source_locator")
        .filter(|value| !value.is_null());
    if intent == "excerpt" && (artifact_id.is_none() || locator.is_none()) {
        return Err(DbError::Invalid(
            "Excerpt Notes require source_artifact_id and source_locator".into(),
        ));
    }
    if locator.is_some_and(|value| !value.is_object()) {
        return Err(DbError::Invalid(
            "source_locator must be a JSON object".into(),
        ));
    }
    if let Some(artifact_id) = artifact_id {
        let evidence: Option<String> =
            sqlx::query_scalar("SELECT content FROM artifacts WHERE id=$1")
                .bind(artifact_id)
                .fetch_optional(&mut **tx)
                .await?
                .flatten();
        let evidence = evidence.ok_or_else(|| {
            DbError::Invalid("source_artifact_id must reference captured text".into())
        })?;
        if intent == "excerpt" && !evidence.contains(content) {
            return Err(DbError::Invalid(
                "Excerpt Note content must occur verbatim in its source Artifact".into(),
            ));
        }
    } else if locator.is_some() {
        return Err(DbError::Invalid(
            "source_locator requires source_artifact_id".into(),
        ));
    }
    Ok(())
}

fn value_string(value: &Value) -> Result<String, DbError> {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| DbError::Invalid("field must be a string".into()))
}

fn required_field_string(fields: &Map<String, Value>, name: &str) -> Result<String, DbError> {
    fields
        .get(name)
        .ok_or_else(|| DbError::Invalid(format!("{name} is required")))
        .and_then(value_string)
}

fn string_or(fields: &Map<String, Value>, name: &str, default: &str) -> Result<String, DbError> {
    fields
        .get(name)
        .map(value_string)
        .transpose()
        .map(|value| value.unwrap_or_else(|| default.to_owned()))
}

fn optional_string(fields: &Map<String, Value>, name: &str) -> Result<Option<String>, DbError> {
    fields
        .get(name)
        .filter(|value| !value.is_null())
        .map(value_string)
        .transpose()
}

fn optional_uuid(fields: &Map<String, Value>, name: &str) -> Result<Option<Uuid>, DbError> {
    optional_string(fields, name)?
        .map(|value| {
            Uuid::parse_str(&value).map_err(|_| DbError::Invalid(format!("{name} must be a UUID")))
        })
        .transpose()
}

fn optional_time(
    fields: &Map<String, Value>,
    name: &str,
) -> Result<Option<OffsetDateTime>, DbError> {
    optional_string(fields, name)?
        .map(|value| {
            OffsetDateTime::parse(&value, &Rfc3339)
                .map_err(|_| DbError::Invalid(format!("{name} must be an RFC3339 timestamp")))
        })
        .transpose()
}

fn bool_or(fields: &Map<String, Value>, name: &str, default: bool) -> Result<bool, DbError> {
    fields
        .get(name)
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| DbError::Invalid(format!("{name} must be a boolean")))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_hash_is_stable() {
        let request = ApplyRequest {
            contract_version: "1.1.0".into(),
            idempotency_key: "test".into(),
            chat_object_id: None,
            validate_only: false,
            operations: vec![ApplyOperation::ArchiveObject {
                object_id: Uuid::nil(),
                expected_revision: 1,
            }],
        };
        assert_eq!(
            request_hash(&request).unwrap(),
            request_hash(&request).unwrap()
        );
    }

    #[test]
    fn unconnected_local_object_is_rejected_without_chat() {
        let request = ApplyRequest {
            contract_version: "1.1.0".into(),
            idempotency_key: "test".into(),
            chat_object_id: None,
            validate_only: false,
            operations: vec![ApplyOperation::CreateObject {
                local_ref: "new".into(),
                kind: "entity".into(),
                title: "Entity".into(),
                description: "A specific Entity description.".into(),
                provenance: None,
                fields: Map::from_iter([("entity_kind".into(), json!("person"))]),
            }],
        };
        assert!(validate_new_object_connections(&request).is_err());
    }
}

//! Exact, owner-reviewed fixture deletion. Never mounted on the agent router.
use std::collections::{BTreeMap, BTreeSet};

use axum::{
    Extension, Json,
    extract::{Query, State},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};

use crate::{domain::ActorContext, intake::IntakeError, maintenance::MaintenanceState};

// These are policy-reviewed SQL identifiers, not caller-controlled SQL.
const TABLES: &[&str] = &[
    "objects",
    "connections",
    "task_routine_runs",
    "task_routines",
    "tasks",
    "chats",
    "chat_messages",
    "users",
    "entities",
    "memories",
    "sources",
    "notes",
    "themes",
    "artifacts",
    "runs",
    "embeddings",
    "object_events",
    "context_apply_requests",
];
const DELETE_ORDER: &[&str] = &[
    "context_apply_requests",
    "object_events",
    "embeddings",
    "connections",
    "runs",
    "chat_messages",
    "task_routine_runs",
    "task_routines",
    "tasks",
    "notes",
    "sources",
    "artifacts",
    "chats",
    "entities",
    "memories",
    "themes",
    "users",
    "objects",
];
const SUBTYPES: &[&str] = &[
    "tasks", "chats", "users", "entities", "memories", "sources", "notes", "themes",
];
const MAX_SNAPSHOT_ROWS: i64 = 100_000;

fn digest(value: &Value) -> String {
    format!("{:x}", Sha256::digest(value.to_string().as_bytes()))
}
fn key(table: &str, row: &Value) -> Value {
    if table == "context_apply_requests" {
        json!({"principal_id":row["principal_id"],"idempotency_key":row["idempotency_key"]})
    } else if table == "task_routines" {
        json!({"task_id":row["task_id"]})
    } else if SUBTYPES.contains(&table) {
        json!({"object_id":row["object_id"]})
    } else {
        json!({"id":row["id"]})
    }
}
fn ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AuditQuery {
    table: String,
    cursor: Option<String>,
    limit: Option<i64>,
}

pub(crate) async fn catalog(
    State(state): State<MaintenanceState>,
) -> Result<Json<Value>, IntakeError> {
    let names: Vec<(String, String, bool)> = sqlx::query_as("SELECT c.relname,c.relkind::text,EXISTS(SELECT 1 FROM pg_depend d WHERE d.classid='pg_class'::regclass AND d.objid=c.oid AND d.deptype='e') FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND c.relkind IN ('r','p','v','m','i') ORDER BY c.relname")
        .fetch_all(&state.app.pool).await?;
    let mut data = Vec::new();
    for (name, kind, extension_owned) in names {
        let table = matches!(kind.as_str(), "r" | "p");
        let count: Option<i64> = if table {
            Some(
                sqlx::query_scalar(&format!("SELECT count(*) FROM public.{}", ident(&name)))
                    .fetch_one(&state.app.pool)
                    .await?,
            )
        } else {
            None
        };
        let keys: Vec<String> = sqlx::query_scalar("SELECT a.attname FROM pg_index i JOIN pg_class c ON c.oid=i.indrelid JOIN pg_namespace n ON n.oid=c.relnamespace CROSS JOIN LATERAL unnest(i.indkey) WITH ORDINALITY k(attnum,ord) JOIN pg_attribute a ON a.attrelid=c.oid AND a.attnum=k.attnum WHERE n.nspname='public' AND c.relname=$1 AND i.indisprimary ORDER BY k.ord")
            .bind(&name).fetch_all(&state.app.pool).await?;
        let category = if extension_owned {
            "extension_bookkeeping"
        } else if name == "_sqlx_migrations"
            || name == "maintenance_purge_receipts"
            || name == "maintenance_execution_fences"
        {
            "bookkeeping"
        } else if table {
            "application_table"
        } else if kind == "i" {
            "derived_index"
        } else {
            "view"
        };
        data.push(json!({"name":name,"category":category,"row_count":count,"primary_key":keys,"purge_policy_supported":TABLES.contains(&name.as_str())}));
    }
    Ok(Json(json!({"data":data})))
}

pub(crate) async fn audit_rows(
    State(state): State<MaintenanceState>,
    Query(query): Query<AuditQuery>,
) -> Result<Json<Value>, IntakeError> {
    let limit = query.limit.unwrap_or(100);
    if !(1..=200).contains(&limit) {
        return Err(IntakeError::BadRequest("limit must be 1..200".into()));
    }
    // Names are first resolved from the server catalog, then identifier-quoted.
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND c.relname=$1 AND c.relkind IN ('r','p'))")
        .bind(&query.table).fetch_one(&state.app.pool).await?;
    if !exists {
        return Err(IntakeError::BadRequest("unknown application table".into()));
    }
    let pk: Vec<String> = sqlx::query_scalar("SELECT a.attname FROM pg_index i JOIN pg_class c ON c.oid=i.indrelid JOIN pg_namespace n ON n.oid=c.relnamespace CROSS JOIN LATERAL unnest(i.indkey) WITH ORDINALITY k(attnum,ord) JOIN pg_attribute a ON a.attrelid=c.oid AND a.attnum=k.attnum WHERE n.nspname='public' AND c.relname=$1 AND i.indisprimary ORDER BY k.ord")
        .bind(&query.table).fetch_all(&state.app.pool).await?;
    if pk.is_empty() {
        return Err(IntakeError::BadRequest(
            "table has no stable primary key; manual policy review required".into(),
        ));
    }
    let key_expr = format!(
        "jsonb_build_object({})",
        pk.iter()
            .map(|k| format!("'{}',t.{}", k.replace('\'', "''"), ident(k)))
            .collect::<Vec<_>>()
            .join(",")
    );
    let sql = format!(
        "SELECT to_jsonb(t),{key_expr} FROM public.{} t WHERE ($1::text IS NULL OR ({key_expr})::text>$1) ORDER BY ({key_expr})::text LIMIT $2",
        ident(&query.table)
    );
    let mut rows: Vec<(Value, Value)> = sqlx::query_as(&sql)
        .bind(query.cursor)
        .bind(limit + 1)
        .fetch_all(&state.app.pool)
        .await?;
    let more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    // PostgreSQL jsonb text spacing differs from serde; ask SQL for the cursor.
    let cursor = if more {
        if let Some((_, k)) = rows.last() {
            Some(
                sqlx::query_scalar::<_, String>("SELECT $1::jsonb::text")
                    .bind(k)
                    .fetch_one(&state.app.pool)
                    .await?,
            )
        } else {
            None
        }
    } else {
        None
    };
    let data: Vec<Value> = rows
        .into_iter()
        .map(|(row, key)| json!({"key":key,"row_sha256":digest(&row),"row":row}))
        .collect();
    Ok(Json(json!({"data":data,"next_cursor":cursor})))
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Selection {
    table: String,
    key: Value,
    row_sha256: String,
    reason: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PurgeRequest {
    idempotency_key: String,
    selections: Vec<Selection>,
    #[serde(default)]
    reconciliations: Vec<Reconciliation>,
    #[serde(default)]
    commit: bool,
    manifest_sha256: Option<String>,
    recovery_export_sha256: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Reconciliation {
    CancelInteraction {
        run_id: uuid::Uuid,
        row_sha256: String,
        chat_id: uuid::Uuid,
        owner_observations: Vec<ExecutorObservation>,
        evidence_sha256: String,
        reason: String,
    },
    RetirePlaceholder {
        embedding_id: uuid::Uuid,
        row_sha256: String,
        replacement_id: uuid::Uuid,
        reason: String,
    },
    DetachChat {
        run_id: uuid::Uuid,
        row_sha256: String,
        chat_id: uuid::Uuid,
        reason: String,
    },
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExecutorObservation {
    context_run_id: uuid::Uuid,
    thread_key: String,
    observed_at: String,
    operation: String,
    identity_origin: String,
    response: ExecutorResponse,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExecutorResponse {
    ok: bool,
    interrupted: bool,
    execution_id: Option<uuid::Uuid>,
    thread_key: String,
}
fn thread_keys(value: &Value, keys: &mut BTreeSet<String>) {
    match value {
        Value::Object(o) => {
            for (k, v) in o {
                if k == "source_thread_id" {
                    if let Some(s) = v.as_str().filter(|s| !s.is_empty()) {
                        keys.insert(s.into());
                    }
                } else {
                    thread_keys(v, keys);
                }
            }
        }
        Value::Array(a) => {
            for v in a {
                thread_keys(v, keys);
            }
        }
        _ => (),
    }
}
fn cancellation_change(
    rows: &Snapshot,
    request: &PurgeRequest,
    action: &Reconciliation,
) -> Result<(Value, Vec<Value>), IntakeError> {
    let Reconciliation::CancelInteraction {
        run_id,
        row_sha256,
        chat_id,
        owner_observations,
        evidence_sha256,
        reason,
    } = action
    else {
        return Err(IntakeError::BadRequest(
            "cancellation action required".into(),
        ));
    };
    let (run_id, chat_id, hash, observations, evidence_sha256) = (
        *run_id,
        *chat_id,
        row_sha256.as_str(),
        owner_observations.as_slice(),
        evidence_sha256.as_str(),
    );
    let run = exact_row(rows, "runs", run_id, Some(hash))?;
    let chat = rows["chats"]
        .iter()
        .find(|r| r["object_id"] == json!(chat_id))
        .ok_or_else(|| IntakeError::Conflict("exact provider Chat missing".into()))?;
    if run["kind"] != "slack_interaction"
        || !matches!(run["status"].as_str(), Some("open" | "running"))
        || run["pinned"] != false
        || run["chat_object_id"] != json!(chat_id)
        || chat["provider"] != "slack"
    {
        return Err(IntakeError::Conflict(
            "only unpinned nonterminal Slack interaction wrappers may be cancelled".into(),
        ));
    }
    for col in ["workspace_id", "channel_id", "thread_id"] {
        if chat[col].as_str().is_none_or(str::is_empty) || run["input"][col] != chat[col] {
            return Err(IntakeError::Conflict(
                "ambiguous provider Chat identity".into(),
            ));
        }
    }
    let observation_json =
        serde_json::to_value(observations).map_err(|e| IntakeError::Internal(e.to_string()))?;
    if observations.is_empty()
        || observations.len() > 20
        || digest(&observation_json) != evidence_sha256
    {
        return Err(IntakeError::Conflict(
            "exact authenticated owner observation digest required".into(),
        ));
    }
    let mut descendants = BTreeSet::from([run_id.to_string()]);
    loop {
        let before = descendants.len();
        for r in &rows["runs"] {
            if r["parent_run_id"]
                .as_str()
                .is_some_and(|id| descendants.contains(id))
            {
                descendants.insert(r["id"].as_str().unwrap().into());
            }
        }
        if before == descendants.len() {
            break;
        }
    }
    let mut recorded_keys = BTreeSet::new();
    for r in &rows["runs"] {
        if r["chat_object_id"] == json!(chat_id)
            || r["id"].as_str().is_some_and(|id| descendants.contains(id))
        {
            thread_keys(&r["trace"], &mut recorded_keys);
        }
    }
    // A child tool trace may use the local Chat/Run UUID as its Context key;
    // do not mistake that proven local identity for an external executor key.
    recorded_keys.retain(|k| k != &chat_id.to_string() && !descendants.contains(k));
    if recorded_keys.iter().any(|k| !k.starts_with("slack:")) {
        return Err(IntakeError::Conflict(
            "unclassified related execution identity requires owner review".into(),
        ));
    }
    let mut observed_keys = BTreeSet::new();
    for o in observations {
        let parts: Vec<_> = o.thread_key.split(':').collect();
        let observed_at = time::OffsetDateTime::parse(
            &o.observed_at,
            &time::format_description::well_known::Rfc3339,
        )
        .map_err(|_| IntakeError::Conflict("invalid owner observation timestamp".into()))?;
        let age = time::OffsetDateTime::now_utc() - observed_at;
        if o.context_run_id != run_id
            || o.operation != "interrupt_active_execution"
            || !o.response.ok
            || o.response.interrupted
            || o.response.execution_id.is_some()
            || o.response.thread_key != o.thread_key
            || age.whole_seconds() < -30
            || age > time::Duration::minutes(30)
            || parts.len() != 5
            || parts[0] != "slack"
            || parts[1] != chat["workspace_id"].as_str().unwrap()
            || parts[2].is_empty()
            || parts[3] != chat["channel_id"].as_str().unwrap()
            || parts[4] != chat["thread_id"].as_str().unwrap()
            || (recorded_keys.is_empty() && o.identity_origin != "runtime_sink_mapping")
            || (!recorded_keys.is_empty() && o.identity_origin != "stored_trace")
            || !observed_keys.insert(o.thread_key.clone())
        {
            return Err(IntakeError::Conflict(
                "owner receipt does not prove no active execution for exact provider thread".into(),
            ));
        }
    }
    if !recorded_keys.is_empty() && observed_keys != recorded_keys {
        return Err(IntakeError::Conflict(
            "owner observations must cover all recorded execution thread identities".into(),
        ));
    }
    let cancellation_ids: BTreeSet<String> = request
        .reconciliations
        .iter()
        .filter_map(|a| {
            if let Reconciliation::CancelInteraction { run_id, .. } = a {
                Some(run_id.to_string())
            } else {
                None
            }
        })
        .collect();
    let mut related_identities =
        vec![json!({"id":run["id"],"kind":run["kind"],"idempotency_key":run["idempotency_key"]})];
    let mut proofs = vec![proof_row("chats", chat)];
    for r in &rows["runs"] {
        if r["id"] == json!(run_id) {
            continue;
        }
        if r["chat_object_id"] == json!(chat_id)
            || r["id"].as_str().is_some_and(|id| descendants.contains(id))
        {
            if r["pinned"] != false
                || (!r["id"]
                    .as_str()
                    .is_some_and(|id| cancellation_ids.contains(id))
                    && !crate::runs::is_terminal(&r["kind"], &r["status"], &r["completed_at"]))
            {
                return Err(IntakeError::Conflict(
                    "active or pinned related execution blocks cancellation".into(),
                ));
            }
            proofs.push(proof_row("runs", r));
            related_identities.push(
                json!({"id":r["id"],"kind":r["kind"],"idempotency_key":r["idempotency_key"]}),
            );
        }
    }
    Ok((
        json!({"action":"cancel_interaction","table":"runs","key":key("runs",run),"before_sha256":digest(run),"reason":reason,"changes":{"status":"cancelled","completed_at":"$commit_time"},"fence":{"run_id":run_id,"run_kind":run["kind"],"idempotency_key":run["idempotency_key"],"chat_object_id":chat_id,"provider":"slack","workspace_id":chat["workspace_id"],"channel_id":chat["channel_id"],"thread_id":chat["thread_id"],"related_run_identities":related_identities},"owner_observations":observation_json,"evidence_sha256":evidence_sha256}),
        proofs,
    ))
}

#[derive(Default)]
struct ReconciliationPlan {
    deletes: Vec<Selection>,
    changes: Vec<Value>,
    proofs: Vec<Value>,
    recovery: Vec<Value>,
}
fn proof_row(table: &str, row: &Value) -> Value {
    json!({"table":table,"key":key(table,row),"row_sha256":digest(row)})
}
fn exact_row<'a>(
    rows: &'a Snapshot,
    table: &str,
    id: uuid::Uuid,
    hash: Option<&str>,
) -> Result<&'a Value, IntakeError> {
    let row = rows[table]
        .iter()
        .find(|r| r["id"] == json!(id))
        .ok_or_else(|| IntakeError::Conflict(format!("required {table} row missing")))?;
    if hash.is_some_and(|h| digest(row) != h) {
        return Err(IntakeError::Conflict(
            "reconciliation row hash is stale".into(),
        ));
    }
    Ok(row)
}
async fn reconciliation_plan(
    tx: &mut Transaction<'_, Postgres>,
    rows: &Snapshot,
    request: &PurgeRequest,
    client: Option<&crate::embeddings::EmbeddingClient>,
) -> Result<ReconciliationPlan, IntakeError> {
    let mut plan = ReconciliationPlan::default();
    if request
        .reconciliations
        .iter()
        .any(|a| matches!(a, Reconciliation::CancelInteraction { .. }))
        && (!request.selections.is_empty()
            || request
                .reconciliations
                .iter()
                .any(|a| !matches!(a, Reconciliation::CancelInteraction { .. })))
    {
        return Err(IntakeError::BadRequest(
            "cancellation requires a separate reviewed request before purge".into(),
        ));
    }
    let mut seen = BTreeSet::new();
    let replacements: Vec<uuid::Uuid> = request
        .reconciliations
        .iter()
        .filter_map(|a| match a {
            Reconciliation::RetirePlaceholder { replacement_id, .. } => Some(*replacement_id),
            _ => None,
        })
        .collect();
    let current: Vec<(uuid::Uuid,String,Option<i32>)> = sqlx::query_as("SELECT e.id,object_embedding_source_hash($2,o.kind,o.title,o.description),vector_dims(e.embedding) FROM embeddings e JOIN objects o ON o.id=e.object_id WHERE e.id=ANY($1)")
        .bind(&replacements).bind(crate::embeddings::OBJECT_EMBEDDING_FORMAT).fetch_all(&mut **tx).await?;
    for action in &request.reconciliations {
        let (table, row, reason) = match action {
            Reconciliation::CancelInteraction {
                run_id,
                row_sha256,
                reason,
                ..
            } => {
                let (change, proofs) = cancellation_change(rows, request, action)?;
                let run = exact_row(rows, "runs", *run_id, Some(row_sha256))?;
                plan.changes.push(change);
                plan.proofs.extend(proofs);
                plan.recovery
                    .push(json!({"table":"runs","key":key("runs",run),"row":run}));
                ("runs", run, reason)
            }
            Reconciliation::RetirePlaceholder {
                embedding_id,
                row_sha256,
                replacement_id,
                reason,
            } => {
                let placeholder = exact_row(rows, "embeddings", *embedding_id, Some(row_sha256))?;
                let replacement = exact_row(rows, "embeddings", *replacement_id, None)?;
                let object_id: uuid::Uuid =
                    serde_json::from_value(placeholder["object_id"].clone())
                        .map_err(|e| IntakeError::Internal(e.to_string()))?;
                let object = exact_row(rows, "objects", object_id, None)?;
                let client = client.ok_or_else(|| {
                    IntakeError::Conflict("configured embedding provider required".into())
                })?;
                let metadata = current
                    .iter()
                    .find(|r| r.0 == *replacement_id)
                    .ok_or_else(|| IntakeError::Conflict("replacement proof missing".into()))?;
                if placeholder["model"] != "__unconfigured__"
                    || !placeholder["artifact_id"].is_null()
                    || placeholder["status"] != "pending"
                    || placeholder["attempts"] != 0
                    || placeholder["dimensions"] != 1
                    || !placeholder["embedding"].is_null()
                    || replacement["object_id"] != placeholder["object_id"]
                    || !replacement["artifact_id"].is_null()
                    || replacement["status"] != "completed"
                    || replacement["model"] != client.model()
                    || replacement["dimensions"] != client.dimensions()
                    || replacement["input_mode"] != client.document_mode()
                    || replacement["format_version"] != crate::embeddings::OBJECT_EMBEDDING_FORMAT
                    || replacement["source_hash"] != metadata.1
                    || metadata.2 != Some(client.dimensions())
                {
                    return Err(IntakeError::Conflict(
                        "placeholder lacks a valid current configured replacement".into(),
                    ));
                }
                for (t, r) in [("objects", object), ("embeddings", replacement)] {
                    let mut proof = proof_row(t, r);
                    proof["must_retain"] = json!(true);
                    plan.proofs.push(proof);
                }
                plan.proofs.push(json!({"embedding_configuration":{"model":client.model(),"dimensions":client.dimensions(),"input_mode":client.document_mode(),"format_version":crate::embeddings::OBJECT_EMBEDDING_FORMAT},"current_source_hash":metadata.1}));
                for (t, r) in [("objects", object), ("embeddings", replacement)] {
                    plan.recovery
                        .push(json!({"table":t,"key":key(t,r),"row":r}));
                }
                plan.deletes.push(Selection {
                    table: "embeddings".into(),
                    key: key("embeddings", placeholder),
                    row_sha256: row_sha256.clone(),
                    reason: reason.clone(),
                });
                ("embeddings", placeholder, reason)
            }
            Reconciliation::DetachChat {
                run_id,
                row_sha256,
                chat_id,
                reason,
            } => {
                let run = exact_row(rows, "runs", *run_id, Some(row_sha256))?;
                let chat = exact_row(rows, "objects", *chat_id, None)?;
                if run["chat_object_id"] != json!(chat_id)
                    || chat["kind"] != "chat"
                    || run["pinned"] != false
                    || !crate::runs::is_terminal(&run["kind"], &run["status"], &run["completed_at"])
                {
                    return Err(IntakeError::Conflict(
                        "detach requires an unpinned terminal Run and its exact Chat".into(),
                    ));
                }
                if !request
                    .selections
                    .iter()
                    .any(|s| s.table == "objects" && s.key == json!({"id":chat_id}))
                    || request
                        .selections
                        .iter()
                        .any(|s| s.table == "runs" && s.key == json!({"id":run_id}))
                {
                    return Err(IntakeError::Conflict(
                        "detach requires discarded Chat and retained Run".into(),
                    ));
                }
                let mut after = run.clone();
                after["chat_object_id"] = Value::Null;
                after
                    .as_object_mut()
                    .expect("Run object")
                    .remove("updated_at");
                plan.changes.push(json!({"action":"detach_chat","table":"runs","key":key("runs",run),"before_sha256":digest(run),"after_sha256_excluding_updated_at":digest(&after),"removed_chat_object_id":chat_id,"reason":reason,"changes":{"chat_object_id":null}}));
                plan.proofs.push(proof_row("objects", chat));
                plan.recovery
                    .push(json!({"table":"runs","key":key("runs",run),"row":run}));
                ("runs", run, reason)
            }
        };
        if reason.trim().is_empty()
            || reason.len() > 2000
            || !seen.insert((table.to_owned(), key(table, row).to_string()))
        {
            return Err(IntakeError::BadRequest(
                "reconciliation needs a unique row and explicit bounded reason".into(),
            ));
        }
    }
    plan.proofs.sort_by_key(Value::to_string);
    plan.proofs.dedup();
    plan.recovery.sort_by_key(Value::to_string);
    plan.recovery.dedup();
    Ok(plan)
}

type Snapshot = BTreeMap<String, Vec<Value>>;
type Selected = BTreeMap<(String, String), String>;

fn selected(set: &Selected, table: &str, row: &Value) -> bool {
    set.contains_key(&(table.to_owned(), key(table, row).to_string()))
}
fn selected_id(set: &Selected, table: &str, id: &Value) -> bool {
    !id.is_null() && set.contains_key(&(table.to_owned(), json!({"id":id}).to_string()))
}
fn add(set: &mut Selected, table: &str, row: &Value, reason: &str) -> bool {
    let k = (table.to_owned(), key(table, row).to_string());
    if let std::collections::btree_map::Entry::Vacant(e) = set.entry(k) {
        e.insert(reason.to_owned());
        true
    } else {
        false
    }
}
async fn snapshot(tx: &mut Transaction<'_, Postgres>) -> Result<Snapshot, IntakeError> {
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT c.relname FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND c.relkind IN ('r','p') AND NOT EXISTS (SELECT 1 FROM pg_depend d WHERE d.classid='pg_class'::regclass AND d.objid=c.oid AND d.deptype='e') ORDER BY c.relname",
    )
    .fetch_all(&mut **tx)
    .await?;
    if names.iter().any(|n| {
        !TABLES.contains(&n.as_str())
            && n != "_sqlx_migrations"
            && n != "maintenance_purge_receipts"
            && n != "maintenance_execution_fences"
    }) {
        return Err(IntakeError::Conflict(
            "application schema changed: purge policy requires review".into(),
        ));
    }
    let mut result = BTreeMap::new();
    let mut count = 0;
    for table in TABLES {
        let rows: Vec<Value> =
            sqlx::query_scalar(&format!("SELECT to_jsonb(t) FROM {table} t LIMIT $1"))
                .bind(MAX_SNAPSHOT_ROWS + 1)
                .fetch_all(&mut **tx)
                .await?;
        count += rows.len();
        if count > MAX_SNAPSHOT_ROWS as usize {
            return Err(IntakeError::BadRequest(
                "audit exceeds bounded purge snapshot; narrow operational policy first".into(),
            ));
        }
        result.insert((*table).into(), rows);
    }
    Ok(result)
}
fn contains_id(value: &Value, ids: &BTreeSet<String>) -> bool {
    match value {
        Value::String(s) => s.match_indices('-').any(|(dash, _)| {
            dash >= 8
                && s.as_bytes().get(dash - 8..dash + 28).is_some_and(|w| {
                    w[13] == b'-'
                        && w[18] == b'-'
                        && w[23] == b'-'
                        && std::str::from_utf8(w).is_ok_and(|candidate| ids.contains(candidate))
                })
        }),
        Value::Array(a) => a.iter().any(|v| contains_id(v, ids)),
        Value::Object(o) => o.values().any(|v| contains_id(v, ids)),
        _ => false,
    }
}
async fn preview(
    tx: &mut Transaction<'_, Postgres>,
    request: &PurgeRequest,
    client: Option<&crate::embeddings::EmbeddingClient>,
) -> Result<Value, IntakeError> {
    let rows = snapshot(tx).await?;
    let plan = reconciliation_plan(tx, &rows, request, client).await?;
    // Every live FK is checked from the actual schema, including composite keys.
    let fks: Vec<(String,String,Vec<String>,Vec<String>)> = sqlx::query_as("SELECT src.relname,dst.relname,array_agg(sa.attname ORDER BY k.ord)::text[],array_agg(da.attname ORDER BY k.ord)::text[] FROM pg_constraint c JOIN pg_class src ON src.oid=c.conrelid JOIN pg_namespace n ON n.oid=src.relnamespace JOIN pg_class dst ON dst.oid=c.confrelid CROSS JOIN LATERAL unnest(c.conkey,c.confkey) WITH ORDINALITY k(s,d,ord) JOIN pg_attribute sa ON sa.attrelid=src.oid AND sa.attnum=k.s JOIN pg_attribute da ON da.attrelid=dst.oid AND da.attnum=k.d WHERE c.contype='f' AND n.nspname='public' GROUP BY c.oid,src.relname,dst.relname ORDER BY src.relname,dst.relname,c.oid").fetch_all(&mut **tx).await?;
    let request = request.clone();
    tokio::task::spawn_blocking(move || preview_rows_with_plan(rows, &request, fks, plan))
        .await
        .map_err(|e| IntakeError::Internal(e.to_string()))?
}
type ForeignKey = (String, String, Vec<String>, Vec<String>);
#[cfg(test)]
fn preview_rows(
    rows: Snapshot,
    request: &PurgeRequest,
    fks: Vec<ForeignKey>,
) -> Result<Value, IntakeError> {
    preview_rows_with_plan(rows, request, fks, ReconciliationPlan::default())
}
fn preview_rows_with_plan(
    rows: Snapshot,
    request: &PurgeRequest,
    fks: Vec<ForeignKey>,
    plan: ReconciliationPlan,
) -> Result<Value, IntakeError> {
    let mut set = Selected::new();
    for s in &request.selections {
        if !TABLES.contains(&s.table.as_str())
            || SUBTYPES.contains(&s.table.as_str())
            || s.table == "embeddings"
            || s.reason.trim().is_empty()
            || s.reason.len() > 2000
        {
            return Err(IntakeError::BadRequest("select canonical fixture rows with an explicit reason; subtype/index rows are owned dependencies".into()));
        }
        let row = rows[&s.table]
            .iter()
            .find(|r| key(&s.table, r) == s.key)
            .ok_or_else(|| IntakeError::Conflict("selected row no longer exists".into()))?;
        if digest(row) != s.row_sha256 {
            return Err(IntakeError::Conflict("selected row hash is stale".into()));
        }
        if !add(&mut set, &s.table, row, &s.reason) {
            return Err(IntakeError::BadRequest("duplicate selection".into()));
        }
    }
    for s in &plan.deletes {
        let row = rows[&s.table]
            .iter()
            .find(|r| key(&s.table, r) == s.key)
            .expect("validated reconciliation");
        if !add(&mut set, &s.table, row, &s.reason) {
            return Err(IntakeError::BadRequest(
                "duplicate reconciled deletion".into(),
            ));
        }
    }
    loop {
        let mut added = false;
        for (table, rs) in &rows {
            for row in rs {
                let reason = if SUBTYPES.contains(&table.as_str())
                    && selected_id(&set, "objects", &row["object_id"])
                {
                    Some("owned subtype of selected fixture Object")
                } else if matches!(table.as_str(), "task_routines" | "task_routine_runs")
                    && selected_id(&set, "objects", &row["task_id"])
                {
                    Some("owned Routine configuration or occurrence")
                } else if table == "connections"
                    && (selected_id(&set, "objects", &row["source_object_id"])
                        || selected_id(&set, "objects", &row["target_object_id"]))
                {
                    Some("incident relationship; neighbouring Objects are retained")
                } else if table == "artifacts" && selected_id(&set, "objects", &row["object_id"]) {
                    Some("immutable evidence owned by selected fixture Object")
                } else if table == "embeddings"
                    && (selected_id(&set, "objects", &row["object_id"])
                        || selected_id(&set, "artifacts", &row["artifact_id"]))
                {
                    Some("derived index owned by selected fixture")
                } else if table == "object_events"
                    && selected_id(
                        &set,
                        if row["target_type"] == "object" {
                            "objects"
                        } else {
                            "connections"
                        },
                        &row["target_id"],
                    )
                {
                    Some(
                        "immutable event targeting selected fixture; containing Run is retained unless separately selected",
                    )
                } else if table == "chat_messages"
                    && selected_id(&set, "objects", &row["chat_object_id"])
                {
                    Some("message owned by explicitly selected fixture Chat")
                } else if table == "context_apply_requests"
                    && selected_id(&set, "runs", &row["run_id"])
                {
                    Some("request replay cache of explicitly selected fixture Run")
                } else {
                    None
                };
                if let Some(reason) = reason {
                    added |= add(&mut set, table, row, reason);
                }
            }
        }
        if !added {
            break;
        }
    }
    let mut blockers = Vec::new();
    for proof in &plan.proofs {
        if proof["must_retain"] != true {
            continue;
        }
        if let Some(table) = proof["table"].as_str()
            && set.contains_key(&(table.into(), proof["key"].to_string()))
        {
            blockers.push(json!({"table":table,"key":proof["key"],"reason":"reconciliation proof must remain retained"}));
        }
    }
    for (src, dst, sc, dc) in fks {
        if !rows.contains_key(&src)
            && rows
                .get(&dst)
                .is_some_and(|rs| rs.iter().any(|r| selected(&set, &dst, r)))
        {
            blockers.push(json!({"table":src,"reason":format!("unmanaged relation has a foreign key to selected {dst}; review outside content purge")}));
        }
        if let (Some(srs), Some(drs)) = (rows.get(&src), rows.get(&dst)) {
            let removed_destinations: Vec<&Value> =
                drs.iter().filter(|r| selected(&set, &dst, r)).collect();
            if removed_destinations.is_empty() {
                continue;
            }
            for original in srs.iter().filter(|r| !selected(&set, &src, r)) {
                let projected = plan
                    .changes
                    .iter()
                    .find(|c| {
                        c["action"] == "detach_chat"
                            && c["table"] == src
                            && c["key"] == key(&src, original)
                    })
                    .map(|_| {
                        let mut r = original.clone();
                        r["chat_object_id"] = Value::Null;
                        r
                    });
                let sr = projected.as_ref().unwrap_or(original);
                if removed_destinations.iter().any(|dr| {
                    sc.iter()
                        .zip(&dc)
                        .all(|(s, d)| !sr[s].is_null() && sr[s] == dr[d])
                }) {
                    blockers.push(json!({"table":src,"key":key(&src,sr),"reason":format!("retained live foreign key references selected {dst}"),"row_sha256":digest(sr)}));
                }
            }
        }
    }
    for event in &rows["object_events"] {
        let target = if event["target_type"] == "object" {
            "objects"
        } else {
            "connections"
        };
        if selected(&set, "object_events", event) && !selected_id(&set, target, &event["target_id"])
        {
            blockers.push(json!({"table":"object_events","key":key("object_events",event),"reason":"cannot erase immutable history of a retained target"}));
        }
    }
    for chat in &rows["chats"] {
        if !selected(&set, "chats", chat) {
            for column in [
                "curation_queued_through_message_id",
                "curated_through_message_id",
            ] {
                if selected_id(&set, "chat_messages", &chat[column]) {
                    blockers.push(json!({"table":"chats","key":key("chats",chat),"reason":format!("retained live Chat cursor {column} references selected message")}));
                }
            }
        }
    }
    let mut export = Vec::new();
    let mut manifest = Vec::new();
    let mut counts = BTreeMap::<String, usize>::new();
    let mut ids = BTreeSet::new();
    for ((table, k), reason) in &set {
        let parsed_key: Value = serde_json::from_str(k).expect("serialized row key");
        let row = rows[table]
            .iter()
            .find(|r| key(table, r) == parsed_key)
            .expect("selected snapshot row");
        if let Some(id) = row["id"].as_str() {
            ids.insert(id.to_owned());
        }
        *counts.entry(table.clone()).or_default() += 1;
        manifest.push(
            json!({"table":table,"key":key(table,row),"row_sha256":digest(row),"reason":reason}),
        );
        export.push(json!({"table":table,"key":key(table,row),"row":row}));
    }
    let mut historical = Vec::new();
    for (table, rs) in &rows {
        for row in rs {
            let mentions_selected = contains_id(row, &ids);
            if table == "task_routine_runs"
                && mentions_selected
                && matches!(row["status"].as_str(), Some("pending" | "running"))
            {
                blockers.push(json!({"table":table,"key":key(table,row),"reason":"active Routine occurrence references selected fixtures; wait for execution to finish"}));
            }
            if table == "runs"
                && mentions_selected
                && !crate::runs::is_terminal(&row["kind"], &row["status"], &row["completed_at"])
            {
                blockers.push(json!({"table":table,"key":key(table,row),"row_sha256":digest(row),"reason":"nonterminal Run references selected fixtures; wait for execution to finish"}));
            }
            if !selected(&set, table, row) && mentions_selected {
                historical.push(json!({"table":table,"key":key(table,row),"row_sha256":digest(row),"reason":"retained reference or historical mention; original payload is preserved; live dependencies are separately blocked"}));
            }
        }
    }
    historical.sort_by_key(Value::to_string);
    blockers.sort_by_key(Value::to_string);
    blockers.dedup();
    export.extend(plan.recovery);
    export.sort_by_key(Value::to_string);
    export.dedup();
    let export = json!(export);
    let result = json!({"policy_version":2,"changes":plan.changes,"proofs":plan.proofs,"rows":manifest,"counts":counts,"blockers":blockers,"retained_references":historical,"recovery_export_sha256":digest(&export)});
    Ok(json!({"manifest_sha256":digest(&result),"manifest":result,"recovery_export":export}))
}

pub(crate) async fn purge(
    State(state): State<MaintenanceState>,
    Extension(actor): Extension<ActorContext>,
    Json(request): Json<PurgeRequest>,
) -> Result<Json<Value>, IntakeError> {
    tokio::time::timeout(std::time::Duration::from_secs(25), execute_purge(state, actor, request))
        .await.map_err(|_| IntakeError::Conflict("maintenance operation exceeded 25-second budget; retry the same request key to reconcile its receipt".into()))?
}
async fn execute_purge(
    state: MaintenanceState,
    actor: ActorContext,
    request: PurgeRequest,
) -> Result<Json<Value>, IntakeError> {
    if request.idempotency_key.trim().is_empty()
        || request.idempotency_key.len() > 300
        || request.selections.len() + request.reconciliations.len() == 0
        || request.selections.len() + request.reconciliations.len() > 1000
    {
        return Err(IntakeError::BadRequest(
            "require idempotency key and 1..1000 exact selections".into(),
        ));
    }
    let request_hash =
        digest(&serde_json::to_value(&request).map_err(|e| IntakeError::Internal(e.to_string()))?);
    let mut tx = state.app.pool.begin().await?;
    sqlx::query("SET LOCAL statement_timeout='10s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL lock_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    if request.commit {
        let approved = request.manifest_sha256.as_ref().ok_or_else(|| {
            IntakeError::Forbidden("commit requires reviewed manifest hash".into())
        })?;
        if !state.config.approved_request_hashes.contains(approved) {
            return Err(IntakeError::Forbidden(
                "exact purge manifest has not been approved".into(),
            ));
        }
        // Ingestion checks hold SHARE until commit; this closes the observation/write race.
        sqlx::query("LOCK TABLE maintenance_execution_fences IN EXCLUSIVE MODE")
            .execute(&mut *tx)
            .await?;
        // Serialize receipts and exclude all application writers while checking closure.
        sqlx::query("LOCK TABLE maintenance_purge_receipts IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut *tx)
            .await?;
        let previous: Option<(String,Value)>=sqlx::query_as("SELECT request_sha256,receipt FROM maintenance_purge_receipts WHERE principal_id=$1 AND idempotency_key=$2").bind(&actor.actor_id).bind(&request.idempotency_key).fetch_optional(&mut *tx).await?;
        if let Some((hash, receipt)) = previous {
            if hash != request_hash {
                return Err(IntakeError::Conflict(
                    "purge key was used for a different request".into(),
                ));
            }
            tx.rollback().await?;
            return Ok(Json(json!({"data":receipt,"replayed":true})));
        }
        sqlx::query(&format!(
            "LOCK TABLE {} IN SHARE ROW EXCLUSIVE MODE",
            TABLES.join(",")
        ))
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *tx)
            .await?;
    }
    let result = preview(&mut tx, &request, state.app.embeddings.as_ref()).await?;
    if !request.commit {
        tx.rollback().await?;
        return Ok(Json(json!({"data":result})));
    }
    if result["manifest_sha256"].as_str() != request.manifest_sha256.as_deref()
        || result["manifest"]["recovery_export_sha256"].as_str()
            != request.recovery_export_sha256.as_deref()
    {
        return Err(IntakeError::Conflict(
            "manifest or recovery export is stale; preview and review again".into(),
        ));
    }
    if !result["manifest"]["blockers"]
        .as_array()
        .is_some_and(Vec::is_empty)
    {
        return Err(IntakeError::Conflict(
            "retained live dependencies block purge".into(),
        ));
    }
    sqlx::query(
        "CREATE TEMP TABLE context_reviewed_reconciliation(run_id uuid PRIMARY KEY) ON COMMIT DROP",
    )
    .execute(&mut *tx)
    .await?;
    let mut applied_changes = Vec::new();
    for change in result["manifest"]["changes"].as_array().expect("changes") {
        sqlx::query("INSERT INTO pg_temp.context_reviewed_reconciliation VALUES($1::text::uuid)")
            .bind(change["key"]["id"].as_str())
            .execute(&mut *tx)
            .await?;
        let sql = if change["action"] == "cancel_interaction" {
            let f = &change["fence"];
            sqlx::query("INSERT INTO maintenance_execution_fences(run_id,run_kind,idempotency_key,chat_object_id,provider,workspace_id,channel_id,thread_id,owner_evidence,principal_id,related_run_identities) VALUES($1::text::uuid,'slack_interaction',$2,$3::text::uuid,'slack',$4,$5,$6,$7,$8,$9)")
                .bind(f["run_id"].as_str()).bind(f["idempotency_key"].as_str()).bind(f["chat_object_id"].as_str()).bind(f["workspace_id"].as_str()).bind(f["channel_id"].as_str()).bind(f["thread_id"].as_str()).bind(&change["owner_observations"]).bind(&actor.actor_id).bind(&f["related_run_identities"]).execute(&mut *tx).await?;
            "UPDATE runs SET status='cancelled',completed_at=now(),updated_at=now() WHERE id=$1::text::uuid RETURNING to_jsonb(runs)"
        } else {
            "UPDATE runs SET chat_object_id=NULL,updated_at=now() WHERE id=$1::text::uuid RETURNING to_jsonb(runs)"
        };
        let actual: Value = sqlx::query_scalar(sql)
            .bind(change["key"]["id"].as_str())
            .fetch_one(&mut *tx)
            .await?;
        let mut recorded = change.clone();
        recorded["after_sha256"] = json!(digest(&actual));
        recorded["committed_at"] = actual["updated_at"].clone();
        applied_changes.push(recorded);
    }
    sqlx::query("CREATE TEMP TABLE context_reviewed_purge(table_name text NOT NULL,row_id uuid NOT NULL,PRIMARY KEY(table_name,row_id)) ON COMMIT DROP").execute(&mut *tx).await?;
    let manifest = result["manifest"]["rows"]
        .as_array()
        .expect("manifest rows");
    for row in manifest {
        if matches!(
            row["table"].as_str(),
            Some("artifacts" | "object_events" | "chats")
        ) {
            sqlx::query("INSERT INTO pg_temp.context_reviewed_purge VALUES($1,$2::text::uuid)")
                .bind(row["table"].as_str())
                .bind(
                    row["key"]["id"]
                        .as_str()
                        .or_else(|| row["key"]["object_id"].as_str()),
                )
                .execute(&mut *tx)
                .await?;
        }
    }
    // Selected Chats and their owned messages form a RESTRICT-FK cycle.
    // Break only these soon-to-be-deleted rows, after approval/staleness checks;
    // the recovery export above retains original cursors and rollback restores them.
    let chat_keys: Vec<Value> = manifest
        .iter()
        .filter(|r| r["table"] == "chats")
        .map(|r| r["key"].clone())
        .collect();
    if !chat_keys.is_empty() {
        let count = sqlx::query("UPDATE chats t SET curation_queued_through_message_id=NULL, curated_through_message_id=NULL FROM jsonb_to_recordset($1::jsonb) AS k(object_id uuid) WHERE t.object_id=k.object_id")
            .bind(json!(chat_keys)).execute(&mut *tx).await?.rows_affected();
        if count != chat_keys.len() as u64 {
            return Err(IntakeError::Conflict(
                "selected Chat count changed; transaction rolled back".into(),
            ));
        }
    }
    for table in DELETE_ORDER {
        let keys: Vec<Value> = manifest
            .iter()
            .filter(|r| r["table"] == *table)
            .map(|r| r["key"].clone())
            .collect();
        if keys.is_empty() {
            continue;
        }
        // Match only typed primary keys: serializing immutable Event payloads here
        // multiplies large history by the number of selected keys and prevents
        // indexed lookup. Table and column names come solely from policy constants.
        let (columns, predicate) = if *table == "context_apply_requests" {
            (
                "principal_id text, idempotency_key text",
                "t.principal_id = k.principal_id AND t.idempotency_key = k.idempotency_key",
            )
        } else if *table == "task_routines" {
            ("task_id uuid", "t.task_id = k.task_id")
        } else if SUBTYPES.contains(table) {
            ("object_id uuid", "t.object_id = k.object_id")
        } else {
            ("id uuid", "t.id = k.id")
        };
        let count = sqlx::query(&format!(
            "DELETE FROM {table} t USING jsonb_to_recordset($1::jsonb) AS k({columns}) WHERE {predicate}"
        ))
        .bind(json!(keys))
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if count != keys.len() as u64 {
            return Err(IntakeError::Conflict(
                "purge row count changed; transaction rolled back".into(),
            ));
        }
    }
    // Force deferred schema constraints before emitting the durable receipt.
    sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
        .execute(&mut *tx)
        .await?;
    let receipt = json!({"changes":applied_changes,"proofs":result["manifest"]["proofs"],"actor_id":actor.actor_id,"recorded_at":time::OffsetDateTime::now_utc().to_string(),"manifest_sha256":result["manifest_sha256"],"counts":result["manifest"]["counts"],"rows":manifest.iter().map(|r|json!({"table":r["table"],"key":r["key"],"row_sha256":r["row_sha256"]})).collect::<Vec<_>>(),"retained_reference_count":result["manifest"]["retained_references"].as_array().map(Vec::len),"recovery_export_sha256":result["manifest"]["recovery_export_sha256"]});
    sqlx::query("INSERT INTO maintenance_purge_receipts(principal_id,idempotency_key,request_sha256,manifest_sha256,receipt) VALUES($1,$2,$3,$4,$5)").bind(&actor.actor_id).bind(&request.idempotency_key).bind(request_hash).bind(request.manifest_sha256).bind(&receipt).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({"data":receipt,"replayed":false})))
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;
    use uuid::Uuid;

    #[test]
    fn fifteen_thousand_rows_and_large_history_fit_preview_budget() {
        let mut rows: Snapshot = TABLES
            .iter()
            .map(|t| ((*t).to_owned(), Vec::new()))
            .collect();
        for i in 0..1000_u128 {
            let object = Uuid::from_u128(i + 1);
            let run = Uuid::from_u128(i + 2001);
            rows.get_mut("objects").unwrap().push(json!({"id":object,"kind":"note","revision":1,"description":"Synthetic concise research fixture."}));
            rows.get_mut("notes")
                .unwrap()
                .push(json!({"object_id":object,"content":"Exact original synthetic wording"}));
            rows.get_mut("runs").unwrap().push(json!({"id":run,"status":"completed","input":{"object_id":object,"text":"Research history without UUIDs. ".repeat(2048)}}));
            rows.get_mut("embeddings").unwrap().push(json!({"id":Uuid::from_u128(i+30001),"object_id":object,"artifact_id":null,"embedding":"[0.1,0.2]"}));
            rows.get_mut("connections").unwrap().push(json!({"id":Uuid::from_u128(i+4001),"source_object_id":object,"target_object_id":Uuid::from_u128((i+1)%1000+1)}));
            for n in 0..10_u128 {
                rows.get_mut("object_events").unwrap().push(json!({"id":Uuid::from_u128(i*10+n+10001),"run_id":run,"target_type":"object","target_id":object,"after_state":{"object_id":object}}));
            }
        }
        assert_eq!(rows.values().map(Vec::len).sum::<usize>(), 15_000);
        let selections = rows["objects"]
            .iter()
            .take(50)
            .map(|row| Selection {
                table: "objects".into(),
                key: key("objects", row),
                row_sha256: digest(row),
                reason: "Synthetic disposable performance fixture".into(),
            })
            .collect();
        let request = PurgeRequest {
            idempotency_key: "synthetic-performance".into(),
            selections,
            reconciliations: vec![],
            commit: false,
            manifest_sha256: None,
            recovery_export_sha256: None,
        };
        let fks = [
            ("notes", "objects", "object_id", "id"),
            ("object_events", "runs", "run_id", "id"),
            ("embeddings", "objects", "object_id", "id"),
            ("connections", "objects", "source_object_id", "id"),
            ("connections", "objects", "target_object_id", "id"),
        ]
        .into_iter()
        .map(|(s, d, sc, dc)| (s.into(), d.into(), vec![sc.into()], vec![dc.into()]))
        .collect();
        let started = Instant::now();
        let preview = preview_rows(rows, &request, fks).unwrap();
        assert_eq!(preview["manifest"]["counts"]["objects"], 50);
        assert!(
            preview["manifest"]["blockers"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(
            started.elapsed().as_secs() < 5,
            "analysis exceeded 5 seconds: {:?}",
            started.elapsed()
        );
        eprintln!(
            "15,000-row preview with 64MB history: {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn reference_scan_finds_embedded_uuid_without_repeated_payload_scans() {
        let id = Uuid::from_u128(7).to_string();
        let ids = BTreeSet::from([id.clone()]);
        assert!(contains_id(
            &json!({"text":format!("prefix{id}suffix")}),
            &ids
        ));
        assert!(!contains_id(
            &json!("A non-ASCII café with no reference."),
            &ids
        ));
    }
}

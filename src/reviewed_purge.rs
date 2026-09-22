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
    let names: Vec<(String, String)> = sqlx::query_as("SELECT c.relname,c.relkind::text FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND c.relkind IN ('r','p','v','m','i') ORDER BY c.relname")
        .fetch_all(&state.app.pool).await?;
    let mut data = Vec::new();
    for (name, kind) in names {
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
        let category = if name == "_sqlx_migrations" || name == "maintenance_purge_receipts" {
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
    commit: bool,
    manifest_sha256: Option<String>,
    recovery_export_sha256: Option<String>,
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
        "SELECT tablename FROM pg_tables WHERE schemaname='public' ORDER BY tablename",
    )
    .fetch_all(&mut **tx)
    .await?;
    if names.iter().any(|n| {
        !TABLES.contains(&n.as_str())
            && n != "_sqlx_migrations"
            && n != "maintenance_purge_receipts"
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
        Value::String(s) => ids.iter().any(|id| s.contains(id)),
        Value::Array(a) => a.iter().any(|v| contains_id(v, ids)),
        Value::Object(o) => o.values().any(|v| contains_id(v, ids)),
        _ => false,
    }
}
async fn preview(
    tx: &mut Transaction<'_, Postgres>,
    request: &PurgeRequest,
) -> Result<Value, IntakeError> {
    let rows = snapshot(tx).await?;
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
    loop {
        let mut added = false;
        for (table, rs) in &rows {
            for row in rs {
                let reason = if SUBTYPES.contains(&table.as_str())
                    && selected_id(&set, "objects", &row["object_id"])
                {
                    Some("owned subtype of selected fixture Object")
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
    // Every live FK is checked from the actual schema, including composite keys.
    let fks: Vec<(String,String,Vec<String>,Vec<String>)> = sqlx::query_as("SELECT src.relname,dst.relname,array_agg(sa.attname ORDER BY k.ord)::text[],array_agg(da.attname ORDER BY k.ord)::text[] FROM pg_constraint c JOIN pg_class src ON src.oid=c.conrelid JOIN pg_namespace n ON n.oid=src.relnamespace JOIN pg_class dst ON dst.oid=c.confrelid CROSS JOIN LATERAL unnest(c.conkey,c.confkey) WITH ORDINALITY k(s,d,ord) JOIN pg_attribute sa ON sa.attrelid=src.oid AND sa.attnum=k.s JOIN pg_attribute da ON da.attrelid=dst.oid AND da.attnum=k.d WHERE c.contype='f' AND n.nspname='public' GROUP BY c.oid,src.relname,dst.relname ORDER BY src.relname,dst.relname,c.oid").fetch_all(&mut **tx).await?;
    for (src, dst, sc, dc) in fks {
        if let (Some(srs), Some(drs)) = (rows.get(&src), rows.get(&dst)) {
            for sr in srs.iter().filter(|r| !selected(&set, &src, r)) {
                if drs.iter().filter(|r| selected(&set, &dst, r)).any(|dr| {
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
                "last_ingested_message_id",
                "last_queued_message_id",
                "last_curated_message_id",
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
            if !selected(&set, table, row) && contains_id(row, &ids) {
                historical.push(json!({"table":table,"key":key(table,row),"row_sha256":digest(row),"reason":"retained reference or historical mention; original payload is preserved; live dependencies are separately blocked"}));
            }
        }
    }
    historical.sort_by_key(Value::to_string);
    blockers.sort_by_key(Value::to_string);
    blockers.dedup();
    let export = json!(export);
    let result = json!({"policy_version":1,"rows":manifest,"counts":counts,"blockers":blockers,"retained_references":historical,"recovery_export_sha256":digest(&export)});
    Ok(json!({"manifest_sha256":digest(&result),"manifest":result,"recovery_export":export}))
}

pub(crate) async fn purge(
    State(state): State<MaintenanceState>,
    Extension(actor): Extension<ActorContext>,
    Json(request): Json<PurgeRequest>,
) -> Result<Json<Value>, IntakeError> {
    if request.idempotency_key.trim().is_empty()
        || request.idempotency_key.len() > 300
        || request.selections.is_empty()
        || request.selections.len() > 1000
    {
        return Err(IntakeError::BadRequest(
            "require idempotency key and 1..1000 exact selections".into(),
        ));
    }
    let request_hash =
        digest(&serde_json::to_value(&request).map_err(|e| IntakeError::Internal(e.to_string()))?);
    let mut tx = state.app.pool.begin().await?;
    sqlx::query("SET LOCAL statement_timeout='30s'")
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
    let result = preview(&mut tx, &request).await?;
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
    sqlx::query("CREATE TEMP TABLE context_reviewed_purge(table_name text NOT NULL,row_id uuid NOT NULL,PRIMARY KEY(table_name,row_id)) ON COMMIT DROP").execute(&mut *tx).await?;
    let manifest = result["manifest"]["rows"]
        .as_array()
        .expect("manifest rows");
    for row in manifest {
        if matches!(row["table"].as_str(), Some("artifacts" | "object_events")) {
            sqlx::query("INSERT INTO pg_temp.context_reviewed_purge VALUES($1,$2::text::uuid)")
                .bind(row["table"].as_str())
                .bind(row["key"]["id"].as_str())
                .execute(&mut *tx)
                .await?;
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
        let count=sqlx::query(&format!("DELETE FROM {table} t WHERE EXISTS (SELECT 1 FROM jsonb_array_elements($1::jsonb) k WHERE to_jsonb(t) @> k)")).bind(json!(keys)).execute(&mut *tx).await?.rows_affected();
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
    let receipt = json!({"manifest_sha256":result["manifest_sha256"],"counts":result["manifest"]["counts"],"rows":manifest.iter().map(|r|json!({"table":r["table"],"key":r["key"],"row_sha256":r["row_sha256"]})).collect::<Vec<_>>(),"retained_reference_count":result["manifest"]["retained_references"].as_array().map(Vec::len),"recovery_export_sha256":result["manifest"]["recovery_export_sha256"]});
    sqlx::query("INSERT INTO maintenance_purge_receipts(principal_id,idempotency_key,request_sha256,manifest_sha256,receipt) VALUES($1,$2,$3,$4,$5)").bind(&actor.actor_id).bind(&request.idempotency_key).bind(request_hash).bind(request.manifest_sha256).bind(&receipt).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({"data":receipt,"replayed":false})))
}

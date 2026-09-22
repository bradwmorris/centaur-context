use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use centaur_context::{
    api::{AppState, agent_router},
    config::{EmbeddingConfig, EmbeddingInputMode, TextSearchConfig},
    db,
    embeddings::EmbeddingClient,
    intake,
    maintenance::MaintenanceConfig,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

const TOKEN: &str = "synthetic-reviewed-purge-maintenance-token";
fn state(pool: &PgPool) -> AppState {
    AppState {
        pool: pool.clone(),
        embeddings: Some(
            EmbeddingClient::new(&EmbeddingConfig {
                endpoint: "http://127.0.0.1:1/never-called".into(),
                api_token: "synthetic".into(),
                model: "reconciliation-test".into(),
                dimensions: 2,
                input_mode: EmbeddingInputMode::Shared,
                poll_interval: std::time::Duration::from_secs(60),
            })
            .unwrap(),
        ),
        text_search_config: TextSearchConfig::SIMPLE,
    }
}
fn app(pool: &PgPool, hash: Option<&str>) -> Router {
    intake::router_with_maintenance(
        state(pool),
        "synthetic-intake-token".into(),
        None,
        Some(MaintenanceConfig {
            api_token: TOKEN.into(),
            allowed_principal: "fixture-reviewer".into(),
            approved_request_hashes: hash.into_iter().map(str::to_owned).collect(),
        }),
    )
}
async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: &str,
    value: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("authorization", format!("Bearer {token}"))
                .header("x-centaur-principal-id", "fixture-reviewer")
                .header("x-centaur-thread-key", "synthetic-purge-review")
                .header("content-type", "application/json")
                .body(Body::from(value.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}
async fn object(pool: &PgPool, kind: &str) -> Uuid {
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,$2,'Synthetic purge fixture','Synthetic research fixture used solely to verify reviewed deletion.','system','fixture','system','fixture')").bind(id).bind(kind).execute(&mut *tx).await.unwrap();
    let sql = match kind {
        "source" => "INSERT INTO sources(object_id,source_kind) VALUES($1,'article')",
        "note" => {
            "INSERT INTO notes(object_id,content,content_format) VALUES($1,'Exact original synthetic wording','plain_text')"
        }
        "chat" => "INSERT INTO chats(object_id) VALUES($1)",
        "user" => "INSERT INTO users(object_id,user_kind) VALUES($1,'human')",
        _ => panic!(),
    };
    sqlx::query(sql).bind(id).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    id
}
async fn selection(pool: &PgPool, table: &str, id: Uuid) -> Value {
    assert!(
        [
            "objects",
            "connections",
            "runs",
            "artifacts",
            "object_events",
            "chat_messages"
        ]
        .contains(&table)
    );
    let row: Value = sqlx::query_scalar(&format!("SELECT to_jsonb(t) FROM {table} t WHERE id=$1"))
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
    json!({"table":table,"key":{"id":id},"row_sha256":format!("{:x}",Sha256::digest(row.to_string().as_bytes())),"reason":"Confirmed synthetic fixture in isolated integration test"})
}
fn request(selections: Vec<Value>) -> Value {
    json!({"idempotency_key":Uuid::new_v4().to_string(),"selections":selections})
}
fn commit(mut request: Value, preview: &Value) -> Value {
    request["commit"] = json!(true);
    request["manifest_sha256"] = preview["data"]["manifest_sha256"].clone();
    request["recovery_export_sha256"] =
        preview["data"]["manifest"]["recovery_export_sha256"].clone();
    request
}
async fn exists(pool: &PgPool, id: Uuid) -> bool {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM objects WHERE id=$1)")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn row(pool: &PgPool, table: &str, id: Uuid) -> Value {
    assert!(["objects", "runs", "embeddings", "object_events"].contains(&table));
    sqlx::query_scalar(&format!("SELECT to_jsonb(t) FROM {table} t WHERE id=$1"))
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}
fn hash(row: &Value) -> String {
    format!("{:x}", Sha256::digest(row.to_string().as_bytes()))
}
async fn purge(app: &Router, request: Value) -> (StatusCode, Value) {
    call(app, "POST", "/api/v2/maintenance/purge", TOKEN, request).await
}
async fn pair(pool: &PgPool, object_id: Uuid) -> (Uuid, Uuid) {
    let placeholder = Uuid::new_v4();
    let replacement = Uuid::new_v4();
    sqlx::query("INSERT INTO embeddings(id,object_id,model,dimensions,source_hash,format_version,input_mode,status) SELECT $1,id,'__unconfigured__',1,object_embedding_source_hash('centaur-object-v1',kind,title,description),'centaur-object-v1','shared','pending' FROM objects WHERE id=$2")
        .bind(placeholder).bind(object_id).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO embeddings(id,object_id,model,dimensions,source_hash,format_version,input_mode,status,embedding,completed_at) SELECT $1,id,'reconciliation-test',2,object_embedding_source_hash('centaur-object-v1',kind,title,description),'centaur-object-v1','shared','completed','[1,0]'::vector,now() FROM objects WHERE id=$2")
        .bind(replacement).bind(object_id).execute(pool).await.unwrap();
    (placeholder, replacement)
}
async fn retire(pool: &PgPool, placeholder: Uuid, replacement: Uuid) -> Value {
    json!({"action":"retire_placeholder","embedding_id":placeholder,"row_sha256":hash(&row(pool,"embeddings",placeholder).await),"replacement_id":replacement,"reason":"Untouched obsolete synthetic placeholder with current configured replacement"})
}
async fn detach(pool: &PgPool, run: Uuid, chat: Uuid) -> Value {
    json!({"action":"detach_chat","run_id":run,"row_sha256":hash(&row(pool,"runs",run).await),"chat_id":chat,"reason":"Confirmed fixture Chat; preserve terminal execution history"})
}
#[tokio::test]
async fn reconciliation_requires_current_proof_and_preserves_terminal_history_atomically() {
    let Ok(url) = std::env::var("TEST_DATABASE_URL") else {
        return;
    };
    assert!(url.contains("centaur_context_test"));
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    db::migrate(&pool).await.unwrap();
    let unapproved = app(&pool, None);
    let retained = object(&pool, "note").await;
    let (placeholder, replacement) = pair(&pool, retained).await;
    let mut req = request(vec![]);
    req["reconciliations"] = json!([retire(&pool, placeholder, replacement).await]);
    assert_eq!(
        call(
            &unapproved,
            "POST",
            "/api/v2/maintenance/purge",
            "wrong",
            req.clone()
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        purge(&agent_router(state(&pool), TOKEN.into()), req.clone())
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    // Provider configuration is itself required proof; no HTTP embedding call occurs.
    let mut no_provider = state(&pool);
    no_provider.embeddings = None;
    let no_provider = intake::router_with_maintenance(
        no_provider,
        "synthetic".into(),
        None,
        Some(MaintenanceConfig {
            api_token: TOKEN.into(),
            allowed_principal: "fixture-reviewer".into(),
            approved_request_hashes: Default::default(),
        }),
    );
    assert_eq!(
        purge(&no_provider, req.clone()).await.0,
        StatusCode::CONFLICT
    );
    let mut missing = req.clone();
    missing["reconciliations"][0]["replacement_id"] = json!(Uuid::new_v4());
    assert_eq!(purge(&unapproved, missing).await.0, StatusCode::CONFLICT);
    // All malformed proof states here obey SQL constraints, exercising HTTP validation.
    for change in [
        "model='wrong-model'",
        "dimensions=3,embedding='[1,0,0]'::vector",
        "source_hash=repeat('a',64)",
        "format_version='old-format'",
        "input_mode='search_document'",
        "status='pending',embedding=NULL,completed_at=NULL",
    ] {
        sqlx::query(&format!("UPDATE embeddings SET {change} WHERE id=$1"))
            .bind(replacement)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            purge(&unapproved, req.clone()).await.0,
            StatusCode::CONFLICT,
            "accepted {change}"
        );
        sqlx::query("UPDATE embeddings e SET model='reconciliation-test',dimensions=2,embedding='[1,0]'::vector,status='completed',completed_at=now(),format_version='centaur-object-v1',input_mode='shared',source_hash=object_embedding_source_hash('centaur-object-v1',o.kind,o.title,o.description) FROM objects o WHERE e.id=$1 AND o.id=e.object_id").bind(replacement).execute(&pool).await.unwrap();
    }
    let mut deleting_proof = req.clone();
    deleting_proof["selections"] = json!([selection(&pool, "objects", retained).await]);
    let (status, blocked) = purge(&unapproved, deleting_proof).await;
    assert_eq!(status, StatusCode::OK, "{blocked}");
    assert!(
        !blocked["data"]["manifest"]["blockers"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let (status, preview) = purge(&unapproved, req.clone()).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["data"]["manifest"]["blockers"], json!([]));
    assert_eq!(
        purge(&unapproved, commit(req.clone(), &preview)).await.0,
        StatusCode::FORBIDDEN
    );
    // A replacement change invalidates the approved manifest even while still valid.
    sqlx::query("UPDATE embeddings SET embedding='[0,1]'::vector WHERE id=$1")
        .bind(replacement)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        purge(
            &app(&pool, preview["data"]["manifest_sha256"].as_str()),
            commit(req.clone(), &preview)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    // Bind the full retained Object proof, even when its embedding text is unchanged.
    let (_, object_preview) = purge(&unapproved, req.clone()).await;
    sqlx::query("UPDATE objects SET updated_by_id='synthetic-proof-change' WHERE id=$1")
        .bind(retained)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        purge(
            &app(&pool, object_preview["data"]["manifest_sha256"].as_str()),
            commit(req.clone(), &object_preview)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    // SQL additionally forbids a completed replacement without a dimensionally valid vector.
    assert!(
        sqlx::query("UPDATE embeddings SET embedding=NULL WHERE id=$1")
            .bind(replacement)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE embeddings SET embedding='[1]'::vector WHERE id=$1")
            .bind(replacement)
            .execute(&pool)
            .await
            .is_err()
    );
    // A stale placeholder cannot be reconciled under an old row hash.
    sqlx::query("UPDATE embeddings SET attempts=1 WHERE id=$1")
        .bind(placeholder)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        purge(&unapproved, req.clone()).await.0,
        StatusCode::CONFLICT
    );
    sqlx::query("UPDATE embeddings SET attempts=0 WHERE id=$1")
        .bind(placeholder)
        .execute(&pool)
        .await
        .unwrap();
    req["reconciliations"] = json!([retire(&pool, placeholder, replacement).await]);
    let (_, preview) = purge(&unapproved, req.clone()).await;
    let approved = app(&pool, preview["data"]["manifest_sha256"].as_str());
    let committing = commit(req, &preview);
    let (status, result) = purge(&approved, committing.clone()).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(exists(&pool, retained).await);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT content FROM notes WHERE object_id=$1")
            .bind(retained)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "Exact original synthetic wording"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM embeddings WHERE id=$1")
            .bind(placeholder)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        row(&pool, "embeddings", replacement).await["status"],
        "completed"
    );
    assert_eq!(
        purge(&approved, committing.clone()).await.1["replayed"],
        true
    );
    let mut altered = committing;
    altered["reconciliations"][0]["reason"] = json!("different request");
    assert_eq!(purge(&approved, altered).await.0, StatusCode::CONFLICT);

    let chat = object(&pool, "chat").await;
    let run = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,chat_object_id,input,result,trace,completed_at) VALUES($1,'human_mutation','completed','human','fixture',$2,$3,$4,$5,$6,now())")
        .bind(run).bind(run.to_string()).bind(chat).bind(json!({"original_chat":chat,"words":"exact original words"})).bind(json!({"answer":"real retained result"})).bind(json!([{"history":"retain exactly"}])).execute(&pool).await.unwrap();
    let event = Uuid::new_v4();
    sqlx::query("INSERT INTO object_events(id,run_id,sequence,target_type,target_id,action,actor_type,actor_id,to_revision,after_state,reversible,created_at) VALUES($1,$2,1,'object',$3,'created','human','fixture',1,'{}',false,now())").bind(event).bind(run).bind(retained).execute(&pool).await.unwrap();
    let event_before = row(&pool, "object_events", event).await;
    let mut req = request(vec![selection(&pool, "objects", chat).await]);
    req["reconciliations"] = json!([detach(&pool, run, chat).await]);
    let mut retained_chat = req.clone();
    retained_chat["selections"] = json!([]);
    assert_eq!(
        purge(&unapproved, retained_chat).await.0,
        StatusCode::CONFLICT
    );
    for change in ["pinned=true", "status='running',completed_at=NULL"] {
        sqlx::query(&format!("UPDATE runs SET {change} WHERE id=$1"))
            .bind(run)
            .execute(&pool)
            .await
            .unwrap();
        req["reconciliations"] = json!([detach(&pool, run, chat).await]);
        assert_eq!(
            purge(&unapproved, req.clone()).await.0,
            StatusCode::CONFLICT,
            "accepted {change}"
        );
        sqlx::query(
            "UPDATE runs SET pinned=false,status='completed',completed_at=now() WHERE id=$1",
        )
        .bind(run)
        .execute(&pool)
        .await
        .unwrap();
    }
    req["reconciliations"] = json!([detach(&pool, run, chat).await]);
    let (_, preview) = purge(&unapproved, req.clone()).await;
    // Explicitly change a real payload after preview; commit must leave Chat intact.
    sqlx::query("UPDATE runs SET result=$2 WHERE id=$1")
        .bind(run)
        .bind(json!({"changed":true}))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        purge(
            &app(&pool, preview["data"]["manifest_sha256"].as_str()),
            commit(req.clone(), &preview)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert!(exists(&pool, chat).await);
    req["reconciliations"] = json!([detach(&pool, run, chat).await]);
    let before = row(&pool, "runs", run).await;
    let (status, preview) = purge(&unapproved, req.clone()).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["data"]["manifest"]["blockers"], json!([]));
    let approved = app(&pool, preview["data"]["manifest_sha256"].as_str());
    let committing = commit(req.clone(), &preview);
    // Induce a late delete failure after Run detachment; remove the trigger before assertions.
    sqlx::query(&format!("CREATE FUNCTION reconciliation95_reject() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF OLD.id='{chat}'::uuid THEN RAISE EXCEPTION 'synthetic rollback'; END IF; RETURN OLD; END $$")).execute(&pool).await.unwrap();
    sqlx::query("CREATE TRIGGER reconciliation95_reject BEFORE DELETE ON objects FOR EACH ROW EXECUTE FUNCTION reconciliation95_reject()").execute(&pool).await.unwrap();
    let failed = purge(&approved, committing.clone()).await;
    sqlx::query("DROP TRIGGER reconciliation95_reject ON objects")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION reconciliation95_reject()")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(failed.0, StatusCode::INTERNAL_SERVER_ERROR, "{}", failed.1);
    assert_eq!(row(&pool, "runs", run).await, before);
    assert!(exists(&pool, chat).await);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM maintenance_purge_receipts WHERE idempotency_key=$1"
        )
        .bind(req["idempotency_key"].as_str())
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    let (status, result) = purge(&approved, committing.clone()).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(!exists(&pool, chat).await);
    let mut after = row(&pool, "runs", run).await;
    assert!(after["chat_object_id"].is_null());
    after["chat_object_id"] = before["chat_object_id"].clone();
    after["updated_at"] = before["updated_at"].clone();
    assert_eq!(
        after, before,
        "only Chat pointer and maintenance timestamp may change"
    );
    assert_eq!(row(&pool, "object_events", event).await, event_before);
    let detail = centaur_context::runs::detail(&pool, run).await.unwrap();
    assert!(detail.maintenance_history.iter().any(|entry| {
        entry["changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|change| change["removed_chat_object_id"] == json!(chat))
    }));
    assert_eq!(purge(&approved, committing).await.1["replayed"], true);
}

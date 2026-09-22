use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use centaur_context::{
    api::{AppState, agent_router},
    config::TextSearchConfig,
    db, intake,
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
        embeddings: None,
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

#[tokio::test]
async fn reviewed_purge_is_exact_atomic_replayable_and_preserves_real_history() {
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
    let fixture = object(&pool, "source").await;
    let retained = object(&pool, "note").await;
    let connection = Uuid::new_v4();
    sqlx::query("INSERT INTO connections(id,source_object_id,kind,target_object_id,description,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,$2,'related_to',$3,'A synthetic test relationship.','system','fixture','system','fixture')").bind(connection).bind(fixture).bind(retained).execute(&pool).await.unwrap();
    let artifact = Uuid::new_v4();
    let text = "Synthetic captured evidence";
    sqlx::query("INSERT INTO artifacts(id,object_id,kind,content,sha256,size_bytes,capture_outcome) VALUES($1,$2,'text',$3,$4,$5,'complete')").bind(artifact).bind(fixture).bind(text).bind(format!("{:x}",Sha256::digest(text))).bind(text.len() as i64).execute(&pool).await.unwrap();
    sqlx::query("UPDATE sources SET current_artifact_id=$2 WHERE object_id=$1")
        .bind(fixture)
        .bind(artifact)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO embeddings(object_id,model,dimensions,source_hash,format_version,input_mode,status) VALUES($1,'synthetic',1,$2,'synthetic','shared','pending')").bind(fixture).bind("a".repeat(64)).execute(&pool).await.unwrap();
    let run = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,input) VALUES($1,'human_mutation','completed','human','fixture',$2,$3)").bind(run).bind(Uuid::new_v4().to_string()).bind(json!({"historical_fixture":fixture})).execute(&pool).await.unwrap();
    for (n, id) in [(1, fixture), (2, retained)] {
        sqlx::query("INSERT INTO object_events(id,run_id,sequence,target_type,target_id,action,actor_type,actor_id,to_revision,after_state,reversible,created_at) VALUES($1,$2,$3,'object',$4,'created','human','fixture',1,'{}',false,now())").bind(Uuid::new_v4()).bind(run).bind(n).bind(id).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO context_apply_requests(principal_id,idempotency_key,request_hash,contract_version,run_id,response,completed_at) VALUES('fixture',$1,$2,'1.1.0',$3,$4,now())").bind(Uuid::new_v4().to_string()).bind("b".repeat(64)).bind(run).bind(json!({"historical_fixture":fixture})).execute(&pool).await.unwrap();
    let chat = object(&pool, "chat").await;
    let user = object(&pool, "user").await;
    let message = Uuid::new_v4();
    sqlx::query("INSERT INTO chat_messages(id,chat_object_id,provider_message_id,sender_user_object_id,content,source_created_at) VALUES($1,$2,$3,$4,$5,now())").bind(message).bind(chat).bind(message.to_string()).bind(user).bind(format!("Real discussion included fixture {fixture}; retain my words.")).execute(&pool).await.unwrap();
    let original: Value = sqlx::query_scalar("SELECT to_jsonb(n) FROM notes n WHERE object_id=$1")
        .bind(retained)
        .fetch_one(&pool)
        .await
        .unwrap();
    let req = request(vec![selection(&pool, "objects", fixture).await]);
    let unapproved = app(&pool, None);
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
        call(
            &agent_router(state(&pool), TOKEN.into()),
            "POST",
            "/api/v2/maintenance/purge",
            TOKEN,
            req.clone()
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, preview) = call(
        &unapproved,
        "POST",
        "/api/v2/maintenance/purge",
        TOKEN,
        req.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let m = &preview["data"]["manifest"];
    assert_eq!(m["counts"]["objects"], 1);
    assert_eq!(m["counts"]["sources"], 1);
    assert_eq!(m["counts"]["artifacts"], 1);
    assert_eq!(m["counts"]["embeddings"], 1);
    assert_eq!(m["counts"]["connections"], 1);
    assert_eq!(m["counts"]["object_events"], 1);
    assert!(m["blockers"].as_array().unwrap().is_empty());
    assert!(m["retained_references"].as_array().unwrap().len() >= 3);
    assert_eq!(
        call(
            &unapproved,
            "POST",
            "/api/v2/maintenance/purge",
            TOKEN,
            req.clone()
        )
        .await
        .1,
        preview
    );
    let committing = commit(req.clone(), &preview);
    assert_eq!(
        call(
            &unapproved,
            "POST",
            "/api/v2/maintenance/purge",
            TOKEN,
            committing.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let approved = app(&pool, preview["data"]["manifest_sha256"].as_str());
    // Changed dependent row (without changing the selected Object) must invalidate approval.
    sqlx::query("UPDATE connections SET description='Changed synthetic dependency' WHERE id=$1")
        .bind(connection)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        call(
            &approved,
            "POST",
            "/api/v2/maintenance/purge",
            TOKEN,
            committing
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert!(exists(&pool, fixture).await);
    let (_, fresh) = call(
        &unapproved,
        "POST",
        "/api/v2/maintenance/purge",
        TOKEN,
        req.clone(),
    )
    .await;
    let approved = app(&pool, fresh["data"]["manifest_sha256"].as_str());
    let committing = commit(req, &fresh);
    let (status, receipt) = call(
        &approved,
        "POST",
        "/api/v2/maintenance/purge",
        TOKEN,
        committing.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert!(!exists(&pool, fixture).await);
    assert!(exists(&pool, retained).await);
    let current: Value = sqlx::query_scalar("SELECT to_jsonb(n) FROM notes n WHERE object_id=$1")
        .bind(retained)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(current, original);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM object_events WHERE run_id=$1")
        .bind(run)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let message_text: String = sqlx::query_scalar("SELECT content FROM chat_messages WHERE id=$1")
        .bind(message)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(message_text.contains("retain my words"));
    assert_eq!(
        call(
            &approved,
            "POST",
            "/api/v2/maintenance/purge",
            TOKEN,
            committing.clone()
        )
        .await
        .1["replayed"],
        true
    );
    let mut altered = committing;
    altered["selections"][0]["reason"] = json!("different scope");
    assert_eq!(
        call(
            &approved,
            "POST",
            "/api/v2/maintenance/purge",
            TOKEN,
            altered
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    // Shared Run cannot be removed while it owns real Events, even if explicitly selected.
    let run_req = request(vec![selection(&pool, "runs", run).await]);
    let (_, blocked) = call(
        &unapproved,
        "POST",
        "/api/v2/maintenance/purge",
        TOKEN,
        run_req.clone(),
    )
    .await;
    assert!(
        !blocked["data"]["manifest"]["blockers"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let approved = app(&pool, blocked["data"]["manifest_sha256"].as_str());
    assert_eq!(
        call(
            &approved,
            "POST",
            "/api/v2/maintenance/purge",
            TOKEN,
            commit(run_req, &blocked)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert!(
        sqlx::query("DELETE FROM object_events WHERE run_id=$1")
            .bind(run)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE object_events SET after_state='{}' WHERE run_id=$1")
            .bind(run)
            .execute(&pool)
            .await
            .is_err()
    );
    // A retained excerpt's live evidence pointer blocks deleting its Source.
    let source2 = object(&pool, "source").await;
    let artifact2 = Uuid::new_v4();
    sqlx::query("INSERT INTO artifacts(id,object_id,kind,content,sha256,size_bytes,capture_outcome) VALUES($1,$2,'text',$3,$4,$5,'complete')").bind(artifact2).bind(source2).bind(text).bind(format!("{:x}",Sha256::digest(text))).bind(text.len() as i64).execute(&pool).await.unwrap();
    sqlx::query("UPDATE notes SET intent='excerpt',source_artifact_id=$2,source_locator=$3 WHERE object_id=$1").bind(retained).bind(artifact2).bind(json!({"kind":"text_offset","start":0,"end":10})).execute(&pool).await.unwrap();
    let req = request(vec![selection(&pool, "objects", source2).await]);
    let (_, blocked) = call(
        &unapproved,
        "POST",
        "/api/v2/maintenance/purge",
        TOKEN,
        req.clone(),
    )
    .await;
    assert!(
        blocked["data"]["manifest"]["blockers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["table"] == "notes")
    );
    let approved = app(&pool, blocked["data"]["manifest_sha256"].as_str());
    assert_eq!(
        call(
            &approved,
            "POST",
            "/api/v2/maintenance/purge",
            TOKEN,
            commit(req, &blocked)
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert!(exists(&pool, source2).await);
    assert!(
        sqlx::query("DELETE FROM artifacts WHERE id=$1")
            .bind(artifact2)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE artifacts SET title='not allowed' WHERE id=$1")
            .bind(artifact2)
            .execute(&pool)
            .await
            .is_err()
    );
    // A pure fixture Run and its request cache can be removed together.
    let pure = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key) VALUES($1,'human_mutation','completed','human','fixture',$2)").bind(pure).bind(Uuid::new_v4().to_string()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO context_apply_requests(principal_id,idempotency_key,request_hash,contract_version,run_id,response,completed_at) VALUES('fixture',$1,$2,'1.1.0',$3,'{}',now())").bind(Uuid::new_v4().to_string()).bind("c".repeat(64)).bind(pure).execute(&pool).await.unwrap();
    let req = request(vec![selection(&pool, "runs", pure).await]);
    let (_, preview) = call(
        &unapproved,
        "POST",
        "/api/v2/maintenance/purge",
        TOKEN,
        req.clone(),
    )
    .await;
    assert_eq!(
        preview["data"]["manifest"]["counts"]["context_apply_requests"],
        1
    );
    let approved = app(&pool, preview["data"]["manifest_sha256"].as_str());
    assert_eq!(
        call(
            &approved,
            "POST",
            "/api/v2/maintenance/purge",
            TOKEN,
            commit(req, &preview)
        )
        .await
        .0,
        StatusCode::OK
    );
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM context_apply_requests WHERE run_id=$1")
            .bind(pure)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    // References in active Runs are operational dependencies, not historical mentions.
    let active_fixture = object(&pool, "note").await;
    let active_run = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,input) VALUES($1,'human_mutation','running','human','fixture',$2,$3)").bind(active_run).bind(Uuid::new_v4().to_string()).bind(json!({"object_id":active_fixture})).execute(&pool).await.unwrap();
    let req = request(vec![selection(&pool, "objects", active_fixture).await]);
    let (_, preview) = call(&unapproved, "POST", "/api/v2/maintenance/purge", TOKEN, req).await;
    assert!(
        preview["data"]["manifest"]["blockers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["reason"].as_str().unwrap_or("").contains("nonterminal"))
    );
    // A late database rejection rolls back earlier subtype deletion and emits no receipt.
    let rollback_fixture = object(&pool, "note").await;
    sqlx::query(&format!("CREATE FUNCTION fixture77_reject_delete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF OLD.id='{}'::uuid THEN RAISE EXCEPTION 'synthetic late failure'; END IF; RETURN OLD; END $$",rollback_fixture)).execute(&pool).await.unwrap();
    sqlx::query("CREATE TRIGGER fixture77_reject_delete BEFORE DELETE ON objects FOR EACH ROW EXECUTE FUNCTION fixture77_reject_delete()").execute(&pool).await.unwrap();
    let req = request(vec![selection(&pool, "objects", rollback_fixture).await]);
    let (_, preview) = call(
        &unapproved,
        "POST",
        "/api/v2/maintenance/purge",
        TOKEN,
        req.clone(),
    )
    .await;
    let approved = app(&pool, preview["data"]["manifest_sha256"].as_str());
    assert_eq!(
        call(
            &approved,
            "POST",
            "/api/v2/maintenance/purge",
            TOKEN,
            commit(req.clone(), &preview)
        )
        .await
        .0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert!(exists(&pool, rollback_fixture).await);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM notes WHERE object_id=$1")
        .bind(rollback_fixture)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM maintenance_purge_receipts WHERE idempotency_key=$1",
    )
    .bind(req["idempotency_key"].as_str())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
    sqlx::query("DROP TRIGGER fixture77_reject_delete ON objects")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION fixture77_reject_delete()")
        .execute(&pool)
        .await
        .unwrap();
    // Catalog includes replay records, receipt bookkeeping, views and derived indexes.
    let (status, catalog) = call(
        &unapproved,
        "GET",
        "/api/v2/maintenance/tables",
        TOKEN,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let tables = catalog["data"].as_array().unwrap();
    assert!(tables.iter().any(|r| r["name"] == "context_apply_requests"));
    assert!(tables.iter().any(|r| r["category"] == "view"));
    assert!(tables.iter().any(|r| r["category"] == "derived_index"));
    let (status, rows) = call(
        &unapproved,
        "GET",
        "/api/v2/maintenance/table-rows?table=context_apply_requests&limit=1",
        TOKEN,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{rows}");
    assert!(rows["data"][0]["key"]["principal_id"].is_string());
    assert_eq!(
        call(
            &unapproved,
            "GET",
            "/api/v2/maintenance/table-rows?table=objects%3BDROP%20TABLE%20objects",
            TOKEN,
            Value::Null
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

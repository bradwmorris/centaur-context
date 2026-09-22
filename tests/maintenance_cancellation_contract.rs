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

async fn row(pool: &PgPool, table: &str, id: Uuid) -> Value {
    assert!(["runs", "object_events"].contains(&table));
    sqlx::query_scalar(&format!("SELECT to_jsonb(t) FROM {table} t WHERE id=$1"))
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}
fn hash(value: &Value) -> String {
    format!("{:x}", Sha256::digest(value.to_string().as_bytes()))
}
async fn purge(app: &Router, req: Value) -> (StatusCode, Value) {
    call(app, "POST", "/api/v2/maintenance/purge", TOKEN, req).await
}
fn now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap()
}
async fn cancel(pool: &PgPool, run: Uuid, chat: Uuid, thread: &str) -> Value {
    let observations = json!([{"context_run_id":run,"thread_key":thread,"observed_at":now(),"operation":"interrupt_active_execution","identity_origin":"stored_trace","response":{"ok":true,"interrupted":false,"execution_id":null,"thread_key":thread}}]);
    json!({"action":"cancel_interaction","run_id":run,"row_sha256":hash(&row(pool,"runs",run).await),"chat_id":chat,"evidence_sha256":hash(&observations),"owner_observations":observations,"reason":"Synthetic owner confirms no active execution; permanently fence this abandoned wrapper"})
}
async fn fence_count(pool: &PgPool, run: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM maintenance_execution_fences WHERE run_id=$1")
        .bind(run)
        .fetch_one(pool)
        .await
        .unwrap()
}
#[tokio::test]
async fn cancellation_requires_owner_proof_and_permanently_fences_execution_identity() {
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
    let chat = object(&pool, "chat").await;
    let suffix = Uuid::new_v4().simple().to_string();
    let workspace = format!("T{suffix}");
    let channel = format!("C{suffix}");
    let thread_id = "1700000000.000001";
    let thread = format!("slack:{workspace}:researcher:{channel}:{thread_id}");
    sqlx::query("UPDATE chats SET provider='slack',workspace_id=$2,channel_id=$3,thread_id=$4,surface_kind='channel' WHERE object_id=$1").bind(chat).bind(&workspace).bind(&channel).bind(thread_id).execute(&pool).await.unwrap();
    let run = Uuid::new_v4();
    let key = format!("legacy-fixture:{run}");
    let input = json!({"workspace_id":workspace,"channel_id":channel,"thread_id":thread_id,"exact_words":"Keep this original user text."});
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,chat_object_id,input,trace,result,started_at) VALUES($1,'slack_interaction','running','system','chat-ingestor',$2,$3,$4,$5,$6,now())")
        .bind(run).bind(&key).bind(chat).bind(&input).bind(json!([{"source_thread_id":thread}])).bind(json!({"real_result":"preserve evidence"})).execute(&pool).await.unwrap();
    let retained = object(&pool, "note").await;
    let event = Uuid::new_v4();
    sqlx::query("INSERT INTO object_events(id,run_id,sequence,target_type,target_id,action,actor_type,actor_id,to_revision,after_state,reversible,created_at) VALUES($1,$2,1,'object',$3,'created','system','fixture',1,'{}',false,now())").bind(event).bind(run).bind(retained).execute(&pool).await.unwrap();
    let event_before = row(&pool, "object_events", event).await;
    let mut req = request(vec![]);
    req["reconciliations"] = json!([cancel(&pool, run, chat, &thread).await]);
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
    for (path, value) in [
        ("/response/ok", json!(false)),
        ("/response/interrupted", json!(true)),
        ("/response/execution_id", json!(Uuid::new_v4())),
        ("/context_run_id", json!(Uuid::new_v4())),
        (
            "/response/thread_key",
            json!("slack:wrong:researcher:wrong:wrong"),
        ),
        ("/identity_origin", json!("runtime_sink_mapping")),
        ("/observed_at", json!("2000-01-01T00:00:00Z")),
    ] {
        let mut invalid = req.clone();
        *invalid["reconciliations"][0]["owner_observations"][0]
            .pointer_mut(path)
            .unwrap() = value;
        invalid["reconciliations"][0]["evidence_sha256"] =
            json!(hash(&invalid["reconciliations"][0]["owner_observations"]));
        assert_eq!(
            purge(&unapproved, invalid).await.0,
            StatusCode::CONFLICT,
            "accepted invalid {path}"
        );
    }
    let mut invalid = req.clone();
    invalid["reconciliations"][0]["evidence_sha256"] = json!("0".repeat(64));
    assert_eq!(purge(&unapproved, invalid).await.0, StatusCode::CONFLICT);
    sqlx::query("UPDATE runs SET pinned=true WHERE id=$1")
        .bind(run)
        .execute(&pool)
        .await
        .unwrap();
    req["reconciliations"] = json!([cancel(&pool, run, chat, &thread).await]);
    assert_eq!(
        purge(&unapproved, req.clone()).await.0,
        StatusCode::CONFLICT
    );
    sqlx::query("UPDATE runs SET pinned=false WHERE id=$1")
        .bind(run)
        .execute(&pool)
        .await
        .unwrap();
    let child = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,parent_run_id,kind,status,actor_type,actor_id,idempotency_key) VALUES($1,$2,'human_mutation','running','human','fixture',$3)").bind(child).bind(run).bind(child.to_string()).execute(&pool).await.unwrap();
    req["reconciliations"] = json!([cancel(&pool, run, chat, &thread).await]);
    assert_eq!(
        purge(&unapproved, req.clone()).await.0,
        StatusCode::CONFLICT
    );
    sqlx::query("UPDATE runs SET status='completed',completed_at=now() WHERE id=$1")
        .bind(child)
        .execute(&pool)
        .await
        .unwrap();
    let (status, preview) = purge(&unapproved, req.clone()).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    sqlx::query("UPDATE runs SET result=$2 WHERE id=$1")
        .bind(run)
        .bind(json!({"real_result":"new preserved evidence"}))
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
    assert_eq!(fence_count(&pool, run).await, 0);
    req["reconciliations"] = json!([cancel(&pool, run, chat, &thread).await]);
    let before = row(&pool, "runs", run).await;
    let (status, preview) = purge(&unapproved, req.clone()).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["data"]["manifest"]["blockers"], json!([]));
    let approved = app(&pool, preview["data"]["manifest_sha256"].as_str());
    let committing = commit(req.clone(), &preview);
    assert_eq!(
        purge(&unapproved, committing.clone()).await.0,
        StatusCode::FORBIDDEN
    );
    // Fail after fence insertion and Run mutation: the entire transaction must undo both.
    sqlx::query(&format!("CREATE FUNCTION cancellation95_reject() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.id='{run}'::uuid AND NEW.status='cancelled' THEN RAISE EXCEPTION 'synthetic cancellation rollback'; END IF; RETURN NEW; END $$")).execute(&pool).await.unwrap();
    sqlx::query("CREATE TRIGGER cancellation95_reject AFTER UPDATE ON runs FOR EACH ROW EXECUTE FUNCTION cancellation95_reject()").execute(&pool).await.unwrap();
    let failed = purge(&approved, committing.clone()).await;
    sqlx::query("DROP TRIGGER cancellation95_reject ON runs")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION cancellation95_reject()")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(failed.0, StatusCode::INTERNAL_SERVER_ERROR, "{}", failed.1);
    assert_eq!(row(&pool, "runs", run).await, before);
    assert_eq!(fence_count(&pool, run).await, 0);
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
    let mut after = row(&pool, "runs", run).await;
    assert_eq!(after["status"], "cancelled");
    assert!(!after["completed_at"].is_null());
    for field in ["status", "completed_at", "updated_at"] {
        after[field] = before[field].clone();
    }
    assert_eq!(after, before);
    assert_eq!(row(&pool, "object_events", event).await, event_before);
    assert_eq!(fence_count(&pool, run).await, 1);
    assert_eq!(
        purge(&approved, committing.clone()).await.1["replayed"],
        true
    );
    let mut altered = committing;
    altered["reconciliations"][0]["reason"] = json!("broadened request");
    assert_eq!(purge(&approved, altered).await.0, StatusCode::CONFLICT);
    for change in [
        "trace=trace || '[{\"late\":true}]'::jsonb",
        "status='completed',completed_at=now()",
        "result='{}'",
    ] {
        assert!(
            sqlx::query(&format!("UPDATE runs SET {change} WHERE id=$1"))
                .bind(run)
                .execute(&pool)
                .await
                .is_err(),
            "accepted late {change}"
        );
    }
    assert!(sqlx::query("INSERT INTO runs(id,parent_run_id,kind,status,actor_type,actor_id,idempotency_key) VALUES($1,$2,'human_mutation','running','human','fixture',$3)").bind(Uuid::new_v4()).bind(run).bind(Uuid::new_v4().to_string()).execute(&pool).await.is_err());
    assert!(sqlx::query("INSERT INTO object_events(id,run_id,sequence,target_type,target_id,action,actor_type,actor_id,to_revision,after_state,reversible,created_at) VALUES($1,$2,2,'object',$3,'created','system','fixture',1,'{}',false,now())").bind(Uuid::new_v4()).bind(run).bind(retained).execute(&pool).await.is_err());
    // A new modern interaction key for the same provider thread is fenced too.
    let mut tx = pool.begin().await.unwrap();
    assert!(
        centaur_context::runs::open_slack_interaction(
            &mut tx,
            centaur_context::runs::SlackInteractionOpen {
                workspace_id: &workspace,
                channel_id: &channel,
                thread_id,
                interaction_id: "fresh-retry",
                started_at: time::OffsetDateTime::now_utc(),
                title: "Synthetic retry".into(),
                request_message: None
            }
        )
        .await
        .is_err()
    );
    tx.rollback().await.unwrap();
    let ingest_before: i64 = sqlx::query_scalar("SELECT count(*) FROM objects")
        .fetch_one(&pool)
        .await
        .unwrap();
    let ingest = centaur_context::ingest::router(
        state(&pool),
        TOKEN.into(),
        centaur_context::ingest::ApprovedSlackSurfaces::parse(&format!("{workspace}:{channel}"))
            .unwrap(),
    );
    let payload = json!({"workspace_id":workspace,"channel_id":channel,"thread_id":thread_id,"surface_kind":"channel","channel_name":"synthetic","title":"Retry","messages":[{"provider_message_id":"1700000000.000002","sender":{"provider_user_id":"USYNTHETIC95","display_name":"Synthetic","user_kind":"human"},"content":"Must not ingest","source_created_at":now()}],"run":{"interaction_id":"new-attempt","status":"running","started_at":now()}});
    let (status, response) = call(
        &ingest,
        "POST",
        "/api/v2/ingest/slack/interactions",
        TOKEN,
        payload,
    )
    .await;
    assert!(status.is_client_error(), "{response}");
    assert!(response.to_string().contains("fenced"), "{response}");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM objects")
            .fetch_one(&pool)
            .await
            .unwrap(),
        ingest_before
    );
    // Terminal cancellation permits separate fixture deletion; retained real Event still protects its Run.
    let mut detach_req = request(vec![selection(&pool, "objects", chat).await]);
    detach_req["reconciliations"] = json!([{"action":"detach_chat","run_id":run,"row_sha256":hash(&row(&pool,"runs",run).await),"chat_id":chat,"reason":"Delete confirmed fixture Chat while retaining real execution evidence"}]);
    let (status, preview) = purge(&unapproved, detach_req.clone()).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["data"]["manifest"]["blockers"], json!([]));
    let (status, result) = purge(
        &app(&pool, preview["data"]["manifest_sha256"].as_str()),
        commit(detach_req, &preview),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert!(!exists(&pool, chat).await);
    assert_eq!(row(&pool, "object_events", event).await, event_before);
    let (_, blocked) = purge(
        &unapproved,
        request(vec![selection(&pool, "runs", run).await]),
    )
    .await;
    assert!(
        !blocked["data"]["manifest"]["blockers"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let fresh_chat = object(&pool, "chat").await;
    assert!(sqlx::query("UPDATE chats SET provider='slack',workspace_id=$2,channel_id=$3,thread_id=$4,surface_kind='channel' WHERE object_id=$1").bind(fresh_chat).bind(&workspace).bind(&channel).bind(thread_id).execute(&pool).await.is_err());
    // A second wholly synthetic wrapper proves the tombstone survives Run deletion.
    // This fixture has no retained research history.
    let pure_chat = object(&pool, "chat").await;
    let pure_thread = "1700000000.000099";
    sqlx::query("UPDATE chats SET provider='slack',workspace_id=$2,channel_id=$3,thread_id=$4,surface_kind='channel' WHERE object_id=$1").bind(pure_chat).bind(&workspace).bind(&channel).bind(pure_thread).execute(&pool).await.unwrap();
    let pure = Uuid::new_v4();
    let pure_key = pure.to_string();
    let pure_exec = format!("slack:{workspace}:researcher:{channel}:{pure_thread}");
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,chat_object_id,input,trace) VALUES($1,'slack_interaction','running','system','chat-ingestor',$2,$3,$4,$5)").bind(pure).bind(&pure_key).bind(pure_chat).bind(json!({"workspace_id":workspace,"channel_id":channel,"thread_id":pure_thread})).bind(json!([{"source_thread_id":pure_exec}])).execute(&pool).await.unwrap();
    // Writer wins the race: an already executing update commits before maintenance
    // obtains its table locks. Exact-row revalidation must reject the old approval.
    let mut race_req = request(vec![]);
    race_req["reconciliations"] = json!([cancel(&pool, pure, pure_chat, &pure_exec).await]);
    let (_, race_preview) = purge(&unapproved, race_req.clone()).await;
    let race_app = app(&pool, race_preview["data"]["manifest_sha256"].as_str());
    let mut writer = pool.begin().await.unwrap();
    sqlx::query("UPDATE runs SET result=$2 WHERE id=$1")
        .bind(pure)
        .bind(json!({"won_race":true}))
        .execute(&mut *writer)
        .await
        .unwrap();
    let mut pending =
        tokio::spawn(async move { purge(&race_app, commit(race_req, &race_preview)).await });
    let blocked = tokio::time::timeout(std::time::Duration::from_millis(100), &mut pending)
        .await
        .is_err();
    writer.commit().await.unwrap();
    let race_result = tokio::time::timeout(std::time::Duration::from_secs(15), pending)
        .await
        .unwrap()
        .unwrap();
    assert!(blocked, "maintenance should wait for the earlier writer");
    assert_eq!(race_result.0, StatusCode::CONFLICT, "{}", race_result.1);
    assert_eq!(fence_count(&pool, pure).await, 0);
    let mut pure_req = request(vec![]);
    pure_req["reconciliations"] = json!([cancel(&pool, pure, pure_chat, &pure_exec).await]);
    let (_, preview) = purge(&unapproved, pure_req.clone()).await;
    let (status, result) = purge(
        &app(&pool, preview["data"]["manifest_sha256"].as_str()),
        commit(pure_req, &preview),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let pure_req = request(vec![
        selection(&pool, "objects", pure_chat).await,
        selection(&pool, "runs", pure).await,
    ]);
    let (_, preview) = purge(&unapproved, pure_req.clone()).await;
    assert_eq!(preview["data"]["manifest"]["blockers"], json!([]));
    let (status, result) = purge(
        &app(&pool, preview["data"]["manifest_sha256"].as_str()),
        commit(pure_req, &preview),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(fence_count(&pool, pure).await, 1);
    assert!(sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key) VALUES($1,'slack_interaction','running','system','chat-ingestor',$2)").bind(Uuid::new_v4()).bind(pure_key).execute(&pool).await.is_err());
}

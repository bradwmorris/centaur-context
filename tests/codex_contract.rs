use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use centaur_context::{
    api::AppState,
    codex::{CodexConfig, router},
    config::TextSearchConfig,
    db,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

async fn pool() -> Option<PgPool> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    assert!(
        url.split('?')
            .next()
            .unwrap()
            .rsplit('/')
            .next()
            .unwrap()
            .contains("centaur_context_test")
    );
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(&url)
        .await
        .unwrap();
    db::migrate(&pool).await.unwrap();
    Some(pool)
}
fn request(
    path: &str,
    token: &str,
    session: Uuid,
    repo: &str,
    body: Option<Value>,
) -> Request<Body> {
    let mut b = Request::builder()
        .method(if body.is_some() { "POST" } else { "GET" })
        .uri(path)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .header("x-codex-session-id", session.to_string())
        .header("x-codex-repository", repo);
    if let Some(key) = body.as_ref().and_then(|v| v["idempotency_key"].as_str()) {
        b = b.header("Idempotency-Key", key);
    }
    b.body(Body::from(
        body.map(|v| serde_json::to_vec(&v).unwrap())
            .unwrap_or_default(),
    ))
    .unwrap()
}
async fn response(app: axum::Router, req: Request<Body>) -> (StatusCode, Value) {
    let r = app.oneshot(req).await.unwrap();
    let status = r.status();
    let bytes = r.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(json!(null)),
    )
}
async fn setup(pool: PgPool, curate: bool) -> (axum::Router, CodexConfig) {
    let human = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,'user','Synthetic owner','Human for isolated Codex contract tests.','system','codex-test','system','codex-test')").bind(human).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO users(object_id,user_kind,identities) VALUES($1,'human','[]')")
        .bind(human)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let cfg = CodexConfig {
        addr: "127.0.0.1:0".parse().unwrap(),
        capture_token: "capture-test-only-token-123456789012345".into(),
        tool_token: "tools-test-only-token-12345678901234567".into(),
        host_id: Uuid::new_v4(),
        human_id: human,
        repositories: ["organization".into(), "shared".into()].into(),
        curate,
    };
    (
        router(
            AppState {
                pool,
                embeddings: None,
                text_search_config: TextSearchConfig::SIMPLE,
            },
            cfg.clone(),
        ),
        cfg,
    )
}
fn batch() -> Value {
    let turn = Uuid::new_v4();
    json!({"version":1,"batch_id":Uuid::new_v4(),"title":"Release decision","messages":[{"id":"user-1","turn_id":turn,"role":"human","content":"I decided to release the documentation on Friday.","created_at":"2026-09-22T01:00:00Z"},{"id":"assistant-1","turn_id":turn,"role":"agent","content":"I recorded your decision in this conversation.","created_at":"2026-09-22T01:00:01Z"}],"finished_turn_id":turn})
}

#[tokio::test]
async fn captures_replays_curates_and_supplies_canonical_write_provenance() {
    let Some(pool) = pool().await else { return };
    let (app, cfg) = setup(pool.clone(), true).await;
    let sid = Uuid::new_v4();
    let b = batch();
    let (status, first) = response(
        app.clone(),
        request(
            "/api/v2/codex/capture",
            &cfg.capture_token,
            sid,
            "organization",
            Some(b.clone()),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let chat = first["data"]["chat_object_id"].as_str().unwrap();
    assert_eq!(first["data"]["inserted_messages"], 2);
    assert!(first["data"]["curator_run_id"].is_string());
    let (_, replayed) = response(
        app.clone(),
        request(
            "/api/v2/codex/capture",
            &cfg.capture_token,
            sid,
            "organization",
            Some(b.clone()),
        ),
    )
    .await;
    assert_eq!(first, replayed);
    let mut changed = b.clone();
    changed["messages"][0]["content"] = json!("Rewritten history");
    assert_eq!(
        response(
            app.clone(),
            request(
                "/api/v2/codex/capture",
                &cfg.capture_token,
                sid,
                "organization",
                Some(changed.clone())
            )
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    changed["batch_id"] = json!(Uuid::new_v4());
    assert_eq!(
        response(
            app.clone(),
            request(
                "/api/v2/codex/capture",
                &cfg.capture_token,
                sid,
                "organization",
                Some(changed)
            )
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let apply = json!({"contract_version":"1.1.0","idempotency_key":Uuid::new_v4().to_string(),"chat_object_id":chat,"operations":[{"operation":"create_object","local_ref":"idea","kind":"note","title":"Release rationale","description":"The owner chose Friday for the documentation release.","fields":{"intent":"idea","content":"A deliberately requested note."}}]});
    let (status, result) = response(
        app.clone(),
        request(
            "/api/v2/apply",
            &cfg.tool_token,
            sid,
            "organization",
            Some(apply.clone()),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    let (_, replay) = response(
        app.clone(),
        request(
            "/api/v2/apply",
            &cfg.tool_token,
            sid,
            "organization",
            Some(apply),
        ),
    )
    .await;
    assert_eq!(replay["data"]["replayed"], true);
    let chat_id: Uuid = chat.parse().unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM chat_messages WHERE chat_object_id=$1")
            .bind(chat_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2);
    let provider: String = sqlx::query_scalar("SELECT provider FROM chats WHERE object_id=$1")
        .bind(chat_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(provider, "codex");
    let (_, read) = response(
        app,
        request(
            "/api/v2/read",
            &cfg.tool_token,
            sid,
            "organization",
            Some(json!({"object_ids":[chat_id],"include":["messages","connections"]})),
        ),
    )
    .await;
    assert!(read["data"]["objects"].is_array());
}

#[tokio::test]
async fn credentials_targets_and_system_objects_are_separate() {
    let Some(pool) = pool().await else { return };
    let (app, cfg) = setup(pool, false).await;
    let sid = Uuid::new_v4();
    for (path, token, repo) in [
        (
            "/api/v2/codex/capture",
            cfg.tool_token.as_str(),
            "organization",
        ),
        ("/api/v2/search", cfg.capture_token.as_str(), "organization"),
        (
            "/api/v2/codex/capture",
            cfg.capture_token.as_str(),
            "personal",
        ),
    ] {
        let (status, _) =
            response(app.clone(), request(path, token, sid, repo, Some(batch()))).await;
        assert!(matches!(
            status,
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ));
    }
    let (_, result) = response(
        app.clone(),
        request(
            "/api/v2/codex/capture",
            &cfg.capture_token,
            sid,
            "organization",
            Some(batch()),
        ),
    )
    .await;
    assert_eq!(result["data"]["curation_enabled"], false);
    assert!(result["data"]["curator_run_id"].is_null());
    let chat = result["data"]["chat_object_id"].clone();
    let (status, _) = response(
        app.clone(),
        request(
            "/api/v2/codex/capture",
            &cfg.capture_token,
            sid,
            "shared",
            Some(batch()),
        ),
    )
    .await;
    assert_ne!(status, StatusCode::OK);
    let mut evil = batch();
    evil["messages"][0]["role"] = json!("system");
    assert_eq!(
        response(
            app.clone(),
            request(
                "/api/v2/codex/capture",
                &cfg.capture_token,
                sid,
                "organization",
                Some(evil)
            )
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    for kind in ["memory", "chat", "user"] {
        let body = json!({"contract_version":"1.1.0","idempotency_key":Uuid::new_v4().to_string(),"chat_object_id":chat,"operations":[{"operation":"create_object","local_ref":"forged","kind":kind,"title":"Forged","description":"This must not be writable through ordinary tools.","fields":{}}]});
        assert_ne!(
            response(
                app.clone(),
                request(
                    "/api/v2/apply",
                    &cfg.tool_token,
                    sid,
                    "organization",
                    Some(body)
                )
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    let body = json!({"contract_version":"1.1.0","idempotency_key":Uuid::new_v4().to_string(),"chat_object_id":chat,"operations":[]});
    assert_ne!(
        response(
            app,
            request(
                "/api/v2/apply",
                &cfg.tool_token,
                Uuid::new_v4(),
                "organization",
                Some(body)
            )
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn git_receipts_are_verified_deduplicated_technical_evidence() {
    use sha1::{Digest, Sha1};
    let Some(pool) = pool().await else { return };
    let (app, cfg) = setup(pool.clone(), true).await;
    let sid = Uuid::new_v4();
    let baseline = "a".repeat(40);
    let raw = format!(
        "tree {}\nparent {baseline}\nauthor Example <example@example.invalid> 1 +0000\ncommitter Example <example@example.invalid> 1 +0000\n\nAdd capture recovery\n",
        "b".repeat(40)
    );
    let bytes = [format!("commit {}\0", raw.len()).as_bytes(), raw.as_bytes()].concat();
    let oid = format!("{:x}", Sha1::digest(bytes));
    let mut b = batch();
    b["git_receipts"] = json!([{"turn_id":b["finished_turn_id"],"baseline":baseline,"commits":[{"oid":oid,"raw":raw}]}]);
    let (status, result) = response(
        app.clone(),
        request(
            "/api/v2/codex/capture",
            &cfg.capture_token,
            sid,
            "organization",
            Some(b.clone()),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["data"]["outcome_memory_ids"], json!([]));
    let receipts: i64 = sqlx::query_scalar("SELECT count(*) FROM runs WHERE kind='memory_capture' AND actor_id=$1 AND result->>'evidence_only'='true' AND primary_object_id IS NULL")
        .bind(format!("codex-capture:{}",cfg.host_id)).fetch_one(&pool).await.unwrap();
    assert_eq!(receipts, 1);
    b["batch_id"] = json!(Uuid::new_v4());
    let (_, replayed) = response(
        app.clone(),
        request(
            "/api/v2/codex/capture",
            &cfg.capture_token,
            sid,
            "organization",
            Some(b.clone()),
        ),
    )
    .await;
    assert_eq!(replayed["data"]["outcome_memory_ids"], json!([]));
    assert_eq!(
        replayed["data"]["capture_coverage"],
        result["data"]["capture_coverage"]
    );
    b["batch_id"] = json!(Uuid::new_v4());
    b["git_receipts"][0]["commits"][0]["raw"] = json!("Forged completion evidence");
    assert_eq!(
        response(
            app,
            request(
                "/api/v2/codex/capture",
                &cfg.capture_token,
                sid,
                "organization",
                Some(b)
            )
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn long_codex_turns_queue_bounded_windows() {
    let Some(pool) = pool().await else { return };
    let (app, cfg) = setup(pool.clone(), true).await;
    let sid = Uuid::new_v4();
    let turn = Uuid::new_v4();
    for (offset, count, finished) in [(0, 100, false), (100, 50, true)] {
        let messages=(offset..offset+count).map(|n|json!({"id":format!("message-{n}"),"turn_id":turn,"role":"human","content":"We decided to publish the documentation on Friday.","created_at":"2026-09-22T01:00:00Z"})).collect::<Vec<_>>();
        let b = json!({"version":1,"batch_id":Uuid::new_v4(),"title":"Documentation planning","messages":messages,"finished_turn_id":if finished {Some(turn)} else {None}});
        let (status, result) = response(
            app.clone(),
            request(
                "/api/v2/codex/capture",
                &cfg.capture_token,
                sid,
                "organization",
                Some(b),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{result}");
    }
    let counts:Vec<i32>=sqlx::query_scalar("SELECT (r.input->>'message_count')::integer FROM runs r JOIN chats c ON c.object_id=r.chat_object_id WHERE r.kind='curator' AND c.provider='codex' AND c.workspace_id=$1 AND c.thread_id=$2 ORDER BY r.created_at,r.id")
        .bind(cfg.host_id.to_string()).bind(sid.to_string()).fetch_all(&pool).await.unwrap();
    assert_eq!(counts, vec![100, 50]);
}

#[tokio::test]
async fn curator_summarizes_new_visible_window_once_and_preserves_messages() {
    let Some(admin) = pool().await else {
        return;
    };
    let name = format!("centaur_context_test_summary_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE DATABASE {name}"))
        .execute(&admin)
        .await
        .unwrap();
    let options = std::env::var("TEST_DATABASE_URL")
        .unwrap()
        .parse::<sqlx::postgres::PgConnectOptions>()
        .unwrap()
        .database(&name);
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .unwrap();
    db::migrate(&pool).await.unwrap();
    let (app, cfg) = setup(pool.clone(), true).await;
    let sid = Uuid::new_v4();
    let (_, captured) = response(
        app,
        request(
            "/api/v2/codex/capture",
            &cfg.capture_token,
            sid,
            "organization",
            Some(batch()),
        ),
    )
    .await;
    let chat: Uuid = captured["data"]["chat_object_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let run: Uuid = captured["data"]["curator_run_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = calls.clone();
    let mock=axum::Router::new().route("/",axum::routing::post(move |axum::Json(body):axum::Json<Value>| {
        let counter=counter.clone(); async move {
            counter.fetch_add(1,std::sync::atomic::Ordering::SeqCst);
            let summary=body["messages"][0]["content"].as_str().unwrap_or_default().starts_with("Summarize");
            let output=if summary {json!({"title":"Release review decision","description":"The owner discussed release timing and recorded the review decision."})} else {json!({"create_objects":[],"create_connections":[],"update_objects":[],"update_connections":[]})};
            axum::Json(json!({"choices":[{"message":{"content":output.to_string()}}]}))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, mock).await.unwrap();
    });
    let config = centaur_context::config::CuratorModelConfig {
        transport: centaur_context::config::CuratorModelTransport::DirectApi,
        endpoint: format!("http://{address}/"),
        api_token: "test-token".into(),
        model: "test-model".into(),
        prompt_version: "test".into(),
        poll_interval: std::time::Duration::from_millis(30),
        request_timeout: std::time::Duration::from_secs(3),
    };
    let worker = tokio::spawn(centaur_context::curator::run_worker(
        pool.clone(),
        None,
        config,
        TextSearchConfig::SIMPLE,
        vec!["codex".into()],
    ));
    let mut status = String::new();
    for _ in 0..100 {
        status = sqlx::query_scalar("SELECT status FROM runs WHERE id=$1")
            .bind(run)
            .fetch_one(&pool)
            .await
            .unwrap();
        if status == "completed" || status == "failed" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    }
    let result: Value = sqlx::query_scalar("SELECT to_jsonb(r) FROM runs r WHERE id=$1")
        .bind(run)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "completed", "{result}");
    let object = db::get_object(&pool, chat).await.unwrap();
    assert_eq!(object.title, "Release review decision");
    assert!(object.provenance["chat_summary_through_message_id"].is_string());
    assert_eq!(db::list_chat_messages(&pool, chat).await.unwrap().len(), 2);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    worker.abort();
    server.abort();
}

#[test]
fn legacy_capture_hash_shape_does_not_gain_absent_coverage() {
    let original = batch();
    let decoded: centaur_context::codex::CaptureBatch = serde_json::from_value(original).unwrap();
    let encoded = serde_json::to_value(decoded).unwrap();
    assert!(encoded.get("coverage").is_none());
}

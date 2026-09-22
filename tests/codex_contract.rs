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
    let apply = json!({"contract_version":"1.1.0","idempotency_key":Uuid::new_v4().to_string(),"chat_object_id":chat,"operations":[{"operation":"create_object","local_ref":"idea","kind":"note","title":"Release rationale","description":"The owner chose Friday for the documentation release.","fields":{"intent":"insight","content":"A deliberately requested note."}}]});
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
async fn git_receipts_are_verified_deduplicated_and_reversible() {
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
    let memory: Uuid = result["data"]["outcome_memory_ids"][0]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let (description, actor): (String, String) =
        sqlx::query_as("SELECT description,created_by_id FROM objects WHERE id=$1")
            .bind(memory)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(description.contains("Add capture recovery"));
    assert!(actor.starts_with("codex-capture:"));
    assert!(!description.contains("passed"));
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
    let run: Uuid = sqlx::query_scalar(
        "SELECT id FROM runs WHERE kind='memory_capture' AND primary_object_id=$1",
    )
    .bind(memory)
    .fetch_one(&pool)
    .await
    .unwrap();
    let undo = centaur_context::dreaming::undo(&pool, run).await.unwrap();
    assert_eq!(undo["changes"], 2);
    let archived: bool =
        sqlx::query_scalar("SELECT archived_at IS NOT NULL FROM objects WHERE id=$1")
            .bind(memory)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(archived);
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

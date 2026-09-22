mod support;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use centaur_context::{
    api::{AppState, agent_router},
    config::TextSearchConfig,
    db, embeddings,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

async fn test_pool() -> Option<PgPool> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    assert!(
        url.contains("centaur_context_test"),
        "universal tests require a disposable centaur_context_test database"
    );
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    db::migrate(&pool).await.unwrap();
    Some(pool)
}

fn request(method: &str, path: &str, token: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .header("x-centaur-principal-id", "agent-universal-test")
        .header("x-centaur-thread-key", "test:workspace:channel:thread")
        .header("content-type", "application/json")
        .header(
            "idempotency-key",
            body.get("idempotency_key")
                .and_then(Value::as_str)
                .unwrap_or("not-a-write"),
        )
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn json_body(response: axum::response::Response) -> Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

#[tokio::test]
async fn three_tool_flow_is_atomic_connected_and_idempotent() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping universal contract: TEST_DATABASE_URL is not set");
        return;
    };
    let owner = support::task_owner(&pool).await;
    let anchor = Uuid::new_v4();
    let mut seed = pool.begin().await.unwrap();
    sqlx::query(
        r#"INSERT INTO objects
           (id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id)
           VALUES ($1,'entity','Universal test anchor','Disposable anchor for universal tool tests.',
                   'system','universal-test','system','universal-test')"#,
    )
    .bind(anchor)
    .execute(&mut *seed)
    .await
    .unwrap();
    sqlx::query("INSERT INTO entities (object_id,entity_kind) VALUES ($1,'project')")
        .bind(anchor)
        .execute(&mut *seed)
        .await
        .unwrap();
    seed.commit().await.unwrap();

    let token = "u".repeat(32);
    let app = agent_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        token.clone(),
    );
    let key = format!("universal-test-{}", Uuid::new_v4());
    let apply = json!({
        "contract_version":"1.1.0",
        "idempotency_key":key,
        "operations":[
            {"operation":"create_object","local_ref":"task","kind":"task","title":"Universal task","description":"A disposable Task created by the universal contract test.","fields":{"status":"todo","priority":"medium","owner_object_id":owner,"due_at":"2099-01-01T00:00:00Z","brief_markdown":"Review this task and record verification evidence."}},
            {"operation":"create_object","local_ref":"entity","kind":"entity","title":"Universal entity","description":"A disposable Entity created by the universal contract test.","fields":{"entity_kind":"concept"}},
            {"operation":"create_object","local_ref":"source","kind":"source","title":"Universal source","description":"A disposable Source metadata record without canonical captured content.","fields":{"source_kind":"article","canonical_uri":"https://example.invalid/universal"}},
            {"operation":"create_object","local_ref":"note","kind":"note","title":"Universal note","description":"A disposable Note created by the universal contract test.","fields":{"content":"Test evidence.","content_format":"markdown","intent":"insight"}},
            {"operation":"create_object","local_ref":"theme","kind":"theme","title":"Universal theme","description":"A disposable Theme created by the universal contract test.","fields":{"slug":format!("universal-{}", Uuid::new_v4())}},
            {"operation":"create_connection","source":{"local_ref":"task"},"kind":"related_to","target":{"object_id":anchor},"description":"The test Task relates to the disposable test anchor."},
            {"operation":"create_connection","source":{"local_ref":"entity"},"kind":"related_to","target":{"object_id":anchor},"description":"The test Entity relates to the disposable test anchor."},
            {"operation":"create_connection","source":{"local_ref":"source"},"kind":"related_to","target":{"object_id":anchor},"description":"The test Source relates to the disposable test anchor."},
            {"operation":"create_connection","source":{"local_ref":"note"},"kind":"derived_from","target":{"local_ref":"source"},"description":"The test Note was derived from the test Source metadata."},
            {"operation":"create_connection","source":{"local_ref":"theme"},"kind":"related_to","target":{"object_id":anchor},"description":"The test Theme relates to the disposable test anchor."},
            {"operation":"append_artifact","object":{"local_ref":"source"},"expected_revision":1,"kind":"supporting_text","title":"Disposable evidence","content":"Bounded universal artifact content.","media_type":"text/plain","capture_outcome":"complete"}
        ]
    });

    let first = app
        .clone()
        .oneshot(request("POST", "/api/v2/apply", &token, apply.clone()))
        .await
        .unwrap();
    let first_status = first.status();
    let first = json_body(first).await;
    assert_eq!(first_status, StatusCode::OK, "{first}");
    let object_id = first["data"]["results"][0]["data"]["id"].as_str().unwrap();
    let artifact_id = first["data"]["results"][10]["data"]["id"].as_str().unwrap();
    let connection_id = first["data"]["results"][5]["data"]["connection"]["id"]
        .as_str()
        .unwrap();

    let replay = app
        .clone()
        .oneshot(request("POST", "/api/v2/apply", &token, apply.clone()))
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json_body(replay).await;
    assert_eq!(replay["data"]["replayed"], true);
    assert_eq!(replay["data"]["results"][0]["data"]["id"], object_id);

    let read = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v2/read",
            &token,
            json!({
                "object_ids":[object_id],
                "include":["connections","events"],
                "artifact_windows":[{"artifact_id":artifact_id,"offset":0,"limit":20}]
            }),
        ))
        .await
        .unwrap();
    assert_eq!(read.status(), StatusCode::OK);
    let read = json_body(read).await;
    assert_eq!(read["data"]["objects"][0]["object"]["kind"], "task");
    assert!(
        !read["data"]["objects"][0]["connections"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        read["data"]["artifact_windows"][0]["text"],
        "Bounded universal ar"
    );

    let search = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/v2/search",
            &token,
            json!({"query":"Universal task","object_types":["task"],"lexical_only":true}),
        ))
        .await
        .unwrap();
    assert_eq!(search.status(), StatusCode::OK);

    let update_key = format!("universal-test-update-{}", Uuid::new_v4());
    let update = json!({
        "contract_version":"1.1.0",
        "idempotency_key":update_key,
        "operations":[
            {"operation":"update_object","object_id":object_id,"expected_revision":1,"changes":{"title":"Updated universal task"}},
            {"operation":"update_connection","connection_id":connection_id,"expected_revision":1,"description":"The updated test Task still relates to the disposable test anchor."}
        ]
    });
    let update = app
        .clone()
        .oneshot(request("POST", "/api/v2/apply", &token, update))
        .await
        .unwrap();
    assert_eq!(update.status(), StatusCode::OK);

    let archive_key = format!("universal-test-archive-{}", Uuid::new_v4());
    let archive = json!({
        "contract_version":"1.1.0",
        "idempotency_key":archive_key,
        "operations":[
            {"operation":"archive_connection","connection_id":connection_id,"expected_revision":2},
            {"operation":"archive_object","object_id":object_id,"expected_revision":2}
        ]
    });
    let archive = app
        .clone()
        .oneshot(request("POST", "/api/v2/apply", &token, archive))
        .await
        .unwrap();
    assert_eq!(archive.status(), StatusCode::OK);

    let system_key = format!("universal-test-system-{}", Uuid::new_v4());
    let system_owned = json!({
        "contract_version":"1.1.0",
        "idempotency_key":system_key,
        "operations":[
            {"operation":"create_object","local_ref":"memory","kind":"memory","title":"Forbidden memory","description":"Agents must not directly create this system-managed record.","fields":{}},
            {"operation":"create_connection","source":{"local_ref":"memory"},"kind":"related_to","target":{"object_id":anchor},"description":"This must never commit."}
        ]
    });
    let system_owned = app
        .clone()
        .oneshot(request("POST", "/api/v2/apply", &token, system_owned))
        .await
        .unwrap();
    assert_eq!(system_owned.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let mut conflict = apply.clone();
    conflict["operations"][0]["title"] = json!("Changed retry body");
    let conflict = app
        .clone()
        .oneshot(request("POST", "/api/v2/apply", &token, conflict))
        .await
        .unwrap();
    assert_eq!(conflict.status(), StatusCode::CONFLICT);

    let invalid_key = format!("universal-test-invalid-{}", Uuid::new_v4());
    let invalid = json!({
        "contract_version":"1.1.0",
        "idempotency_key":invalid_key,
        "operations":[
            {"operation":"create_object","local_ref":"orphan","kind":"entity","title":"Orphan","description":"This Object must roll back.","fields":{"entity_kind":"concept"}},
            {"operation":"create_connection","source":{"local_ref":"orphan"},"kind":"related_to","target":{"object_id":Uuid::new_v4()},"description":"This invalid target rolls back the whole batch."}
        ]
    });
    let invalid = app
        .oneshot(request("POST", "/api/v2/apply", &token, invalid))
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let orphan_count: i64 = sqlx::query_scalar("SELECT count(*) FROM objects WHERE title='Orphan'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(orphan_count, 0);
}

#[tokio::test]
async fn description_updates_are_audited_lexical_and_latest_embedding_safe() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping description retrieval contract: TEST_DATABASE_URL is not set");
        return;
    };
    let owner = support::task_owner(&pool).await;
    let anchor = Uuid::new_v4();
    let evidence = Uuid::new_v4();
    let mut seed = pool.begin().await.unwrap();
    for (id, title, description) in [
        (
            anchor,
            "Description test anchor",
            "A disposable project anchoring the description retrieval test.",
        ),
        (
            evidence,
            "Description test evidence",
            "A disposable evaluation record supporting the changed Task description.",
        ),
    ] {
        sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,'entity',$2,$3,'system','description-test','system','description-test','{}')")
            .bind(id)
            .bind(title)
            .bind(description)
            .execute(&mut *seed)
            .await
            .unwrap();
        sqlx::query("INSERT INTO entities(object_id,entity_kind) VALUES($1,'project')")
            .bind(id)
            .execute(&mut *seed)
            .await
            .unwrap();
    }
    seed.commit().await.unwrap();

    let token = "d".repeat(32);
    let app = agent_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        token.clone(),
    );
    let create = json!({
        "contract_version":"1.1.0",
        "idempotency_key":format!("description-create-{}", Uuid::new_v4()),
        "operations":[
            {"operation":"create_object","local_ref":"task","kind":"task","title":"Evaluate retrieval launch","description":"Evaluate the obsoletequartz retrieval launch criteria. This Task records the current release decision.","fields":{"status":"todo","priority":"medium","owner_object_id":owner,"due_at":"2099-01-01T00:00:00Z","brief_markdown":"Review this task and record verification evidence."}},
            {"operation":"create_connection","source":{"local_ref":"task"},"kind":"related_to","target":{"object_id":anchor},"description":"The retrieval evaluation belongs to this disposable project."}
        ]
    });
    let response = app
        .clone()
        .oneshot(request("POST", "/api/v2/apply", &token, create))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = json_body(response).await;
    let object_id = Uuid::parse_str(
        response["data"]["results"][0]["data"]["id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();

    let model = format!("description-model-{}", Uuid::new_v4());
    let initial_hash: String = sqlx::query_scalar(
        "SELECT object_embedding_source_hash($2,kind,title,description) FROM objects WHERE id=$1",
    )
    .bind(object_id)
    .bind(embeddings::OBJECT_EMBEDDING_FORMAT)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO embeddings(object_id,model,dimensions,source_hash,format_version,input_mode,status,embedding,completed_at) VALUES($1,$2,3,$3,$4,'shared','completed','[0.1,0.2,0.3]'::vector,now())")
        .bind(object_id)
        .bind(&model)
        .bind(initial_hash)
        .bind(embeddings::OBJECT_EMBEDDING_FORMAT)
        .execute(&pool)
        .await
        .unwrap();

    let first_update = json!({
        "contract_version":"1.1.0",
        "idempotency_key":format!("description-update-{}", Uuid::new_v4()),
        "operations":[
            {"operation":"update_object","object_id":object_id,"expected_revision":1,"changes":{"description":"Evaluate the newheliotrope retrieval launch after the quality review. The latest evidence removed the earlier release blocker."}},
            {"operation":"create_connection","source":{"object_id":object_id},"kind":"derived_from","target":{"object_id":evidence},"description":"The refreshed release decision is supported by this evaluation evidence."}
        ]
    });
    let first_update = app
        .clone()
        .oneshot(request("POST", "/api/v2/apply", &token, first_update))
        .await
        .unwrap();
    assert_eq!(first_update.status(), StatusCode::OK);
    let first_update = json_body(first_update).await;
    let run_id = Uuid::parse_str(first_update["data"]["run_id"].as_str().unwrap()).unwrap();
    assert_eq!(
        first_update["data"]["event_ids"].as_array().unwrap().len(),
        2
    );
    let event: (String, String, i64, i64) = sqlx::query_as(
        "SELECT actor_type,actor_id,from_revision,to_revision FROM object_events WHERE run_id=$1 AND target_type='object'",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        event,
        ("centaur_agent".into(), "agent-universal-test".into(), 1, 2)
    );
    let run: (String, String) = sqlx::query_as("SELECT status,actor_id FROM runs WHERE id=$1")
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(run.0, "completed");
    assert_eq!(run.1, "agent-universal-test");

    let invalidated: (String, bool, i32) = sqlx::query_as(
        "SELECT status,embedding IS NULL,attempts FROM embeddings WHERE object_id=$1 AND model=$2",
    )
    .bind(object_id)
    .bind(&model)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(invalidated, ("pending".into(), true, 0));
    let stale_job = db::claim_embedding_job(&pool, &model, 3, "shared")
        .await
        .unwrap()
        .unwrap();
    assert!(stale_job.description.contains("newheliotrope"));

    let final_description = "Approve the finalvermillion retrieval launch after the quality review. This Task now records the evidence-backed release outcome.";
    let second_update = json!({
        "contract_version":"1.1.0",
        "idempotency_key":format!("description-update-{}", Uuid::new_v4()),
        "operations":[
            {"operation":"update_object","object_id":object_id,"expected_revision":2,"changes":{"title":"Approve retrieval launch","description":final_description}}
        ]
    });
    let second_update = app
        .clone()
        .oneshot(request("POST", "/api/v2/apply", &token, second_update))
        .await
        .unwrap();
    assert_eq!(second_update.status(), StatusCode::OK);
    assert!(
        db::complete_embedding_job(&pool, &stale_job, &[0.4, 0.5, 0.6])
            .await
            .is_err()
    );

    let current = db::get_object(&pool, object_id).await.unwrap();
    assert_eq!(current.revision, 3);
    assert_eq!(current.title, "Approve retrieval launch");
    assert_eq!(current.description, final_description);
    let old_matches = db::full_text_candidates(
        &pool,
        TextSearchConfig::SIMPLE,
        "obsoletequartz newheliotrope",
        Some("task"),
        10,
        false,
    )
    .await
    .unwrap();
    assert!(
        old_matches
            .iter()
            .all(|candidate| candidate.object.id != object_id)
    );
    let new_matches = db::full_text_candidates(
        &pool,
        TextSearchConfig::SIMPLE,
        "finalvermillion",
        Some("task"),
        10,
        false,
    )
    .await
    .unwrap();
    assert!(
        new_matches
            .iter()
            .any(|candidate| candidate.object.id == object_id)
    );

    let pending: (String, bool, i64, String) = sqlx::query_as(
        "SELECT status,embedding IS NULL,count(*) OVER (),source_hash FROM embeddings WHERE object_id=$1 AND model=$2",
    )
    .bind(object_id)
    .bind(&model)
    .fetch_one(&pool)
    .await
    .unwrap();
    let expected_hash: String = sqlx::query_scalar(
        "SELECT object_embedding_source_hash($2,kind,title,description) FROM objects WHERE id=$1",
    )
    .bind(object_id)
    .bind(embeddings::OBJECT_EMBEDDING_FORMAT)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pending, ("pending".into(), true, 1, expected_hash.clone()));

    let latest_job = db::claim_embedding_job(&pool, &model, 3, "shared")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(latest_job.title, "Approve retrieval launch");
    assert_eq!(latest_job.description, final_description);
    assert_eq!(latest_job.source_hash, expected_hash);
    db::fail_embedding_job(&pool, latest_job.id, "synthetic provider failure")
        .await
        .unwrap();
    sqlx::query("UPDATE embeddings SET available_at=now() WHERE id=$1")
        .bind(latest_job.id)
        .execute(&pool)
        .await
        .unwrap();
    let retry = db::claim_embedding_job(&pool, &model, 3, "shared")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retry.source_hash, latest_job.source_hash);
    assert_eq!(
        embeddings::format_object_document(&retry.kind, &retry.title, &retry.description),
        embeddings::format_object_document("task", "Approve retrieval launch", final_description)
    );
    db::complete_embedding_job(&pool, &retry, &[0.7, 0.8, 0.9])
        .await
        .unwrap();
    let completed: (String, bool, String) = sqlx::query_as(
        "SELECT status,embedding IS NOT NULL,source_hash FROM embeddings WHERE id=$1",
    )
    .bind(retry.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        completed,
        ("completed".into(), true, retry.source_hash.clone())
    );

    let no_op = json!({
        "contract_version":"1.1.0",
        "idempotency_key":format!("description-no-op-{}", Uuid::new_v4()),
        "operations":[
            {"operation":"update_object","object_id":object_id,"expected_revision":3,"changes":{"title":"Approve retrieval launch","description":final_description}}
        ]
    });
    let no_op = app
        .oneshot(request("POST", "/api/v2/apply", &token, no_op))
        .await
        .unwrap();
    assert_eq!(no_op.status(), StatusCode::OK);
    let unchanged_embedding: (String, bool, String) = sqlx::query_as(
        "SELECT status,embedding IS NOT NULL,source_hash FROM embeddings WHERE id=$1",
    )
    .bind(retry.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        unchanged_embedding,
        ("completed".into(), true, retry.source_hash)
    );
}

#[tokio::test]
async fn standalone_notes_retain_attribution_and_can_be_connected_later() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let token = "n".repeat(32);
    let app = agent_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        token.clone(),
    );
    let mut ids = Vec::new();
    for intent in ["insight", "question"] {
        let body = json!({"contract_version":"1.1.0","idempotency_key":format!("standalone-{intent}-{}",Uuid::new_v4()),"operations":[{
            "operation":"create_object","local_ref":"note","kind":"note","title":format!("Standalone {intent}"),
            "description":"An independently captured research thought awaiting further evidence.","fields":{"intent":intent,"content":"What makes research context useful?"}
        }]});
        let response = app
            .clone()
            .oneshot(request("POST", "/api/v2/apply", &token, body.clone()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let first = json_body(response).await["data"].clone();
        assert!(first["run_id"].is_string());
        assert_eq!(first["event_ids"].as_array().unwrap().len(), 1);
        let id = first["results"][0]["data"]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let replay = app
            .clone()
            .oneshot(request("POST", "/api/v2/apply", &token, body))
            .await
            .unwrap();
        assert_eq!(replay.status(), StatusCode::OK);
        let replay = json_body(replay).await["data"].clone();
        assert_eq!(replay["run_id"], first["run_id"]);
        assert_eq!(replay["replayed"], true);
        let read = app
            .clone()
            .oneshot(request(
                "POST",
                "/api/v2/read",
                &token,
                json!({"object_ids":[id],"include":["connections"]}),
            ))
            .await
            .unwrap();
        let read = json_body(read).await["data"].clone();
        assert_eq!(
            read["objects"][0]["object"]["created_by_id"],
            "agent-universal-test"
        );
        assert!(
            read["objects"][0]["connections"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        ids.push(id);
    }
    let connect = json!({"contract_version":"1.1.0","idempotency_key":format!("later-link-{}",Uuid::new_v4()),"operations":[{
        "operation":"create_connection","source":{"object_id":ids[1]},"target":{"object_id":ids[0]},"kind":"derived_from","description":"This question develops the earlier research thought."
    }]});
    let response = app
        .clone()
        .oneshot(request("POST", "/api/v2/apply", &token, connect))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    for (kind, fields) in [
        ("source", json!({"source_kind":"article"})),
        (
            "note",
            json!({"intent":"excerpt","content":"Unverified quotation."}),
        ),
    ] {
        let body = json!({"contract_version":"1.1.0","idempotency_key":format!("not-exempt-{}",Uuid::new_v4()),"operations":[{
            "operation":"create_object","local_ref":"object","kind":kind,"title":"Must not create","description":"A negative case for the standalone exemption.","fields":fields
        }]});
        let response = app
            .clone()
            .oneshot(request("POST", "/api/v2/apply", &token, body))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
    // A linked Excerpt must still supply verifiable captured evidence.
    let invalid_excerpt = json!({"contract_version":"1.1.0","idempotency_key":format!("invalid-evidence-{}",Uuid::new_v4()),"operations":[
        {"operation":"create_object","local_ref":"excerpt","kind":"note","title":"Missing evidence","description":"A negative excerpt evidence case.","fields":{"intent":"excerpt","content":"Unverified quotation."}},
        {"operation":"create_connection","source":{"local_ref":"excerpt"},"target":{"object_id":ids[0]},"kind":"derived_from","description":"A link cannot substitute for captured evidence."}
    ]});
    let response = app
        .oneshot(request("POST", "/api/v2/apply", &token, invalid_excerpt))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn slack_chat_identity_accepts_bot_routes_but_rejects_other_conversations() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let owner = support::task_owner(&pool).await;
    let chat = Uuid::new_v4();
    let thread = Uuid::new_v4().to_string();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,'chat','Synthetic Slack conversation','Disposable identity verification conversation.','system','test','system','test')")
        .bind(chat).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO chats(object_id,provider,workspace_id,channel_id,thread_id,surface_kind) VALUES($1,'slack','T-SYNTHETIC','C-SYNTHETIC',$2,'channel')")
        .bind(chat).bind(&thread).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let token = "k".repeat(32);
    let app = agent_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        token.clone(),
    );
    let keys = [
        (
            format!("slack:T-SYNTHETIC:C-SYNTHETIC:{thread}"),
            StatusCode::OK,
        ),
        (
            format!("Slack:T-SYNTHETIC:bot-researcher:C-SYNTHETIC:{thread}"),
            StatusCode::OK,
        ),
        (
            format!("slack:T-SYNTHETIC:bot-networking:C-SYNTHETIC:{thread}"),
            StatusCode::OK,
        ),
        (
            format!("slack:OTHER:bot-researcher:C-SYNTHETIC:{thread}"),
            StatusCode::FORBIDDEN,
        ),
        (
            format!("slack:T-SYNTHETIC:bot-researcher:OTHER:{thread}"),
            StatusCode::FORBIDDEN,
        ),
        (
            "slack:T-SYNTHETIC:bot-researcher:C-SYNTHETIC:other".into(),
            StatusCode::FORBIDDEN,
        ),
        (
            format!("slack:T-SYNTHETIC:bot-:C-SYNTHETIC:{thread}"),
            StatusCode::BAD_REQUEST,
        ),
        (
            format!("slack:T-SYNTHETIC:unexpected:C-SYNTHETIC:{thread}"),
            StatusCode::BAD_REQUEST,
        ),
        (
            format!("other:T-SYNTHETIC:bot-researcher:C-SYNTHETIC:{thread}"),
            StatusCode::BAD_REQUEST,
        ),
        (
            format!("slack:T-SYNTHETIC::C-SYNTHETIC:{thread}"),
            StatusCode::BAD_REQUEST,
        ),
        (
            format!("slack:T-SYNTHETIC:bot-researcher:C-SYNTHETIC:{thread}:extra"),
            StatusCode::BAD_REQUEST,
        ),
    ];
    for (key, expected) in keys {
        let body = json!({"contract_version":"1.1.0","idempotency_key":Uuid::new_v4().to_string(),"chat_object_id":chat,"operations":[{"operation":"create_object","local_ref":"task","kind":"task","title":"Synthetic chat-linked task","description":"Task for verifying the authenticated conversation boundary.","fields":{"owner_object_id":owner,"due_at":"2099-01-01T00:00:00Z","brief_markdown":"Verify task creation in the matching conversation."}}]});
        let mut req = request("POST", "/api/v2/apply", &token, body);
        req.headers_mut()
            .insert("x-centaur-thread-key", key.parse().unwrap());
        let response = app.clone().oneshot(req).await.unwrap();
        let status = response.status();
        let result = json_body(response).await;
        assert_eq!(status, expected, "write {key}: {result}");
        if status == StatusCode::OK {
            assert_eq!(
                result["data"]["results"][0]["data"]["created_by_id"],
                "agent-universal-test"
            );
        }
        let mut req = request(
            "GET",
            &format!("/api/v2/context?q=synthetic&chat_object_id={chat}"),
            &token,
            json!({}),
        );
        req.headers_mut()
            .insert("x-centaur-thread-key", key.parse().unwrap());
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            response.status(),
            expected,
            "retrieval {key}: {}",
            json_body(response).await
        );
    }
}

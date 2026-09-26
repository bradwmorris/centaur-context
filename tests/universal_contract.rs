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
            {"operation":"create_object","local_ref":"note","kind":"note","title":"Universal note","description":"A disposable Note created by the universal contract test.","fields":{"content":"Test evidence.","content_format":"markdown","intent":"idea"}},
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
    for intent in ["idea", "fact"] {
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
        "operation":"create_connection","source":{"object_id":ids[1]},"target":{"object_id":ids[0]},"kind":"derived_from","description":"This fact develops the earlier research thought."
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
            json!({"intent":"question","content":"An unresolved question belongs in working notes."}),
        ),
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
async fn legacy_note_intents_survive_unrelated_universal_edits() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let token = "l".repeat(32);
    let app = agent_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        token.clone(),
    );
    for legacy in [Some("insight"), Some("question"), None] {
        let id = Uuid::new_v4();
        let mut seed = pool.begin().await.unwrap();
        sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,'note','Legacy thought','A historical research Note.','system','test','system','test')")
            .bind(id).execute(&mut *seed).await.unwrap();
        sqlx::query("INSERT INTO notes(object_id,content,content_format,intent) VALUES($1,'Original words','plain_text',$2)")
            .bind(id).bind(legacy).execute(&mut *seed).await.unwrap();
        seed.commit().await.unwrap();
        let body = json!({"contract_version":"1.1.0","idempotency_key":Uuid::new_v4().to_string(),"operations":[
            {"operation":"update_object","object_id":id,"expected_revision":1,"changes":{"title":"Legacy thought retitled"}}
        ]});
        let response = app
            .clone()
            .oneshot(request("POST", "/api/v2/apply", &token, body))
            .await
            .unwrap();
        let status = response.status();
        let payload = json_body(response).await;
        assert_eq!(status, StatusCode::OK, "{payload}");
        let actual: (String, Option<String>) =
            sqlx::query_as("SELECT content,intent FROM notes WHERE object_id=$1")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            actual,
            ("Original words".to_owned(), legacy.map(str::to_owned))
        );
    }
}

#[tokio::test]
async fn universal_artifact_revision_keeps_predecessor_and_note_content() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let token = "a".repeat(32);
    let app = agent_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        token.clone(),
    );
    let create = json!({"contract_version":"1.1.0","idempotency_key":Uuid::new_v4().to_string(),"operations":[
        {"operation":"create_object","local_ref":"idea","kind":"note","title":"Synthetic idea","description":"A source-free synthetic Idea.","fields":{"intent":"idea","content":"Original idea wording."}}
    ]});
    let created = json_body(
        app.clone()
            .oneshot(request("POST", "/api/v2/apply", &token, create))
            .await
            .unwrap(),
    )
    .await;
    let note: Uuid = created["data"]["results"][0]["data"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let mut predecessor = None;
    for (revision, copy) in [(1, "Draft copy"), (2, "Edited draft copy")] {
        let apply = json!({"contract_version":"1.1.0","idempotency_key":Uuid::new_v4().to_string(),"operations":[
            {"operation":"append_artifact","object":{"object_id":note},"expected_revision":revision,"kind":"publication_receipt","content":json!({"status":"draft","copy":copy}).to_string(),"media_type":"application/json","metadata":{"status":"draft"},"supersedes_artifact_id":predecessor}
        ]});
        let response = app
            .clone()
            .oneshot(request("POST", "/api/v2/apply", &token, apply))
            .await
            .unwrap();
        let status = response.status();
        let body = json_body(response).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            body["data"]["results"][0]["data"]["supersedes_artifact_id"],
            json!(predecessor)
        );
        predecessor = Some(
            body["data"]["results"][0]["data"]["id"]
                .as_str()
                .unwrap()
                .parse::<Uuid>()
                .unwrap(),
        );
    }
    assert_eq!(
        db::get_note(&pool, note).await.unwrap().content,
        "Original idea wording."
    );
}

#[tokio::test]
async fn protected_excerpt_capture_accepts_runtime_chat_identity_without_broadening_writes() {
    use sha2::{Digest, Sha256};
    let Some(pool) = test_pool().await else {
        return;
    };
    let source = Uuid::new_v4();
    let chat = Uuid::new_v4();
    let unrelated_source = Uuid::new_v4();
    let artifact = Uuid::new_v4();
    let channel = format!("C{}", Uuid::new_v4());
    let mut seed = pool.begin().await.unwrap();
    for (id, kind) in [
        (source, "source"),
        (unrelated_source, "source"),
        (chat, "chat"),
    ] {
        sqlx::query("INSERT INTO objects (id,kind,title,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES ($1,$2,'Protected excerpt fixture','Synthetic protected capture test.',true,'system','test','system','test')")
            .bind(id).bind(kind).execute(&mut *seed).await.unwrap();
    }
    sqlx::query("INSERT INTO sources (object_id,source_kind) VALUES ($1,'article'),($2,'article')")
        .bind(source)
        .bind(unrelated_source)
        .execute(&mut *seed)
        .await
        .unwrap();
    sqlx::query("INSERT INTO chats (object_id,provider,workspace_id,channel_id,thread_id,surface_kind) VALUES ($1,'slack','Tsynthetic',$2,'123.456','channel')").bind(chat).bind(&channel).execute(&mut *seed).await.unwrap();
    seed.commit().await.unwrap();
    let content = "Let agents message each other.";
    sqlx::query("INSERT INTO artifacts (id,object_id,kind,content,media_type,sha256,size_bytes,capture_outcome) VALUES ($1,$2,'transcript',$3,'text/plain',$4,$5,'complete')")
        .bind(artifact).bind(source).bind(content).bind(format!("{:x}",Sha256::digest(content))).bind(content.len() as i64).execute(&pool).await.unwrap();
    let token = "e".repeat(32);
    let app = agent_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        token.clone(),
    );
    let body = json!({"contract_version":"1.1.0","idempotency_key":format!("protected-excerpt-{}",Uuid::new_v4()),"chat_object_id":chat,"operations":[
        {"operation":"create_object","local_ref":"excerpt","kind":"note","title":"Primitive agent messaging","description":"Verbatim synthetic evidence of agent collaboration.","fields":{"intent":"excerpt","content":content,"source_artifact_id":artifact,"source_locator":{"type":"character_range","start":0,"end":content.len()}}},
        {"operation":"create_connection","source":{"local_ref":"excerpt"},"target":{"object_id":source},"kind":"derived_from","description":"This exact excerpt comes from the protected Source's transcript."}
    ]});
    let req = |b: Value, thread: &str| {
        let mut r = request("POST", "/api/v2/apply", &token, b);
        r.headers_mut()
            .insert("x-centaur-thread-key", thread.parse().unwrap());
        r
    };
    let short = format!("slack:{channel}:123.456");
    // Mismatched identities fail before any note is committed.
    for thread in [
        "slack:other:123.456".to_owned(),
        format!("slack:wrong:{channel}:123.456"),
    ] {
        let r = app
            .clone()
            .oneshot(req(body.clone(), &thread))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
    }
    let mut invalid_evidence = body.clone();
    invalid_evidence["operations"][0]["fields"]["content"] =
        json!("Not present in the transcript.");
    let r = app
        .clone()
        .oneshot(req(invalid_evidence, &short))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let unauthorized = request("POST", "/api/v2/apply", "wrong-token", body.clone());
    let r = app.clone().oneshot(unauthorized).await.unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    let mut wrong_source = body.clone();
    wrong_source["operations"][1]["target"]["object_id"] = json!(unrelated_source);
    let r = app
        .clone()
        .oneshot(req(wrong_source, &short))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    // An invalid unrelated protected edge rolls back the otherwise valid Note.
    let mut bad = body.clone();
    bad["operations"][1]["kind"] = json!("related_to");
    let r = app.clone().oneshot(req(bad, &short)).await.unwrap();
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM notes WHERE source_artifact_id=$1")
        .bind(artifact)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let r = app
        .clone()
        .oneshot(req(body.clone(), &short))
        .await
        .unwrap();
    let status = r.status();
    let result = json_body(r).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["data"]["results"].as_array().unwrap().len(), 3);
    let note =
        Uuid::parse_str(result["data"]["results"][0]["data"]["id"].as_str().unwrap()).unwrap();
    let r = app
        .clone()
        .oneshot(req(body.clone(), &short))
        .await
        .unwrap();
    assert_eq!(json_body(r).await["data"]["replayed"], true);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM connections WHERE target_object_id=$1 OR source_object_id=$1",
    )
    .bind(note)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 2);
    let (revision, protected): (i64, bool) =
        sqlx::query_as("SELECT revision,protected FROM objects WHERE id=$1")
            .bind(source)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((revision, protected), (1, true));
    // Fully qualified identities still work; validation does not create another Note.
    let mut validate = body.clone();
    validate["validate_only"] = json!(true);
    let r = app
        .clone()
        .oneshot(req(
            validate.clone(),
            &format!("slack:Tsynthetic:{channel}:123.456"),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    // Original thinking and actionable follow-up also connect without editing the Source.
    let owner = support::task_owner(&pool).await;
    for (kind, fields, relation) in [
        (
            "note",
            json!({"intent":"idea","content":"My own interpretation."}),
            "derived_from",
        ),
        (
            "note",
            json!({"intent":"fact","content":"A practical fact from the selected Source."}),
            "derived_from",
        ),
        (
            "task",
            json!({"status":"todo","priority":"medium","owner_object_id":owner,"due_at":"2099-01-01T00:00:00Z","brief_markdown":"Evaluate the Source's coordination model."}),
            "about",
        ),
    ] {
        let research = json!({"contract_version":"1.1.0","idempotency_key":format!("research-{}",Uuid::new_v4()),"chat_object_id":chat,"validate_only":true,"operations":[
            {"operation":"create_object","local_ref":"research","kind":kind,"title":"Research follow-up","description":"A synthetic follow-up to the selected Source.","fields":fields},
            {"operation":"create_connection","source":{"local_ref":"research"},"target":{"object_id":source},"kind":relation,"description":"This research follows directly from the selected Source."}
        ]});
        let r = app.clone().oneshot(req(research, &short)).await.unwrap();
        let status = r.status();
        let response = json_body(r).await;
        assert_eq!(status, StatusCode::OK, "{response}");
    }
    // Caller-supplied links to the protected Chat do not inherit the automatic exception.
    let explicit = json!({"contract_version":"1.1.0","idempotency_key":format!("explicit-chat-{}",Uuid::new_v4()),"operations":[{"operation":"create_connection","source":{"object_id":chat},"target":{"object_id":note},"kind":"about","description":"A caller cannot claim the server-generated provenance exception."}]});
    let r = app.clone().oneshot(req(explicit, &short)).await.unwrap();
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let update = json!({"contract_version":"1.1.0","idempotency_key":format!("protected-edit-{}",Uuid::new_v4()),"operations":[{"operation":"update_object","object_id":source,"expected_revision":1,"changes":{"description":"Unauthorized edit."}}]});
    let r = app.clone().oneshot(req(update, &short)).await.unwrap();
    assert_eq!(r.status(), StatusCode::UNPROCESSABLE_ENTITY);
    // A second workspace using the same short identity makes that identity ambiguous.
    let mut seed = pool.begin().await.unwrap();
    let other = Uuid::new_v4();
    sqlx::query("INSERT INTO objects (id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES ($1,'chat','Other workspace','Synthetic ambiguity test.','system','test','system','test')").bind(other).execute(&mut *seed).await.unwrap();
    sqlx::query("INSERT INTO chats (object_id,provider,workspace_id,channel_id,thread_id,surface_kind) VALUES ($1,'slack','Tother',$2,'123.456','channel')").bind(other).bind(&channel).execute(&mut *seed).await.unwrap();
    seed.commit().await.unwrap();
    let r = app
        .clone()
        .oneshot(req(validate.clone(), &short))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    let r = app
        .clone()
        .oneshot(req(
            validate,
            &format!("slack:Tsynthetic:{channel}:123.456"),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
}

#[tokio::test]
async fn protected_source_research_notes_are_versioned_without_changing_source_evidence() {
    use sha2::{Digest, Sha256};
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping protected Source research-notes contract: TEST_DATABASE_URL is not set"
        );
        return;
    };

    let source = Uuid::new_v4();
    let protected_note = Uuid::new_v4();
    let other_source = Uuid::new_v4();
    let document_key = format!("working-notes-{}", Uuid::new_v4());
    let canonical_artifact = Uuid::new_v4();
    let wrong_kind_artifact = Uuid::new_v4();
    let mut seed = pool.begin().await.unwrap();
    for (id, kind, protected) in [
        (source, "source", true),
        (protected_note, "note", true),
        (other_source, "source", true),
    ] {
        sqlx::query("INSERT INTO objects (id,kind,title,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES ($1,$2,'Protected research fixture','Synthetic protected-source append test.', $3,'system','test','system','test')")
            .bind(id).bind(kind).bind(protected).execute(&mut *seed).await.unwrap();
    }
    sqlx::query("INSERT INTO sources (object_id,source_kind,canonical_uri) VALUES ($1,'article','https://example.invalid/canonical'),($2,'article','https://example.invalid/other')")
        .bind(source).bind(other_source).execute(&mut *seed).await.unwrap();
    sqlx::query(
        "INSERT INTO notes (object_id,content,intent) VALUES ($1,'Protected note fixture','idea')",
    )
    .bind(protected_note)
    .execute(&mut *seed)
    .await
    .unwrap();
    seed.commit().await.unwrap();

    let canonical_content = "Synthetic canonical captured evidence.";
    sqlx::query("INSERT INTO artifacts (id,object_id,kind,content,media_type,sha256,size_bytes,capture_outcome,metadata) VALUES ($1,$2,'research_notes',$3,'text/plain',$4,$5,'complete',$6)")
        .bind(canonical_artifact).bind(source).bind(canonical_content)
        .bind(format!("{:x}", Sha256::digest(canonical_content))).bind(canonical_content.len() as i64)
        .bind(json!({"document_key":document_key}))
        .execute(&pool).await.unwrap();
    let wrong_kind_content = "Synthetic non-notes artifact.";
    sqlx::query("INSERT INTO artifacts (id,object_id,kind,content,media_type,sha256,size_bytes,capture_outcome,metadata) VALUES ($1,$2,'supporting_text',$3,'text/plain',$4,$5,'complete','{}')")
        .bind(wrong_kind_artifact).bind(source).bind(wrong_kind_content)
        .bind(format!("{:x}", Sha256::digest(wrong_kind_content))).bind(wrong_kind_content.len() as i64)
        .execute(&pool).await.unwrap();
    sqlx::query("UPDATE sources SET current_artifact_id=$2 WHERE object_id=$1")
        .bind(source)
        .bind(canonical_artifact)
        .execute(&pool)
        .await
        .unwrap();

    let token = "r".repeat(32);
    let app = agent_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        token.clone(),
    );
    let send = |key: String, operation: Value| {
        let app = app.clone();
        let token = token.clone();
        async move {
            app.oneshot(request(
                "POST",
                "/api/v2/apply",
                &token,
                json!({
                    "contract_version":"1.1.0",
                    "idempotency_key":key,
                    "operations":[operation]
                }),
            ))
            .await
            .unwrap()
        }
    };
    let append = |expected_revision: i64, body: &str, key: &str, predecessor: Option<Uuid>| {
        let mut metadata = json!({"document_key":key});
        if let Some(predecessor) = predecessor {
            metadata["predecessor_artifact_id"] = json!(predecessor);
        }
        json!({
            "operation":"append_artifact",
            "object":{"object_id":source},
            "expected_revision":expected_revision,
            "kind":"research_notes",
            "title":"Research working notes",
            "content":body,
            "media_type":"text/markdown",
            "capture_outcome":"complete",
            "metadata":metadata,
            "supersedes_artifact_id":predecessor
        })
    };

    let first_request_key = format!("protected-notes-first-{}", Uuid::new_v4());
    let first_request = append(1, "First cleaned research note.", &document_key, None);
    let first_response = send(first_request_key.clone(), first_request.clone()).await;
    assert_eq!(first_response.status(), StatusCode::OK);
    let first_response = json_body(first_response).await;
    let first_id: Uuid = first_response["data"]["results"][0]["data"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    let replay = send(first_request_key, first_request).await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(json_body(replay).await["data"]["replayed"], true);

    let second = send(
        format!("protected-notes-second-{}", Uuid::new_v4()),
        append(
            2,
            "Second cleaned research note.",
            &document_key,
            Some(first_id),
        ),
    )
    .await;
    assert_eq!(second.status(), StatusCode::OK);
    let second = json_body(second).await;
    let second_id: Uuid = second["data"]["results"][0]["data"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    let stale = send(
        format!("protected-notes-stale-{}", Uuid::new_v4()),
        append(2, "Stale revision.", &document_key, Some(second_id)),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let missing_key = send(
        format!("protected-notes-missing-key-{}", Uuid::new_v4()),
        json!({
            "operation":"append_artifact","object":{"object_id":source},
            "expected_revision":3,"kind":"research_notes","content":"No key",
            "capture_outcome":"complete"
        }),
    )
    .await;
    assert_eq!(missing_key.status(), StatusCode::UNPROCESSABLE_ENTITY);

    for (name, object_id, revision, metadata, predecessor) in [
        (
            "canonical-evidence",
            source,
            3,
            json!({"document_key":document_key,"predecessor_artifact_id":canonical_artifact}),
            Some(canonical_artifact),
        ),
        (
            "wrong-kind",
            source,
            3,
            json!({"document_key":document_key,"predecessor_artifact_id":wrong_kind_artifact}),
            Some(wrong_kind_artifact),
        ),
        (
            "cross-document",
            source,
            3,
            json!({"document_key":"another-document","predecessor_artifact_id":first_id}),
            Some(first_id),
        ),
        (
            "cross-source",
            other_source,
            1,
            json!({"document_key":document_key,"predecessor_artifact_id":first_id}),
            Some(first_id),
        ),
    ] {
        let response = send(
            format!("protected-notes-{name}-{}", Uuid::new_v4()),
            json!({
                "operation":"append_artifact","object":{"object_id":object_id},
                "expected_revision":revision,"kind":"research_notes","content":"Invalid successor",
                "capture_outcome":"complete","metadata":metadata,
                "supersedes_artifact_id":predecessor
            }),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{name}"
        );
    }

    for (name, object_id, revision) in [
        ("protected-note", protected_note, 1),
        ("protected-source", source, 3),
    ] {
        let update = send(
            format!("protected-notes-update-{name}-{}", Uuid::new_v4()),
            json!({"operation":"update_object","object_id":object_id,"expected_revision":revision,"changes":{"title":"Must remain protected"}}),
        ).await;
        assert_eq!(update.status(), StatusCode::UNPROCESSABLE_ENTITY, "{name}");
        let archive = send(
            format!("protected-notes-archive-{name}-{}", Uuid::new_v4()),
            json!({"operation":"archive_object","object_id":object_id,"expected_revision":revision}),
        ).await;
        assert_eq!(archive.status(), StatusCode::UNPROCESSABLE_ENTITY, "{name}");
    }
    let non_source_artifact = send(
        format!("protected-notes-non-source-{}", Uuid::new_v4()),
        json!({
            "operation":"append_artifact","object":{"object_id":protected_note},
            "expected_revision":1,"kind":"research_notes","content":"Wrong target",
            "capture_outcome":"complete","metadata":{"document_key":document_key}
        }),
    )
    .await;
    assert_eq!(
        non_source_artifact.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let other_artifact_kind = send(
        format!("protected-notes-other-kind-{}", Uuid::new_v4()),
        json!({
            "operation":"append_artifact","object":{"object_id":source},
            "expected_revision":3,"kind":"supporting_text","content":"Wrong artifact kind",
            "capture_outcome":"complete","metadata":{"document_key":document_key}
        }),
    )
    .await;
    assert_eq!(
        other_artifact_kind.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let source_state: (bool, i64, String, Option<Uuid>) = sqlx::query_as(
        "SELECT o.protected,o.revision,s.canonical_uri,s.current_artifact_id FROM objects o JOIN sources s ON s.object_id=o.id WHERE o.id=$1",
    ).bind(source).fetch_one(&pool).await.unwrap();
    assert_eq!(
        source_state,
        (
            true,
            3,
            "https://example.invalid/canonical".into(),
            Some(canonical_artifact)
        )
    );
    let artifact_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM artifacts WHERE object_id=$1")
            .bind(source)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(artifact_count, 4);
    let final_revision: (Uuid, String) = sqlx::query_as(
        "SELECT supersedes_artifact_id,metadata->>'document_key' FROM artifacts WHERE id=$1",
    )
    .bind(second_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(final_revision, (first_id, document_key));
}

#[tokio::test]
async fn protected_preserved_notes_and_their_connections_can_be_archived_atomically() {
    use sha2::{Digest, Sha256};
    let Some(pool) = test_pool().await else {
        eprintln!(
            "skipping protected preserved-Note archival contract: TEST_DATABASE_URL is not set"
        );
        return;
    };

    let token = "p".repeat(32);
    let app = agent_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        token.clone(),
    );
    let note = Uuid::new_v4();
    let source = Uuid::new_v4();
    let content = "Synthetic preserved Note content for archival verification.";
    let document_key = format!("preserved-note-{}", Uuid::new_v4());
    let artifact = Uuid::new_v4();
    let derived = Uuid::new_v4();
    let related = Uuid::new_v4();
    let mut seed = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES ($1,'note','Synthetic preserved Note','A disposable protected Note for archival tests.',true,'system','test','system','test'),($2,'source','Synthetic preservation Source','A disposable protected Source carrying exact test notes.',true,'system','test','system','test')")
        .bind(note).bind(source).execute(&mut *seed).await.unwrap();
    sqlx::query("INSERT INTO notes(object_id,content,intent) VALUES ($1,$2,'idea')")
        .bind(note)
        .bind(content)
        .execute(&mut *seed)
        .await
        .unwrap();
    sqlx::query("INSERT INTO sources(object_id,source_kind,canonical_uri) VALUES ($1,'article','https://example.invalid/preserved-note')")
        .bind(source).execute(&mut *seed).await.unwrap();
    let artifact_body = format!("Complete consolidated text.\n\n{content}\n\nEnd of capture.");
    sqlx::query("INSERT INTO artifacts(id,object_id,kind,content,media_type,sha256,size_bytes,capture_outcome,metadata) VALUES ($1,$2,'research_notes',$3,'text/markdown',$4,$5,'complete',$6)")
        .bind(artifact).bind(source).bind(&artifact_body)
        .bind(format!("{:x}", Sha256::digest(artifact_body.as_bytes())))
        .bind(artifact_body.len() as i64)
        .bind(json!({"document_key":document_key,"source_note_manifest":[{"object_id":note,"revision":1}]}))
        .execute(&mut *seed).await.unwrap();
    for (id, kind) in [(derived, "derived_from"), (related, "related_to")] {
        sqlx::query("INSERT INTO connections(id,source_object_id,kind,target_object_id,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES ($1,$2,$3,$4,'Synthetic preserved-Note connection.',true,'system','test','system','test')")
            .bind(id).bind(note).bind(kind).bind(source).execute(&mut *seed).await.unwrap();
    }
    seed.commit().await.unwrap();

    let apply_request = json!({
        "contract_version":"1.1.0",
        "idempotency_key":format!("archive-preserved-note-{}",Uuid::new_v4()),
        "operations":[
            {"operation":"archive_connection","connection_id":related,"expected_revision":1},
            {"operation":"archive_connection","connection_id":derived,"expected_revision":1},
            {"operation":"archive_object","object_id":note,"expected_revision":1}
        ]
    });
    let mut dry_run_request = apply_request.clone();
    dry_run_request["validate_only"] = json!(true);
    dry_run_request["idempotency_key"] =
        json!(format!("validate-preserved-note-{}", Uuid::new_v4()));
    let dry_run = app
        .clone()
        .oneshot(request("POST", "/api/v2/apply", &token, dry_run_request))
        .await
        .unwrap();
    let dry_status = dry_run.status();
    let dry_body = json_body(dry_run).await;
    assert_eq!(dry_status, StatusCode::OK, "{dry_body}");
    assert_eq!(dry_body["data"]["validated_only"], true);
    assert_eq!(dry_body["data"]["run_id"], Value::Null);
    assert_eq!(dry_body["data"]["event_ids"], json!([]));
    let before_commit: (Option<time::OffsetDateTime>, Option<time::OffsetDateTime>) =
        sqlx::query_as(
            "SELECT n.archived_at,c.archived_at FROM objects n CROSS JOIN connections c WHERE n.id=$1 AND c.id=$2",
        )
        .bind(note)
        .bind(derived)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(before_commit.0.is_none());
    assert!(before_commit.1.is_none());
    let response = app
        .clone()
        .oneshot(request("POST", "/api/v2/apply", &token, apply_request))
        .await
        .unwrap();
    let status = response.status();
    let body = json_body(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["results"].as_array().unwrap().len(), 3);
    let state: (Option<time::OffsetDateTime>, i64, Option<time::OffsetDateTime>, i64) = sqlx::query_as(
        "SELECT n.archived_at,n.revision,c.archived_at,c.revision FROM objects n CROSS JOIN connections c WHERE n.id=$1 AND c.id=$2",
    ).bind(note).bind(derived).fetch_one(&pool).await.unwrap();
    assert!(state.0.is_some());
    assert_eq!(state.1, 2);
    assert!(state.2.is_some());
    assert_eq!(state.3, 2);
    let event_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM object_events WHERE run_id=$1 AND action='archived'",
    )
    .bind(
        body["data"]["run_id"]
            .as_str()
            .unwrap()
            .parse::<Uuid>()
            .unwrap(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event_count, 3);

    // A mismatched manifest cannot authorize either the protected link or the Note.
    let bad_note = Uuid::new_v4();
    let bad_source = Uuid::new_v4();
    let bad_connection = Uuid::new_v4();
    let bad_artifact = Uuid::new_v4();
    let mut seed = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES ($1,'note','Mismatched manifest Note','A disposable protected Note with no matching manifest entry.',true,'system','test','system','test'),($2,'source','Mismatched manifest Source','A disposable protected Source.',true,'system','test','system','test')")
        .bind(bad_note).bind(bad_source).execute(&mut *seed).await.unwrap();
    sqlx::query("INSERT INTO notes(object_id,content,intent) VALUES ($1,'Unpreserved synthetic content.','idea')")
        .bind(bad_note).execute(&mut *seed).await.unwrap();
    sqlx::query("INSERT INTO sources(object_id,source_kind) VALUES ($1,'article')")
        .bind(bad_source)
        .execute(&mut *seed)
        .await
        .unwrap();
    let bad_artifact_body = "Different complete capture.";
    sqlx::query("INSERT INTO artifacts(id,object_id,kind,content,media_type,sha256,size_bytes,capture_outcome,metadata) VALUES ($1,$2,'research_notes',$3,'text/plain',$4,$5,'complete',$6)")
        .bind(bad_artifact).bind(bad_source).bind(bad_artifact_body)
        .bind(format!("{:x}", Sha256::digest(bad_artifact_body.as_bytes())))
        .bind(bad_artifact_body.len() as i64)
        .bind(json!({"document_key":"mismatch","source_note_manifest":[{"object_id":Uuid::new_v4(),"revision":1}]}))
        .execute(&mut *seed).await.unwrap();
    sqlx::query("INSERT INTO connections(id,source_object_id,kind,target_object_id,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES ($1,$2,'derived_from',$3,'Synthetic invalid preservation link.',true,'system','test','system','test')")
        .bind(bad_connection).bind(bad_note).bind(bad_source).execute(&mut *seed).await.unwrap();
    seed.commit().await.unwrap();
    let denied = app.clone().oneshot(request("POST","/api/v2/apply",&token,json!({
        "contract_version":"1.1.0",
        "idempotency_key":format!("deny-unpreserved-note-{}",Uuid::new_v4()),
        "operations":[
            {"operation":"archive_connection","connection_id":bad_connection,"expected_revision":1},
            {"operation":"archive_object","object_id":bad_note,"expected_revision":1}
        ]
    }))).await.unwrap();
    assert_eq!(denied.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let still_active: (Option<time::OffsetDateTime>, Option<time::OffsetDateTime>) = sqlx::query_as(
        "SELECT o.archived_at,c.archived_at FROM objects o CROSS JOIN connections c WHERE o.id=$1 AND c.id=$2",
    ).bind(bad_note).bind(bad_connection).fetch_one(&pool).await.unwrap();
    assert!(still_active.0.is_none());
    assert!(still_active.1.is_none());

    async fn seed_invalid_case(pool: &PgPool, case: &str) -> (Uuid, Uuid, bool) {
        use sha2::{Digest, Sha256};
        let note = Uuid::new_v4();
        let source = Uuid::new_v4();
        let artifact_source = if case == "wrong-source" {
            Uuid::new_v4()
        } else {
            source
        };
        let connection = Uuid::new_v4();
        let artifact = Uuid::new_v4();
        let note_content = if case == "wrong-body" {
            "Different content absent from the Artifact."
        } else {
            "Synthetic boundary Note content."
        };
        let artifact_body = "Boundary capture containing Synthetic boundary Note content.";
        let manifest_revision = if case == "stale-revision" { 2 } else { 1 };
        let manifest = match case {
            "missing-manifest" => None,
            "wrong-manifest" => Some(json!([{"object_id":Uuid::new_v4(),"revision":1}])),
            _ => Some(json!([{"object_id":note,"revision":manifest_revision}])),
        };
        let metadata = match case {
            "missing-key" => json!({"source_note_manifest":manifest.unwrap()}),
            "missing-manifest" => json!({"document_key":"boundary-capture"}),
            _ => {
                json!({"document_key":"boundary-capture","source_note_manifest":manifest.unwrap()})
            }
        };
        let capture_outcome = if case == "incomplete" {
            "incomplete"
        } else {
            "complete"
        };
        let mut seed = pool.begin().await.unwrap();
        sqlx::query("INSERT INTO objects(id,kind,title,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES ($1,'note','Boundary Note','Disposable protected Note for archive denial coverage.',true,'system','test','system','test'),($2,'source','Boundary Source','Disposable Source for archive denial coverage.',true,'system','test','system','test')")
            .bind(note).bind(source).execute(&mut *seed).await.unwrap();
        sqlx::query("INSERT INTO notes(object_id,content,intent) VALUES ($1,$2,'idea')")
            .bind(note)
            .bind(note_content)
            .execute(&mut *seed)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sources(object_id,source_kind) VALUES ($1,'article')")
            .bind(source)
            .execute(&mut *seed)
            .await
            .unwrap();
        if artifact_source != source {
            sqlx::query("INSERT INTO objects(id,kind,title,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES ($1,'source','Other Boundary Source','Source that does not back the Note link.',true,'system','test','system','test')")
                .bind(artifact_source).execute(&mut *seed).await.unwrap();
            sqlx::query("INSERT INTO sources(object_id,source_kind) VALUES ($1,'article')")
                .bind(artifact_source)
                .execute(&mut *seed)
                .await
                .unwrap();
        }
        sqlx::query("INSERT INTO artifacts(id,object_id,kind,content,media_type,sha256,size_bytes,capture_outcome,capture_reason,metadata) VALUES ($1,$2,'research_notes',$3,'text/plain',$4,$5,$6,$7,$8)")
            .bind(artifact).bind(artifact_source).bind(artifact_body)
            .bind(format!("{:x}",Sha256::digest(artifact_body.as_bytes())))
            .bind(artifact_body.len() as i64).bind(capture_outcome)
            .bind((capture_outcome != "complete").then_some("Synthetic incomplete capture"))
            .bind(metadata).execute(&mut *seed).await.unwrap();
        sqlx::query("INSERT INTO connections(id,source_object_id,kind,target_object_id,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES ($1,$2,'derived_from',$3,'Synthetic boundary Note link.',true,'system','test','system','test')")
            .bind(connection).bind(note).bind(source).execute(&mut *seed).await.unwrap();
        if case == "prior-archive" {
            sqlx::query("UPDATE connections SET archived_at=now(),revision=revision+1 WHERE id=$1")
                .bind(connection)
                .execute(&mut *seed)
                .await
                .unwrap();
        }
        seed.commit().await.unwrap();
        (note, connection, case == "prior-archive")
    }

    for case in [
        "stale-revision",
        "missing-manifest",
        "missing-key",
        "wrong-manifest",
        "wrong-body",
        "wrong-source",
        "incomplete",
        "prior-archive",
    ] {
        let (case_note, case_connection, prior_archive) = seed_invalid_case(&pool, case).await;
        let operations = if prior_archive {
            json!([{"operation":"archive_object","object_id":case_note,"expected_revision":1}])
        } else {
            json!([{"operation":"archive_connection","connection_id":case_connection,"expected_revision":1}])
        };
        let denied = app
            .clone()
            .oneshot(request(
                "POST",
                "/api/v2/apply",
                &token,
                json!({
                    "contract_version":"1.1.0",
                    "idempotency_key":format!("deny-{case}-{}",Uuid::new_v4()),
                    "operations":operations
                }),
            ))
            .await
            .unwrap();
        assert_eq!(denied.status(), StatusCode::UNPROCESSABLE_ENTITY, "{case}");
        let archived: Option<time::OffsetDateTime> =
            sqlx::query_scalar("SELECT archived_at FROM objects WHERE id=$1")
                .bind(case_note)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(archived.is_none(), "{case}");
    }
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

#[tokio::test]
async fn protected_research_connections_are_creation_only_and_endpoint_scoped() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping protected research Connection contract: TEST_DATABASE_URL is not set");
        return;
    };

    let source_ids = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    let entity_ids = [
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    ];
    let archived_entity = Uuid::new_v4();
    let chat = Uuid::new_v4();
    let mut seed = pool.begin().await.unwrap();
    for (id, kind, protected) in [
        (source_ids[0], "source", true),
        (source_ids[1], "source", false),
        (source_ids[2], "source", true),
        (entity_ids[0], "entity", false),
        (entity_ids[1], "entity", true),
        (entity_ids[2], "entity", true),
        (entity_ids[3], "entity", false),
        (archived_entity, "entity", true),
        (chat, "chat", true),
    ] {
        sqlx::query("INSERT INTO objects(id,kind,title,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,$2,'Protected-link synthetic fixture','Synthetic protected Connection boundary test.',$3,'system','protected-link-test','system','protected-link-test')")
            .bind(id).bind(kind).bind(protected).execute(&mut *seed).await.unwrap();
        if kind == "source" {
            sqlx::query("INSERT INTO sources(object_id,source_kind) VALUES($1,'article')")
                .bind(id)
                .execute(&mut *seed)
                .await
                .unwrap();
        } else if kind == "entity" {
            sqlx::query("INSERT INTO entities(object_id,entity_kind) VALUES($1,'person')")
                .bind(id)
                .execute(&mut *seed)
                .await
                .unwrap();
        } else if kind == "chat" {
            sqlx::query("INSERT INTO chats(object_id) VALUES($1)")
                .bind(id)
                .execute(&mut *seed)
                .await
                .unwrap();
        }
    }
    sqlx::query("UPDATE objects SET archived_at=now() WHERE id=$1")
        .bind(archived_entity)
        .execute(&mut *seed)
        .await
        .unwrap();
    let protected_edge = Uuid::new_v4();
    sqlx::query("INSERT INTO connections(id,source_object_id,kind,target_object_id,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,$2,'involves',$3,'A preexisting synthetic protected attribution.',true,'system','protected-link-test','system','protected-link-test')")
        .bind(protected_edge).bind(source_ids[0]).bind(entity_ids[3]).execute(&mut *seed).await.unwrap();
    seed.commit().await.unwrap();

    let token = "p".repeat(32);
    let app = agent_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        token.clone(),
    );
    let send = |operation: Value, key: String, validate_only: bool| {
        let app = app.clone();
        let token = token.clone();
        async move {
            app.oneshot(request(
                "POST",
                "/api/v2/apply",
                &token,
                json!({
                    "contract_version":"1.1.0", "idempotency_key":key,
                    "validate_only":validate_only, "operations":[operation]
                }),
            ))
            .await
            .unwrap()
        }
    };
    let connection = |source, kind: &str, target, description: &str| {
        json!({
            "operation":"create_connection", "source":{"object_id":source},
            "kind":kind, "target":{"object_id":target}, "description":description
        })
    };

    let tracked_objects = [
        source_ids[0],
        source_ids[1],
        source_ids[2],
        entity_ids[0],
        entity_ids[1],
        entity_ids[2],
        entity_ids[3],
    ];
    let revisions_before: Vec<(Uuid, i64, String, String, bool)> = sqlx::query_as(
        "SELECT id,revision,title,description,protected FROM objects WHERE id=ANY($1) ORDER BY id",
    )
    .bind(tracked_objects)
    .fetch_all(&pool)
    .await
    .unwrap();

    // Each allowed shape works with either endpoint protected, and with both protected.
    let allowed = [
        (
            source_ids[0],
            "involves",
            entity_ids[0],
            "A protected Source involves a synthetic guest.",
        ),
        (
            source_ids[1],
            "about",
            entity_ids[1],
            "A synthetic publication is about a protected speaker.",
        ),
        (
            source_ids[2],
            "involves",
            entity_ids[2],
            "A protected Source involves a protected guest.",
        ),
        (
            source_ids[0],
            "about",
            entity_ids[2],
            "A protected publication is about a protected speaker.",
        ),
        (
            entity_ids[0],
            "related_to",
            entity_ids[1],
            "A synthetic guest is related to a protected host.",
        ),
        (
            entity_ids[1],
            "related_to",
            entity_ids[2],
            "A protected host is related to a protected guest.",
        ),
    ];
    let mut first_edge = None;
    for (index, (source, kind, target, description)) in allowed.into_iter().enumerate() {
        let response = send(
            connection(source, kind, target, description),
            format!("protected-link-create-{index}-{}", Uuid::new_v4()),
            false,
        )
        .await;
        let status = response.status();
        let body = json_body(response).await;
        assert_eq!(status, StatusCode::OK, "{kind}: {body}");
        let edge = body["data"]["results"][0]["data"]["connection"].clone();
        assert_eq!(edge["protected"], false);
        if index == 0 {
            first_edge = Some(edge);
        }
    }
    let first_edge = first_edge.unwrap();
    let first_edge_id = first_edge["id"].as_str().unwrap();

    let first_edge_uuid = Uuid::parse_str(first_edge_id).unwrap();
    let event_count_before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM object_events WHERE target_type='connection' AND target_id=$1",
    )
    .bind(first_edge_uuid)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Identical replay reuses the edge. A different assertion cannot alter it.
    let replay = send(
        connection(source_ids[0], "involves", entity_ids[0], allowed[0].3),
        format!("protected-link-replay-{}", Uuid::new_v4()),
        false,
    )
    .await;
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json_body(replay).await;
    assert_eq!(replay["data"]["results"][0]["data"]["reused"], true);
    let changed = send(
        connection(
            source_ids[0],
            "involves",
            entity_ids[0],
            "A different synthetic attribution.",
        ),
        format!("protected-link-change-{}", Uuid::new_v4()),
        false,
    )
    .await;
    assert_eq!(changed.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let still_same: (i64, String) =
        sqlx::query_as("SELECT revision,description FROM connections WHERE id=$1")
            .bind(Uuid::parse_str(first_edge_id).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        still_same,
        (
            first_edge["revision"].as_i64().unwrap(),
            allowed[0].3.to_owned()
        )
    );
    let event_count_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM object_events WHERE target_type='connection' AND target_id=$1",
    )
    .bind(first_edge_uuid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event_count_after, event_count_before);
    let revisions_after: Vec<(Uuid, i64, String, String, bool)> = sqlx::query_as(
        "SELECT id,revision,title,description,protected FROM objects WHERE id=ANY($1) ORDER BY id",
    )
    .bind(tracked_objects)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(revisions_after, revisions_before);

    let protected_edge_same = send(
        connection(
            source_ids[0],
            "involves",
            entity_ids[3],
            "A preexisting synthetic protected attribution.",
        ),
        format!("protected-link-protected-replay-{}", Uuid::new_v4()),
        false,
    )
    .await;
    assert_eq!(protected_edge_same.status(), StatusCode::OK);
    let protected_edge_change = send(
        connection(
            source_ids[0],
            "involves",
            entity_ids[3],
            "A conflicting synthetic attribution.",
        ),
        format!("protected-link-protected-change-{}", Uuid::new_v4()),
        false,
    )
    .await;
    assert_eq!(
        protected_edge_change.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let protected_edge_update = send(
        json!({"operation":"update_connection","connection_id":protected_edge,"expected_revision":1,"description":"Attempted protected edge edit."}),
        format!("protected-link-protected-update-{}", Uuid::new_v4()),
        false,
    )
    .await;
    assert_eq!(
        protected_edge_update.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let protected_edge_archive = send(
        json!({"operation":"archive_connection","connection_id":protected_edge,"expected_revision":1}),
        format!("protected-link-protected-archive-{}", Uuid::new_v4()),
        false,
    )
    .await;
    assert_eq!(
        protected_edge_archive.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // Validation accepts the new shape while rolling back both the edge and its events.
    let dry_source = source_ids[2];
    let dry_target = entity_ids[3];
    let dry_events_before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM object_events WHERE target_type='object' AND target_id=ANY($1)",
    )
    .bind([dry_source, dry_target])
    .fetch_one(&pool)
    .await
    .unwrap();
    let dry_run = send(
        connection(
            dry_source,
            "about",
            dry_target,
            "A validate-only synthetic attribution.",
        ),
        format!("protected-link-dry-{}", Uuid::new_v4()),
        true,
    )
    .await;
    assert_eq!(dry_run.status(), StatusCode::OK);
    let dry_count: i64 = sqlx::query_scalar("SELECT count(*) FROM connections WHERE source_object_id=$1 AND kind='about' AND target_object_id=$2 AND archived_at IS NULL")
        .bind(dry_source).bind(dry_target).fetch_one(&pool).await.unwrap();
    assert_eq!(dry_count, 0);
    let dry_events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM object_events WHERE target_type='object' AND target_id=ANY($1)",
    )
    .bind([dry_source, dry_target])
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(dry_events, dry_events_before);

    // The creation exception does not grant endpoint edits, edge edits, or other shapes.
    let protected_update = send(json!({"operation":"update_object","object_id":source_ids[0],"expected_revision":1,"changes":{"description":"Attempted protected edit."}}), format!("protected-link-object-update-{}", Uuid::new_v4()), false).await;
    assert_eq!(protected_update.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let edge_update = send(json!({"operation":"update_connection","connection_id":first_edge_id,"expected_revision":1,"description":"Attempted edge edit."}), format!("protected-link-edge-update-{}", Uuid::new_v4()), false).await;
    assert_eq!(edge_update.status(), StatusCode::UNPROCESSABLE_ENTITY);
    for (source, kind, target) in [
        (source_ids[0], "derived_from", entity_ids[3]),
        (source_ids[0], "about", chat),
        (entity_ids[3], "related_to", source_ids[0]),
        (source_ids[0], "involves", archived_entity),
    ] {
        let rejected = send(
            connection(source, kind, target, "Unsupported protected-link boundary."),
            format!("protected-link-reject-{}", Uuid::new_v4()),
            false,
        )
        .await;
        assert_eq!(
            rejected.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{kind}"
        );
    }
}

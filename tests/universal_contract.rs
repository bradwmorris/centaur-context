use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use centaur_context::{
    api::{AppState, agent_router},
    config::TextSearchConfig,
    db,
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
        "contract_version":"1.0.0",
        "idempotency_key":key,
        "operations":[
            {"operation":"create_object","local_ref":"task","kind":"task","title":"Universal task","description":"A disposable Task created by the universal contract test.","fields":{"status":"todo","priority":"medium"}},
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
        "contract_version":"1.0.0",
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
        "contract_version":"1.0.0",
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
        "contract_version":"1.0.0",
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
        "contract_version":"1.0.0",
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

mod support;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use centaur_context::{api::AppState, config::TextSearchConfig, db, networking_mutation::router};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

static DB_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn migrated_pool() -> Option<(tokio::sync::MutexGuard<'static, ()>, PgPool)> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    assert!(url.contains("centaur_context_test"));
    let guard = DB_TEST_LOCK.lock().await;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    db::migrate(&pool).await.unwrap();
    Some((guard, pool))
}

fn request(method: &str, uri: &str, token: &str, body: Option<&Value>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .header("x-centaur-principal-id", "workflow-enyu-netz-entity")
        .header("x-centaur-thread-key", "slack:T_NETZ:C_NETZ:thread-1");
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    builder
        .body(
            body.map(|value| Body::from(serde_json::to_vec(value).unwrap()))
                .unwrap_or_else(Body::empty),
        )
        .unwrap()
}

fn create_request(uri: &str, token: &str, key: &str, body: &Value) -> Request<Body> {
    let mut request = request("POST", uri, token, Some(body));
    request
        .headers_mut()
        .insert("idempotency-key", key.parse().unwrap());
    request
}

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

#[tokio::test]
async fn networking_listener_searches_entities_and_replays_bounded_writes() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let Some((_guard, pool)) = migrated_pool().await else {
        return;
    };
    let token = "n".repeat(32);
    let app = router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        token.clone(),
        "workflow-enyu-netz-entity".into(),
    );
    let suffix = Uuid::new_v4().simple().to_string();
    let title = format!("Synthetic Network Entity {suffix}");
    let entity = json!({
        "kind":"entity",
        "title":title,
        "description":"A synthetic organization used to verify the bounded networking mutation contract.",
        "entity_kind":"organization",
        "provenance":{"source_type":"explicit_networking_request","source_ref":"slackbot-netz-test"}
    });
    let entity_key = format!("netz-{suffix}-entity");

    let created = app
        .clone()
        .oneshot(create_request(
            "/api/v2/objects",
            &token,
            &entity_key,
            &entity,
        ))
        .await
        .unwrap();
    let created_status = created.status();
    let created = response_json(created).await;
    assert_eq!(created_status, StatusCode::CREATED, "{created}");
    let entity_id = created["data"]["id"].as_str().unwrap();
    assert_eq!(created["data"]["entity_kind"], "organization");
    assert_eq!(
        created["data"]["created_by_id"],
        "workflow-enyu-netz-entity"
    );

    let replayed = app
        .clone()
        .oneshot(create_request(
            "/api/v2/objects",
            &token,
            &entity_key,
            &entity,
        ))
        .await
        .unwrap();
    assert_eq!(replayed.status(), StatusCode::CREATED);
    let replayed = response_json(replayed).await;
    assert_eq!(replayed["data"]["id"], entity_id);
    let mut changed_entity = entity.clone();
    changed_entity["description"] =
        json!("A changed replay body that must not be accepted under the original key.");
    let changed_replay = app
        .clone()
        .oneshot(create_request(
            "/api/v2/objects",
            &token,
            &entity_key,
            &changed_entity,
        ))
        .await
        .unwrap();
    assert_eq!(changed_replay.status(), StatusCode::CONFLICT);
    let entity_count: i64 = sqlx::query_scalar("SELECT count(*) FROM objects WHERE title=$1")
        .bind(&title)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(entity_count, 1);

    let read = app
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/v2/objects/{entity_id}"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(read.status(), StatusCode::OK);
    assert_eq!(
        response_json(read).await["data"]["entity_kind"],
        "organization"
    );

    let duplicate_key = format!("netz-{suffix}-ambiguous");
    let duplicate = app
        .clone()
        .oneshot(create_request(
            "/api/v2/objects",
            &token,
            &duplicate_key,
            &entity,
        ))
        .await
        .unwrap();
    assert_eq!(duplicate.status(), StatusCode::CREATED);
    let search = app
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/v2/objects?q={title}&kind=entity&limit=10&sort=recent")
                .replace(' ', "%20"),
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(search.status(), StatusCode::OK);
    let candidates = response_json(search).await;
    assert_eq!(
        candidates["data"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|record| record["title"] == title)
            .count(),
        2
    );

    let connection = json!({
        "source_object_id":entity_id,
        "target_object_id":response_json(duplicate).await["data"]["id"],
        "kind":"related_to",
        "description":"The synthetic records verify exact approved networking relationship creation.",
        "provenance":{"source_type":"exact_approved_networking_action","source_ref":"bradwmorris"},
        "protected":false
    });
    let connection_key = format!("netz-{suffix}-connection");
    let connected = app
        .clone()
        .oneshot(create_request(
            "/api/v2/connections",
            &token,
            &connection_key,
            &connection,
        ))
        .await
        .unwrap();
    assert_eq!(connected.status(), StatusCode::CREATED);
    let connection_id = response_json(connected).await["data"]["record"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let connection_replay = app
        .clone()
        .oneshot(create_request(
            "/api/v2/connections",
            &token,
            &connection_key,
            &connection,
        ))
        .await
        .unwrap();
    assert_eq!(connection_replay.status(), StatusCode::OK);
    let connection_replay = response_json(connection_replay).await;
    assert_eq!(connection_replay["data"]["record"]["id"], connection_id);
    assert_eq!(connection_replay["data"]["reused"], true);

    let owner = support::task_owner(&pool).await;
    let task = json!({
        "owner_object_id":owner,"due_at":"2099-01-01T00:00:00Z","brief_markdown":"Contact the selected person using the reviewed proposal.",
        "title":format!("Follow up with synthetic Entity {suffix}"),
        "description":"An exact approved networking follow-up created through the workflow-only listener.",
        "status":"todo",
        "priority":"high",
        "agent_suitable":false,
        "provenance":{"source_type":"exact_approved_networking_action","source_ref":"bradwmorris"},
        "derived_from_source_object_ids":[]
    });
    let task_key = format!("netz-{suffix}-task");
    let task_created = app
        .clone()
        .oneshot(create_request("/api/v2/tasks", &token, &task_key, &task))
        .await
        .unwrap();
    assert_eq!(task_created.status(), StatusCode::CREATED);
    let task_created = response_json(task_created).await;
    let task_id = task_created["data"]["object_id"].as_str().unwrap();
    let task_replay = app
        .clone()
        .oneshot(create_request("/api/v2/tasks", &token, &task_key, &task))
        .await
        .unwrap();
    assert_eq!(
        response_json(task_replay).await["data"]["object_id"],
        task_id
    );
    let mut changed_task = task.clone();
    changed_task["priority"] = json!("low");
    let changed_task_replay = app
        .clone()
        .oneshot(create_request(
            "/api/v2/tasks",
            &token,
            &task_key,
            &changed_task,
        ))
        .await
        .unwrap();
    assert_eq!(changed_task_replay.status(), StatusCode::CONFLICT);

    let task_connection = json!({
        "source_object_id":task_id,
        "target_object_id":entity_id,
        "kind":"related_to",
        "description":"Approved networking Task for this Entity.",
        "provenance":{"source_type":"exact_approved_networking_action","source_ref":"bradwmorris"},
        "protected":false
    });
    let task_connection_key = format!("netz-{suffix}-task-entity");
    let task_connected = app
        .clone()
        .oneshot(create_request(
            "/api/v2/connections",
            &token,
            &task_connection_key,
            &task_connection,
        ))
        .await
        .unwrap();
    assert_eq!(task_connected.status(), StatusCode::CREATED);
    let task_connection_replay = app
        .clone()
        .oneshot(create_request(
            "/api/v2/connections",
            &token,
            &task_connection_key,
            &task_connection,
        ))
        .await
        .unwrap();
    assert_eq!(task_connection_replay.status(), StatusCode::OK);
}

#[tokio::test]
async fn networking_listener_rejects_unknown_fields_and_broader_writes() {
    let app = router(
        AppState {
            pool: PgPoolOptions::new()
                .connect_lazy("postgres://unused:unused@127.0.0.1/unused")
                .unwrap(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        "n".repeat(32),
        "workflow-enyu-netz-entity".into(),
    );
    let token = "n".repeat(32);
    let unknown = json!({
        "kind":"entity",
        "title":"Unknown field test",
        "description":"A request that must fail before accessing persistence because it contains an extra field.",
        "entity_kind":"person",
        "provenance":{},
        "protected":true
    });
    let response = app
        .clone()
        .oneshot(create_request(
            "/api/v2/objects",
            &token,
            "unknown-field",
            &unknown,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let response = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/v2/objects?q=test&kind=entity&limit=10&sort=recent&unexpected=true",
            &token,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let broader_task = json!({
        "title":"Broader Task",
        "owner_object_id":Uuid::new_v4(),"due_at":"2099-01-01T00:00:00Z","brief_markdown":"Rejected broader task fixture.",
        "description":"A Task request whose broader state must be rejected before persistence access.",
        "status":"doing",
        "priority":"medium",
        "agent_suitable":false,
        "provenance":{},
        "derived_from_source_object_ids":[]
    });
    let response = app
        .oneshot(create_request(
            "/api/v2/tasks",
            &token,
            "broader-task",
            &broader_task,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

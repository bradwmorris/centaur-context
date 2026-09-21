mod support;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use centaur_context::{
    api::{AppState, agent_router, human_router},
    config::TextSearchConfig,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

async fn pool() -> Option<PgPool> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    assert!(url.contains("centaur_context_test"));
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    Some(pool)
}
fn request(path: &str, actor: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("authorization", format!("Bearer {}", "t".repeat(32)))
        .header("x-centaur-principal-id", actor)
        .header("x-centaur-thread-key", "task-contract-test")
        .header(
            "idempotency-key",
            body.get("idempotency_key")
                .and_then(Value::as_str)
                .unwrap_or("not-a-write"),
        )
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}
async fn body(response: axum::response::Response) -> Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}
fn create(owner: Uuid, fields: Value) -> Value {
    let mut values = json!({"status":"todo","priority":"high","owner_object_id":owner,
        "agent_suitable":true,"due_at":"2099-01-01T00:00:00Z",
        "brief_markdown":"Outcome: inspect the supplied fixture. Acceptance: evidence recorded. Next: run the check."});
    values
        .as_object_mut()
        .unwrap()
        .extend(fields.as_object().unwrap().clone());
    json!({"contract_version":"1.1.0","idempotency_key":Uuid::new_v4().to_string(),"operations":[
        {"operation":"create_object","local_ref":"task","kind":"task","title":"Synthetic executable task",
         "description":"Verify task ownership, readiness and atomic execution using synthetic data.","fields":values},
        {"operation":"create_connection","source":{"local_ref":"task"},"kind":"involves","target":{"object_id":owner},"description":"The assigned agent owns delivery of this synthetic task."}
    ]})
}
fn update(id: &str, revision: i64, changes: Value) -> Value {
    json!({"contract_version":"1.1.0","idempotency_key":Uuid::new_v4().to_string(),"operations":[{
        "operation":"update_object","object_id":id,"expected_revision":revision,"changes":changes}]})
}

#[tokio::test]
async fn task_requirements_apply_to_creation_updates_claims_and_readback() {
    let Some(pool) = pool().await else { return };
    let owner = support::task_owner(&pool).await;
    let app = agent_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        "t".repeat(32),
    );
    for fields in [
        json!({"owner_object_id":null}),
        json!({"owner_object_id":Uuid::new_v4()}),
        json!({"due_at":null}),
        json!({"work_kind":"code"}),
        json!({"github_issue_url":"https://github.com/a/b/pull/1"}),
        json!({"github_issue_url":"https://github.com.evil.test/a/b/issues/1"}),
        json!({"work_kind":"other"}),
    ] {
        let response = app
            .clone()
            .oneshot(request("/api/v2/apply", "worker-a", create(owner, fields)))
            .await
            .unwrap();
        assert!(
            response.status().is_client_error(),
            "{}",
            body(response).await
        );
    }
    let draft = create(
        owner,
        json!({"work_kind":"code","github_issue_url":"https://github.com/example/project/issues/1"}),
    );
    let response = app
        .clone()
        .oneshot(request("/api/v2/apply", "worker-a", draft.clone()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let created = body(response).await;
    let id = created["data"]["results"][0]["data"]["id"]
        .as_str()
        .unwrap();
    let first = update(id, 1, json!({"status":"doing"}));
    let (a, b) = tokio::join!(
        app.clone()
            .oneshot(request("/api/v2/apply", "worker-a", first.clone())),
        app.clone().oneshot(request(
            "/api/v2/apply",
            "worker-b",
            update(id, 1, json!({"status":"doing"}))
        ))
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert!(
        (a.status() == StatusCode::OK && b.status() == StatusCode::CONFLICT)
            || (b.status() == StatusCode::OK && a.status() == StatusCode::CONFLICT)
    );
    let winner = if a.status() == StatusCode::OK {
        "worker-a"
    } else {
        "worker-b"
    };
    let loser = if winner == "worker-a" {
        "worker-b"
    } else {
        "worker-a"
    };
    // A fresh revision cannot be used to steal the current execution.
    let stolen = app
        .clone()
        .oneshot(request(
            "/api/v2/apply",
            loser,
            update(
                id,
                2,
                json!({"status":"done","completed_at":"2099-01-01T01:00:00Z"}),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(stolen.status(), StatusCode::UNPROCESSABLE_ENTITY);
    for changes in [
        json!({"owner_object_id":null}),
        json!({"due_at":null}),
        json!({"github_issue_url":null}),
        json!({"status":"doing"}),
    ] {
        let response = app
            .clone()
            .oneshot(request("/api/v2/apply", winner, update(id, 2, changes)))
            .await
            .unwrap();
        assert!(response.status().is_client_error());
    }
    let complete = update(
        id,
        2,
        json!({"status":"done","completed_at":"2099-01-01T01:00:00Z","brief_markdown":"Verified the fixture; all assertions passed. Deliverable: synthetic report."}),
    );
    let done = app
        .clone()
        .oneshot(request("/api/v2/apply", winner, complete.clone()))
        .await
        .unwrap();
    assert_eq!(done.status(), StatusCode::OK);
    let replay = body(
        app.clone()
            .oneshot(request("/api/v2/apply", winner, complete))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(replay["data"]["replayed"], true);
    let read = body(
        app.clone()
            .oneshot(request("/api/v2/read", winner, json!({"object_ids":[id]})))
            .await
            .unwrap(),
    )
    .await;
    let object = &read["data"]["objects"][0];
    assert_eq!(object["object"]["created_by_id"], "worker-a");
    assert_eq!(object["subtype"]["owner_object_id"], owner.to_string());
    assert_eq!(object["subtype"]["status"], "done");
    assert!(object["subtype"]["completed_at"].is_string());
    assert!(
        object["subtype"]["brief_markdown"]
            .as_str()
            .unwrap()
            .contains("Verified")
    );
    assert!(object["subtype"].get("execution_actor_id").is_none());
}

#[tokio::test]
async fn queue_orders_by_priority_then_due_date_and_skips_incomplete_dependencies() {
    let Some(pool) = pool().await else { return };
    let owner = support::task_owner(&pool).await;
    let app = agent_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        "t".repeat(32),
    );
    let mut ids = Vec::new();
    for (priority, date) in [
        ("low", "2098-01-01T00:00:00Z"),
        ("high", "2099-02-01T00:00:00Z"),
        ("high", "2099-01-01T00:00:00Z"),
    ] {
        let result = body(
            app.clone()
                .oneshot(request(
                    "/api/v2/apply",
                    "queue-worker",
                    create(owner, json!({"priority":priority,"due_at":date})),
                ))
                .await
                .unwrap(),
        )
        .await;
        ids.push(
            result["data"]["results"][0]["data"]["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    let page = body(
        app.clone()
            .oneshot(request(
                "/api/v2/search",
                "queue-worker",
                json!({"query":"","limit":2,"task_filters":{"owner_object_id":owner,"ready":true}}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(page["data"]["objects"][0]["id"], ids[2]);
    assert_eq!(page["data"]["objects"][1]["id"], ids[1]);
    let next=body(app.clone().oneshot(request("/api/v2/search","queue-worker",json!({"task_filters":{"owner_object_id":owner,"ready":true,"cursor":page["data"]["next_cursor"]}}))).await.unwrap()).await;
    assert_eq!(next["data"]["objects"].as_array().unwrap().len(), 1);
    assert_eq!(next["data"]["objects"][0]["id"], ids[0]);
    let connection = json!({"contract_version":"1.1.0","idempotency_key":Uuid::new_v4().to_string(),"operations":[{"operation":"create_connection","source":{"object_id":ids[2]},"kind":"depends_on","target":{"object_id":ids[0]},"description":"The high-priority synthetic task needs the prerequisite completed first."}]});
    assert_eq!(
        app.clone()
            .oneshot(request("/api/v2/apply", "queue-worker", connection))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let filtered = body(
        app.oneshot(request(
            "/api/v2/search",
            "queue-worker",
            json!({"task_filters":{"owner_object_id":owner,"ready":true}}),
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(filtered["data"]["objects"].as_array().unwrap().len(), 2);
    assert_eq!(filtered["data"]["objects"][0]["id"], ids[1]);
}

#[tokio::test]
async fn human_creation_requires_assignment_and_agent_execution_requires_repair() {
    let Some(pool) = pool().await else { return };
    let owner = support::task_owner(&pool).await;
    let app = human_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        std::path::PathBuf::from("/tmp/no-task-assets"),
        std::path::PathBuf::from("/tmp/no-task-identities"),
    );
    let send = |value: Value| {
        Request::builder()
            .method("POST")
            .uri("/api/v2/tasks")
            .header("content-type", "application/json")
            .header("idempotency-key", Uuid::new_v4().to_string())
            .body(Body::from(value.to_string()))
            .unwrap()
    };
    let values = json!({"title":"Human follow-up","description":"A human-owned task with no promised completion date."});
    assert_eq!(
        app.clone()
            .oneshot(send(values.clone()))
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let mut assigned = values;
    assigned["owner_object_id"] = json!(owner);
    let created = app.oneshot(send(assigned)).await.unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let id = body(created).await["data"]["object_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let agent = agent_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        "t".repeat(32),
    );
    assert_eq!(
        agent
            .clone()
            .oneshot(request(
                "/api/v2/apply",
                "worker",
                update(&id, 1, json!({"status":"doing"}))
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(agent.oneshot(request("/api/v2/apply","worker",update(&id,1,json!({"status":"doing","due_at":"2099-01-01T00:00:00Z","brief_markdown":"Execute the reviewed synthetic fixture and verify its output."})))).await.unwrap().status(),StatusCode::OK);
}

#[tokio::test]
async fn legacy_tasks_remain_readable_but_cannot_be_claimed_without_repair() {
    let Some(pool) = pool().await else { return };
    let owner = support::task_owner(&pool).await;
    let id = Uuid::new_v4();
    // Reproduce a pre-migration record in a transaction; no bypass survives setup.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("ALTER TABLE tasks DISABLE TRIGGER tasks_actionable")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,'task','Legacy task','A pre-migration task with no assignment or deadline.','system','legacy-test','system','legacy-test')").bind(id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO tasks(object_id,status,priority,agent_suitable) VALUES($1,'todo','medium',true)").bind(id).execute(&mut *tx).await.unwrap();
    sqlx::query("ALTER TABLE tasks ENABLE TRIGGER tasks_actionable")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let app = agent_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        "t".repeat(32),
    );
    let read = body(
        app.clone()
            .oneshot(request(
                "/api/v2/read",
                "worker",
                json!({"object_ids":[id]}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        read["data"]["objects"][0]["object"]["created_by_id"],
        "legacy-test"
    );
    let id = id.to_string();
    assert_eq!(
        app.clone()
            .oneshot(request(
                "/api/v2/apply",
                "worker",
                update(
                    &id,
                    1,
                    json!({"description":"This legacy task needs assignment and deadline triage."})
                )
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        app.clone()
            .oneshot(request(
                "/api/v2/apply",
                "worker",
                update(&id, 2, json!({"status":"doing"}))
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // A Task Object is not a User and cannot serve as its own assignee.
    assert_eq!(
        app.clone()
            .oneshot(request(
                "/api/v2/apply",
                "worker",
                update(&id, 2, json!({"owner_object_id":id}))
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(app.clone().oneshot(request("/api/v2/apply","worker",update(&id,2,json!({"owner_object_id":owner,"due_at":"2099-01-01T00:00:00Z","brief_markdown":"Verify the synthetic record and record evidence.","status":"doing"})))).await.unwrap().status(),StatusCode::OK);
    let read = body(
        app.clone()
            .oneshot(request(
                "/api/v2/read",
                "worker",
                json!({"object_ids":[id]}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        read["data"]["objects"][0]["object"]["created_by_id"],
        "legacy-test"
    );
    // Inactive assignees cannot receive newly created tasks.
    sqlx::query("UPDATE objects SET archived_at=now() WHERE id=$1")
        .bind(owner)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        app.oneshot(request("/api/v2/apply", "worker", create(owner, json!({}))))
            .await
            .unwrap()
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

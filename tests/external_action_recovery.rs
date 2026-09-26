use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use centaur_context::{api::AppState, config::TextSearchConfig, db, external_actions::router};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

async fn call(app: Router, principal: &str, path: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", "t".repeat(32)))
                .header("x-centaur-principal-id", principal)
                .header("x-centaur-thread-key", "workflow:synthetic-retry")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    (status, body)
}

#[tokio::test]
async fn reservation_recovery_is_exact_principal_bound_and_concurrency_safe() {
    let Ok(url) = std::env::var("TEST_DATABASE_URL") else {
        return;
    };
    assert!(url.contains("centaur_context_test"));
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .unwrap();
    db::migrate(&pool).await.unwrap();
    let app = router(
        AppState {
            pool,
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        "t".repeat(32),
        ["workflow-a".to_owned(), "workflow-b".to_owned()].into(),
    );
    let key = uuid::Uuid::new_v4().to_string();
    let request = json!({"version":"centaur-context-external-action-v2", "idempotency_key":format!("first:{key}"),
        "provider":"synthetic", "action_kind":"test", "external_key":key,
        "title":"Test reservation", "summary":"Synthetic retry fixture", "metadata":{"body_sha256":"hash-a"}});
    let path = "/api/v2/external-actions/reserve";
    let (status, first) = call(app.clone(), "workflow-a", path, request.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let id = first["data"]["id"].as_str().unwrap();
    let event = json!({"version":"centaur-context-external-action-v2", "idempotency_key":format!("preview:{key}"),
        "event_type":"previewed", "expected_state":"reserved", "metadata":{}});
    assert_eq!(
        call(
            app.clone(),
            "workflow-a",
            &format!("/api/v2/external-actions/{id}/events"),
            event
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut retry = request.clone();
    retry["idempotency_key"] = json!(format!("retry:{key}"));
    let (status, recovered) = call(app.clone(), "workflow-a", path, retry.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(recovered["data"]["id"], first["data"]["id"]);
    assert_eq!(recovered["data"]["state"], "previewed");
    assert_eq!(recovered["idempotent"], true);
    assert_eq!(
        call(app.clone(), "workflow-b", path, retry.clone()).await.0,
        StatusCode::CONFLICT
    );
    retry["metadata"]["body_sha256"] = json!("changed");
    assert_eq!(
        call(app.clone(), "workflow-a", path, retry).await.0,
        StatusCode::CONFLICT
    );
    let mut changed = request.clone();
    changed["metadata"]["body_sha256"] = json!("changed");
    assert_eq!(
        call(app.clone(), "workflow-a", path, changed).await.0,
        StatusCode::CONFLICT
    );

    let mut a = request;
    a["external_key"] = json!(format!("concurrent:{key}"));
    a["idempotency_key"] = json!(format!("concurrent-a:{key}"));
    let mut b = a.clone();
    b["idempotency_key"] = json!(format!("concurrent-b:{key}"));
    let (a, b) = tokio::join!(
        call(app.clone(), "workflow-a", path, a),
        call(app, "workflow-a", path, b)
    );
    assert_eq!(a.0, StatusCode::OK);
    assert_eq!(b.0, StatusCode::OK);
    assert_eq!(a.1["data"]["id"], b.1["data"]["id"]);
}

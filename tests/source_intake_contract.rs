use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use centaur_context::{api::AppState, config::TextSearchConfig, db, source_intake::router};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;

async fn test_pool() -> Option<PgPool> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    assert!(url.contains("centaur_context_test") || url.contains("centaur_os_test"));
    Some(
        PgPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .unwrap(),
    )
}

fn request(path: &str, token: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .header("x-centaur-principal-id", "workflow-enyu-source-ingestion")
        .header("x-centaur-thread-key", "slack:TTEST:CTEST:thread-1")
        .header("x-centaur-execution-id", "source-intake-contract")
        .body(Body::from(serde_json::to_vec(body).unwrap()))
        .unwrap()
}

#[tokio::test]
async fn validates_commits_replays_conflicts_and_reports_retrieval_readiness() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping Source-intake contract: TEST_DATABASE_URL is not set");
        return;
    };
    db::migrate(&pool).await.unwrap();
    let token = "s".repeat(32);
    let key = format!("enyu-source-contract-{}", uuid::Uuid::new_v4());
    let payload = json!({
        "version":"centaur-context-source-intake-v1",
        "idempotency_key":key,
        "source":{
            "title":"A durable Enyu research source",
            "description":"A verified Source used to exercise the permanent Enyu ingestion contract.",
            "source_kind":"article",
            "canonical_uri":format!("https://example.test/enyu-source/{key}"),
            "byline":"Example Author",
            "publisher":"Example Publisher",
            "published_at":null,
            "accessed_at":"2026-08-30T00:00:00Z",
            "language":"en",
            "media_type":"text/plain",
            "artifact_reference":null,
            "content_kind":"article_text",
            "content":format!("The permanent Enyu workflow stores this distinctive retrievable evidence: {key}."),
            "extraction_method":"enyu-researcher",
            "extraction_version":"1",
            "provenance":{"source_type":"enyu_workflow","source_ref":"contract-test"}
        },
        "connections":[],
        "originating_chat_object_id":null
    });
    let state = AppState {
        pool: pool.clone(),
        embeddings: None,
        text_search_config: TextSearchConfig::SIMPLE,
    };
    let app = router(state, token.clone());

    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM objects")
        .fetch_one(&pool)
        .await
        .unwrap();
    let validated = app
        .clone()
        .oneshot(request("/api/v1/source-intake/validate", &token, &payload))
        .await
        .unwrap();
    assert_eq!(validated.status(), StatusCode::OK);
    let validated_body: Value =
        serde_json::from_slice(&validated.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(validated_body["data"]["writes"], 0);
    let after_validate: i64 = sqlx::query_scalar("SELECT count(*) FROM objects")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(before, after_validate);

    let committed = app
        .clone()
        .oneshot(request("/api/v1/source-intake/commit", &token, &payload))
        .await
        .unwrap();
    assert_eq!(committed.status(), StatusCode::CREATED);
    let committed_body: Value =
        serde_json::from_slice(&committed.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let object_id = committed_body["data"]["object_id"].as_str().unwrap();

    let replayed = app
        .clone()
        .oneshot(request("/api/v1/source-intake/commit", &token, &payload))
        .await
        .unwrap();
    assert_eq!(replayed.status(), StatusCode::OK);

    let status = app
        .clone()
        .oneshot(request("/api/v1/source-intake/status", &token, &payload))
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let status_body: Value =
        serde_json::from_slice(&status.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(status_body["data"]["ready"], true);
    assert_eq!(status_body["data"]["lexical_ready"], true);

    let stored: (String, String, bool) = sqlx::query_as(
        r#"SELECT o.title,c.normalized_text,o.protected
           FROM objects o JOIN source_contents c ON c.source_object_id=o.id
           WHERE o.id=$1::uuid"#,
    )
    .bind(object_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored.0, "A durable Enyu research source");
    assert!(stored.1.contains("distinctive retrievable evidence"));
    assert!(stored.2);

    let mut conflicting = payload.clone();
    conflicting["source"]["title"] = json!("A changed title");
    let conflict = app
        .clone()
        .oneshot(request(
            "/api/v1/source-intake/commit",
            &token,
            &conflicting,
        ))
        .await
        .unwrap();
    assert_eq!(conflict.status(), StatusCode::CONFLICT);

    let mut duplicate_identity = payload.clone();
    duplicate_identity["idempotency_key"] = json!(format!("{key}-duplicate"));
    let duplicate = app
        .oneshot(request(
            "/api/v1/source-intake/validate",
            &token,
            &duplicate_identity,
        ))
        .await
        .unwrap();
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
}

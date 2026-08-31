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
        "version":"centaur-context-source-intake-v2",
        "idempotency_key":key,
        "source":{
            "title":"A durable Enyu research source",
            "description":"A verified Source used to exercise the permanent Enyu ingestion contract.",
            "source_kind":"article",
            "canonical_uri":format!("https://example.test/enyu-source/{key}"),
            "byline":"Example Author",
            "publisher":"Example Publisher",
            "published_at":null,
            "published_at_precision":null,
            "last_accessed_at":"2026-08-30T00:00:00Z",
            "original_language":"en",
            "original_media_type":"text/plain",
            "original_artifact_reference":null,
            "capture_artifact_reference":null,
            "content_kind":"article_text",
            "content":format!("The permanent Enyu workflow stores this distinctive retrievable evidence: {key}."),
            "extraction_method":"enyu-researcher",
            "extraction_version":"1",
            "coverage":"complete",
            "captured_at":"2026-08-30T00:00:00Z",
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

    let validated = app
        .clone()
        .oneshot(request("/api/v2/source-intake/validate", &token, &payload))
        .await
        .unwrap();
    assert_eq!(validated.status(), StatusCode::OK);
    let validated_body: Value =
        serde_json::from_slice(&validated.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(validated_body["data"]["writes"], 0);
    let object_id = validated_body["data"]["object_id"].as_str().unwrap();
    let after_validate: i64 = sqlx::query_scalar("SELECT count(*) FROM objects WHERE id=$1::uuid")
        .bind(object_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(after_validate, 0);

    let committed = app
        .clone()
        .oneshot(request("/api/v2/source-intake/commit", &token, &payload))
        .await
        .unwrap();
    assert_eq!(committed.status(), StatusCode::CREATED);
    let committed_body: Value =
        serde_json::from_slice(&committed.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(committed_body["data"]["object_id"], object_id);

    let replayed = app
        .clone()
        .oneshot(request("/api/v2/source-intake/commit", &token, &payload))
        .await
        .unwrap();
    assert_eq!(replayed.status(), StatusCode::OK);

    let status = app
        .clone()
        .oneshot(request("/api/v2/source-intake/status", &token, &payload))
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let status_body: Value =
        serde_json::from_slice(&status.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(status_body["data"]["ready"], true);
    assert_eq!(status_body["data"]["lexical_ready"], true);

    let stored: (String, String, bool) = sqlx::query_as(
        r#"SELECT o.title,c.content,o.protected
           FROM objects o JOIN artifacts c ON c.object_id=o.id
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
            "/api/v2/source-intake/commit",
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
            "/api/v2/source-intake/validate",
            &token,
            &duplicate_identity,
        ))
        .await
        .unwrap();
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn adopts_only_an_incomplete_unprotected_curator_source() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping Source-intake contract: TEST_DATABASE_URL is not set");
        return;
    };
    db::migrate(&pool).await.unwrap();
    let token = "s".repeat(32);
    let key = format!("enyu-source-adoption-{}", uuid::Uuid::new_v4());
    let canonical_uri = format!("https://example.test/enyu-source/{key}");
    let placeholder_id = uuid::Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query(
        r#"INSERT INTO objects
           (id,kind,title,description,protected,created_by_type,created_by_id,
            updated_by_type,updated_by_id,provenance)
           VALUES ($1,'source','Placeholder','Created from the ingestion request.',false,
                   'centaur_agent','context_curator','centaur_agent','context_curator',
                   '{"source_type":"context_curator","source_ref":"test-run"}'::jsonb)"#,
    )
    .bind(placeholder_id)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO sources (object_id,source_kind,canonical_uri) VALUES ($1,'web_page',$2)",
    )
    .bind(placeholder_id)
    .bind(&canonical_uri)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let payload = json!({
        "version":"centaur-context-source-intake-v2",
        "idempotency_key":key,
        "source":{
            "title":"A captured Enyu research source",
            "description":"A complete source captured by the Enyu ingestion workflow.",
            "source_kind":"article",
            "canonical_uri":canonical_uri,
            "byline":"Example Author",
            "publisher":"Example Publisher",
            "published_at":null,
            "published_at_precision":null,
            "last_accessed_at":"2026-08-30T00:00:00Z",
            "original_language":"en",
            "original_media_type":"text/plain",
            "original_artifact_reference":null,
            "capture_artifact_reference":null,
            "content_kind":"article_text",
            "content":format!("The adopted Source contains complete workflow evidence: {key}."),
            "extraction_method":"enyu-researcher",
            "extraction_version":"1",
            "coverage":"complete",
            "captured_at":"2026-08-30T00:00:00Z",
            "provenance":{"source_type":"enyu_workflow","source_ref":"adoption-test"}
        },
        "connections":[],
        "originating_chat_object_id":null
    });
    let app = router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        token.clone(),
    );

    let validated = app
        .clone()
        .oneshot(request("/api/v2/source-intake/validate", &token, &payload))
        .await
        .unwrap();
    assert_eq!(validated.status(), StatusCode::OK);
    let validated_body: Value =
        serde_json::from_slice(&validated.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        validated_body["data"]["object_id"],
        placeholder_id.to_string()
    );

    let committed = app
        .clone()
        .oneshot(request("/api/v2/source-intake/commit", &token, &payload))
        .await
        .unwrap();
    assert_eq!(committed.status(), StatusCode::CREATED);
    let replayed = app
        .oneshot(request("/api/v2/source-intake/commit", &token, &payload))
        .await
        .unwrap();
    assert_eq!(replayed.status(), StatusCode::OK);

    let stored: (String, bool, i64, String, String) = sqlx::query_as(
        r#"SELECT o.title,o.protected,count(c.id),o.provenance->>'source_type',
                  o.provenance->'adopted_curator_provenance'->>'source_type'
           FROM objects o JOIN sources s ON s.object_id=o.id
           JOIN artifacts c ON c.object_id=o.id
           WHERE o.id=$1 GROUP BY o.id"#,
    )
    .bind(placeholder_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored.0, "A captured Enyu research source");
    assert!(stored.1);
    assert_eq!(stored.2, 1);
    assert_eq!(stored.3, "enyu_workflow");
    assert_eq!(stored.4, "context_curator");
}

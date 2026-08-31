use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use centaur_context::{api::AppState, config::TextSearchConfig, db, source_intake::router};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
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
    let content =
        format!("The permanent Enyu workflow stores this distinctive retrievable evidence: {key}.");
    let payload = json!({
        "version":"centaur-context-source-intake-v3",
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
            "content":content,
            "content_sha256":format!("{:x}",Sha256::digest(content.as_bytes())),
            "content_size_bytes":content.len(),
            "extraction_method":"enyu-researcher",
            "extraction_version":"1",
            "capture_outcome":"complete",
            "capture_reason":null,
            "expected_size_bytes":content.len(),
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
async fn reuses_exact_source_for_new_connection_batches_without_duplicates() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping Source-intake contract: TEST_DATABASE_URL is not set");
        return;
    };
    db::migrate(&pool).await.unwrap();
    let token = "s".repeat(32);
    let key = format!("enyu-source-reuse-{}", uuid::Uuid::new_v4());
    let canonical_uri = format!("https://example.test/enyu-source/{key}");
    let content = format!("Exact captured evidence for Source reuse: {key}.");
    let target_id = uuid::Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO objects
           (id,kind,title,description,protected,created_by_type,created_by_id,
            updated_by_type,updated_by_id,provenance)
           VALUES ($1,'entity','Existing identity','Canonical test identity.',true,
                   'human','contract-test','human','contract-test','{}'::jsonb)"#,
    )
    .bind(target_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO entities (object_id,entity_kind) VALUES ($1,'person')")
        .bind(target_id)
        .execute(&pool)
        .await
        .unwrap();

    let payload = json!({
        "version":"centaur-context-source-intake-v3",
        "idempotency_key":key,
        "source":{
            "title":"A reusable captured source",
            "description":"A complete Source used to verify exact-content enrichment.",
            "source_kind":"article",
            "canonical_uri":canonical_uri,
            "byline":"Example Author",
            "publisher":"Example Publisher",
            "published_at":null,
            "published_at_precision":null,
            "last_accessed_at":"2026-08-31T00:00:00Z",
            "original_language":"en",
            "original_media_type":"text/plain",
            "original_artifact_reference":null,
            "capture_artifact_reference":null,
            "content_kind":"article_text",
            "content":content,
            "content_sha256":format!("{:x}",Sha256::digest(content.as_bytes())),
            "content_size_bytes":content.len(),
            "extraction_method":"enyu-researcher",
            "extraction_version":"1",
            "capture_outcome":"complete",
            "capture_reason":null,
            "expected_size_bytes":content.len(),
            "captured_at":"2026-08-31T00:00:00Z",
            "provenance":{"source_type":"enyu_workflow","source_ref":"reuse-test"}
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

    let committed = app
        .clone()
        .oneshot(request("/api/v2/source-intake/commit", &token, &payload))
        .await
        .unwrap();
    assert_eq!(committed.status(), StatusCode::CREATED);
    let committed_body: Value =
        serde_json::from_slice(&committed.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let source_id = committed_body["data"]["object_id"].as_str().unwrap();

    let mut enriched = payload.clone();
    enriched["idempotency_key"] = json!(format!("{key}-enriched"));
    enriched["connections"] = json!([{
        "target_object_id":target_id,
        "kind":"references",
        "description":"The captured source directly names this canonical identity.",
        "provenance":{"source_type":"enyu_workflow","source_ref":"reuse-test"}
    }]);
    let enriched_response = app
        .clone()
        .oneshot(request("/api/v2/source-intake/commit", &token, &enriched))
        .await
        .unwrap();
    assert_eq!(enriched_response.status(), StatusCode::CREATED);
    let enriched_body: Value = serde_json::from_slice(
        &enriched_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(enriched_body["data"]["object_id"], source_id);
    assert_eq!(enriched_body["data"]["counts"]["objects"], 0);
    assert_eq!(enriched_body["data"]["counts"]["artifacts"], 0);
    assert_eq!(enriched_body["data"]["counts"]["connections"], 1);

    let mut repeated_enrichment = enriched.clone();
    repeated_enrichment["idempotency_key"] = json!(format!("{key}-enriched-again"));
    let repeated = app
        .clone()
        .oneshot(request(
            "/api/v2/source-intake/commit",
            &token,
            &repeated_enrichment,
        ))
        .await
        .unwrap();
    assert_eq!(repeated.status(), StatusCode::CREATED);
    let repeated_body: Value =
        serde_json::from_slice(&repeated.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(repeated_body["data"]["object_id"], source_id);
    assert_eq!(repeated_body["data"]["counts"]["connections"], 0);

    let stored_counts: (i64, i64, i64) = sqlx::query_as(
        r#"SELECT
             (SELECT count(*) FROM sources WHERE canonical_uri=$1),
             (SELECT count(*) FROM artifacts WHERE object_id=$2::uuid),
             (SELECT count(*) FROM connections
              WHERE source_object_id=$2::uuid AND kind='references'
                AND target_object_id=$3 AND archived_at IS NULL)"#,
    )
    .bind(&canonical_uri)
    .bind(source_id)
    .bind(target_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored_counts, (1, 1, 1));

    let mut content_only = payload.clone();
    content_only["idempotency_key"] = json!(format!("{key}-content-only"));
    content_only["source"]["canonical_uri"] =
        json!(format!("https://example.test/enyu-source/{key}-different"));
    let content_conflict = app
        .clone()
        .oneshot(request(
            "/api/v2/source-intake/validate",
            &token,
            &content_only,
        ))
        .await
        .unwrap();
    assert_eq!(content_conflict.status(), StatusCode::CONFLICT);

    let original_artifact_id: uuid::Uuid =
        sqlx::query_scalar("SELECT current_artifact_id FROM sources WHERE object_id=$1::uuid")
            .bind(source_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let mut recaptured = payload.clone();
    recaptured["idempotency_key"] = json!(format!("{key}-recaptured"));
    let changed_content = format!("Different complete capture for the same URI: {key}.");
    let changed_sha256 = format!("{:x}", Sha256::digest(changed_content.as_bytes()));
    recaptured["source"]["content"] = json!(changed_content);
    recaptured["source"]["content_sha256"] = json!(changed_sha256);
    recaptured["source"]["content_size_bytes"] = json!(changed_content.len());
    recaptured["source"]["expected_size_bytes"] = json!(changed_content.len());
    let recaptured_response = app
        .clone()
        .oneshot(request("/api/v2/source-intake/commit", &token, &recaptured))
        .await
        .unwrap();
    assert_eq!(recaptured_response.status(), StatusCode::CREATED);
    let recaptured_body: Value = serde_json::from_slice(
        &recaptured_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(recaptured_body["data"]["object_id"], source_id);
    assert_eq!(recaptured_body["data"]["counts"]["objects"], 0);
    assert_eq!(recaptured_body["data"]["counts"]["artifacts"], 1);

    let recaptured_status = app
        .clone()
        .oneshot(request("/api/v2/source-intake/status", &token, &recaptured))
        .await
        .unwrap();
    assert_eq!(recaptured_status.status(), StatusCode::OK);
    let artifact_state: (i64, String, Option<uuid::Uuid>) = sqlx::query_as(
        r#"SELECT count(a.id),current.sha256,current.supersedes_artifact_id
           FROM sources s JOIN artifacts current ON current.id=s.current_artifact_id
           JOIN artifacts a ON a.object_id=s.object_id
           WHERE s.object_id=$1::uuid
           GROUP BY current.id"#,
    )
    .bind(source_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(artifact_state.0, 2);
    assert_eq!(artifact_state.1, changed_sha256);
    assert_eq!(artifact_state.2, Some(original_artifact_id));
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
    let content = format!("The adopted Source contains complete workflow evidence: {key}.");
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
        "version":"centaur-context-source-intake-v3",
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
            "content":content,
            "content_sha256":format!("{:x}",Sha256::digest(content.as_bytes())),
            "content_size_bytes":content.len(),
            "extraction_method":"enyu-researcher",
            "extraction_version":"1",
            "capture_outcome":"complete",
            "capture_reason":null,
            "expected_size_bytes":content.len(),
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

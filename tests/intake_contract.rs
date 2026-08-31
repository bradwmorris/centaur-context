use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use centaur_context::{api::AppState, config::TextSearchConfig, db, intake::router};
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

fn request(path: &str, token: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .header("x-centaur-principal-id", "intake-contract-test")
        .header("x-centaur-thread-key", "test:intake-contract")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

#[tokio::test]
async fn validates_commits_and_replays_one_atomic_source_batch() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping intake contract: TEST_DATABASE_URL is not set");
        return;
    };
    db::migrate(&pool).await.unwrap();
    let token = "i".repeat(32);
    let fixture_id = uuid::Uuid::new_v4();
    let batch_id = format!("intake-contract-{fixture_id}");
    let normalized_text = "  A high-quality source body.\n";
    let content_hash = format!("{:x}", Sha256::digest(normalized_text.as_bytes()));
    let manifest_sha256 = "a".repeat(64);
    let payload = json!({
        "batch_id":batch_id,
        "manifest_sha256":manifest_sha256,
        "objects":[
            {"client_key":"owner","kind":"user","title":"Test Owner","description":"Human owner of the synthetic imported research corpus.","protected":true,"provenance":{"source_type":"test","source_ref":"owner"},"user_kind":"human","identities":[{"id":null,"provider":"slack","workspace_id":"TTEST","provider_user_id":format!("U{fixture_id}"),"display_name":"Test Owner"}]},
            {"client_key":"source-1","kind":"source","title":"A durable research source","description":"A verified source used to exercise the bounded intake contract.","protected":true,"provenance":{"source_type":"test","source_ref":"source-1"},"source":{"source_kind":"paper","canonical_uri":format!("https://example.test/paper/{fixture_id}"),"byline":null,"publisher":null,"published_at":null,"published_at_precision":null,"last_accessed_at":null,"original_language":"en","original_media_type":"text/plain","original_artifact_reference":null}},
            {"client_key":"note-1","kind":"note","title":"A grounded research note","description":"A grounded note derived from the verified test source.","protected":true,"provenance":{"source_type":"test","source_ref":"note-1"},"note":{"content":"A durable, source-grounded observation.","content_format":"markdown"}},
            {"client_key":"theme-1","kind":"theme","title":"Durable Research","description":"A research vertical for durable, source-grounded evidence and related work.","protected":true,"provenance":{"source_type":"test","source_ref":"theme-1"},"theme":{"slug":format!("durable-research-{fixture_id}")}}
        ],
        "artifacts":[{"client_key":"source-1-v1","object":{"client_key":"source-1"},"kind":"paper_text","title":"Captured paper text","content":normalized_text,"uri":null,"media_type":"text/plain","sha256":content_hash,"language":"en","captured_at":"2026-08-30T00:00:00Z","metadata":{"extraction_method":"test","extraction_version":"1","coverage":"complete"},"supersedes_artifact_id":null}],
        "connections":[
            {"client_key":"note-source","source":{"client_key":"note-1"},"kind":"derived_from","target":{"client_key":"source-1"},"description":"The note is directly derived from this verified source.","protected":true,"provenance":{"source_type":"test","source_ref":"edge-1"}},
            {"client_key":"source-theme","source":{"client_key":"source-1"},"kind":"themed","target":{"client_key":"theme-1"},"description":"The verified source is directly relevant to this approved research Theme.","protected":true,"provenance":{"source_type":"test","source_ref":"edge-2"}}
        ]
    });
    let state = AppState {
        pool: pool.clone(),
        embeddings: None,
        text_search_config: TextSearchConfig::SIMPLE,
    };
    let app = router(state, token.clone(), Some(manifest_sha256));

    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM objects")
        .fetch_one(&pool)
        .await
        .unwrap();
    let validated = app
        .clone()
        .oneshot(request(
            "/api/v2/intake/batches/validate",
            &token,
            payload.clone(),
        ))
        .await
        .unwrap();
    assert_eq!(validated.status(), StatusCode::OK);
    let after_validate: i64 = sqlx::query_scalar("SELECT count(*) FROM objects")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(before, after_validate);

    let committed = app
        .clone()
        .oneshot(request(
            "/api/v2/intake/batches/commit",
            &token,
            payload.clone(),
        ))
        .await
        .unwrap();
    assert_eq!(committed.status(), StatusCode::CREATED);
    let body: Value =
        serde_json::from_slice(&committed.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body["data"]["counts"]["objects"], 4);
    assert_eq!(body["data"]["counts"]["events"], 7);
    let stored_text: String = sqlx::query_scalar("SELECT content FROM artifacts WHERE sha256=$1")
        .bind(&content_hash)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored_text, normalized_text);

    let replayed = app
        .oneshot(request("/api/v2/intake/batches/commit", &token, payload))
        .await
        .unwrap();
    assert_eq!(replayed.status(), StatusCode::OK);
    let after_replay: i64 = sqlx::query_scalar("SELECT count(*) FROM objects")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(after_replay, before + 4);
}

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use centaur_os::{
    api::{AppState, agent_router, human_router},
    curator::router as curator_router,
    ingest::{ApprovedSlackSurfaces, router as ingest_router},
};
use http_body_util::BodyExt;
use sqlx::postgres::PgPoolOptions;
use std::path::PathBuf;
use tower::ServiceExt;

fn state() -> AppState {
    AppState {
        pool: PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1/unused")
            .unwrap(),
        embeddings: None,
        text_search_config: centaur_os::config::TextSearchConfig::SIMPLE,
    }
}

#[tokio::test]
async fn curator_listener_uses_its_own_credential_and_is_not_an_agent_surface() {
    let router = curator_router(state(), "c".repeat(32));
    let agent_credential = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(agent_credential.status(), StatusCode::UNAUTHORIZED);

    let curator_credential = router
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("authorization", format!("Bearer {}", "c".repeat(32)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(curator_credential.status(), StatusCode::OK);
}

#[tokio::test]
async fn human_api_declares_v1_and_unknown_versions_fail_closed() {
    let router = human_router(state(), PathBuf::from("web/dist"));
    let meta = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/meta")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(meta.status(), StatusCode::OK);
    let body = meta.into_body().collect().await.unwrap().to_bytes();
    let metadata: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let metadata = &metadata["data"];
    assert_eq!(metadata["product"], "centaur-os");
    assert_eq!(metadata["product_version"], "0.1.0");
    assert_eq!(metadata["api_version"], "v1");
    assert_eq!(metadata["ontology_version"], "v1");
    assert_eq!(metadata["database_schema_version"], 8);
    assert_eq!(metadata["tool_version"], "0.1.0");
    assert_eq!(metadata["compatibility_policy"], "fail_closed");
    let unsupported = router
        .oneshot(
            Request::builder()
                .uri("/api/v2/objects")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unsupported.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn human_api_rejects_a_weak_object_description_before_database_access() {
    let response = human_router(state(), PathBuf::from("web/dist"))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/objects")
                .header("content-type", "application/json")
                .header("idempotency-key", "weak-object-description")
                .body(Body::from(
                    r#"{
                        "kind":"entity",
                        "title":"Northwind",
                        "description":"Northwind"
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let error: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"]["code"], "validation_error");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("add concrete context")
    );
}

#[tokio::test]
async fn agent_listener_rejects_missing_token() {
    let response = agent_router(state(), "a".repeat(32))
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn agent_listener_requires_principal_and_thread_context() {
    let response = agent_router(state(), "a".repeat(32))
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn agent_listener_accepts_scoped_request() {
    let response = agent_router(state(), "a".repeat(32))
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .header("x-centaur-principal-id", "prn_test")
                .header("x-centaur-thread-key", "cli:test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn agent_context_requires_a_canonical_chat_before_database_access() {
    let response = agent_router(state(), "a".repeat(32))
        .oneshot(
            Request::builder()
                .uri("/api/v1/context?q=shared")
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .header("x-centaur-principal-id", "prn_test")
                .header("x-centaur-thread-key", "slack:T1:C1:thread-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let error: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(error["error"]["message"], "chat_object_id is required");
}

#[tokio::test]
async fn agent_listener_does_not_expose_write_routes() {
    let response = agent_router(state(), "a".repeat(32))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/objects")
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .header("x-centaur-principal-id", "prn_test")
                .header("x-centaur-thread-key", "slack:test")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn ingestion_listener_uses_a_separate_bearer_credential() {
    let approved = ApprovedSlackSurfaces::parse("T1:C1").unwrap();
    let router = ingest_router(state(), "i".repeat(32), approved);
    let missing = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);

    let agent_credential = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(agent_credential.status(), StatusCode::UNAUTHORIZED);

    let ingestion_credential = router
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("authorization", format!("Bearer {}", "i".repeat(32)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ingestion_credential.status(), StatusCode::OK);
}

#[tokio::test]
async fn ingestion_listener_rejects_unapproved_slack_surfaces_before_database_access() {
    let approved = ApprovedSlackSurfaces::parse("T1:C1").unwrap();
    let response = ingest_router(state(), "i".repeat(32), approved)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/ingest/slack/interactions")
                .header("authorization", format!("Bearer {}", "i".repeat(32)))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{
                        "workspace_id":"T1",
                        "channel_id":"C_DENIED",
                        "thread_id":"1780000000.000100",
                        "surface_kind":"channel",
                        "messages":[{
                            "provider_message_id":"1780000000.000100",
                            "sender":{"provider_user_id":"U1","display_name":"Example User","user_kind":"human"},
                            "content":"This surface is not approved.",
                            "source_created_at":"2026-08-28T00:00:00Z"
                        }]
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

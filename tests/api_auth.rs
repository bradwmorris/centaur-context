use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use centaur_context::{
    api::{AppState, agent_router, human_router, note_write_router, theme_proposal_router},
    curator::router as curator_router,
    ingest::{ApprovedSlackSurfaces, router as ingest_router},
    intake::router as intake_router,
    source_intake::router as source_intake_router,
};
use http_body_util::BodyExt;
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPoolOptions;
use std::path::PathBuf;
use tower::ServiceExt;

fn state() -> AppState {
    AppState {
        pool: PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1/unused")
            .unwrap(),
        embeddings: None,
        text_search_config: centaur_context::config::TextSearchConfig::SIMPLE,
    }
}

#[tokio::test]
async fn theme_proposal_listener_uses_a_distinct_agent_credential() {
    let router = theme_proposal_router(state(), "p".repeat(32));
    let wrong = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .header("x-centaur-principal-id", "agent-researcher")
                .header("x-centaur-thread-key", "codex:issue:39:test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
    let correct = router
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("authorization", format!("Bearer {}", "p".repeat(32)))
                .header("x-centaur-principal-id", "agent-researcher")
                .header("x-centaur-thread-key", "codex:issue:39:test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(correct.status(), StatusCode::OK);
}

#[tokio::test]
async fn source_intake_listener_requires_its_token_and_exact_workflow_principal() {
    let router = source_intake_router(state(), "s".repeat(32));
    let wrong_token = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .header("x-centaur-principal-id", "workflow-enyu-source-ingestion")
                .header("x-centaur-thread-key", "slack:test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong_token.status(), StatusCode::UNAUTHORIZED);

    let wrong_principal = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("authorization", format!("Bearer {}", "s".repeat(32)))
                .header("x-centaur-principal-id", "agent-enyu-editor")
                .header("x-centaur-thread-key", "slack:test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong_principal.status(), StatusCode::FORBIDDEN);

    let researcher_agent = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("authorization", format!("Bearer {}", "s".repeat(32)))
                .header("x-centaur-principal-id", "agent-enyu-researcher")
                .header("x-centaur-thread-key", "slack:test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(researcher_agent.status(), StatusCode::FORBIDDEN);

    let workflow = router
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("authorization", format!("Bearer {}", "s".repeat(32)))
                .header("x-centaur-principal-id", "workflow-enyu-source-ingestion")
                .header("x-centaur-thread-key", "slack:test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(workflow.status(), StatusCode::OK);
}

#[tokio::test]
async fn intake_listener_uses_a_separate_bearer_credential() {
    let router = intake_router(state(), "t".repeat(32), Some("a".repeat(64)));
    let wrong = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .header("x-centaur-principal-id", "migration-operator")
                .header("x-centaur-thread-key", "codex:issue-27")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);

    let correct = router
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .header("authorization", format!("Bearer {}", "t".repeat(32)))
                .header("x-centaur-principal-id", "migration-operator")
                .header("x-centaur-thread-key", "codex:issue-27")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(correct.status(), StatusCode::OK);
}

#[tokio::test]
async fn human_ui_deep_links_serve_the_spa_with_ok_status() {
    let static_dir = std::env::temp_dir().join(format!(
        "centaur-context-spa-deep-link-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&static_dir).unwrap();
    std::fs::write(
        static_dir.join("index.html"),
        "<main>Centaur Context</main>",
    )
    .unwrap();

    let router = human_router(
        state(),
        static_dir.clone(),
        static_dir.join("identity-assets"),
    );
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/objects")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(body.as_ref(), b"<main>Centaur Context</main>");
    let schema = router
        .oneshot(
            Request::builder()
                .uri("/schema/objects/rows")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    std::fs::remove_dir_all(static_dir).unwrap();

    assert_eq!(schema.status(), StatusCode::OK);
}

#[tokio::test]
async fn identity_assets_are_content_addressed_same_origin_images() {
    let root = std::env::temp_dir().join(format!(
        "centaur-context-identity-assets-{}",
        uuid::Uuid::new_v4()
    ));
    let bytes = b"\x89PNG\r\n\x1a\nsynthetic-image";
    let digest = format!("{:x}", Sha256::digest(bytes));
    std::fs::create_dir_all(root.join(&digest)).unwrap();
    std::fs::write(root.join(&digest).join("avatar.png"), bytes).unwrap();
    let wrong_mime = b"\xff\xd8\xffsynthetic-jpeg";
    let wrong_mime_digest = format!("{:x}", Sha256::digest(wrong_mime));
    std::fs::create_dir_all(root.join(&wrong_mime_digest)).unwrap();
    std::fs::write(root.join(&wrong_mime_digest).join("wrong.png"), wrong_mime).unwrap();
    let router = human_router(state(), PathBuf::from("web/dist"), root.clone());

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/identity-assets/{digest}/avatar.png"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/png");
    assert_eq!(
        response.headers()["cache-control"],
        "public, max-age=31536000, immutable"
    );
    assert_eq!(response.headers()["etag"], format!("\"{digest}\""));

    for path in [
        format!("/api/v1/identity-assets/{digest}/../avatar.png"),
        format!("/api/v1/identity-assets/{}/avatar.png", "0".repeat(64)),
        format!("/api/v1/identity-assets/{digest}/avatar.svg"),
        format!("/api/v1/identity-assets/{wrong_mime_digest}/wrong.png"),
    ] {
        assert_eq!(
            router
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    std::fs::remove_dir_all(root).unwrap();
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
    let router = human_router(
        state(),
        PathBuf::from("web/dist"),
        PathBuf::from("identity-assets"),
    );
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
    assert_eq!(metadata["product"], "centaur-context");
    assert_eq!(metadata["product_version"], "0.2.0");
    assert_eq!(metadata["api_version"], "v1");
    assert_eq!(metadata["ontology_version"], "v2");
    assert_eq!(metadata["database_schema_version"], 14);
    assert_eq!(metadata["tool_version"], "0.2.0");
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
    let response = human_router(
        state(),
        PathBuf::from("web/dist"),
        PathBuf::from("identity-assets"),
    )
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
async fn agent_source_routes_are_read_only_and_validate_content_bounds() {
    let router = agent_router(state(), "a".repeat(32));
    let write = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sources")
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .header("x-centaur-principal-id", "prn_test")
                .header("x-centaur-thread-key", "slack:test:test:test")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(write.status(), StatusCode::NOT_FOUND);

    for uri in [
        "/api/v1/sources/00000000-0000-0000-0000-000000000001/content?offset=-1",
        "/api/v1/sources/00000000-0000-0000-0000-000000000001/content?version=0",
        "/api/v1/sources/00000000-0000-0000-0000-000000000001/content?limit=20001",
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header("authorization", format!("Bearer {}", "a".repeat(32)))
                    .header("x-centaur-principal-id", "prn_test")
                    .header("x-centaur-thread-key", "slack:test:test:test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
    }
}

#[tokio::test]
async fn note_write_listener_is_a_separate_attributed_idempotent_grant() {
    let write_token = "w".repeat(32);
    let body = r##"{"title":"Research note","description":"A bounded note created by an authorized research agent.","content":"# Evidence\nSynthetic evidence only.","content_format":"markdown","provenance":{"source_type":"human"}}"##;
    let wrong = note_write_router(state(), write_token.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .header("x-centaur-principal-id", "researcher")
                .header("x-centaur-thread-key", "slack:T:C:thread")
                .header("idempotency-key", "note-1")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);

    let missing_attribution = note_write_router(state(), write_token.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("authorization", format!("Bearer {write_token}"))
                .header("idempotency-key", "note-1")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_attribution.status(), StatusCode::BAD_REQUEST);

    let missing_retry_key = note_write_router(state(), write_token)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("authorization", format!("Bearer {}", "w".repeat(32)))
                .header("x-centaur-principal-id", "researcher")
                .header("x-centaur-thread-key", "slack:T:C:thread")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_retry_key.status(), StatusCode::BAD_REQUEST);

    let read_surface = agent_router(state(), "a".repeat(32))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/notes")
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .header("x-centaur-principal-id", "researcher")
                .header("x-centaur-thread-key", "slack:T:C:thread")
                .header("idempotency-key", "note-1")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(read_surface.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn eval_read_and_annotation_routes_exist_only_on_the_human_listener() {
    let agent = agent_router(state(), "a".repeat(32))
        .oneshot(
            Request::builder()
                .uri("/api/v1/evals")
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .header("x-centaur-principal-id", "prn_test")
                .header("x-centaur-thread-key", "slack:test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(agent.status(), StatusCode::NOT_FOUND);

    let curator = curator_router(state(), "c".repeat(32))
        .oneshot(
            Request::builder()
                .uri("/api/v1/evals")
                .header("authorization", format!("Bearer {}", "c".repeat(32)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(curator.status(), StatusCode::NOT_FOUND);

    let ingestion = ingest_router(
        state(),
        "i".repeat(32),
        ApprovedSlackSurfaces::parse("T1:C1").unwrap(),
    )
    .oneshot(
        Request::builder()
            .uri("/api/v1/evals")
            .header("authorization", format!("Bearer {}", "i".repeat(32)))
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(ingestion.status(), StatusCode::NOT_FOUND);

    let invalid_human_filter = human_router(
        state(),
        PathBuf::from("web/dist"),
        PathBuf::from("identity-assets"),
    )
    .oneshot(
        Request::builder()
            .uri("/api/v1/evals?kind=automatic_score")
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        invalid_human_filter.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let invalid_human_annotation = human_router(
        state(),
        PathBuf::from("web/dist"),
        PathBuf::from("identity-assets"),
    )
    .oneshot(
        Request::builder()
            .method("PATCH")
            .uri("/api/v1/evals/00000000-0000-4000-8000-000000000001/annotation")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"verdict":"automatic_score","notes":null,"expected_revision":0}"#,
            ))
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        invalid_human_annotation.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn schema_routes_are_read_only_and_exist_only_on_the_human_listener() {
    let agent = agent_router(state(), "a".repeat(32))
        .oneshot(
            Request::builder()
                .uri("/api/v1/schema")
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .header("x-centaur-principal-id", "prn_test")
                .header("x-centaur-thread-key", "slack:test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(agent.status(), StatusCode::NOT_FOUND);

    let curator = curator_router(state(), "c".repeat(32))
        .oneshot(
            Request::builder()
                .uri("/api/v1/schema")
                .header("authorization", format!("Bearer {}", "c".repeat(32)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(curator.status(), StatusCode::NOT_FOUND);

    let ingestion = ingest_router(
        state(),
        "i".repeat(32),
        ApprovedSlackSurfaces::parse("T1:C1").unwrap(),
    )
    .oneshot(
        Request::builder()
            .uri("/api/v1/schema")
            .header("authorization", format!("Bearer {}", "i".repeat(32)))
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(ingestion.status(), StatusCode::NOT_FOUND);

    for method in ["POST", "PUT", "PATCH", "DELETE"] {
        let response = human_router(
            state(),
            PathBuf::from("web/dist"),
            PathBuf::from("identity-assets"),
        )
        .oneshot(
            Request::builder()
                .method(method)
                .uri("/api/v1/schema")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
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

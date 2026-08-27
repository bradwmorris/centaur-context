use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use centaur_os::api::{AppState, agent_router};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

fn state() -> AppState {
    AppState {
        pool: PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1/unused")
            .unwrap(),
    }
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

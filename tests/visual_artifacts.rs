use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use centaur_context::{
    api::{AppState, agent_router},
    config::TextSearchConfig,
    db,
};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::io::Cursor;
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
    db::migrate(&pool).await.unwrap();
    Some(pool)
}

fn png(color: [u8; 3]) -> Vec<u8> {
    let image = image::RgbImage::from_pixel(120, 80, image::Rgb(color));
    let mut output = Cursor::new(Vec::new());
    image
        .write_to(&mut output, image::ImageFormat::Png)
        .unwrap();
    output.into_inner()
}

fn upload(
    source: Uuid,
    data: Vec<u8>,
    key: &str,
    doc: &str,
    revision: i64,
    predecessor: Option<Uuid>,
) -> Request<Body> {
    let mut req = Request::builder()
        .method("POST")
        .uri(format!("/api/v2/sources/{source}/visual-artifacts"))
        .header("authorization", format!("Bearer {}", "v".repeat(32)))
        .header("x-centaur-principal-id", "visual-test-agent")
        .header("x-centaur-thread-key", "test:visual:thread")
        .header("idempotency-key", key)
        .header("content-type", "image/png")
        .header("x-artifact-title", "Feedback loop")
        .header(
            "x-artifact-description",
            "A simple synthetic feedback loop.",
        )
        .header("x-artifact-document-key", doc)
        .header("x-artifact-expected-revision", revision.to_string());
    if let Some(id) = predecessor {
        req = req.header("x-artifact-supersedes-id", id.to_string());
    }
    req.body(Body::from(data)).unwrap()
}

async fn data(response: axum::response::Response) -> Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

#[tokio::test]
async fn visuals_keep_source_evidence_and_have_retry_safe_binary_identity() {
    let Some(pool) = pool().await else {
        return;
    };
    let source = Uuid::new_v4();
    let other = Uuid::new_v4();
    let mut seed = pool.begin().await.unwrap();
    for (id, protected) in [(source, true), (other, false)] {
        sqlx::query("INSERT INTO objects (id,kind,title,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES ($1,'source','Synthetic visual Source','A disposable Source for visual tests.',$2,'system','visual-test','system','visual-test')")
            .bind(id).bind(protected).execute(&mut *seed).await.unwrap();
        sqlx::query("INSERT INTO sources (object_id,source_kind) VALUES ($1,'article')")
            .bind(id)
            .execute(&mut *seed)
            .await
            .unwrap();
    }
    seed.commit().await.unwrap();
    let app = agent_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        "v".repeat(32),
    );
    let first_bytes = png([18, 90, 160]);
    let first_key = format!("visual-first-{source}");
    let first = app
        .clone()
        .oneshot(upload(
            source,
            first_bytes.clone(),
            &first_key,
            "loop",
            1,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);
    let first_id = Uuid::parse_str(
        data(first).await["data"]["artifact"]["id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let repeat = app
        .clone()
        .oneshot(upload(
            source,
            first_bytes.clone(),
            &first_key,
            "loop",
            1,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(repeat.status(), StatusCode::CREATED);
    assert_eq!(
        data(repeat).await["data"]["artifact"]["id"],
        first_id.to_string()
    );
    let changed_key = app
        .clone()
        .oneshot(upload(
            source,
            png([200, 20, 20]),
            &first_key,
            "loop",
            1,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(changed_key.status(), StatusCode::CONFLICT);
    let stale = app
        .clone()
        .oneshot(upload(
            source,
            png([8, 8, 8]),
            &format!("visual-stale-{source}"),
            "stale",
            1,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let raw = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v2/artifacts/{first_id}/binary"))
                .header("authorization", format!("Bearer {}", "v".repeat(32)))
                .header("x-centaur-principal-id", "visual-test-agent")
                .header("x-centaur-thread-key", "test:visual:thread")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(raw.status(), StatusCode::OK);
    assert_eq!(raw.headers()["content-type"], "image/png");
    assert_eq!(
        raw.into_body().collect().await.unwrap().to_bytes(),
        first_bytes
    );
    let second = app
        .clone()
        .oneshot(upload(
            source,
            first_bytes.clone(),
            &format!("visual-second-{source}"),
            "comparison",
            2,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::CREATED);
    let successor = app
        .clone()
        .oneshot(upload(
            source,
            png([20, 140, 80]),
            &format!("visual-third-{source}"),
            "loop",
            3,
            Some(first_id),
        ))
        .await
        .unwrap();
    assert_eq!(successor.status(), StatusCode::CREATED);
    let bad_predecessor = app
        .clone()
        .oneshot(upload(
            other,
            png([20, 140, 80]),
            &format!("visual-cross-{source}"),
            "loop",
            1,
            Some(first_id),
        ))
        .await
        .unwrap();
    assert_eq!(bad_predecessor.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let unprotected = app
        .clone()
        .oneshot(upload(
            other,
            first_bytes.clone(),
            &format!("visual-other-{source}"),
            "new",
            1,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(unprotected.status(), StatusCode::CREATED);
    let missing_auth = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v2/sources/{source}/visual-artifacts"))
                .header("content-type", "image/png")
                .body(Body::from(first_bytes.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_auth.status(), StatusCode::UNAUTHORIZED);
    let mismatched_mime = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v2/sources/{source}/visual-artifacts"))
                .header("authorization", format!("Bearer {}", "v".repeat(32)))
                .header("x-centaur-principal-id", "visual-test-agent")
                .header("x-centaur-thread-key", "test:visual:thread")
                .header("idempotency-key", format!("visual-mime-{source}"))
                .header("content-type", "image/jpeg")
                .body(Body::from(first_bytes.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mismatched_mime.status(), StatusCode::BAD_REQUEST);
    let corrupt = app
        .clone()
        .oneshot(upload(
            source,
            b"<svg>bad</svg>".to_vec(),
            &format!("visual-corrupt-{source}"),
            "bad",
            4,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(corrupt.status(), StatusCode::BAD_REQUEST);
    let (revision, canonical): (i64, Option<Uuid>) = sqlx::query_as("SELECT o.revision,s.current_artifact_id FROM objects o JOIN sources s ON s.object_id=o.id WHERE o.id=$1")
        .bind(source).fetch_one(&pool).await.unwrap();
    assert_eq!(revision, 4);
    assert_eq!(canonical, None);
    let payloads: i64 = sqlx::query_scalar("SELECT count(*) FROM artifact_binary_payloads p JOIN artifacts a ON a.id=p.artifact_id WHERE a.object_id=$1")
        .bind(source).fetch_one(&pool).await.unwrap();
    assert_eq!(payloads, 3);
    let leaked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM runs WHERE primary_object_id=$1 AND (input::text LIKE '%iVBOR%' OR result::text LIKE '%iVBOR%'))")
        .bind(source).fetch_one(&pool).await.unwrap();
    assert!(!leaked);
    sqlx::query("UPDATE objects SET archived_at=now() WHERE id=$1")
        .bind(other)
        .execute(&pool)
        .await
        .unwrap();
    let archived = app
        .oneshot(upload(
            other,
            png([2, 4, 6]),
            &format!("visual-archived-{source}"),
            "archived",
            3,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(archived.status(), StatusCode::NOT_FOUND);
}

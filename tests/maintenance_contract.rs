use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use centaur_context::{
    api::{AppState, agent_router},
    config::TextSearchConfig,
    db, intake,
    maintenance::{MaintenanceConfig, approval_hash},
    universal::ApplyRequest,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const TOKEN: &str = "synthetic-maintenance-token-32-characters";
const INTAKE: &str = "synthetic-import-token-32-characters";
fn state(pool: &PgPool) -> AppState {
    AppState {
        pool: pool.clone(),
        embeddings: None,
        text_search_config: TextSearchConfig::SIMPLE,
    }
}
fn app(pool: &PgPool, approved: &[Value]) -> Router {
    intake::router_with_maintenance(
        state(pool),
        INTAKE.into(),
        None,
        Some(MaintenanceConfig {
            api_token: TOKEN.into(),
            allowed_principal: "maintenance-test".into(),
            approved_request_hashes: approved
                .iter()
                .map(|v| {
                    approval_hash(&serde_json::from_value::<ApplyRequest>(v.clone()).unwrap())
                        .unwrap()
                })
                .collect(),
        }),
    )
}
async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: &str,
    principal: &str,
    value: Value,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .header("x-centaur-principal-id", principal)
        .header("x-centaur-thread-key", "maintenance-synthetic-review")
        .header("content-type", "application/json")
        .header(
            "idempotency-key",
            value
                .get("idempotency_key")
                .and_then(Value::as_str)
                .unwrap_or("read-only"),
        )
        .body(Body::from(value.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
fn batch(operations: Value) -> Value {
    json!({"contract_version":"1.1.0","idempotency_key":Uuid::new_v4().to_string(),"operations":operations})
}
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
async fn seed(pool: &PgPool, kind: &str) -> Uuid {
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,$2,'Synthetic protected research','A synthetic original description.',true,'system','original-creator','system','original-creator')")
        .bind(id).bind(kind).execute(&mut *tx).await.unwrap();
    match kind {
        "note" => {
            sqlx::query("INSERT INTO notes(object_id,content,content_format) VALUES($1,'Original thought','plain_text')").bind(id).execute(&mut *tx).await.unwrap();
        }
        "source" => {
            sqlx::query("INSERT INTO sources(object_id,source_kind,canonical_uri) VALUES($1,'article','https://example.com/old')").bind(id).execute(&mut *tx).await.unwrap();
        }
        "user" => {
            sqlx::query("INSERT INTO users(object_id,user_kind) VALUES($1,'human')")
                .bind(id)
                .execute(&mut *tx)
                .await
                .unwrap();
        }
        _ => unreachable!(),
    }
    tx.commit().await.unwrap();
    id
}

#[tokio::test]
async fn exact_reviewed_maintenance_preserves_protection_history_and_replay() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else {
        return;
    };
    let note = seed(&pool, "note").await;
    let source = seed(&pool, "source").await;
    let author = seed(&pool, "user").await;
    let edge = Uuid::new_v4();
    sqlx::query("INSERT INTO connections(id,source_object_id,kind,target_object_id,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,$2,'derived_from',$3,'Legacy weak assertion',true,'system','original-creator','system','original-creator')")
        .bind(edge).bind(note).bind(source).execute(&pool).await.unwrap();
    let body = batch(json!([
        {"operation":"append_artifact","object":{"object_id":note},"expected_revision":1,"kind":"research_notes","content":"Prior approved assisted wording","metadata":{"purpose":"preserved_history"}},
        {"operation":"update_object","object_id":note,"expected_revision":2,"changes":{"content":"Original unedited wording","intent":"insight","description":"A faithful original research thought."}},
        {"operation":"update_object","object_id":source,"expected_revision":1,"changes":{"canonical_uri":"https://example.com/CaseSensitive"}},
        {"operation":"archive_connection","connection_id":edge,"expected_revision":1},
        {"operation":"create_connection","source":{"object_id":source},"target":{"object_id":author},"kind":"involves","description":"The connected person authored this article."}
    ]));
    let unapproved = app(&pool, &[]);
    for (token, principal, status) in [
        ("wrong", "maintenance-test", StatusCode::UNAUTHORIZED),
        (INTAKE, "maintenance-test", StatusCode::UNAUTHORIZED),
        (TOKEN, "ordinary-agent", StatusCode::FORBIDDEN),
    ] {
        assert_eq!(
            call(
                &unapproved,
                "POST",
                "/api/v2/maintenance/apply",
                token,
                principal,
                body.clone()
            )
            .await
            .0,
            status
        );
    }
    assert_eq!(
        call(
            &unapproved,
            "POST",
            "/api/v2/maintenance/apply",
            TOKEN,
            "maintenance-test",
            body.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &unapproved,
            "POST",
            "/api/v2/intake/batches/commit",
            TOKEN,
            "maintenance-test",
            json!({})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let events_before: i64 = sqlx::query_scalar("SELECT count(*) FROM object_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    let runs_before: i64 = sqlx::query_scalar("SELECT count(*) FROM runs")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut preview = body.clone();
    preview["validate_only"] = json!(true);
    let (status, preview) = call(
        &unapproved,
        "POST",
        "/api/v2/maintenance/apply",
        TOKEN,
        "maintenance-test",
        preview,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["data"]["validated_only"], true);
    assert_eq!(
        preview["data"]["results"][1]["before"]["subtype"]["content"],
        "Original thought"
    );
    assert_eq!(db::get_object(&pool, note).await.unwrap().revision, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM object_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        events_before
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        runs_before
    );
    let approved = app(&pool, std::slice::from_ref(&body));
    let mut tampered = body.clone();
    tampered["operations"][1]["changes"]["content"] = json!("Unapproved rewrite");
    assert_eq!(
        call(
            &approved,
            "POST",
            "/api/v2/maintenance/apply",
            TOKEN,
            "maintenance-test",
            tampered
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, result) = call(
        &approved,
        "POST",
        "/api/v2/maintenance/apply",
        TOKEN,
        "maintenance-test",
        body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["approval_sha256"], preview["approval_sha256"]);
    assert_eq!(result["data"]["event_ids"].as_array().unwrap().len(), 5);
    let object = db::get_object(&pool, note).await.unwrap();
    assert!(object.protected);
    assert_eq!(object.created_by_id, "original-creator");
    assert_eq!(object.updated_by_id, "maintenance-test");
    assert_eq!(object.revision, 3);
    let stored: Value = sqlx::query_scalar("SELECT result FROM runs WHERE id=$1")
        .bind(Uuid::parse_str(result["data"]["run_id"].as_str().unwrap()).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        stored["results"][1]["before"]["subtype"]["content"],
        "Original thought"
    );
    let (_, replay) = call(
        &approved,
        "POST",
        "/api/v2/maintenance/apply",
        TOKEN,
        "maintenance-test",
        body.clone(),
    )
    .await;
    assert_eq!(replay["data"]["replayed"], true);
    assert_eq!(replay["data"]["run_id"], result["data"]["run_id"]);
    assert_eq!(
        call(
            &unapproved,
            "POST",
            "/api/v2/maintenance/apply",
            TOKEN,
            "maintenance-test",
            body
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, read) = call(
        &approved,
        "POST",
        "/api/v2/maintenance/read",
        TOKEN,
        "maintenance-test",
        json!({"object_ids":[note],"include":["events","artifacts"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        read["data"]["objects"][0]["subtype"]["content_excerpt"],
        "Original unedited wording"
    );
    let (status, full_note) = call(
        &approved,
        "GET",
        &format!("/api/v2/maintenance/notes/{note}"),
        TOKEN,
        "maintenance-test",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(full_note["data"]["content"], "Original unedited wording");
    let ordinary = agent_router(state(&pool), "ordinary-token-32-characters-long".into());
    let correction = batch(
        json!([{"operation":"update_object","object_id":note,"expected_revision":3,"changes":{"content":"Not authorized"}}]),
    );
    assert_eq!(
        call(
            &ordinary,
            "POST",
            "/api/v2/apply",
            "ordinary-token-32-characters-long",
            "ordinary-agent",
            correction
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (_, archived) = call(
        &approved,
        "GET",
        "/api/v2/maintenance/connections?lifecycle=archived",
        TOKEN,
        "maintenance-test",
        Value::Null,
    )
    .await;
    assert!(
        archived["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == edge.to_string())
    );
}

#[tokio::test]
async fn maintenance_conflicts_are_atomic_and_inventory_includes_archives() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else {
        return;
    };
    let first = seed(&pool, "note").await;
    let second = seed(&pool, "source").await;
    let invalid = batch(json!([
        {"operation":"update_object","object_id":first,"expected_revision":1,"changes":{"description":"A proposed correction."}},
        {"operation":"update_object","object_id":second,"expected_revision":9,"changes":{"description":"Stale correction."}}
    ]));
    let reviewed = app(&pool, std::slice::from_ref(&invalid));
    assert_eq!(
        call(
            &reviewed,
            "POST",
            "/api/v2/maintenance/apply",
            TOKEN,
            "maintenance-test",
            invalid
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(db::get_object(&pool, first).await.unwrap().revision, 1);
    let archive =
        batch(json!([{"operation":"archive_object","object_id":first,"expected_revision":1}]));
    let reviewed = app(&pool, std::slice::from_ref(&archive));
    assert_eq!(
        call(
            &reviewed,
            "POST",
            "/api/v2/maintenance/apply",
            TOKEN,
            "maintenance-test",
            archive
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut ids = std::collections::HashSet::new();
    let mut cursor = None;
    loop {
        let url = match cursor {
            Some(ref c) => format!("/api/v2/maintenance/objects?limit=2&cursor={c}"),
            None => "/api/v2/maintenance/objects?limit=2".into(),
        };
        let (status, page) = call(
            &reviewed,
            "GET",
            &url,
            TOKEN,
            "maintenance-test",
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        for row in page["data"].as_array().unwrap() {
            assert!(ids.insert(row["id"].as_str().unwrap().to_owned()));
        }
        cursor = page["next_cursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert!(ids.contains(&first.to_string()));
    assert!(ids.contains(&second.to_string()));
    let (_, archived) = call(
        &reviewed,
        "GET",
        "/api/v2/maintenance/objects?lifecycle=archived",
        TOKEN,
        "maintenance-test",
        Value::Null,
    )
    .await;
    assert!(
        archived["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == first.to_string())
    );
    for changes in [
        json!({"protected":false}),
        json!({"created_by_id":"rewritten"}),
        json!({"provenance":{}}),
    ] {
        let forbidden = batch(
            json!([{"operation":"update_object","object_id":second,"expected_revision":1,"changes":changes}]),
        );
        assert_eq!(
            call(
                &app(&pool, std::slice::from_ref(&forbidden)),
                "POST",
                "/api/v2/maintenance/apply",
                TOKEN,
                "maintenance-test",
                forbidden
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
}

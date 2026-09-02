use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use centaur_context::{
    api::AppState,
    config::TextSearchConfig,
    db,
    ingest::{ApprovedSlackSurfaces, router},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

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

fn request(token: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/v2/ingest/slack/interactions")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(serde_json::to_vec(body).unwrap()))
        .unwrap()
}

#[tokio::test]
async fn one_slack_message_opens_and_finishes_one_idempotent_run() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping Slack Run contract: TEST_DATABASE_URL is not set");
        return;
    };
    db::migrate(&pool).await.unwrap();
    let fixture = Uuid::new_v4().simple().to_string();
    let workspace = format!("T{fixture}");
    let channel = format!("C{fixture}");
    let thread = "1780000000.000100";
    let interaction = "1780000001.000100";
    let token = "i".repeat(32);
    let app = router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        token.clone(),
        ApprovedSlackSurfaces::parse(&format!("{workspace}:{channel}")).unwrap(),
    );

    let note_id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,'note','Captured result','A durable result created during the synthetic Slack interaction.','system','test','system','test','{}')")
        .bind(note_id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO notes(object_id,content,content_format) VALUES($1,'Synthetic note body.','markdown')")
        .bind(note_id).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();

    let human = json!({
        "provider_message_id":interaction,
        "sender":{"provider_user_id":format!("U{fixture}"),"display_name":"Example Human","user_kind":"human"},
        "content":"Create a note about the result.",
        "source_created_at":"2026-08-28T00:00:01Z"
    });
    let running = json!({
        "workspace_id":workspace,"channel_id":channel,"thread_id":thread,"surface_kind":"channel",
        "messages":[human.clone()],"interaction_finished":false,
        "run":{"interaction_id":interaction,"status":"running","started_at":"2026-08-28T00:00:01Z","trace":[{
            "id":format!("{interaction}:context"),"entry_type":"context_retrieval","name":"retrieve Context","status":"completed","component":"centaur_context"
        }]}
    });
    let started = app
        .clone()
        .oneshot(request(&token, &running))
        .await
        .unwrap();
    assert_eq!(started.status(), StatusCode::ACCEPTED);
    let started_body: Value =
        serde_json::from_slice(&started.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let run_id = started_body["data"]["run_id"].as_str().unwrap();

    let completed = json!({
        "workspace_id":workspace,"channel_id":channel,"thread_id":thread,"surface_kind":"channel",
        "messages":[human,{
            "provider_message_id":"1780000002.000100",
            "sender":{"provider_user_id":format!("B{fixture}"),"display_name":"Example Bot","user_kind":"agent"},
            "content":"I created the note.","source_created_at":"2026-08-28T00:00:02Z"
        }],
        "interaction_finished":false,
        "agent_usage":[{
            "component":"centaur_agent","provider":"openai","model_id":"gpt-test","execution_type":"codex_harness",
            "auth_mode":"chatgpt_subscription","upstream_service":"chatgpt.com","billing_mode":"subscription_allowance",
            "source_execution_id":format!("execution-{fixture}"),"usage_status":"reported","input_tokens":10,"output_tokens":5,"total_tokens":15
        }],
        "run":{"interaction_id":interaction,"status":"completed","started_at":"2026-08-28T00:00:01Z","completed_at":"2026-08-28T00:00:03Z",
            "affected_object_ids":[note_id],"trace":[{
                "id":format!("{interaction}:tool:1"),"entry_type":"tool_call","name":"create_note","status":"completed","component":"centaur_agent"
            }]}
    });
    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(request(&token, &completed))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(body["data"]["run_id"], run_id);
    }

    let row: (String, Option<Uuid>, Value, Vec<Uuid>) = sqlx::query_as(
        "SELECT status,primary_object_id,trace,consulted_object_ids FROM runs WHERE id=$1",
    )
    .bind(Uuid::parse_str(run_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, "completed");
    assert_eq!(row.1, Some(note_id));
    assert!(row.3.len() >= 3, "Chat and both participants are linked");
    let trace = row.2.as_array().unwrap();
    assert_eq!(
        trace
            .iter()
            .filter(|entry| entry["id"] == format!("{interaction}:tool:1"))
            .count(),
        1
    );
}

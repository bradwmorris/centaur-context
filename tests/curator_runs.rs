use centaur_context::{
    curator::{
        CreateConnection, CreateObject, MemoryFields, ObjectRef, ReconciliationPlan, reconcile,
        undo, validate_plan,
    },
    db, runs,
};
use serde::Deserialize;
use serde_json::json;
use sqlx::{PgPool, postgres::PgPoolOptions};
use time::OffsetDateTime;
use uuid::Uuid;

async fn test_pool() -> Option<PgPool> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    assert!(url.contains("centaur_context_test"));
    Some(
        PgPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .unwrap(),
    )
}

struct Fixture {
    run_id: Uuid,
    chat_id: Uuid,
    message_id: Uuid,
    entity_id: Uuid,
}

async fn fixture(pool: &PgPool, message: &str) -> Fixture {
    db::migrate(pool).await.unwrap();
    let run_id = Uuid::new_v4();
    let chat_id = Uuid::new_v4();
    let message_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let entity_id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    for (id, kind, title, description) in [
        (
            user_id,
            "user",
            "Fixture human",
            "A human participant in the Curator fixture.",
        ),
        (
            chat_id,
            "chat",
            "Fixture chat",
            "A source Chat for the Curator fixture.",
        ),
        (
            entity_id,
            "entity",
            "Existing entity",
            "An unambiguous existing Entity candidate.",
        ),
    ] {
        sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,$2,$3,$4,'system','curator-test','system','curator-test','{}')")
            .bind(id).bind(kind).bind(title).bind(description).execute(&mut *tx).await.unwrap();
    }
    sqlx::query("INSERT INTO users(object_id,user_kind) VALUES($1,'human')")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO chats(object_id) VALUES($1)")
        .bind(chat_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO entities(object_id,entity_kind) VALUES($1,'person')")
        .bind(entity_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO chat_messages(id,chat_object_id,provider_message_id,sender_user_object_id,content,source_created_at) VALUES($1,$2,$3,$4,$5,'2026-09-18T00:00:00Z')")
        .bind(message_id).bind(chat_id).bind(message_id.to_string()).bind(user_id).bind(message)
        .execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,chat_object_id,idempotency_key,input,result,consulted_object_ids,available_at) VALUES($1,'curator','queued','system','context-curator',$2,$3,$4,'{}',$5,now())")
        .bind(run_id)
        .bind(chat_id)
        .bind(format!("curator-window:{chat_id}:{message_id}"))
        .bind(json!({"trigger":"inactivity","first_message_id":message_id,"last_message_id":message_id,"message_count":1}))
        .bind(vec![entity_id])
        .execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    Fixture {
        run_id,
        chat_id,
        message_id,
        entity_id,
    }
}

fn memory_plan(fixture: &Fixture) -> ReconciliationPlan {
    ReconciliationPlan {
        create_objects: vec![CreateObject {
            client_id: "memory-1".into(),
            kind: "memory".into(),
            title: "Research request recorded".into(),
            description: "Bradley requested research on an existing person and asked for the result to be recorded.".into(),
            supporting_message_ids: vec![fixture.message_id],
            entity_kind: None,
            task: None,
            memory: Some(MemoryFields {
                primary_event: true,
                happened_at: OffsetDateTime::parse(
                    "2026-09-18T00:00:00Z",
                    &time::format_description::well_known::Rfc3339,
                )
                .unwrap(),
            }),
            source: None,
        }],
        update_objects: vec![],
        create_connections: vec![
            CreateConnection {
                source: ObjectRef::Created { client_id: "memory-1".into() },
                kind: "derived_from".into(),
                target: ObjectRef::Existing { object_id: fixture.chat_id },
                description: "This Memory was derived from the originating Chat.".into(),
                supporting_message_ids: vec![fixture.message_id],
            },
            CreateConnection {
                source: ObjectRef::Created { client_id: "memory-1".into() },
                kind: "about".into(),
                target: ObjectRef::Existing { object_id: fixture.entity_id },
                description: "This Memory is about the unambiguous existing Entity.".into(),
                supporting_message_ids: vec![fixture.message_id],
            },
        ],
        update_connections: vec![],
    }
}

#[derive(Deserialize)]
struct EvalSuite {
    cases: Vec<EvalCase>,
}

#[derive(Deserialize)]
struct EvalCase {
    name: String,
    expected_valid: bool,
    plan: ReconciliationPlan,
}

#[test]
fn context_curator_mvp_policy_evals() {
    let suite: EvalSuite =
        serde_json::from_str(include_str!("../evals/context_curator_mvp.json")).unwrap();
    for mut case in suite.cases {
        let original_descriptions = case
            .plan
            .create_objects
            .iter()
            .map(|object| object.description.clone())
            .collect::<Vec<_>>();
        let valid = validate_plan(&mut case.plan).is_ok();
        assert_eq!(valid, case.expected_valid, "eval case: {}", case.name);
        if valid {
            assert_eq!(
                case.plan
                    .create_objects
                    .iter()
                    .map(|object| object.description.clone())
                    .collect::<Vec<_>>(),
                original_descriptions,
                "clear descriptions should not be rewritten: {}",
                case.name
            );
        }
    }
}

#[tokio::test]
async fn append_only_memory_commit_preserves_audit_retry_review_watermark_and_undo() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping Curator database contract: TEST_DATABASE_URL is not set");
        return;
    };
    let fixture = fixture(&pool, "Research and add the existing person").await;
    let entity_revision: i64 = sqlx::query_scalar("SELECT revision FROM objects WHERE id=$1")
        .bind(fixture.entity_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let plan = memory_plan(&fixture);

    let first = reconcile(
        &pool,
        fixture.run_id,
        "test-model",
        "issue-39",
        plan.clone(),
    )
    .await
    .unwrap();
    assert_eq!(first["status"], "completed");
    assert_eq!(first["change_count"], 3);
    let memory_id = first["created_objects"]["memory-1"]
        .as_str()
        .and_then(|value| Uuid::parse_str(value).ok())
        .unwrap();
    let replay = reconcile(&pool, fixture.run_id, "test-model", "issue-39", plan)
        .await
        .unwrap();
    assert_eq!(replay, first);

    let kinds: Vec<String> =
        sqlx::query_scalar("SELECT kind FROM objects WHERE id=$1 OR id=$2 ORDER BY kind")
            .bind(memory_id)
            .bind(fixture.entity_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(kinds, vec!["entity", "memory"]);
    let current_entity_revision: i64 =
        sqlx::query_scalar("SELECT revision FROM objects WHERE id=$1")
            .bind(fixture.entity_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(current_entity_revision, entity_revision);
    let provenance_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM connections WHERE source_object_id=$1 AND kind='derived_from' AND target_object_id=$2 AND archived_at IS NULL",
    )
    .bind(memory_id)
    .bind(fixture.chat_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(provenance_count, 1);
    let cursor: Option<Uuid> =
        sqlx::query_scalar("SELECT curated_through_message_id FROM chats WHERE object_id=$1")
            .bind(fixture.chat_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(cursor, Some(fixture.message_id));
    let event_count: i64 = sqlx::query_scalar("SELECT count(*) FROM object_events WHERE run_id=$1")
        .bind(fixture.run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(event_count, 3);

    let reviewed = runs::review(
        &pool,
        fixture.run_id,
        "pass",
        Some("Append-only Memory contract verified."),
        Some(true),
        "curator-test",
        0,
    )
    .await
    .unwrap();
    assert_eq!(reviewed.verdict, "pass");
    assert!(reviewed.pinned);

    let reversed = undo(&pool, fixture.run_id).await.unwrap();
    assert_eq!(reversed["status"], "reversed");
    let memory_archived: bool =
        sqlx::query_scalar("SELECT archived_at IS NOT NULL FROM objects WHERE id=$1")
            .bind(memory_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(memory_archived);
    let entity_active: bool =
        sqlx::query_scalar("SELECT archived_at IS NULL FROM objects WHERE id=$1")
            .bind(fixture.entity_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(entity_active);
}

#[tokio::test]
async fn no_changes_advances_the_watermark_without_domain_mutation() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let fixture = fixture(&pool, "Hello there").await;
    let plan = ReconciliationPlan {
        create_objects: vec![],
        update_objects: vec![],
        create_connections: vec![],
        update_connections: vec![],
    };
    let result = reconcile(&pool, fixture.run_id, "test-model", "issue-39", plan)
        .await
        .unwrap();
    assert_eq!(result["status"], "no_changes");
    assert_eq!(result["change_count"], 0);
    let cursor: Option<Uuid> =
        sqlx::query_scalar("SELECT curated_through_message_id FROM chats WHERE object_id=$1")
            .bind(fixture.chat_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(cursor, Some(fixture.message_id));
}

#[tokio::test]
async fn missing_existing_target_rolls_back_before_creating_memory() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let mut fixture = fixture(&pool, "Record this durable fact").await;
    fixture.entity_id = Uuid::new_v4();
    let before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM objects WHERE created_by_id='context-curator'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        reconcile(
            &pool,
            fixture.run_id,
            "test-model",
            "issue-39",
            memory_plan(&fixture),
        )
        .await
        .is_err()
    );
    let after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM objects WHERE created_by_id='context-curator'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(after, before);
}

#[tokio::test]
async fn commit_layer_rejects_missing_provenance_and_non_memory_edges() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let first_fixture = fixture(&pool, "Record this durable fact").await;
    let mut missing_provenance = memory_plan(&first_fixture);
    let provenance = missing_provenance
        .create_connections
        .iter_mut()
        .find(|connection| connection.kind == "derived_from")
        .unwrap();
    provenance.target = ObjectRef::Existing {
        object_id: first_fixture.entity_id,
    };
    assert!(
        reconcile(
            &pool,
            first_fixture.run_id,
            "test-model",
            "issue-39",
            missing_provenance,
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("every new Memory")
    );

    let second = fixture(&pool, "No durable memory is warranted").await;
    let non_memory_edge = ReconciliationPlan {
        create_objects: vec![],
        update_objects: vec![],
        create_connections: vec![CreateConnection {
            source: ObjectRef::Existing {
                object_id: second.entity_id,
            },
            kind: "related_to".into(),
            target: ObjectRef::Existing {
                object_id: second.chat_id,
            },
            description: "A forbidden edge between two non-Memory Objects.".into(),
            supporting_message_ids: vec![second.message_id],
        }],
        update_connections: vec![],
    };
    assert!(
        reconcile(
            &pool,
            second.run_id,
            "test-model",
            "issue-39",
            non_memory_edge,
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("Memory endpoint")
    );
}

#[tokio::test]
async fn ambiguous_existing_targets_are_left_unlinked() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let fixture = fixture(&pool, "Bradley requested a note about Jordan").await;
    let duplicate_id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,'entity','Jordan','A second ambiguous Entity candidate.','system','curator-test','system','curator-test','{}')")
        .bind(duplicate_id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO entities(object_id,entity_kind) VALUES($1,'person')")
        .bind(duplicate_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE runs SET consulted_object_ids=array_append(consulted_object_ids,$2) WHERE id=$1",
    )
    .bind(fixture.run_id)
    .bind(duplicate_id)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let mut plan = memory_plan(&fixture);
    plan.create_connections
        .retain(|connection| connection.kind == "derived_from");
    let result = reconcile(&pool, fixture.run_id, "test-model", "issue-39", plan)
        .await
        .unwrap();
    let memory_id =
        Uuid::parse_str(result["created_objects"]["memory-1"].as_str().unwrap()).unwrap();
    let domain_links: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM connections WHERE (source_object_id=$1 OR target_object_id=$1) AND kind<>'derived_from' AND archived_at IS NULL",
    )
    .bind(memory_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(domain_links, 0);
}

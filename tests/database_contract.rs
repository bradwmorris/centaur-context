use centaur_os::{
    db::{self, DbError, NewConnection, NewObject, NewTask, ObjectChanges, ObjectListFilter},
    domain::ActorContext,
};
use serde_json::json;
use sqlx::{PgPool, postgres::PgPoolOptions};

async fn test_pool() -> Option<PgPool> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    assert!(
        url.contains("centaur_os_test"),
        "TEST_DATABASE_URL must name a disposable centaur_os_test database"
    );
    Some(
        PgPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .unwrap(),
    )
}

fn actor() -> ActorContext {
    ActorContext::human()
}

#[tokio::test]
async fn canonical_ontology_and_revision_conflicts() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping database contract: TEST_DATABASE_URL is not set");
        return;
    };
    db::migrate(&pool).await.unwrap();
    sqlx::query("TRUNCATE object_events, connections, tasks, chats, entities, memories, objects")
        .execute(&pool)
        .await
        .unwrap();

    let first = db::create_object(
        &pool,
        &actor(),
        NewObject {
            kind: "note".to_owned(),
            title: "Shared context".to_owned(),
            body: "One canonical record".to_owned(),
            provenance: json!({"source_type": "human"}),
        },
        "create-first",
    )
    .await
    .unwrap();
    let replay = db::create_object(
        &pool,
        &actor(),
        NewObject {
            kind: "note".to_owned(),
            title: "Ignored retry".to_owned(),
            body: String::new(),
            provenance: json!({}),
        },
        "create-first",
    )
    .await
    .unwrap();
    assert_eq!(first.id, replay.id);

    let second = db::create_object(
        &pool,
        &actor(),
        NewObject {
            kind: "decision".to_owned(),
            title: "Use one database owner".to_owned(),
            body: String::new(),
            provenance: json!({"source_type": "human"}),
        },
        "create-second",
    )
    .await
    .unwrap();

    for (kind, table, key) in [
        ("chat", "chats", "create-chat"),
        ("entity", "entities", "create-entity"),
        ("memory", "memories", "create-memory"),
    ] {
        let object = db::create_object(
            &pool,
            &actor(),
            NewObject {
                kind: kind.to_owned(),
                title: format!("Canonical {kind}"),
                body: String::new(),
                provenance: json!({"source_type": "human"}),
            },
            key,
        )
        .await
        .unwrap();
        let subtype_exists: bool = sqlx::query_scalar(&format!(
            "SELECT EXISTS (SELECT 1 FROM {table} WHERE object_id=$1)"
        ))
        .bind(object.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(subtype_exists, "{kind} must have a canonical subtype row");
    }

    let search_by_title = db::list_objects(
        &pool,
        ObjectListFilter {
            query: Some("SHARED CONTEXT".to_owned()),
            kind: None,
            lifecycle: None,
            limit: 20,
        },
    )
    .await
    .unwrap();
    assert_eq!(search_by_title.len(), 1);
    assert_eq!(search_by_title[0].id, first.id);

    let search_by_body = db::list_objects(
        &pool,
        ObjectListFilter {
            query: Some("canonical record".to_owned()),
            kind: None,
            lifecycle: None,
            limit: 20,
        },
    )
    .await
    .unwrap();
    assert_eq!(search_by_body.len(), 1);
    assert_eq!(search_by_body[0].id, first.id);

    for literal_wildcard in ["%", "_"] {
        let matches = db::list_objects(
            &pool,
            ObjectListFilter {
                query: Some(literal_wildcard.to_owned()),
                kind: None,
                lifecycle: None,
                limit: 20,
            },
        )
        .await
        .unwrap();
        assert!(matches.is_empty());
    }

    let updated = db::update_object(
        &pool,
        &actor(),
        first.id,
        1,
        ObjectChanges {
            body: Some("Updated safely".to_owned()),
            ..Default::default()
        },
        None,
    )
    .await
    .unwrap();
    assert_eq!(updated.revision, 2);
    assert!(matches!(
        db::update_object(&pool, &actor(), first.id, 1, ObjectChanges::default(), None,).await,
        Err(DbError::Conflict)
    ));

    db::create_connection(
        &pool,
        &actor(),
        NewConnection {
            source_object_id: first.id,
            kind: "supports".to_owned(),
            target_object_id: second.id,
            reason: "The context supports the decision.".to_owned(),
            provenance: json!({"source_type": "human"}),
        },
        "connect-first-second",
    )
    .await
    .unwrap();

    let task = db::create_task(
        &pool,
        &actor(),
        NewTask {
            title: "Verify the shared loop".to_owned(),
            body: String::new(),
            provenance: json!({"source_type": "human"}),
            status: "todo".to_owned(),
            owner_type: None,
            owner_id: None,
            agent_eligible: true,
            due_at: None,
        },
        "create-task",
    )
    .await
    .unwrap();
    assert_eq!(task.revision, 1);
    assert!(task.agent_eligible);

    let table_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.tables WHERE table_schema='public' AND table_name IN ('objects','connections','tasks','chats','entities','memories','object_events')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(table_count, 7);
    assert!(db::list_events(&pool, first.id).await.unwrap().len() >= 3);
}

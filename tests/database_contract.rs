use centaur_os::{
    db::{
        self, ConnectionChanges, DbError, NewConnection, NewObject, NewTask, ObjectChanges,
        ObjectListFilter,
    },
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
    sqlx::query(
        "TRUNCATE object_events, connections, external_identities, tasks, chats, users, entities, memories, objects",
    )
        .execute(&pool)
        .await
        .unwrap();

    let first = db::create_object(
        &pool,
        &actor(),
        NewObject {
            kind: "memory".to_owned(),
            title: "Shared context".to_owned(),
            description: "One canonical record".to_owned(),
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
            kind: "memory".to_owned(),
            title: "Ignored retry".to_owned(),
            description: "Ignored retry description".to_owned(),
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
            kind: "entity".to_owned(),
            title: "Centaur OS".to_owned(),
            description: "The canonical product under test.".to_owned(),
            provenance: json!({"source_type": "human"}),
        },
        "create-second",
    )
    .await
    .unwrap();

    for (kind, table, key) in [("chat", "chats", "create-chat")] {
        let object = db::create_object(
            &pool,
            &actor(),
            NewObject {
                kind: kind.to_owned(),
                title: format!("Canonical {kind}"),
                description: format!("A canonical {kind} used by the contract test."),
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

    let search_by_description = db::list_objects(
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
    assert_eq!(search_by_description.len(), 1);
    assert_eq!(search_by_description[0].id, first.id);

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
            description: Some("Updated safely".to_owned()),
            protected: Some(true),
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

    let connection = db::create_connection(
        &pool,
        &actor(),
        NewConnection {
            source_object_id: first.id,
            kind: "related_to".to_owned(),
            target_object_id: second.id,
            description: "The memory is about this shared context engine.".to_owned(),
            provenance: json!({"source_type": "human"}),
            protected: false,
        },
        "connect-first-second",
    )
    .await
    .unwrap();
    let connection = db::update_connection(
        &pool,
        &actor(),
        connection.id,
        1,
        ConnectionChanges {
            description: Some("The memory clearly concerns this product.".to_owned()),
            protected: Some(true),
            ..Default::default()
        },
        None,
    )
    .await
    .unwrap();
    assert_eq!(connection.revision, 2);
    assert!(connection.protected);
    assert!(matches!(
        db::update_connection(
            &pool,
            &actor(),
            connection.id,
            1,
            ConnectionChanges::default(),
            None,
        )
        .await,
        Err(DbError::Conflict)
    ));

    let task = db::create_task(
        &pool,
        &actor(),
        NewTask {
            title: "Verify the shared loop".to_owned(),
            description: "Verify the canonical graph contract.".to_owned(),
            provenance: json!({"source_type": "human"}),
            status: "todo".to_owned(),
            priority: "high".to_owned(),
            owner_object_id: None,
            agent_eligible: true,
            due_at: None,
        },
        "create-task",
    )
    .await
    .unwrap();
    assert_eq!(task.revision, 1);
    assert!(task.agent_eligible);
    assert_eq!(task.priority, "high");

    let table_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.tables WHERE table_schema='public' AND table_name IN ('objects','connections','tasks','chats','users','external_identities','entities','memories','object_events')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(table_count, 9);
    let blank_descriptions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM objects WHERE btrim(description) = ''")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(blank_descriptions, 0);
    assert!(db::list_events(&pool, first.id).await.unwrap().len() >= 3);

    let mut invalid_tx = pool.begin().await.unwrap();
    sqlx::query(
        r#"INSERT INTO objects
           (id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id)
           VALUES (gen_random_uuid(),'memory','Orphan memory','Must not commit without its subtype.','system','contract-test','system','contract-test')"#,
    )
    .execute(&mut *invalid_tx)
    .await
    .unwrap();
    assert!(
        invalid_tx.commit().await.is_err(),
        "a first-class Object must not commit without its required subtype"
    );

    let mut delete_subtype_tx = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM memories WHERE object_id=$1")
        .bind(first.id)
        .execute(&mut *delete_subtype_tx)
        .await
        .unwrap();
    assert!(
        delete_subtype_tx.commit().await.is_err(),
        "a canonical subtype must not be removable while its Object remains"
    );
}

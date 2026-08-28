use centaur_os::{
    db::{
        self, ConnectionChanges, DbError, NewConnection, NewObject, NewTask, ObjectChanges,
        ObjectListFilter,
    },
    domain::ActorContext,
    ingest::{
        SlackInteractionInput, SlackMessageInput, SlackSenderInput, ingest,
        queue_inactive_interactions,
    },
    search,
};
use serde_json::json;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

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
        "TRUNCATE object_events, curator_runs, chat_messages, object_embeddings, object_embedding_jobs, connections, external_identities, tasks, chats, users, entities, memories, objects RESTART IDENTITY",
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

    let context = search::search(&pool, None, "shared context engine", None, 10, true)
        .await
        .unwrap();
    assert_eq!(context.retrieval, "full_text");
    assert!(context.objects.len() <= 10);
    assert!(context.objects.iter().any(|item| item.id == second.id));
    assert!(
        context
            .objects
            .iter()
            .find(|item| item.id == second.id)
            .is_some_and(|item| !item.connections.is_empty())
    );
    let memory_only_context =
        search::search(&pool, None, "shared context", Some("memory"), 10, true)
            .await
            .unwrap();
    assert!(
        memory_only_context
            .objects
            .iter()
            .all(|item| item.kind == "memory"),
        "one-hop expansion must preserve an explicit kind filter"
    );
    let plain_search = search::search(&pool, None, "shared context", None, 10, false)
        .await
        .unwrap();
    assert!(
        plain_search
            .objects
            .iter()
            .all(|item| item.connections.is_empty())
    );
    assert!(
        plain_search
            .objects
            .iter()
            .all(|item| !item.relevance.rationale.contains("active connection"))
    );

    db::ensure_embedding_index(&pool, 3).await.unwrap();
    let source_hash: String = sqlx::query_scalar(
        "SELECT object_embedding_source_hash(kind,title,description) FROM objects WHERE id=$1",
    )
    .bind(second.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO object_embeddings (object_id,model,dimensions,source_hash,embedding) VALUES ($1,'test-model',3,$2,'[1,0,0]'::vector)",
    )
    .bind(second.id)
    .bind(source_hash)
    .execute(&pool)
    .await
    .unwrap();
    let semantic =
        db::semantic_candidates(&pool, &[1.0, 0.0, 0.0], "test-model", 3, None, 10, false)
            .await
            .unwrap();
    assert_eq!(semantic[0].object.id, second.id);

    db::update_object(
        &pool,
        &actor(),
        second.id,
        second.revision,
        ObjectChanges {
            description: Some("The canonical product now has newer source text.".to_owned()),
            ..Default::default()
        },
        None,
    )
    .await
    .unwrap();
    assert!(
        db::semantic_candidates(&pool, &[1.0, 0.0, 0.0], "test-model", 3, None, 10, false,)
            .await
            .unwrap()
            .is_empty(),
        "an embedding must stop participating as soon as its Object text changes"
    );

    assert!(
        db::queue_missing_embeddings(&pool, "worker-test-model")
            .await
            .unwrap()
            > 0
    );

    let claimed = db::claim_embedding_job(&pool)
        .await
        .unwrap()
        .expect("migration and Object writes must queue embedding work");
    db::complete_embedding_job(&pool, &claimed, "worker-test-model", 3, &[0.0, 1.0, 0.0])
        .await
        .unwrap();
    let stored_worker_embedding: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM object_embeddings WHERE object_id=$1 AND model='worker-test-model')",
    )
    .bind(claimed.object_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(stored_worker_embedding);

    for index in 0..12 {
        db::create_object(
            &pool,
            &actor(),
            NewObject {
                kind: "memory".to_owned(),
                title: format!("Ranking eval {index}"),
                description: "A distinct Object used to prove the context packet limit.".to_owned(),
                provenance: json!({"source_type": "human"}),
            },
            &format!("ranking-eval-{index}"),
        )
        .await
        .unwrap();
    }
    let capped_context = search::search(&pool, None, "ranking eval", None, 50, true)
        .await
        .unwrap();
    assert_eq!(capped_context.objects.len(), 10);

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
        "SELECT count(*) FROM information_schema.tables WHERE table_schema='public' AND table_name IN ('objects','connections','tasks','chats','users','external_identities','entities','memories','object_events','chat_messages','curator_runs','object_embeddings','object_embedding_jobs')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(table_count, 13);
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

    let timestamp = |value: &str| OffsetDateTime::parse(value, &Rfc3339).unwrap();
    let human = SlackSenderInput {
        provider_user_id: "U_HUMAN".to_owned(),
        display_name: "Example Human".to_owned(),
        user_kind: "human".to_owned(),
    };
    let agent = SlackSenderInput {
        provider_user_id: "U_AGENT".to_owned(),
        display_name: "Centaur Agent".to_owned(),
        user_kind: "agent".to_owned(),
    };
    let message = |id: &str, sender: SlackSenderInput, content: &str, at: &str| SlackMessageInput {
        provider_message_id: id.to_owned(),
        sender,
        content: content.to_owned(),
        source_created_at: timestamp(at),
    };
    let interaction = |messages: Vec<SlackMessageInput>, finished| SlackInteractionInput {
        workspace_id: "T_PUBLIC".to_owned(),
        channel_id: "C_APPROVED".to_owned(),
        thread_id: "1780000000.000100".to_owned(),
        surface_kind: "channel".to_owned(),
        channel_name: Some("example-project".to_owned()),
        title: None,
        messages,
        interaction_finished: finished,
    };

    let first_messages = vec![
        message(
            "1780000000.000100",
            human.clone(),
            "Please summarize the project.",
            "2026-05-27T00:00:00Z",
        ),
        message(
            "1780000001.000100",
            agent.clone(),
            "Here is the concise summary.",
            "2026-05-27T00:00:01Z",
        ),
    ];
    let first_ingest = ingest(
        &pool,
        interaction(first_messages.clone(), false)
            .validate()
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(first_ingest.inserted_message_count, 2);
    assert_eq!(first_ingest.duplicate_message_count, 0);
    assert!(first_ingest.curator_run_id.is_none());
    assert_eq!(first_ingest.participant_object_ids.len(), 2);

    let replay = ingest(
        &pool,
        interaction(first_messages, false).validate().unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(replay.chat_object_id, first_ingest.chat_object_id);
    assert_eq!(replay.inserted_message_count, 0);
    assert_eq!(replay.duplicate_message_count, 2);

    let finished = ingest(
        &pool,
        interaction(
            vec![message(
                "1780000002.000100",
                human.clone(),
                "Finished.",
                "2026-05-27T00:00:02Z",
            )],
            true,
        )
        .validate()
        .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(finished.inserted_message_count, 1);
    assert!(finished.curator_run_id.is_some());

    let finished_replay = ingest(
        &pool,
        interaction(
            vec![message(
                "1780000002.000100",
                human.clone(),
                "Finished.",
                "2026-05-27T00:00:02Z",
            )],
            true,
        )
        .validate()
        .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(finished_replay.inserted_message_count, 0);
    assert!(finished_replay.curator_run_id.is_none());

    let continuation = ingest(
        &pool,
        interaction(
            vec![
                message(
                    "1780086401.000100",
                    agent,
                    "A later reply delivered first.",
                    "2026-05-28T00:00:01Z",
                ),
                message(
                    "1780086400.000100",
                    human,
                    "A new message in the same thread tomorrow.",
                    "2026-05-28T00:00:00Z",
                ),
            ],
            false,
        )
        .validate()
        .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(continuation.chat_object_id, first_ingest.chat_object_id);
    assert_eq!(continuation.inserted_message_count, 2);
    assert_eq!(
        queue_inactive_interactions(&pool, Duration::ZERO)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        queue_inactive_interactions(&pool, Duration::ZERO)
            .await
            .unwrap(),
        0
    );

    let counts: (i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"SELECT
             (SELECT count(*) FROM chats WHERE provider='slack'),
             (SELECT count(*) FROM users),
             (SELECT count(*) FROM chat_messages),
             (SELECT count(*) FROM connections WHERE kind='involves' AND archived_at IS NULL),
             (SELECT count(*) FROM curator_runs)"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (1, 2, 5, 2, 2));
    let windows: Vec<(String, i32)> =
        sqlx::query_as("SELECT trigger,message_count FROM curator_runs ORDER BY created_at,id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        windows,
        vec![
            ("explicit_finish".to_owned(), 3),
            ("inactivity".to_owned(), 2)
        ]
    );
    let ordered_messages = db::list_chat_messages(&pool, first_ingest.chat_object_id)
        .await
        .unwrap();
    assert_eq!(ordered_messages.len(), 5);
    assert_eq!(ordered_messages[3].provider_message_id, "1780086400.000100");
    assert_eq!(ordered_messages[4].provider_message_id, "1780086401.000100");
}

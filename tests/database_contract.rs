use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use centaur_context::{
    api::{AppState, agent_router},
    config::{EmbeddingConfig, EmbeddingInputMode, TextSearchConfig},
    curator::{
        self, CreateConnection as CuratorConnection, CreateObject as CuratorObject, MemoryFields,
        ObjectRef, ReconciliationPlan, TaskFields,
    },
    db::{
        self, ConnectionChanges, DbError, NewConnection, NewObject, NewTask, ObjectChanges,
        ObjectListFilter, TaskChanges,
    },
    domain::ActorContext,
    embeddings::{EmbeddingClient, OBJECT_EMBEDDING_FORMAT},
    ingest::{
        SlackInteractionInput, SlackMessageInput, SlackSenderInput, ingest,
        queue_inactive_interactions,
    },
    search,
};
use http_body_util::BodyExt;
use serde_json::json;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tower::ServiceExt;

async fn test_pool() -> Option<PgPool> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    assert!(
        url.contains("centaur_context_test") || url.contains("centaur_os_test"),
        "TEST_DATABASE_URL must name a disposable centaur_context_test or centaur_os_test database"
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
        "TRUNCATE object_events, curator_run_changes, curator_runs, chat_messages, object_embeddings, object_embedding_jobs, connections, external_identities, tasks, chats, users, entities, memories, objects RESTART IDENTITY",
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
            title: "Centaur Context".to_owned(),
            description: "The canonical product under test.".to_owned(),
            provenance: json!({"source_type": "human"}),
        },
        "create-second",
    )
    .await
    .unwrap();

    let weak_create = db::create_object(
        &pool,
        &actor(),
        NewObject {
            kind: "entity".to_owned(),
            title: "Unclear record".to_owned(),
            description: "TBD".to_owned(),
            provenance: json!({"source_type": "human"}),
        },
        "reject-weak-create",
    )
    .await;
    assert!(matches!(weak_create, Err(DbError::Validation(_))));
    let weak_update = db::update_object(
        &pool,
        &actor(),
        second.id,
        second.revision,
        ObjectChanges {
            description: Some(second.title.clone()),
            ..Default::default()
        },
        None,
    )
    .await;
    assert!(matches!(weak_update, Err(DbError::Validation(_))));
    assert_eq!(db::get_object(&pool, second.id).await.unwrap().revision, 1);

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
            text_search_config: TextSearchConfig::SIMPLE,
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
            text_search_config: TextSearchConfig::SIMPLE,
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
                text_search_config: TextSearchConfig::SIMPLE,
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

    let context = search::search(
        &pool,
        None,
        TextSearchConfig::SIMPLE,
        "shared context engine",
        None,
        10,
    )
    .await
    .unwrap();
    assert_eq!(context.retrieval, "full_text");
    assert!(context.objects.len() <= 10);
    assert!(context.objects.iter().any(|item| item.id == first.id));
    let memory_only_context = search::search(
        &pool,
        None,
        TextSearchConfig::SIMPLE,
        "shared context",
        Some("memory"),
        10,
    )
    .await
    .unwrap();
    assert!(
        memory_only_context
            .objects
            .iter()
            .all(|item| item.kind == "memory"),
        "search must preserve an explicit kind filter"
    );
    let plain_search = search::search(
        &pool,
        None,
        TextSearchConfig::SIMPLE,
        "shared context",
        None,
        10,
    )
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
    let multilingual = db::create_object(
        &pool,
        &actor(),
        NewObject {
            kind: "entity".to_owned(),
            title: "東京移行計画 Northwind-42".to_owned(),
            description: "Les équipes françaises préparent les migrations client.".to_owned(),
            provenance: json!({"source_type": "human"}),
        },
        "create-multilingual-search-fixture",
    )
    .await
    .unwrap();
    let neutral_matches = db::full_text_candidates(
        &pool,
        TextSearchConfig::SIMPLE,
        "東京移行計画 Northwind-42",
        None,
        10,
        false,
    )
    .await
    .unwrap();
    assert!(
        neutral_matches
            .iter()
            .any(|candidate| candidate.object.id == multilingual.id)
    );
    let french_matches = db::full_text_candidates(
        &pool,
        TextSearchConfig::parse("french").unwrap(),
        "migration",
        None,
        10,
        false,
    )
    .await
    .unwrap();
    assert!(
        french_matches
            .iter()
            .any(|candidate| candidate.object.id == multilingual.id)
    );
    let unavailable_embeddings = EmbeddingClient::new(&EmbeddingConfig {
        endpoint: "http://127.0.0.1:1/embeddings".to_owned(),
        api_token: "local-test-token".to_owned(),
        model: "unavailable-test-model".to_owned(),
        dimensions: 3,
        input_mode: EmbeddingInputMode::Shared,
        poll_interval: Duration::from_secs(1),
    })
    .unwrap();
    let fallback = search::search(
        &pool,
        Some(&unavailable_embeddings),
        TextSearchConfig::SIMPLE,
        "shared context",
        None,
        10,
    )
    .await
    .unwrap();
    assert_eq!(fallback.retrieval, "full_text");

    db::ensure_embedding_index(&pool, 3).await.unwrap();
    let source_hash: String = sqlx::query_scalar(
        "SELECT object_embedding_source_hash($2,kind,title,description) FROM objects WHERE id=$1",
    )
    .bind(second.id)
    .bind(OBJECT_EMBEDDING_FORMAT)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO object_embeddings (object_id,model,dimensions,format_version,input_mode,source_hash,embedding) VALUES ($1,'test-model',3,$2,'shared',$3,'[1,0,0]'::vector)",
    )
    .bind(second.id)
    .bind(OBJECT_EMBEDDING_FORMAT)
    .bind(source_hash)
    .execute(&pool)
    .await
    .unwrap();
    let semantic = db::semantic_candidates(
        &pool,
        &[1.0, 0.0, 0.0],
        "test-model",
        3,
        OBJECT_EMBEDDING_FORMAT,
        "shared",
        None,
        10,
        false,
    )
    .await
    .unwrap();
    assert_eq!(semantic[0].object.id, second.id);
    assert!(
        db::semantic_candidates(
            &pool,
            &[1.0, 0.0, 0.0],
            "test-model",
            3,
            "centaur-object-v2",
            "shared",
            None,
            10,
            false,
        )
        .await
        .unwrap()
        .is_empty(),
        "a stale embedding format must never participate"
    );
    assert!(
        db::semantic_candidates(
            &pool,
            &[1.0, 0.0, 0.0],
            "test-model",
            3,
            OBJECT_EMBEDDING_FORMAT,
            "search_document",
            None,
            10,
            false,
        )
        .await
        .unwrap()
        .is_empty(),
        "a stale provider input mode must never participate"
    );

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
        db::semantic_candidates(
            &pool,
            &[1.0, 0.0, 0.0],
            "test-model",
            3,
            OBJECT_EMBEDDING_FORMAT,
            "shared",
            None,
            10,
            false,
        )
        .await
        .unwrap()
        .is_empty(),
        "an embedding must stop participating as soon as its Object text changes"
    );

    assert!(
        db::queue_missing_embeddings(
            &pool,
            "worker-test-model",
            3,
            OBJECT_EMBEDDING_FORMAT,
            "shared",
        )
        .await
        .unwrap()
            > 0
    );

    let claimed = db::claim_embedding_job(&pool)
        .await
        .unwrap()
        .expect("migration and Object writes must queue embedding work");
    db::complete_embedding_job(
        &pool,
        &claimed,
        "worker-test-model",
        3,
        OBJECT_EMBEDDING_FORMAT,
        "shared",
        &[0.0, 1.0, 0.0],
    )
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
    assert!(
        db::queue_missing_embeddings(
            &pool,
            "worker-test-model",
            3,
            OBJECT_EMBEDDING_FORMAT,
            "search_document",
        )
        .await
        .unwrap()
            > 0,
        "changing provider input mode must queue rebuilds"
    );
    let queued_mode: String =
        sqlx::query_scalar("SELECT input_mode FROM object_embedding_jobs WHERE object_id=$1")
            .bind(claimed.object_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(queued_mode, "search_document");
    sqlx::query("DELETE FROM object_embedding_jobs WHERE object_id<>$1")
        .bind(claimed.object_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE object_embedding_jobs SET available_at=now() WHERE object_id=$1")
        .bind(claimed.object_id)
        .execute(&pool)
        .await
        .unwrap();
    let failed_job = db::claim_embedding_job(&pool).await.unwrap().unwrap();
    db::fail_embedding_job(&pool, failed_job.object_id, "temporary local test failure")
        .await
        .unwrap();
    let failed_state: (String, i32) =
        sqlx::query_as("SELECT status,attempts FROM object_embedding_jobs WHERE object_id=$1")
            .bind(failed_job.object_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(failed_state, ("failed".to_owned(), 1));
    sqlx::query("UPDATE object_embedding_jobs SET available_at=now() WHERE object_id=$1")
        .bind(failed_job.object_id)
        .execute(&pool)
        .await
        .unwrap();
    let retried_job = db::claim_embedding_job(&pool).await.unwrap().unwrap();
    let retry_attempts: i32 =
        sqlx::query_scalar("SELECT attempts FROM object_embedding_jobs WHERE object_id=$1")
            .bind(retried_job.object_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(retry_attempts, 2);
    db::complete_embedding_job(
        &pool,
        &retried_job,
        "worker-test-model",
        3,
        OBJECT_EMBEDDING_FORMAT,
        "search_document",
        &[0.0, 1.0, 0.0],
    )
    .await
    .unwrap();
    assert!(
        db::queue_missing_embeddings(
            &pool,
            "worker-test-model",
            4,
            OBJECT_EMBEDDING_FORMAT,
            "search_document",
        )
        .await
        .unwrap()
            > 0,
        "changing dimensions must queue rebuilds"
    );
    assert!(
        db::queue_missing_embeddings(
            &pool,
            "worker-test-model",
            4,
            "centaur-object-v2",
            "search_document",
        )
        .await
        .unwrap()
            > 0,
        "changing the formatter version must queue rebuilds"
    );

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
    let capped_context = search::search(
        &pool,
        None,
        TextSearchConfig::SIMPLE,
        "ranking eval",
        None,
        10,
    )
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
    assert_eq!(
        db::get_object(&pool, task.object_id).await.unwrap().id,
        task.object_id
    );
    let task_api_contract = serde_json::to_value(&task).unwrap();
    assert_eq!(task_api_contract["object_id"], task.object_id.to_string());
    assert!(task_api_contract.get("id").is_none());
    let protected_task = db::update_task(
        &pool,
        &actor(),
        task.object_id,
        task.revision,
        TaskChanges {
            protected: Some(true),
            ..TaskChanges::default()
        },
        Some("protect-task"),
    )
    .await
    .unwrap();
    assert!(protected_task.protected);

    let table_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.tables WHERE table_schema='public' AND table_name IN ('objects','connections','tasks','chats','users','external_identities','entities','memories','object_events','chat_messages','curator_runs','curator_run_changes','object_embeddings','object_embedding_jobs')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(table_count, 14);
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
        avatar_url: Some("https://avatars.slack-edge.example/U_HUMAN.png".to_owned()),
    };
    let agent = SlackSenderInput {
        provider_user_id: "U_AGENT".to_owned(),
        display_name: "Centaur Agent".to_owned(),
        user_kind: "agent".to_owned(),
        avatar_url: None,
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
    let users = db::list_users(&pool, 100).await.unwrap();
    assert_eq!(users.len(), 2);
    assert!(
        first_ingest
            .participant_object_ids
            .contains(&users[0].object_id)
    );
    let user_api_contract = serde_json::to_value(&users[0]).unwrap();
    assert_eq!(
        user_api_contract["object_id"],
        users[0].object_id.to_string()
    );
    assert!(user_api_contract.get("id").is_none());
    assert_eq!(
        db::list_external_identities(&pool, first_ingest.participant_object_ids[0])
            .await
            .unwrap()
            .len(),
        1
    );
    let chat_visuals = db::list_object_visuals(&pool).await.unwrap();
    let chat_visual = chat_visuals
        .iter()
        .find(|visual| visual.object_id == first_ingest.chat_object_id)
        .unwrap();
    assert_eq!(chat_visual.source_provider.as_deref(), Some("slack"));
    assert_eq!(chat_visual.users.len(), 2);
    assert!(
        chat_visual
            .users
            .iter()
            .all(|user| user.role == "participant")
    );
    assert!(chat_visual.users.iter().any(|user| {
        user.avatar_url.as_deref() == Some("https://avatars.slack-edge.example/U_HUMAN.png")
    }));

    let run_id = finished.curator_run_id.unwrap();
    let supporting_message_id: uuid::Uuid =
        sqlx::query_scalar("SELECT last_message_id FROM curator_runs WHERE id=$1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let invalid_plan = ReconciliationPlan {
        create_objects: vec![CuratorObject {
            client_id: "event-memory".to_owned(),
            kind: "memory".to_owned(),
            title: "Project summary interaction completed".to_owned(),
            description: "The user asked for a project summary and the agent supplied it."
                .to_owned(),
            supporting_message_ids: vec![supporting_message_id],
            task: None,
            memory: Some(MemoryFields {
                primary_event: true,
                happened_at: timestamp("2026-05-27T00:00:02Z"),
            }),
        }],
        update_objects: vec![],
        create_connections: vec![],
        update_connections: vec![],
    };
    let before_failed_plan: i64 = sqlx::query_scalar("SELECT count(*) FROM objects")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        curator::reconcile(&pool, run_id, "contract-model", "prompt-v1", invalid_plan)
            .await
            .is_err(),
        "a plan without the mandatory source-Chat edge must fail"
    );
    let after_failed_plan: i64 = sqlx::query_scalar("SELECT count(*) FROM objects")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        before_failed_plan, after_failed_plan,
        "a failed reconciliation must commit no partial Objects"
    );

    let valid_plan = ReconciliationPlan {
        create_objects: vec![CuratorObject {
            client_id: "event-memory".to_owned(),
            kind: "memory".to_owned(),
            title: "Project summary interaction completed".to_owned(),
            description: "The user asked for a project summary and the agent supplied it."
                .to_owned(),
            supporting_message_ids: vec![supporting_message_id],
            task: None,
            memory: Some(MemoryFields {
                primary_event: true,
                happened_at: timestamp("2026-05-27T00:00:02Z"),
            }),
        }],
        update_objects: vec![],
        create_connections: vec![CuratorConnection {
            source: ObjectRef::Created {
                client_id: "event-memory".to_owned(),
            },
            kind: "derived_from".to_owned(),
            target: ObjectRef::Existing {
                object_id: finished.chat_object_id,
            },
            description: "This event Memory was derived from the completed Slack interaction."
                .to_owned(),
            supporting_message_ids: vec![supporting_message_id],
        }],
        update_connections: vec![],
    };
    let result = curator::reconcile(
        &pool,
        run_id,
        "contract-model",
        "prompt-v1",
        valid_plan.clone(),
    )
    .await
    .unwrap();
    let memory_id =
        uuid::Uuid::parse_str(result["created_objects"]["event-memory"].as_str().unwrap()).unwrap();
    let memory_visuals = db::list_object_visuals(&pool).await.unwrap();
    let memory_visual = memory_visuals
        .iter()
        .find(|visual| visual.object_id == memory_id)
        .unwrap();
    assert_eq!(memory_visual.source_provider.as_deref(), Some("slack"));
    assert!(
        memory_visual
            .users
            .iter()
            .any(|user| user.role == "source author" && user.user_kind == "human")
    );
    assert_eq!(
        curator::get_run(&pool, run_id).await.unwrap().status,
        "completed"
    );
    let run_detail = curator::run_detail(&pool, run_id).await.unwrap();
    assert_eq!(run_detail.messages.len(), 3);
    assert_eq!(run_detail.changes.len(), 2);
    assert_eq!(curator::list_runs(&pool, 100).await.unwrap().len(), 2);
    let provenance: serde_json::Value =
        sqlx::query_scalar("SELECT provenance FROM objects WHERE id=$1")
            .bind(memory_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(provenance["curator_run_id"], run_id.to_string());
    assert_eq!(
        curator::reconcile(&pool, run_id, "contract-model", "prompt-v1", valid_plan)
            .await
            .unwrap()["created_objects"]["event-memory"],
        memory_id.to_string(),
        "replaying the same committed plan must be idempotent"
    );

    let undo = curator::undo_as(&pool, run_id, &actor()).await.unwrap();
    assert_eq!(undo["status"], "reversed");
    let reversed: (String, i64, i64) = sqlx::query_as(
        r#"SELECT o.lifecycle,o.revision,
                  (SELECT count(*) FROM connections WHERE source_object_id=o.id AND archived_at IS NULL)
           FROM objects o WHERE o.id=$1"#,
    )
    .bind(memory_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reversed, ("archived".to_owned(), 2, 0));
    let undo_actor: String = sqlx::query_scalar(
        "SELECT actor_type FROM object_events WHERE entity_type='curator_run' AND entity_id=$1 AND action='curator_undone'",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(undo_actor, "human");

    let update_run_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM curator_runs WHERE trigger='inactivity'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let update_supporting_message_id: uuid::Uuid =
        sqlx::query_scalar("SELECT last_message_id FROM curator_runs WHERE id=$1")
            .bind(update_run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let update_plan = |object_id, expected_revision, description: &str| ReconciliationPlan {
        create_objects: vec![CuratorObject {
            client_id: "continuation-memory".to_owned(),
            kind: "memory".to_owned(),
            title: "Slack thread continued".to_owned(),
            description: "The existing Slack thread received a later user message and agent reply."
                .to_owned(),
            supporting_message_ids: vec![update_supporting_message_id],
            task: None,
            memory: Some(MemoryFields {
                primary_event: true,
                happened_at: timestamp("2026-05-28T00:00:01Z"),
            }),
        }],
        update_objects: vec![centaur_context::curator::UpdateObject {
            object_id,
            expected_revision,
            title: None,
            description: Some(description.to_owned()),
            supporting_message_ids: vec![update_supporting_message_id],
            task: None,
        }],
        create_connections: vec![
            CuratorConnection {
                source: ObjectRef::Created {
                    client_id: "continuation-memory".to_owned(),
                },
                kind: "derived_from".to_owned(),
                target: ObjectRef::Existing {
                    object_id: finished.chat_object_id,
                },
                description: "This event Memory came from the continued Slack interaction."
                    .to_owned(),
                supporting_message_ids: vec![update_supporting_message_id],
            },
            CuratorConnection {
                source: ObjectRef::Existing { object_id },
                kind: "derived_from".to_owned(),
                target: ObjectRef::Existing {
                    object_id: finished.chat_object_id,
                },
                description: "This Object was updated from the continued Slack interaction."
                    .to_owned(),
                supporting_message_ids: vec![update_supporting_message_id],
            },
        ],
        update_connections: vec![],
    };

    let before_protected_failure: i64 = sqlx::query_scalar("SELECT count(*) FROM objects")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        curator::reconcile(
            &pool,
            update_run_id,
            "contract-model",
            "prompt-v1",
            update_plan(
                first.id,
                first.revision + 1,
                "The curator must not write this."
            ),
        )
        .await
        .is_err(),
        "protected Objects must reject curator updates"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM objects")
            .fetch_one(&pool)
            .await
            .unwrap(),
        before_protected_failure,
        "a protected-record failure must roll back the Memory created earlier in the plan"
    );

    let original_second = db::get_object(&pool, second.id).await.unwrap();
    let updated_description =
        "The canonical product was explicitly clarified in the later interaction.";
    curator::reconcile(
        &pool,
        update_run_id,
        "contract-model",
        "prompt-v1",
        update_plan(second.id, original_second.revision, updated_description),
    )
    .await
    .unwrap();
    let curated_second = db::get_object(&pool, second.id).await.unwrap();
    assert_eq!(curated_second.description, updated_description);
    assert_eq!(curated_second.revision, original_second.revision + 1);

    curator::undo(&pool, update_run_id).await.unwrap();
    let restored_second = db::get_object(&pool, second.id).await.unwrap();
    assert_eq!(restored_second.description, original_second.description);
    assert_eq!(
        restored_second.revision,
        original_second.revision + 2,
        "Undo restores content through a compensating revision"
    );

    let dm_human_a = SlackSenderInput {
        provider_user_id: "U_DM_A".to_owned(),
        display_name: "Alex Example".to_owned(),
        user_kind: "human".to_owned(),
        avatar_url: None,
    };
    let dm_human_b = SlackSenderInput {
        provider_user_id: "U_DM_B".to_owned(),
        display_name: "Alex Example".to_owned(),
        user_kind: "human".to_owned(),
        avatar_url: None,
    };
    let dm_agent = SlackSenderInput {
        provider_user_id: "U_DM_AGENT".to_owned(),
        display_name: "Centaur Assistant".to_owned(),
        user_kind: "agent".to_owned(),
        avatar_url: None,
    };
    let dm = ingest(
        &pool,
        SlackInteractionInput {
            workspace_id: "T_PUBLIC".to_owned(),
            channel_id: "D_CONTEXT".to_owned(),
            thread_id: "1780200000.000100".to_owned(),
            surface_kind: "dm".to_owned(),
            channel_name: None,
            title: Some("Context planning DM".to_owned()),
            messages: vec![
                message(
                    "1780200000.000100",
                    dm_human_a,
                    "Please create a task to verify the customer import tomorrow.",
                    "2026-05-29T00:00:00Z",
                ),
                message(
                    "1780200001.000100",
                    dm_human_b,
                    "I confirm that task should be tracked.",
                    "2026-05-29T00:00:01Z",
                ),
                message(
                    "1780200002.000100",
                    dm_agent,
                    "Confirmed. The task is ready to record.",
                    "2026-05-29T00:00:02Z",
                ),
            ],
            interaction_finished: true,
        }
        .validate()
        .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(dm.participant_object_ids.len(), 3);
    assert_eq!(
        dm.participant_object_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        3,
        "matching display names must not merge distinct provider identities"
    );
    let dm_surface: String =
        sqlx::query_scalar("SELECT surface_kind FROM chats WHERE object_id=$1")
            .bind(dm.chat_object_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(dm_surface, "dm");
    let dm_run_id = dm.curator_run_id.unwrap();
    let dm_supporting_message_id: uuid::Uuid =
        sqlx::query_scalar("SELECT first_message_id FROM curator_runs WHERE id=$1")
            .bind(dm_run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let dm_plan = ReconciliationPlan {
        create_objects: vec![
            CuratorObject {
                client_id: "dm-memory".to_owned(),
                kind: "memory".to_owned(),
                title: "Customer import verification was confirmed".to_owned(),
                description: "The users explicitly confirmed that customer import verification should be tracked as a task.".to_owned(),
                supporting_message_ids: vec![dm_supporting_message_id],
                task: None,
                memory: Some(MemoryFields {
                    primary_event: true,
                    happened_at: timestamp("2026-05-29T00:00:02Z"),
                }),
            },
            CuratorObject {
                client_id: "dm-task".to_owned(),
                kind: "task".to_owned(),
                title: "Verify the customer import".to_owned(),
                description: "Verify the customer import as explicitly requested and confirmed in the Slack DM."
                    .to_owned(),
                supporting_message_ids: vec![dm_supporting_message_id],
                task: Some(TaskFields {
                    confirmed: true,
                    status: "todo".to_owned(),
                    priority: "medium".to_owned(),
                    owner_object_id: None,
                    agent_eligible: false,
                    due_at: Some(timestamp("2026-05-30T00:00:00Z")),
                }),
                memory: None,
            },
        ],
        update_objects: vec![],
        create_connections: vec![
            CuratorConnection {
                source: ObjectRef::Created { client_id: "dm-memory".to_owned() },
                kind: "derived_from".to_owned(),
                target: ObjectRef::Existing { object_id: dm.chat_object_id },
                description: "This Memory records the confirmed outcome of the Slack DM.".to_owned(),
                supporting_message_ids: vec![dm_supporting_message_id],
            },
            CuratorConnection {
                source: ObjectRef::Created { client_id: "dm-task".to_owned() },
                kind: "derived_from".to_owned(),
                target: ObjectRef::Existing { object_id: dm.chat_object_id },
                description: "This Task was explicitly requested and confirmed in the Slack DM.".to_owned(),
                supporting_message_ids: vec![dm_supporting_message_id],
            },
        ],
        update_connections: vec![],
    };
    let dm_result = curator::reconcile(&pool, dm_run_id, "contract-model", "prompt-v1", dm_plan)
        .await
        .unwrap();
    let dm_task_id =
        uuid::Uuid::parse_str(dm_result["created_objects"]["dm-task"].as_str().unwrap()).unwrap();
    let dm_task = db::get_task(&pool, dm_task_id).await.unwrap();
    assert_eq!(dm_task.status, "todo");
    assert_eq!(dm_task.due_at, Some(timestamp("2026-05-30T00:00:00Z")));
    let graph_entity = db::create_object(
        &pool,
        &actor(),
        NewObject {
            kind: "entity".to_owned(),
            title: "Northwind validation service".to_owned(),
            description: "A validation system used by the migration verification workflow."
                .to_owned(),
            provenance: json!({"source_type": "human"}),
        },
        "create-context-graph-entity",
    )
    .await
    .unwrap();
    db::create_connection(
        &pool,
        &actor(),
        NewConnection {
            source_object_id: dm_task_id,
            kind: "about".to_owned(),
            target_object_id: graph_entity.id,
            description: "The verification Task concerns this customer system.".to_owned(),
            provenance: json!({"source_type": "human"}),
            protected: false,
        },
        "connect-context-graph-entity",
    )
    .await
    .unwrap();

    let context_chat = db::get_context_chat(&pool, dm.chat_object_id)
        .await
        .unwrap();
    assert_eq!(
        context_chat.thread_key().as_deref(),
        Some("slack:T_PUBLIC:D_CONTEXT:1780200000.000100")
    );
    let context = search::context(
        &pool,
        None,
        TextSearchConfig::SIMPLE,
        "customer import",
        None,
        dm.chat_object_id,
        10,
    )
    .await
    .unwrap();
    assert_eq!(context.objects.first().unwrap().id, dm.chat_object_id);
    assert!(
        dm.participant_object_ids
            .iter()
            .all(|participant_id| context
                .objects
                .iter()
                .any(|object| object.id == *participant_id)),
        "all current Chat participants must be deterministic context candidates"
    );
    let context_task = context
        .objects
        .iter()
        .find(|object| object.id == dm_task_id)
        .expect("the directly connected Task must be anchored");
    assert_eq!(context_task.subtype.as_ref().unwrap()["status"], "todo");
    assert_eq!(context_task.subtype.as_ref().unwrap()["priority"], "medium");
    assert!(
        context_task
            .relevance
            .rationale
            .contains("Directly connected")
    );
    assert!(
        context
            .objects
            .iter()
            .find(|object| object.id == graph_entity.id)
            .is_some_and(|object| object.relevance.rationale.starts_with("Connected by about")),
        "one-hop graph expansion must explain why a neighboring Object was included"
    );
    let context_chat_result = context
        .objects
        .iter()
        .find(|object| object.id == dm.chat_object_id)
        .unwrap();
    assert_eq!(
        context_chat_result.subtype.as_ref().unwrap()["current_thread"],
        true
    );
    assert!(context.objects.iter().all(|object| {
        object
            .subtype
            .as_ref()
            .and_then(|subtype| subtype["kind"].as_str())
            == Some(object.kind.as_str())
    }));
    let entity_read = search::read_object(&pool, second.id).await.unwrap();
    assert_eq!(
        entity_read.subtype.as_ref().unwrap()["entity_kind"],
        "general"
    );
    let memory_read = search::read_object(&pool, first.id).await.unwrap();
    assert!(memory_read.subtype.as_ref().unwrap()["happened_at"].is_string());
    let budget = context.budget.as_ref().unwrap();
    assert!(context.objects.len() <= 10);
    assert!(
        serde_json::to_string(&context).unwrap().chars().count() <= budget.max_characters,
        "the complete serialized packet must stay inside its declared budget"
    );
    assert_eq!(
        budget.serialized_characters,
        serde_json::to_string(&context).unwrap().chars().count()
    );
    let repeated_context = search::context(
        &pool,
        None,
        TextSearchConfig::SIMPLE,
        "customer import",
        None,
        dm.chat_object_id,
        10,
    )
    .await
    .unwrap();
    assert_eq!(
        context
            .objects
            .iter()
            .map(|object| object.id)
            .collect::<Vec<_>>(),
        repeated_context
            .objects
            .iter()
            .map(|object| object.id)
            .collect::<Vec<_>>()
    );

    let agent_state = AppState {
        pool: pool.clone(),
        embeddings: None,
        text_search_config: TextSearchConfig::SIMPLE,
    };
    let router = agent_router(agent_state, "a".repeat(32));
    let authorized_uri = format!(
        "/api/v1/context?q=customer&chat_object_id={}",
        dm.chat_object_id
    );
    let authorized = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(&authorized_uri)
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .header("x-centaur-principal-id", "principal-test")
                .header(
                    "x-centaur-thread-key",
                    "Slack:T_PUBLIC:D_CONTEXT:1780200000.000100",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(authorized.status(), StatusCode::OK);
    let authorized_body = authorized.into_body().collect().await.unwrap().to_bytes();
    let authorized_json: serde_json::Value = serde_json::from_slice(&authorized_body).unwrap();
    assert_eq!(
        authorized_json["data"]["objects"][0]["id"],
        dm.chat_object_id.to_string()
    );

    let mismatched = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(&authorized_uri)
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .header("x-centaur-principal-id", "principal-test")
                .header(
                    "x-centaur-thread-key",
                    "slack:T_PUBLIC:D_OTHER:1780200000.000100",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mismatched.status(), StatusCode::FORBIDDEN);

    let wrong_type = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/context?q=customer&chat_object_id={}",
                    second.id
                ))
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .header("x-centaur-principal-id", "principal-test")
                .header(
                    "x-centaur-thread-key",
                    "slack:T_PUBLIC:D_CONTEXT:1780200000.000100",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong_type.status(), StatusCode::NOT_FOUND);

    let search_without_chat = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/search/objects?q=customer")
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .header("x-centaur-principal-id", "principal-test")
                .header("x-centaur-thread-key", "any:valid:thread:key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(search_without_chat.status(), StatusCode::OK);

    let dm_object = db::get_object(&pool, dm.chat_object_id).await.unwrap();
    db::update_object(
        &pool,
        &actor(),
        dm.chat_object_id,
        dm_object.revision,
        ObjectChanges {
            archive: true,
            ..Default::default()
        },
        None,
    )
    .await
    .unwrap();
    let inactive = router
        .oneshot(
            Request::builder()
                .uri(&authorized_uri)
                .header("authorization", format!("Bearer {}", "a".repeat(32)))
                .header("x-centaur-principal-id", "principal-test")
                .header(
                    "x-centaur-thread-key",
                    "slack:T_PUBLIC:D_CONTEXT:1780200000.000100",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(inactive.status(), StatusCode::BAD_REQUEST);
}

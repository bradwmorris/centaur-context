use centaur_os::{
    curator::{
        self, CreateConnection as CuratorConnection, CreateObject as CuratorObject, MemoryFields,
        ObjectRef, ReconciliationPlan, TaskFields,
    },
    db::{
        self, ConnectionChanges, DbError, NewConnection, NewObject, NewTask, ObjectChanges,
        ObjectListFilter, TaskChanges,
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
    let protected_task = db::update_task(
        &pool,
        &actor(),
        task.id,
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
    assert_eq!(db::list_users(&pool, 100).await.unwrap().len(), 2);
    assert_eq!(
        db::list_external_identities(&pool, first_ingest.participant_object_ids[0])
            .await
            .unwrap()
            .len(),
        1
    );

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
        update_objects: vec![centaur_os::curator::UpdateObject {
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
    };
    let dm_human_b = SlackSenderInput {
        provider_user_id: "U_DM_B".to_owned(),
        display_name: "Alex Example".to_owned(),
        user_kind: "human".to_owned(),
    };
    let dm_agent = SlackSenderInput {
        provider_user_id: "U_DM_AGENT".to_owned(),
        display_name: "Centaur Assistant".to_owned(),
        user_kind: "agent".to_owned(),
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
}

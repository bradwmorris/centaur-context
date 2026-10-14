//! Runs only in the existing guarded CI database environment.
use centaur_context::{db, domain::ActorContext, event_memory as em, universal};
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

async fn pool() -> Option<PgPool> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    assert!(url.contains("centaur_context_test"));
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .unwrap();
    let name = format!(
        "centaur_context_test_event_memory_{}",
        Uuid::new_v4().simple()
    );
    sqlx::query(&format!("CREATE DATABASE {name}"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    let options = url
        .parse::<sqlx::postgres::PgConnectOptions>()
        .unwrap()
        .database(&name);
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
        .unwrap();
    db::migrate(&pool).await.unwrap();
    Some(pool)
}
async fn event(pool: &PgPool, title: &str) -> Uuid {
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,'event',$2,'Alex explicitly chose morning meetings.','human','test-user','human','test-user','{}')").bind(id).bind(title).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO real_world_events(object_id) VALUES($1)")
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    id
}
async fn run(pool: &PgPool, batch: &em::SynthesisBatch) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,input,result) VALUES($1,'memory_synthesis','running','system',$2,$3,$4,$5)").bind(id).bind(em::SYNTHESIS_ACTOR).bind(id.to_string()).bind(json!({"fingerprints":batch.fingerprints,"preview":false})).bind(json!({"policy_version":em::POLICY})).execute(pool).await.unwrap();
    id
}
fn create(id: Uuid) -> em::MemoryPlan {
    em::MemoryPlan {
        memories: vec![em::MemoryProposal {
            action: em::MemoryAction::Create,
            object_id: None,
            expected_revision: None,
            title: "Meeting preference".into(),
            description: "Alex prefers morning meetings.".into(),
            event_ids: vec![id],
            reason: "Explicit preference in the new Event.".into(),
        }],
    }
}
#[tokio::test]
async fn cutover_zero_output_new_memory_causal_invalidation_and_authority() {
    let Some(pool) = pool().await else { return };
    let legacy = event(&pool, "Legacy meeting").await;
    em::activate(&pool).await.unwrap();
    em::activate(&pool).await.unwrap();
    assert!(
        em::synthesis_batch(&pool, false)
            .await
            .unwrap()
            .fingerprints
            .is_empty()
    );
    let id = event(&pool, "Morning meetings").await;
    let batch = em::synthesis_batch(&pool, false).await.unwrap();
    assert!(batch.fingerprints.contains_key(&id.to_string()));
    assert!(!batch.fingerprints.contains_key(&legacy.to_string()));
    let first = run(&pool, &batch).await;
    em::apply_memories(
        &pool,
        first,
        &batch,
        &em::MemoryPlan { memories: vec![] },
        false,
    )
    .await
    .unwrap();
    assert!(
        em::synthesis_batch(&pool, false)
            .await
            .unwrap()
            .fingerprints
            .is_empty()
    );
    let state = em::status(&pool).await.unwrap();
    assert_eq!(state["events"]["reviewed_no_memory"], 1);
    // Unrelated object changes do not reopen a successful zero-output decision.
    sqlx::query("UPDATE objects SET revision=revision+1 WHERE id=$1")
        .bind(legacy)
        .execute(&pool)
        .await
        .unwrap();
    let pending = em::synthesis_batch(&pool, false).await.unwrap();
    assert!(!pending.fingerprints.contains_key(&id.to_string()));
    // New Event version may warrant a Memory. Preview is entirely read-only.
    sqlx::query("UPDATE objects SET revision=revision+1 WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let batch = em::synthesis_batch(&pool, false).await.unwrap();
    let next = run(&pool, &batch).await;
    let plan = create(id);
    em::apply_memories(&pool, next, &batch, &plan, true)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM objects WHERE kind='memory'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let result = em::apply_memories(&pool, next, &batch, &plan, false)
        .await
        .unwrap();
    let memory = Uuid::parse_str(result["memory_ids"][0].as_str().unwrap()).unwrap();
    let time: Option<time::OffsetDateTime> =
        sqlx::query_scalar("SELECT happened_at FROM memories WHERE object_id=$1")
            .bind(memory)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(time.is_none());
    em::apply_memories(&pool, next, &batch, &plan, false)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM objects WHERE kind='memory'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert!(
        em::synthesis_batch(&pool, false)
            .await
            .unwrap()
            .fingerprints
            .is_empty()
    );
    let actor = ActorContext {
        actor_type: "centaur_agent",
        actor_id: "test-agent".into(),
        centaur_thread_key: None,
        centaur_execution_id: None,
        is_agent: true,
    };
    let request:universal::ApplyRequest=serde_json::from_value(json!({"contract_version":"1.2.0","idempotency_key":"forbidden-memory","operations":[{"operation":"create_object","local_ref":"new-memory","kind":"memory","title":"Bypass","description":"An ordinary agent attempted to bypass synthesis.","provenance":{"source_type":"test"},"fields":{}}]})).unwrap();
    assert!(universal::apply(&pool, &actor, request).await.is_err());
    let before: Value = sqlx::query_scalar("SELECT to_jsonb(o) FROM objects o WHERE id=$1")
        .bind(memory)
        .fetch_one(&pool)
        .await
        .unwrap();
    // Exact revision conflicts are atomic, including zero-output plans.
    sqlx::query("UPDATE objects SET revision=revision+1 WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let stale = run(&pool, &batch).await;
    assert!(
        em::apply_memories(&pool, stale, &batch, &plan, false)
            .await
            .is_err()
    );
    let after: Value = sqlx::query_scalar("SELECT to_jsonb(o) FROM objects o WHERE id=$1")
        .bind(memory)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(before, after);
    pool.close().await;
}

async fn message(pool: &PgPool, chat: Uuid, user: Uuid, content: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO chat_messages(id,chat_object_id,provider_message_id,sender_user_object_id,content,source_created_at) VALUES($1,$2,$3,$4,$5,now())").bind(id).bind(chat).bind(id.to_string()).bind(user).bind(content).execute(pool).await.unwrap();
    id
}
async fn conversation_evidence(pool: &PgPool, chat: Uuid) -> Vec<Value> {
    sqlx::query_scalar("SELECT jsonb_build_object('id',m.id,'type','message','chat_object_id',m.chat_object_id,'content',m.content,'sender_id',m.sender_user_object_id,'at',m.source_created_at,'corrections',COALESCE((SELECT jsonb_agg(to_jsonb(c) ORDER BY c.object_revision) FROM evidence_corrections c WHERE c.target_type='message' AND c.target_id=m.id),'[]'::jsonb)) FROM chat_messages m WHERE m.chat_object_id=$1 ORDER BY m.ingestion_sequence").bind(chat).fetch_all(pool).await.unwrap()
}
async fn capture_run(pool: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,result) VALUES($1,'event_capture','running','system',$2,$3,'{}')").bind(id).bind(em::CAPTURE_ACTOR).bind(id.to_string()).execute(pool).await.unwrap();
    id
}
#[tokio::test]
async fn coherent_capture_overlap_and_unrelated_chat_growth() {
    let Some(pool) = pool().await else { return };
    em::activate(&pool).await.unwrap();
    let chat = Uuid::new_v4();
    let user = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    for (id, kind, title) in [(chat, "chat", "Meeting discussion"), (user, "user", "Alex")] {
        sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,$2,$3,'A synthetic conversation fixture.','system','test','system','test')").bind(id).bind(kind).bind(title).execute(&mut *tx).await.unwrap();
    }
    sqlx::query("INSERT INTO chats(object_id) VALUES($1)")
        .bind(chat)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users(object_id,user_kind) VALUES($1,'human')")
        .bind(user)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let one = message(&pool, chat, user, "I prefer morning meetings.").await;
    let two = message(
        &pool,
        chat,
        user,
        "They leave the afternoon free for research.",
    )
    .await;
    let batch = em::EvidenceBatch {
        evidence: conversation_evidence(&pool, chat).await,
        candidates: vec![],
    };
    // Background Chat summarization or unrelated metadata must not make exact
    // message evidence stale.
    sqlx::query("UPDATE objects SET revision=revision+1 WHERE id=$1")
        .bind(chat)
        .execute(&pool)
        .await
        .unwrap();
    let plan = em::EventPlan {
        events: vec![em::EventProposal {
            object_id: None,
            expected_revision: None,
            title: "Morning meeting preference".into(),
            description: "Alex chose morning meetings to leave afternoons free for research."
                .into(),
            starts_at: None,
            starts_at_precision: None,
            ends_at: None,
            ends_at_precision: None,
            timezone: None,
            evidence_ids: vec![one, two],
            connections: vec![],
        }],
    };
    let run_id = capture_run(&pool).await;
    let result = em::apply_events(&pool, run_id, &batch, &plan, None)
        .await
        .unwrap();
    assert_eq!(result["captured_event_ids"].as_array().unwrap().len(), 1);
    em::apply_events(&pool, run_id, &batch, &plan, None)
        .await
        .unwrap();
    let duplicate = capture_run(&pool).await;
    assert!(
        em::apply_events(&pool, duplicate, &batch, &plan, None)
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM objects WHERE kind='event'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let synthesis = em::synthesis_batch(&pool, false).await.unwrap();
    let review = run(&pool, &synthesis).await;
    em::apply_memories(
        &pool,
        review,
        &synthesis,
        &em::MemoryPlan { memories: vec![] },
        false,
    )
    .await
    .unwrap();
    message(&pool, chat, user, "Unrelated logistical acknowledgement.").await;
    assert!(
        em::synthesis_batch(&pool, false)
            .await
            .unwrap()
            .fingerprints
            .is_empty()
    );
    let event_id = Uuid::parse_str(result["captured_event_ids"][0].as_str().unwrap()).unwrap();
    let edges:i64=sqlx::query_scalar("SELECT count(*) FROM connections WHERE source_object_id=$1 AND target_object_id=$2 AND kind='derived_from' AND archived_at IS NULL").bind(event_id).bind(chat).fetch_one(&pool).await.unwrap();
    assert_eq!(edges, 1);
    centaur_context::dreaming::undo(&pool, run_id)
        .await
        .unwrap();
    let archived: bool =
        sqlx::query_scalar("SELECT archived_at IS NOT NULL FROM objects WHERE id=$1")
            .bind(event_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(archived);
    pool.close().await;
}

#[tokio::test]
async fn late_commit_and_changed_preference_update_preserve_prior_evidence() {
    let Some(pool) = pool().await else { return };
    // Begin and insert before activation, commit after its baseline snapshot.
    let late = Uuid::new_v4();
    let mut late_tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,'event','Morning meeting preference','Alex explicitly chose morning meetings.','human','test','human','test','{}')").bind(late).execute(&mut *late_tx).await.unwrap();
    sqlx::query("INSERT INTO real_world_events(object_id) VALUES($1)")
        .bind(late)
        .execute(&mut *late_tx)
        .await
        .unwrap();
    em::activate(&pool).await.unwrap();
    late_tx.commit().await.unwrap();
    let first = em::synthesis_batch(&pool, false).await.unwrap();
    assert!(first.fingerprints.contains_key(&late.to_string()));
    let first_run = run(&pool, &first).await;
    let result = em::apply_memories(&pool, first_run, &first, &create(late), false)
        .await
        .unwrap();
    let memory = Uuid::parse_str(result["memory_ids"][0].as_str().unwrap()).unwrap();
    let later = event(&pool, "Afternoon meeting preference").await;
    sqlx::query("UPDATE objects SET description='Alex now prefers afternoon meetings after a change in schedule.',revision=revision+1 WHERE id=$1").bind(later).execute(&pool).await.unwrap();
    let batch = em::synthesis_batch(&pool, false).await.unwrap();
    assert!(batch.memories.iter().any(|m| m["id"] == memory.to_string()));
    let update=em::MemoryPlan{memories:vec![em::MemoryProposal{action:em::MemoryAction::Update,object_id:Some(memory),expected_revision:Some(1),title:"Meeting preference".into(),description:"Alex now prefers afternoon meetings; morning meetings were preferred before the schedule changed.".into(),event_ids:vec![later,late],reason:"The newer explicit decision updates the earlier preference.".into()}]};
    let next = run(&pool, &batch).await;
    em::apply_memories(&pool, next, &batch, &update, false)
        .await
        .unwrap();
    let provenance_count:i64=sqlx::query_scalar("SELECT count(*) FROM connections WHERE source_object_id=$1 AND kind='derived_from' AND archived_at IS NULL").bind(memory).fetch_one(&pool).await.unwrap();
    assert_eq!(provenance_count, 2);
    let all_memories: i64 = sqlx::query_scalar("SELECT count(*) FROM objects WHERE kind='memory'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(all_memories, 1);
    assert!(
        em::synthesis_batch(&pool, false)
            .await
            .unwrap()
            .fingerprints
            .is_empty()
    );
    centaur_context::dreaming::undo(&pool, next).await.unwrap();
    let description: String = sqlx::query_scalar("SELECT description FROM objects WHERE id=$1")
        .bind(memory)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(description, "Alex prefers morning meetings.");
    pool.close().await;
}

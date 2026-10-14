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
    // Another saved plan for the identical Event version cannot create a duplicate.
    let competing = run(&pool, &batch).await;
    assert!(
        em::apply_memories(&pool, competing, &batch, &plan, false)
            .await
            .is_err()
    );
    assert!(
        em::synthesis_batch(&pool, false)
            .await
            .unwrap()
            .fingerprints
            .is_empty()
    );
    let legacy_run = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key) VALUES($1,'memory_dream','running','system','context-memory-dream',$2)").bind(legacy_run).bind(legacy_run.to_string()).execute(&pool).await.unwrap();
    let legacy_batch = centaur_context::dreaming::Batch {
        memories: vec![],
        connections: vec![],
        evidence: vec![],
        targets: vec![],
    };
    assert!(
        centaur_context::dreaming::apply_plan(
            &pool,
            legacy_run,
            &legacy_batch,
            &centaur_context::dreaming::Plan { changes: vec![] }
        )
        .await
        .is_err()
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
    sqlx::query_scalar("SELECT jsonb_build_object('id',m.id,'type','message','chat_object_id',m.chat_object_id,'content',m.content,'sender_id',m.sender_user_object_id,'sender_kind',(SELECT user_kind FROM users WHERE object_id=m.sender_user_object_id),'at',m.source_created_at,'corrections',COALESCE((SELECT jsonb_agg(to_jsonb(c) ORDER BY c.object_revision) FROM evidence_corrections c WHERE (c.target_type='message' AND c.target_id=m.id) OR (c.target_type='object' AND c.target_id=m.chat_object_id)),'[]'::jsonb)) FROM chat_messages m WHERE m.chat_object_id=$1 ORDER BY m.ingestion_sequence").bind(chat).fetch_all(pool).await.unwrap()
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
    // Correcting cited evidence reopens exactly the affected Event version.
    sqlx::query("INSERT INTO evidence_corrections(id,object_id,target_type,target_id,reason,representation,actor_type,actor_id,object_revision) VALUES($1,$2,'message',$3,'Correction to the stated preference','{\"content\":\"Afternoon meetings are preferred\"}','human','test',2)").bind(Uuid::new_v4()).bind(chat).bind(one).execute(&pool).await.unwrap();
    let corrected = em::synthesis_batch(&pool, false).await.unwrap();
    assert_eq!(corrected.fingerprints.len(), 1);
    assert!(
        corrected
            .evidence
            .evidence
            .iter()
            .any(|e| e["corrections"].as_array().is_some_and(|c| !c.is_empty()))
    );
    let stale_review = run(&pool, &synthesis).await;
    assert!(
        em::apply_memories(
            &pool,
            stale_review,
            &synthesis,
            &em::MemoryPlan { memories: vec![] },
            false
        )
        .await
        .is_err()
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
    let shared = message(
        &pool,
        chat,
        user,
        "I met Sam yesterday and booked a separate appointment for Friday.",
    )
    .await;
    let evidence = conversation_evidence(&pool, chat)
        .await
        .into_iter()
        .filter(|e| e["id"] == shared.to_string())
        .collect();
    let shared_batch = em::EvidenceBatch {
        evidence,
        candidates: vec![],
    };
    let mut meeting = plan.events[0].clone();
    meeting.title = "Meeting with Sam".into();
    meeting.description = "Alex reported meeting Sam yesterday.".into();
    meeting.evidence_ids = vec![shared];
    let mut booking = meeting.clone();
    booking.title = "Appointment booking".into();
    booking.description = "Alex booked a separate appointment for Friday.".into();
    let multiple = em::EventPlan {
        events: vec![meeting, booking],
    };
    let shared_run = capture_run(&pool).await;
    let mut agent_only = shared_batch.clone();
    agent_only.evidence[0]["sender_kind"] = json!("agent");
    assert!(
        em::apply_events(&pool, shared_run, &agent_only, &multiple, None)
            .await
            .is_err()
    );
    let result = em::apply_events(&pool, shared_run, &shared_batch, &multiple, None)
        .await
        .unwrap();
    assert_eq!(result["captured_event_ids"].as_array().unwrap().len(), 2);
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
    assert!(
        em::synthesis_batch(&pool, false)
            .await
            .unwrap()
            .fingerprints
            .is_empty()
    );
    pool.close().await;
}

#[tokio::test]
async fn concurrent_saved_and_worker_plans_cannot_duplicate_a_memory() {
    let Some(pool) = pool().await else { return };
    em::activate(&pool).await.unwrap();
    let id = event(&pool, "Meeting preference").await;
    let batch = em::synthesis_batch(&pool, false).await.unwrap();
    let plan = create(id);
    let first = run(&pool, &batch).await;
    let second = run(&pool, &batch).await;
    let (a, b) = tokio::join!(
        em::apply_memories(&pool, first, &batch, &plan, false),
        em::apply_memories(&pool, second, &batch, &plan, false)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM objects WHERE kind='memory'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    pool.close().await;
}

#[tokio::test]
async fn action_cutover_preserves_pending_scope_without_backfilling_old_updates() {
    let Some(pool) = pool().await else { return };
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,created_at) VALUES($1,'memory_capture_start','completed','system','test','start',now()-interval '1 hour')")
        .bind(Uuid::new_v4()).execute(&pool).await.unwrap();
    let mut late = pool.begin().await.unwrap();
    for (kind, action, after_cutover) in [
        ("note", "updated", false),
        ("task", "created", false),
        ("note", "updated", true),
    ] {
        let run_id = Uuid::new_v4();
        let sql = "INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key) VALUES($1,'mutation','completed','system','test',$2)";
        let evidence_sql = "INSERT INTO object_events(id,run_id,sequence,target_type,target_id,action,actor_type,actor_id,to_revision,after_state,reversible,created_at) VALUES($1,$2,1,'object',$3,$4,'system','test',1,$5,true,now())";
        if after_cutover {
            sqlx::query(sql)
                .bind(run_id)
                .bind(run_id.to_string())
                .execute(&mut *late)
                .await
                .unwrap();
            sqlx::query(evidence_sql)
                .bind(Uuid::new_v4())
                .bind(run_id)
                .bind(Uuid::new_v4())
                .bind(action)
                .bind(json!({"kind":kind}))
                .execute(&mut *late)
                .await
                .unwrap();
        } else {
            sqlx::query(sql)
                .bind(run_id)
                .bind(run_id.to_string())
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query(evidence_sql)
                .bind(Uuid::new_v4())
                .bind(run_id)
                .bind(Uuid::new_v4())
                .bind(action)
                .bind(json!({"kind":kind}))
                .execute(&pool)
                .await
                .unwrap();
        }
    }
    em::activate(&pool).await.unwrap();
    assert_eq!(
        em::status(&pool).await.unwrap()["pending_action_evidence"],
        1
    );
    late.commit().await.unwrap();
    assert_eq!(
        em::status(&pool).await.unwrap()["pending_action_evidence"],
        2
    );
    pool.close().await;
}

#[tokio::test]
async fn status_excludes_cutover_events_and_irrelevant_run_payloads() {
    let Some(pool) = pool().await else { return };
    event(&pool, "Existing occurrence").await;
    em::activate(&pool).await.unwrap();
    for _ in 0..32 {
        sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,result) VALUES($1,'mutation','completed','system','test',$1::text,$2)")
            .bind(Uuid::new_v4()).bind(json!({"payload": "x".repeat(100_000)})).execute(&pool).await.unwrap();
    }
    assert_eq!(em::status(&pool).await.unwrap()["events"]["captured"], 0);
    let id = event(&pool, "New occurrence").await;
    assert_eq!(em::status(&pool).await.unwrap()["events"]["pending"], 1);
    let batch = em::synthesis_batch(&pool, true).await.unwrap();
    assert!(batch.fingerprints.contains_key(&id.to_string()));
    let run_id = run(&pool, &batch).await;
    em::apply_memories(
        &pool,
        run_id,
        &batch,
        &em::MemoryPlan { memories: vec![] },
        false,
    )
    .await
    .unwrap();
    let state = em::status(&pool).await.unwrap();
    assert_eq!(state["events"]["reviewed_no_memory"], 1);
    assert_eq!(state["events"]["pending"], 0);
}

#[tokio::test]
async fn ordinary_evidence_gets_one_text_correction_and_oversized_evidence_defers() {
    for repair_succeeds in [true, false] {
        let Some(pool) = pool().await else { return };
        em::activate(&pool).await.unwrap();
        let chat = Uuid::new_v4();
        let user = Uuid::new_v4();
        for (id, kind) in [(chat, "chat"), (user, "user")] {
            sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,$2,'Fixture','Synthetic evidence.','system','test','system','test')").bind(id).bind(kind).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO chats(object_id) VALUES($1)")
            .bind(chat)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO users(object_id,user_kind) VALUES($1,'human')")
            .bind(user)
            .execute(&pool)
            .await
            .unwrap();
        let evidence = message(
            &pool,
            chat,
            user,
            &"I prefer morning meetings. ".repeat(1600),
        )
        .await;
        let id = event(&pool, "Meeting preference discussion").await;
        sqlx::query("UPDATE objects SET provenance=jsonb_build_object('evidence_ids',jsonb_build_array($2::text)) WHERE id=$1").bind(id).bind(evidence.to_string()).execute(&pool).await.unwrap();
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = calls.clone();
        let router = axum::Router::new().route("/", axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
            let count = count.clone();
            async move {
                let attempt = count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let payload = body.to_string();
                assert!(payload.len()>24_000, "original long evidence must not be truncated");
                let mut plan = create(id);
                if attempt == 0 || !repair_succeeds { plan.memories[0].description = "supported detail ".repeat(50); }
                if attempt == 1 {
                    assert!(payload.contains("validation_error"));
                    assert!(payload.contains("previous_plan"));
                }
                axum::Json(json!({"choices":[{"message":{"content":serde_json::to_string(&plan).unwrap()}}]}))
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let config = centaur_context::config::CuratorModelConfig {
            transport: centaur_context::config::CuratorModelTransport::DirectApi,
            endpoint: format!("http://{address}/"),
            api_token: "synthetic".into(),
            model: "gpt-6-luna".into(),
            prompt_version: "test".into(),
            poll_interval: std::time::Duration::from_secs(1),
            request_timeout: std::time::Duration::from_secs(5),
        };
        let run = em::synthesis_pass(&pool, &reqwest::Client::new(), &config, false)
            .await
            .unwrap()
            .unwrap();
        let (status, result): (String, Value) =
            sqlx::query_as("SELECT status,result FROM runs WHERE id=$1")
                .bind(run)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert_eq!(result["correction_calls"], 1);
        assert_eq!(
            status,
            if repair_succeeds {
                "completed"
            } else {
                "failed"
            }
        );
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM objects WHERE kind='memory'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, if repair_succeeds { 1 } else { 0 });
        if repair_succeeds {
            assert!(
                em::synthesis_pass(&pool, &reqwest::Client::new(), &config, false)
                    .await
                    .unwrap()
                    .is_none()
            );
            let large = message(&pool, chat, user, &"Original evidence. ".repeat(8000)).await;
            let large_event = event(&pool, "Oversized occurrence").await;
            sqlx::query("UPDATE objects SET provenance=jsonb_build_object('evidence_ids',jsonb_build_array($2::text)) WHERE id=$1").bind(large_event).bind(large.to_string()).execute(&pool).await.unwrap();
            let deferred = em::synthesis_pass(&pool, &reqwest::Client::new(), &config, false)
                .await
                .unwrap()
                .unwrap();
            let result: Value = sqlx::query_scalar("SELECT result FROM runs WHERE id=$1")
                .bind(deferred)
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(result["deferred"], true);
            assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        }
        server.abort();
        pool.close().await;
    }
}

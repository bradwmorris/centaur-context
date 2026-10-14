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

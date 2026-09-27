use centaur_context::{
    db,
    dreaming::{self, Batch, Change, Plan},
    memory,
};
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn pool() -> Option<PgPool> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    assert!(url.contains("centaur_context_test"));
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .unwrap();
    let name = format!("centaur_context_test_memory_{}", Uuid::new_v4().simple());
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
async fn fixture(pool: &PgPool, kind: &str, provenance: Value) -> Uuid {
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,$2,'Research event','Alex discussed a useful research event, recorded with evidence.','system','context-curator','system','context-curator',$3)").bind(id).bind(kind).bind(provenance).execute(&mut *tx).await.unwrap();
    if kind == "memory" {
        sqlx::query(
            "INSERT INTO memories(object_id,happened_at) VALUES($1,'2026-09-20T00:00:00Z')",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    if kind == "source" {
        sqlx::query("INSERT INTO sources(object_id,source_kind) VALUES($1,'article')")
            .bind(id)
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();
    id
}
async fn run(pool: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,result) VALUES($1,'memory_dream','running','system','context-memory-dream',$2,'{}')").bind(id).bind(id.to_string()).execute(pool).await.unwrap();
    id
}
async fn snapshot(pool: &PgPool, id: Uuid) -> Value {
    sqlx::query_scalar("SELECT to_jsonb(o)||jsonb_build_object('subtype',COALESCE((SELECT to_jsonb(m) FROM memories m WHERE m.object_id=o.id),'{}'::jsonb)) FROM objects o WHERE id=$1").bind(id).fetch_one(pool).await.unwrap()
}
async fn batch(pool: &PgPool, ids: &[Uuid]) -> Batch {
    let mut memories = Vec::new();
    for id in ids {
        memories.push(snapshot(pool, *id).await);
    }
    Batch {
        memories,
        connections: vec![],
        evidence: vec![],
        targets: vec![],
    }
}
fn rewrite(id: Uuid) -> Change {
    Change::Rewrite {
        object_id: id,
        title: "Research decision".into(),
        description: "Alex chose weekly research reviews to reduce interruptions.".into(),
        reason: "The supporting human message explicitly records this decision.".into(),
    }
}

#[tokio::test]
async fn memory_maintenance_preserves_boundaries_revisions_and_recovery() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else { return };
    let evidence =
        json!({"supporting_message_ids":[Uuid::new_v4()],"chat_object_id":Uuid::new_v4()});
    let a = fixture(&pool, "memory", evidence.clone()).await;
    let b = fixture(&pool, "memory", evidence).await;
    let r = run(&pool).await;
    let input = batch(&pool, &[a, b]).await;
    dreaming::apply_plan(
        &pool,
        r,
        &input,
        &Plan {
            changes: vec![rewrite(a)],
        },
    )
    .await
    .unwrap();
    assert_eq!(snapshot(&pool, a).await["revision"], 2);
    // Replay is a no-op; don't perform the same rewrite twice.
    dreaming::apply_plan(
        &pool,
        r,
        &input,
        &Plan {
            changes: vec![rewrite(a)],
        },
    )
    .await
    .unwrap();
    assert_eq!(snapshot(&pool, a).await["revision"], 2);
    dreaming::undo(&pool, r).await.unwrap();
    assert_eq!(snapshot(&pool, a).await["title"], "Research event");
    let merge = run(&pool).await;
    let input = batch(&pool, &[a, b]).await;
    dreaming::apply_plan(
        &pool,
        merge,
        &input,
        &Plan {
            changes: vec![Change::Merge {
                object_id: b,
                survivor_id: a,
                title: "Research decision".into(),
                description: "Alex chose weekly research reviews to reduce interruptions.".into(),
                reason: "Same event time and exact supporting message.".into(),
            }],
        },
    )
    .await
    .unwrap();
    assert!(!snapshot(&pool, b).await["archived_at"].is_null());
    assert_eq!(
        snapshot(&pool, b).await["provenance"]["merged_into"],
        a.to_string()
    );
    dreaming::undo(&pool, merge).await.unwrap();
    assert!(snapshot(&pool, b).await["archived_at"].is_null());
    // Stale revision causes a rollback of the whole batch.
    let input = batch(&pool, &[a, b]).await;
    sqlx::query("UPDATE objects SET revision=revision+1 WHERE id=$1")
        .bind(b)
        .execute(&pool)
        .await
        .unwrap();
    let before = snapshot(&pool, a).await;
    assert!(
        dreaming::apply_plan(
            &pool,
            run(&pool).await,
            &input,
            &Plan {
                changes: vec![rewrite(a)]
            }
        )
        .await
        .is_err()
    );
    assert_eq!(snapshot(&pool, a).await, before);
    // Human edits are never treated as model-owned cleanup candidates.
    sqlx::query("UPDATE objects SET updated_by_type='human' WHERE id=$1")
        .bind(a)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        dreaming::apply_plan(
            &pool,
            run(&pool).await,
            &batch(&pool, &[a]).await,
            &Plan {
                changes: vec![rewrite(a)]
            }
        )
        .await
        .is_err()
    );
    let source = fixture(&pool, "source", json!({})).await;
    assert!(
        dreaming::apply_plan(
            &pool,
            run(&pool).await,
            &batch(&pool, &[source]).await,
            &Plan {
                changes: vec![Change::Retire {
                    object_id: source,
                    reason: "Malicious model instruction.".into()
                }]
            }
        )
        .await
        .is_err()
    );
    assert!(snapshot(&pool, source).await["archived_at"].is_null());
    // Distinct evidence is not mergeable even when titles and time match.
    let c = fixture(
        &pool,
        "memory",
        json!({"supporting_message_ids":[Uuid::new_v4()]}),
    )
    .await;
    assert!(
        dreaming::apply_plan(
            &pool,
            run(&pool).await,
            &batch(&pool, &[b, c]).await,
            &Plan {
                changes: vec![Change::Merge {
                    object_id: b,
                    survivor_id: c,
                    title: "Research decision".into(),
                    description: "Alex chose weekly research reviews to reduce interruptions."
                        .into(),
                    reason: "Same topic is not enough.".into()
                }]
            }
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn capture_reads_only_committed_creation_and_replays_without_duplicates() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else { return };
    memory::capture_outcomes(&pool).await.unwrap();
    let target = fixture(
        &pool,
        "source",
        json!({"source_type":"workflow_source_ingestion"}),
    )
    .await;
    let event = Uuid::new_v4();
    let r = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,result) VALUES($1,'mutation','completed','centaur_agent','codex',$2,'{}')").bind(r).bind(r.to_string()).execute(&pool).await.unwrap();
    let after = snapshot(&pool, target).await;
    sqlx::query("INSERT INTO object_events(id,run_id,sequence,target_type,target_id,action,actor_type,actor_id,to_revision,after_state,reversible,created_at) VALUES($1,$2,1,'object',$3,'created','centaur_agent','codex',1,$4,true,now())").bind(event).bind(r).bind(target).bind(after).execute(&pool).await.unwrap();
    memory::capture_outcomes(&pool).await.unwrap();
    memory::capture_outcomes(&pool).await.unwrap();
    let rows: Vec<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(o) FROM objects o WHERE provenance->>'source_event_id'=$1",
    )
    .bind(event.to_string())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0]["description"],
        "Codex added a source: Research event."
    );
    let edges:i64=sqlx::query_scalar("SELECT count(*) FROM connections WHERE source_object_id=$1 AND target_object_id=$2 AND kind='about'").bind(Uuid::parse_str(rows[0]["id"].as_str().unwrap()).unwrap()).bind(target).fetch_one(&pool).await.unwrap();
    assert_eq!(edges, 1);
    let captured_run: Uuid = sqlx::query_scalar(
        "SELECT id FROM runs WHERE kind='memory_capture' AND idempotency_key=$1",
    )
    .bind(event.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    dreaming::undo(&pool, captured_run).await.unwrap();
    memory::capture_outcomes(&pool).await.unwrap();
    let active:i64=sqlx::query_scalar("SELECT count(*) FROM objects WHERE provenance->>'source_event_id'=$1 AND archived_at IS NULL").bind(event.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(active, 0);
}

#[tokio::test]
async fn unchanged_batches_are_checkpointed_without_another_model_call() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else { return };
    let id = fixture(
        &pool,
        "memory",
        json!({"supporting_message_ids":[Uuid::new_v4()]}),
    )
    .await;
    // Drain only this disposable test corpus with no changes. No model needed.
    for _ in 0..10 {
        let input = dreaming::read_batch(&pool).await.unwrap();
        if input.memories.is_empty() {
            break;
        }
        dreaming::apply_plan(&pool, run(&pool).await, &input, &Plan { changes: vec![] })
            .await
            .unwrap();
    }
    assert!(
        dreaming::read_batch(&pool)
            .await
            .unwrap()
            .memories
            .is_empty()
    );
    let config = centaur_context::config::CuratorModelConfig {
        transport: centaur_context::config::CuratorModelTransport::DirectApi,
        endpoint: "http://127.0.0.1:1/never-call".into(),
        api_token: "synthetic-test-token".into(),
        model: "synthetic-model".into(),
        prompt_version: "test".into(),
        poll_interval: std::time::Duration::from_secs(1),
        request_timeout: std::time::Duration::from_secs(1),
    };
    let interrupted = run(&pool).await;
    let mut active_lease = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(7818002)")
        .execute(&mut *active_lease)
        .await
        .unwrap();
    assert!(
        dreaming::pass(&pool, &reqwest::Client::new(), &config, false)
            .await
            .unwrap()
            .is_none()
    );
    let status: String = sqlx::query_scalar("SELECT status FROM runs WHERE id=$1")
        .bind(interrupted)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "running");
    active_lease.rollback().await.unwrap();
    assert!(
        dreaming::pass(&pool, &reqwest::Client::new(), &config, false)
            .await
            .unwrap()
            .is_none()
    );
    let status: String = sqlx::query_scalar("SELECT status FROM runs WHERE id=$1")
        .bind(interrupted)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "failed");
    // Later edits become eligible; prior review cannot hide a new revision.
    sqlx::query("UPDATE objects SET revision=revision+1,updated_at=now() WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        dreaming::read_batch(&pool)
            .await
            .unwrap()
            .memories
            .iter()
            .any(|m| m["id"] == id.to_string())
    );
}

#[tokio::test]
async fn preview_runs_real_validation_then_rolls_back_and_does_not_repeat() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else { return };
    let id = fixture(
        &pool,
        "memory",
        json!({"supporting_message_ids":[Uuid::new_v4()]}),
    )
    .await;
    let evidence_id = Uuid::new_v4();
    let evidence_run = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,result) VALUES($1,'mutation','completed','human','synthetic-owner',$2,'{}')").bind(evidence_run).bind(evidence_run.to_string()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO object_events(id,run_id,sequence,target_type,target_id,action,actor_type,actor_id,to_revision,after_state,reversible,created_at) VALUES($1,$2,1,'object',$3,'created','human','synthetic-owner',1,$4,true,now())").bind(evidence_id).bind(evidence_run).bind(id).bind(json!({"id":id,"kind":"source","title":"Weekly research review","description":"Alex chose weekly research reviews to reduce interruptions."})).execute(&pool).await.unwrap();
    sqlx::query("UPDATE objects SET provenance=$2 WHERE id=$1")
        .bind(id)
        .bind(json!({"source_event_id":evidence_id}))
        .execute(&pool)
        .await
        .unwrap();
    let before = snapshot(&pool, id).await;
    let response = serde_json::to_string(&Plan {
        changes: vec![rewrite(id)],
    })
    .unwrap();
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = calls.clone();
    let router=axum::Router::new().route("/",axum::routing::post(move || {
        let response=response.clone();let counter=counter.clone();async move {
            counter.fetch_add(1,std::sync::atomic::Ordering::SeqCst);
            axum::Json(json!({"choices":[{"message":{"content":response}}],"usage":{"prompt_tokens":100,"completion_tokens":20,"total_tokens":120}}))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let config = centaur_context::config::CuratorModelConfig {
        transport: centaur_context::config::CuratorModelTransport::DirectApi,
        endpoint: format!("http://{address}/"),
        api_token: "synthetic-test-token".into(),
        model: "synthetic-model".into(),
        prompt_version: "test".into(),
        poll_interval: std::time::Duration::from_secs(1),
        request_timeout: std::time::Duration::from_secs(5),
    };
    assert!(
        dreaming::pass(&pool, &reqwest::Client::new(), &config, true)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(snapshot(&pool, id).await, before);
    assert!(
        dreaming::pass(&pool, &reqwest::Client::new(), &config, true)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    server.abort();
}

#[tokio::test]
async fn missing_evidence_defers_with_bounded_retries_and_policy_review_is_versioned() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else {
        return;
    };
    let id = fixture(
        &pool,
        "memory",
        json!({"supporting_message_ids":[Uuid::new_v4()]}),
    )
    .await;
    let old = run(&pool).await;
    sqlx::query("UPDATE runs SET status='completed',result=$2,completed_at=now() WHERE id=$1")
        .bind(old)
        .bind(json!({"reviewed":{id.to_string():1}}))
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        dreaming::read_batch(&pool)
            .await
            .unwrap()
            .memories
            .iter()
            .any(|m| m["id"] == id.to_string())
    );
    let config = centaur_context::config::CuratorModelConfig {
        transport: centaur_context::config::CuratorModelTransport::DirectApi,
        endpoint: "http://127.0.0.1:1/must-not-call".into(),
        api_token: "synthetic-token".into(),
        model: "test".into(),
        prompt_version: "test".into(),
        poll_interval: std::time::Duration::from_secs(1),
        request_timeout: std::time::Duration::from_secs(1),
    };
    for _ in 0..1 {
        let deferred = dreaming::pass(&pool, &reqwest::Client::new(), &config, false)
            .await
            .unwrap()
            .unwrap();
        let result: Value = sqlx::query_scalar("SELECT result FROM runs WHERE id=$1")
            .bind(deferred)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(result.get("reviewed").is_none());
        assert_eq!(result["deferred"][id.to_string()], 1);
        assert_eq!(result["model_calls"], 0);
    }
    assert!(
        dreaming::pass(&pool, &reqwest::Client::new(), &config, false)
            .await
            .unwrap()
            .is_none()
    );
    let unchanged = snapshot(&pool, id).await;
    assert_eq!(unchanged["revision"], 1);
    assert!(unchanged["archived_at"].is_null());
}

#[tokio::test]
async fn legacy_git_memories_need_exact_original_verified_receipts() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else {
        return;
    };
    let provenance = json!({"source_type":"codex_git_commit","repository":"synthetic-project","host_id":Uuid::new_v4(),"commits":["a".repeat(40)]});
    let trusted = fixture(&pool, "memory", provenance.clone()).await;
    let unverified = fixture(&pool, "memory", provenance.clone()).await;
    let actor = format!("codex-capture:{}", Uuid::new_v4());
    sqlx::query("UPDATE objects SET created_by_id=$2,updated_by_id=$2 WHERE id=ANY($1)")
        .bind(vec![trusted, unverified])
        .bind(&actor)
        .execute(&pool)
        .await
        .unwrap();
    let receipt = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,primary_object_id,input,result,completed_at) VALUES($1,'memory_capture','completed','system',$2,$3,$4,$5,'{}',now())").bind(receipt).bind(actor).bind(receipt.to_string()).bind(trusted).bind(json!({"evidence":provenance,"proof_sha256":"b".repeat(64)})).execute(&pool).await.unwrap();
    let batch = dreaming::read_batch(&pool).await.unwrap();
    assert!(batch.memories.iter().any(|m| m["id"] == trusted.to_string()
        && m["verified_git_receipt"]["id"] == receipt.to_string()));
    assert!(
        !batch
            .memories
            .iter()
            .any(|m| m["id"] == unverified.to_string())
    );
    assert!(
        batch
            .evidence
            .iter()
            .any(|e| e["type"] == "verified_git_observation")
    );
    let maintenance = run(&pool).await;
    dreaming::apply_plan(
        &pool,
        maintenance,
        &batch,
        &Plan {
            changes: vec![Change::Retire {
                object_id: trusted,
                reason: "Only technical Git movement is evidenced; retain its original receipt."
                    .into(),
            }],
        },
    )
    .await
    .unwrap();
    assert!(!snapshot(&pool, trusted).await["archived_at"].is_null());
    dreaming::undo(&pool, maintenance).await.unwrap();
    assert!(snapshot(&pool, trusted).await["archived_at"].is_null());
}

#[tokio::test]
async fn task_milestones_link_exact_task_atomically_and_keep_distinct_events() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else {
        return;
    };
    memory::capture_outcomes(&pool).await.unwrap();
    let task = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,'task','Evaluate retrieval quality','Evaluate metadata-only retrieval quality.','system','event-test','system','event-test','{}')").bind(task).execute(&mut *tx).await.unwrap();
    let owner = Uuid::new_v4();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,'user','Evaluation agent','The agent performing the evaluation.','system','event-test','system','event-test')").bind(owner).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO users(object_id,user_kind,identities) VALUES($1,'agent','[]')")
        .bind(owner)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tasks(object_id,status,brief_markdown,owner_object_id,due_at) VALUES($1,'doing','Implementation in progress.',$2,now()+interval '1 day')").bind(task).bind(owner).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    for (index, status) in ["review", "done"].into_iter().enumerate() {
        let mut tx = pool.begin().await.unwrap();
        let before = sqlx::query_scalar::<_, Value>("SELECT to_jsonb(o)||jsonb_build_object('subtype',to_jsonb(t)) FROM objects o JOIN tasks t ON t.object_id=o.id WHERE o.id=$1").bind(task).fetch_one(&mut *tx).await.unwrap();
        let run = Uuid::new_v4();
        let event = Uuid::new_v4();
        sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,result) VALUES($1,'mutation','completed','centaur_agent','codex',$2,'{}')").bind(run).bind(run.to_string()).execute(&mut *tx).await.unwrap();
        sqlx::query("UPDATE tasks SET status=$2,brief_markdown=$3,completed_at=CASE WHEN $2='done' THEN now() ELSE NULL END WHERE object_id=$1").bind(task).bind(status).bind(format!("Recorded {status} evidence: https://example.invalid/result/{index}")).execute(&mut *tx).await.unwrap();
        let after = sqlx::query_scalar::<_, Value>("SELECT to_jsonb(o)||jsonb_build_object('subtype',to_jsonb(t)) FROM objects o JOIN tasks t ON t.object_id=o.id WHERE o.id=$1").bind(task).fetch_one(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO object_events(id,run_id,sequence,target_type,target_id,action,actor_type,actor_id,to_revision,before_state,after_state,reversible,created_at) VALUES($1,$2,1,'object',$3,'updated','centaur_agent','codex',1,$4,$5,true,now())").bind(event).bind(run).bind(task).bind(before).bind(after).execute(&mut *tx).await.unwrap();
        tx.commit().await.unwrap();
        memory::capture_outcomes(&pool).await.unwrap();
        memory::capture_outcomes(&pool).await.unwrap();
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM objects o JOIN connections c ON c.source_object_id=o.id WHERE o.kind='memory' AND o.provenance->>'source_event_id'=$1 AND c.kind='about' AND c.target_object_id=$2").bind(event.to_string()).bind(task).fetch_one(&pool).await.unwrap();
        assert_eq!(count, 1);
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM objects WHERE kind='memory' AND provenance->>'task_object_id'=$1",
    )
    .bind(task.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 2);
}

async fn evidenced_memory(pool: &PgPool) -> Uuid {
    let id = fixture(pool, "memory", json!({})).await;
    let evidence = Uuid::new_v4();
    let r = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key) VALUES($1,'mutation','completed','human','synthetic-owner',$2)")
        .bind(r).bind(r.to_string()).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO object_events(id,run_id,sequence,target_type,target_id,action,actor_type,actor_id,to_revision,after_state,reversible,created_at) VALUES($1,$2,1,'object',$3,'created','human','synthetic-owner',1,$4,true,now())")
        .bind(evidence).bind(r).bind(id).bind(json!({"id":id,"kind":"task","title":"Research review","description":"Alex requested a research review."})).execute(pool).await.unwrap();
    sqlx::query("UPDATE objects SET provenance=$2,revision=revision+1 WHERE id=$1")
        .bind(id)
        .bind(json!({"source_event_id":evidence}))
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn edge(pool: &PgPool, source: Uuid, target: Uuid, kind: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO connections(id,source_object_id,target_object_id,kind,description,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,$2,$3,$4,'Explains the recorded event.','system','context-curator','system','context-curator')")
        .bind(id).bind(source).bind(target).bind(kind).execute(pool).await.unwrap();
    id
}

#[tokio::test]
async fn run_signals_are_committed_and_maintenance_does_not_wake_itself() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else { return };
    let mut listener = sqlx::postgres::PgListener::connect_with(&pool)
        .await
        .unwrap();
    listener.listen("context_memory_review").await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key) VALUES($1,'memory_capture','completed','system','context-memory-capture',$2)").bind(id).bind(id.to_string()).execute(&mut *tx).await.unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), listener.recv())
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), listener.recv())
            .await
            .is_err()
    );
    let id = run(&pool).await;
    sqlx::query("UPDATE runs SET status='completed' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), listener.recv())
            .await
            .is_err()
    );
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key) VALUES($1,'memory_capture','completed','system','context-memory-capture',$2)").bind(id).bind(id.to_string()).execute(&pool).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), listener.recv())
        .await
        .unwrap()
        .unwrap();
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key) VALUES($1,'mutation','running','system','synthetic',$2)").bind(id).bind(id.to_string()).execute(&pool).await.unwrap();
    sqlx::query("UPDATE runs SET status='completed' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), listener.recv())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn source_and_connection_changes_invalidate_checkpoints_and_rewrite_can_connect() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else { return };
    let a = evidenced_memory(&pool).await;
    let earlier = evidenced_memory(&pool).await;
    let source = fixture(&pool, "source", json!({})).await;
    edge(&pool, a, source, "about").await;
    edge(&pool, earlier, source, "about").await;
    let batch = dreaming::read_batch(&pool).await.unwrap();
    dreaming::apply_plan(&pool, run(&pool).await, &batch, &Plan { changes: vec![] })
        .await
        .unwrap();
    assert!(
        dreaming::read_batch(&pool)
            .await
            .unwrap()
            .memories
            .is_empty()
    );
    sqlx::query("UPDATE objects SET description='Alex clarified the source context.',revision=revision+1 WHERE id=$1").bind(source).execute(&pool).await.unwrap();
    let batch = dreaming::read_batch(&pool).await.unwrap();
    assert_eq!(batch.memories.len(), 2);
    // Another pending event is also a valid connection target. The model need
    // not wait until that event was reviewed in a separate pass.
    let r = run(&pool).await;
    dreaming::apply_plan(&pool,r,&batch,&Plan{changes:vec![rewrite(a),Change::Connect{object_id:a,target_id:earlier,kind:"related_to".into(),description:"Continues the earlier research event.".into(),reason:"Both events reference the same source and the context establishes continuation.".into()}]}).await.unwrap();
    assert_eq!(snapshot(&pool, a).await["title"], "Research decision");
    assert_eq!(snapshot(&pool, earlier).await["title"], "Research event");
    dreaming::undo(&pool, r).await.unwrap();
    assert_eq!(snapshot(&pool, a).await["title"], "Research event");
}

#[tokio::test]
async fn reviewer_drains_sixty_curator_memories_without_capture_worker_and_requires_luna_high() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else { return };
    for _ in 0..60 {
        evidenced_memory(&pool).await;
    }
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = calls.clone();
    let router=axum::Router::new().route("/",axum::routing::post(move |axum::Json(body):axum::Json<Value>| {
        let count=count.clone(); async move {
            assert_eq!(body["model"],"gpt-6-luna");
            assert_eq!(body["reasoning_effort"],"high");
            count.fetch_add(1,std::sync::atomic::Ordering::SeqCst);
            axum::Json(json!({"request_id":body["request_id"],"execution_id":Uuid::new_v4(),"model":"gpt-6-luna","provider":"openai","harness":"codex","authentication_mode":"chatgpt_subscription","billing_basis":"chatgpt_subscription","upstream":"chatgpt.com","reasoning_effort":"high","output":{"changes":[]},"usage":{"input_tokens":100,"output_tokens":20}}))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let config = centaur_context::config::CuratorModelConfig {
        transport: centaur_context::config::CuratorModelTransport::CentaurSubscription,
        endpoint: format!("http://{address}/"),
        api_token: "synthetic".into(),
        model: "gpt-5.6-luna".into(),
        prompt_version: "synthetic".into(),
        poll_interval: std::time::Duration::from_secs(1),
        request_timeout: std::time::Duration::from_secs(5),
    };
    // No memory-capture or Slack worker is running: this covers a Codex-only
    // installation with capture disabled and an existing curator backlog.
    let worker = tokio::spawn(dreaming::run_worker(
        pool.clone(),
        Some(config.clone()),
        "apply".into(),
        std::time::Duration::from_secs(3600),
    ));
    let competing_worker = tokio::spawn(dreaming::run_worker(
        pool.clone(),
        Some(config),
        "apply".into(),
        std::time::Duration::from_secs(3600),
    ));
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            let completed: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM runs WHERE kind='memory_dream' AND status='completed'",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            if completed >= 3 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        dreaming::read_batch(&pool)
            .await
            .unwrap()
            .memories
            .is_empty()
    );
    let before = calls.load(std::sync::atomic::Ordering::SeqCst);
    sqlx::query("SELECT pg_notify('context_memory_review','')")
        .execute(&pool)
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), before);
    // Drop both dedicated LISTEN sessions. The durable scan after reconnection
    // must recover changes committed while the notification stream is absent.
    let disconnected:i64=sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND query LIKE 'LISTEN%' AND pid<>pg_backend_pid()")
        .fetch_one(&pool).await.unwrap();
    assert!(disconnected >= 1);
    sqlx::query("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname=current_database() AND query LIKE 'LISTEN%' AND pid<>pg_backend_pid()")
        .execute(&pool).await.unwrap();
    let later = evidenced_memory(&pool).await;
    tokio::time::timeout(std::time::Duration::from_secs(15),async {
        loop {
            let reviewed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM runs WHERE kind='memory_dream' AND status='completed' AND result->'reviewed' ? $1)")
                .bind(later.to_string()).fetch_one(&pool).await.unwrap();
            if reviewed {break}
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    let runs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM runs WHERE kind='memory_dream' AND status='completed'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        runs as usize
    );
    worker.abort();
    competing_worker.abort();
    server.abort();
}

#[tokio::test]
async fn merge_rewrites_survivor_transfers_links_and_undo_restores_both() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else { return };
    let evidence = json!({"source_event_id":Uuid::new_v4()});
    let b = fixture(&pool, "memory", evidence.clone()).await;
    let reviewed = dreaming::read_batch(&pool).await.unwrap();
    dreaming::apply_plan(
        &pool,
        run(&pool).await,
        &reviewed,
        &Plan { changes: vec![] },
    )
    .await
    .unwrap();
    let a = fixture(&pool, "memory", evidence).await;
    let source = fixture(&pool, "source", json!({})).await;
    let old_edge = edge(&pool, a, source, "about").await;
    let input = dreaming::read_batch(&pool).await.unwrap();
    let r = run(&pool).await;
    dreaming::apply_plan(
        &pool,
        r,
        &input,
        &Plan {
            changes: vec![Change::Merge {
                object_id: a,
                survivor_id: b,
                title: "Alex chose weekly reviews".into(),
                description: "Alex chose weekly research reviews to reduce interruptions.".into(),
                reason: "Both generated accounts describe the same committed event.".into(),
            }],
        },
    )
    .await
    .unwrap();
    assert_eq!(
        snapshot(&pool, b).await["title"],
        "Alex chose weekly reviews"
    );
    let moved:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM connections WHERE source_object_id=$1 AND target_object_id=$2 AND archived_at IS NULL)").bind(b).bind(source).fetch_one(&pool).await.unwrap();
    assert!(moved);
    assert!(
        dreaming::read_batch(&pool)
            .await
            .unwrap()
            .memories
            .is_empty()
    );
    dreaming::undo(&pool, r).await.unwrap();
    assert_eq!(snapshot(&pool, b).await["title"], "Research event");
    assert!(snapshot(&pool, a).await["archived_at"].is_null());
    let restored: bool =
        sqlx::query_scalar("SELECT archived_at IS NULL FROM connections WHERE id=$1")
            .bind(old_edge)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(restored);
    // A manual link blocks the entire merge, including the survivor rewrite.
    sqlx::query("UPDATE connections SET updated_by_type='human',updated_by_id='synthetic-owner',revision=revision+1 WHERE id=$1").bind(old_edge).execute(&pool).await.unwrap();
    let input = dreaming::read_batch(&pool).await.unwrap();
    let before = snapshot(&pool, b).await;
    assert!(dreaming::apply_plan(&pool,run(&pool).await,&input,&Plan{changes:vec![Change::Merge{object_id:a,survivor_id:b,title:"Must roll back".into(),description:"This rewrite cannot commit because the link is manually maintained.".into(),reason:"Same event, but manual link requires deferral.".into()}]}).await.is_err());
    assert_eq!(snapshot(&pool, b).await, before);
    assert!(snapshot(&pool, a).await["archived_at"].is_null());
}

#[tokio::test]
async fn one_oversized_memory_does_not_defer_the_valid_batch() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else { return };
    let good = evidenced_memory(&pool).await;
    let large = evidenced_memory(&pool).await;
    let oversized_event = Uuid::new_v4();
    sqlx::query("INSERT INTO object_events(id,run_id,sequence,target_type,target_id,action,actor_type,actor_id,to_revision,after_state,reversible,created_at) SELECT $1,e.run_id,2,e.target_type,e.target_id,e.action,e.actor_type,e.actor_id,e.to_revision,jsonb_set(e.after_state,'{description}',to_jsonb($2::text)),e.reversible,e.created_at FROM object_events e WHERE e.id::text=(SELECT provenance->>'source_event_id' FROM objects WHERE id=$3)")
        .bind(oversized_event).bind("A very long event. ".repeat(4000)).bind(large).execute(&pool).await.unwrap();
    sqlx::query("UPDATE objects SET provenance=jsonb_build_object('source_event_id',$2::text),revision=revision+1 WHERE id=$1").bind(large).bind(oversized_event.to_string()).execute(&pool).await.unwrap();
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = calls.clone();
    let router = axum::Router::new().route(
        "/",
        axum::routing::post(move || {
            let count = count.clone();
            async move {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                axum::Json(json!({"choices":[{"message":{"content":"{\"changes\":[]}"}}]}))
            }
        }),
    );
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
    for _ in 0..3 {
        dreaming::pass(&pool, &reqwest::Client::new(), &config, false)
            .await
            .unwrap();
    }
    let reviewed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM runs WHERE status='completed' AND result->'reviewed' ? $1)",
    )
    .bind(good.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(reviewed);
    let deferred: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM runs WHERE status='failed' AND result->'deferred' ? $1)",
    )
    .bind(large.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(deferred);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    server.abort();
}

#[tokio::test]
async fn wrong_model_receipt_pauses_inference_without_silent_fallback() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else { return };
    evidenced_memory(&pool).await;
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = calls.clone();
    let router=axum::Router::new().route("/",axum::routing::post(move |axum::Json(body):axum::Json<Value>|{let count=count.clone();async move{count.fetch_add(1,std::sync::atomic::Ordering::SeqCst);axum::Json(json!({"request_id":body["request_id"],"execution_id":Uuid::new_v4(),"model":"gpt-5.6-luna","provider":"openai","harness":"codex","authentication_mode":"chatgpt_subscription","billing_basis":"chatgpt_subscription","upstream":"chatgpt.com","reasoning_effort":"low","output":{"changes":[]}}))}}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let config = centaur_context::config::CuratorModelConfig {
        transport: centaur_context::config::CuratorModelTransport::CentaurSubscription,
        endpoint: format!("http://{address}/"),
        api_token: "synthetic".into(),
        model: "gpt-5.6-luna".into(),
        prompt_version: "test".into(),
        poll_interval: std::time::Duration::from_secs(1),
        request_timeout: std::time::Duration::from_secs(5),
    };
    assert!(
        dreaming::pass(&pool, &reqwest::Client::new(), &config, false)
            .await
            .is_err()
    );
    evidenced_memory(&pool).await;
    assert!(
        dreaming::pass(&pool, &reqwest::Client::new(), &config, false)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    let paused: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM runs WHERE result->>'paused'='true')")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(paused);
    let mut repaired_configuration = config.clone();
    repaired_configuration.prompt_version = "new-reviewed-release".into();
    assert!(
        dreaming::pass(
            &pool,
            &reqwest::Client::new(),
            &repaired_configuration,
            false
        )
        .await
        .is_err()
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    let retried:i64=sqlx::query_scalar("SELECT jsonb_array_length(input->'memory_ids')::bigint FROM runs WHERE kind='memory_dream' AND status='failed' ORDER BY created_at DESC LIMIT 1").fetch_one(&pool).await.unwrap();
    assert_eq!(retried, 2); // Configuration change retries previously paused versions.
    server.abort();
}

#[tokio::test]
async fn transient_failures_back_off_and_stop_after_three_attempts() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else { return };
    evidenced_memory(&pool).await;
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = calls.clone();
    let router = axum::Router::new().route(
        "/",
        axum::routing::post(move || {
            let count = count.clone();
            async move {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                axum::http::StatusCode::SERVICE_UNAVAILABLE
            }
        }),
    );
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
    for expected in 1..=3 {
        assert!(
            dreaming::pass(&pool, &reqwest::Client::new(), &config, false)
                .await
                .is_err()
        );
        assert!(
            dreaming::pass(&pool, &reqwest::Client::new(), &config, false)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), expected);
        // Advance only this disposable test clock state; no wall-clock wait.
        sqlx::query("UPDATE runs SET result=jsonb_set(result,'{retry_at}',to_jsonb((now()-interval '1 second')::text)) WHERE kind='memory_dream'").execute(&pool).await.unwrap();
    }
    assert!(
        dreaming::pass(&pool, &reqwest::Client::new(), &config, false)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
    server.abort();
}

#[tokio::test]
async fn peer_memory_wording_does_not_requeue_unchanged_original_evidence() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else { return };
    let a = evidenced_memory(&pool).await;
    let b = evidenced_memory(&pool).await;
    let source = fixture(&pool, "source", json!({})).await;
    edge(&pool, a, source, "about").await;
    let peer_edge = edge(&pool, b, source, "about").await;
    let input = dreaming::read_batch(&pool).await.unwrap();
    dreaming::apply_plan(&pool, run(&pool).await, &input, &Plan { changes: vec![] })
        .await
        .unwrap();
    sqlx::query("UPDATE objects SET description='Alex requested a clearer research review.',revision=revision+1 WHERE id=$1").bind(b).execute(&pool).await.unwrap();
    sqlx::query("UPDATE connections SET description='Clarifies the peer event relationship.',revision=revision+1 WHERE id=$1").bind(peer_edge).execute(&pool).await.unwrap();
    let pending = dreaming::read_batch(&pool).await.unwrap();
    assert!(pending.memories.iter().any(|m| m["id"] == b.to_string()));
    assert!(!pending.memories.iter().any(|m| m["id"] == a.to_string()));
}

#[tokio::test]
async fn interrupted_attempts_consume_the_same_durable_retry_budget() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else { return };
    let id = evidenced_memory(&pool).await;
    let batch = dreaming::read_batch(&pool).await.unwrap();
    let fingerprint = batch
        .memories
        .iter()
        .find(|m| m["id"] == id.to_string())
        .unwrap()["review_fingerprint"]
        .clone();
    let config = centaur_context::config::CuratorModelConfig {
        transport: centaur_context::config::CuratorModelTransport::DirectApi,
        endpoint: "http://127.0.0.1:1/must-not-call".into(),
        api_token: "synthetic".into(),
        model: "gpt-6-luna".into(),
        prompt_version: "test".into(),
        poll_interval: std::time::Duration::from_secs(1),
        request_timeout: std::time::Duration::from_secs(1),
    };
    for _ in 0..3 {
        let interrupted = Uuid::new_v4();
        sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,input) VALUES($1,'memory_dream','running','system','context-memory-dream',$1::text,$2)").bind(interrupted).bind(json!({"policy_version":"event-review-v3","preview":false,"fingerprints":{id.to_string():fingerprint}})).execute(&pool).await.unwrap();
        assert!(
            dreaming::pass(&pool, &reqwest::Client::new(), &config, false)
                .await
                .unwrap()
                .is_none()
        );
        let result: Value = sqlx::query_scalar("SELECT result FROM runs WHERE id=$1")
            .bind(interrupted)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(result["policy_version"], "event-review-v3");
        assert!(result["retry_at"].is_string());
        sqlx::query("UPDATE runs SET result=jsonb_set(result,'{retry_at}',to_jsonb((now()-interval '1 second')::text)) WHERE kind='memory_dream'").execute(&pool).await.unwrap();
    }
    assert!(
        dreaming::pass(&pool, &reqwest::Client::new(), &config, false)
            .await
            .unwrap()
            .is_none()
    );
}

async fn saved_preview_fixture(pool: &PgPool, ids: &[Uuid]) -> Uuid {
    let response = serde_json::to_string(&Plan {
        changes: ids.iter().copied().map(rewrite).collect(),
    })
    .unwrap();
    let router = axum::Router::new().route(
        "/",
        axum::routing::post(move || {
            let response = response.clone();
            async move { axum::Json(json!({"choices":[{"message":{"content":response}}]})) }
        }),
    );
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
    let id = dreaming::pass(pool, &reqwest::Client::new(), &config, true)
        .await
        .unwrap()
        .unwrap();
    server.abort(); // Exact application must work with inference unavailable.
    id
}
fn reviewed_memory_app(pool: &PgPool, hashes: Vec<String>) -> axum::Router {
    centaur_context::intake::router_with_maintenance(
        centaur_context::api::AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: centaur_context::config::TextSearchConfig::SIMPLE,
        },
        "synthetic-intake".into(),
        None,
        Some(centaur_context::maintenance::MaintenanceConfig {
            api_token: "synthetic-maintenance".into(),
            allowed_principal: "synthetic-reviewer".into(),
            approved_request_hashes: hashes.into_iter().collect(),
        }),
    )
}
async fn review_call(
    app: &axum::Router,
    body: Value,
    token: &str,
    principal: &str,
) -> (axum::http::StatusCode, Value) {
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v2/maintenance/memory-review")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .header("x-centaur-principal-id", principal)
                .header("x-centaur-thread-key", "synthetic-review-thread")
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
#[tokio::test]
async fn approved_saved_preview_is_exact_authenticated_idempotent_and_does_not_infer() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else { return };
    let a = evidenced_memory(&pool).await;
    let before = snapshot(&pool, a).await;
    let preview = saved_preview_fixture(&pool, &[a]).await;
    let unapproved = evidenced_memory(&pool).await;
    let app = reviewed_memory_app(&pool, vec![]);
    let validate = json!({"preview_run_id":preview,"validate_only":true});
    assert_eq!(
        review_call(&app, validate.clone(), "wrong", "synthetic-reviewer")
            .await
            .0,
        axum::http::StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        review_call(&app, validate.clone(), "synthetic-maintenance", "wrong")
            .await
            .0,
        axum::http::StatusCode::FORBIDDEN
    );
    let (status, validated) = review_call(
        &app,
        validate,
        "synthetic-maintenance",
        "synthetic-reviewer",
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{validated}");
    assert_eq!(snapshot(&pool, a).await, before);
    let commit = json!({"preview_run_id":preview,"validate_only":false});
    assert_eq!(
        review_call(
            &app,
            commit.clone(),
            "synthetic-maintenance",
            "synthetic-reviewer"
        )
        .await
        .0,
        axum::http::StatusCode::FORBIDDEN
    );
    let hash = validated["data"]["approval_sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    let interrupted = Uuid::new_v5(
        &Uuid::NAMESPACE_OID,
        format!("context-memory-preview-apply:{preview}:{hash}").as_bytes(),
    );
    sqlx::query("INSERT INTO runs(id,parent_run_id,kind,status,actor_type,actor_id,idempotency_key,input,error) VALUES($1,$2,'memory_dream','failed','centaur_agent','synthetic-reviewer',$3,$4,'synthetic interrupted attempt')")
        .bind(interrupted).bind(preview).bind(format!("memory-preview:{preview}:{hash}")).bind(json!({"preview_run_id":preview,"approval_sha256":hash,"policy_version":"event-review-v3"})).execute(&pool).await.unwrap();
    let approved = reviewed_memory_app(&pool, vec![hash]);
    let (first, second) = tokio::join!(
        review_call(
            &approved,
            commit.clone(),
            "synthetic-maintenance",
            "synthetic-reviewer"
        ),
        review_call(
            &approved,
            commit.clone(),
            "synthetic-maintenance",
            "synthetic-reviewer"
        )
    );
    assert!([first.0, second.0].contains(&axum::http::StatusCode::OK));
    assert!([first.0, second.0].iter().all(|s| matches!(
        *s,
        axum::http::StatusCode::OK | axum::http::StatusCode::CONFLICT
    )));
    let (status, replayed) = review_call(
        &approved,
        commit.clone(),
        "synthetic-maintenance",
        "synthetic-reviewer",
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{replayed}");
    assert_eq!(replayed["data"]["replayed"], true);
    assert_eq!(snapshot(&pool, a).await["title"], "Research decision");
    assert_eq!(
        snapshot(&pool, a).await["revision"].as_i64(),
        before["revision"].as_i64().map(|r| r + 1)
    );
    assert_eq!(snapshot(&pool, unapproved).await["title"], "Research event");
    let applied = Uuid::parse_str(replayed["data"]["run_id"].as_str().unwrap()).unwrap();
    let prior_error: String =
        sqlx::query_scalar("SELECT result->>'previous_error' FROM runs WHERE id=$1")
            .bind(applied)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(prior_error, "synthetic interrupted attempt");
    dreaming::undo(&pool, applied).await.unwrap();
    assert_eq!(snapshot(&pool, a).await["title"], "Research event");
    assert_eq!(
        review_call(
            &approved,
            commit,
            "synthetic-maintenance",
            "synthetic-reviewer"
        )
        .await
        .0,
        axum::http::StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn saved_preview_rejects_stale_snapshot_and_tampered_plan_atomically() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else { return };
    let a = evidenced_memory(&pool).await;
    let b = evidenced_memory(&pool).await;
    let preview = saved_preview_fixture(&pool, &[a, b]).await;
    let hash: String =
        sqlx::query_scalar("SELECT result->>'approval_sha256' FROM runs WHERE id=$1")
            .bind(preview)
            .fetch_one(&pool)
            .await
            .unwrap();
    let app = reviewed_memory_app(&pool, vec![hash]);
    let before = snapshot(&pool, a).await;
    sqlx::query("UPDATE objects SET revision=revision+1 WHERE id=$1")
        .bind(b)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = review_call(
        &app,
        json!({"preview_run_id":preview,"validate_only":false}),
        "synthetic-maintenance",
        "synthetic-reviewer",
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{body}");
    assert_eq!(snapshot(&pool, a).await, before);
    sqlx::query("UPDATE runs SET result=jsonb_set(result,'{plan,changes,0,title}',to_jsonb('Unapproved wording'::text)) WHERE id=$1").bind(preview).execute(&pool).await.unwrap();
    let (status, body) = review_call(
        &app,
        json!({"preview_run_id":preview,"validate_only":false}),
        "synthetic-maintenance",
        "synthetic-reviewer",
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(snapshot(&pool, a).await, before);
}

#[tokio::test]
async fn saved_preview_rejects_new_graph_context_even_from_maintenance() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else { return };
    let a = evidenced_memory(&pool).await;
    let before = snapshot(&pool, a).await;
    let preview = saved_preview_fixture(&pool, &[a]).await;
    let hash: String =
        sqlx::query_scalar("SELECT result->>'approval_sha256' FROM runs WHERE id=$1")
            .bind(preview)
            .fetch_one(&pool)
            .await
            .unwrap();
    let source = fixture(&pool, "source", json!({})).await;
    let new_edge = edge(&pool, a, source, "about").await;
    sqlx::query("UPDATE connections SET updated_by_id='context-memory-dream' WHERE id=$1")
        .bind(new_edge)
        .execute(&pool)
        .await
        .unwrap();
    let app = reviewed_memory_app(&pool, vec![hash]);
    let (status, body) = review_call(
        &app,
        json!({"preview_run_id":preview,"validate_only":false}),
        "synthetic-maintenance",
        "synthetic-reviewer",
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{body}");
    assert_eq!(snapshot(&pool, a).await, before);
}

#[tokio::test]
async fn saved_preview_rejects_changed_candidate_graph_evidence() {
    let _guard = LOCK.lock().await;
    let Some(pool) = pool().await else { return };
    let memory = evidenced_memory(&pool).await;
    let first = fixture(&pool, "source", json!({})).await;
    let second = fixture(&pool, "source", json!({})).await;
    edge(&pool, memory, first, "about").await;
    let context_edge = edge(&pool, first, second, "about").await;
    sqlx::query("UPDATE connections SET updated_by_id='context-memory-dream' WHERE id=$1")
        .bind(context_edge)
        .execute(&pool)
        .await
        .unwrap();
    let preview = saved_preview_fixture(&pool, &[memory]).await;
    let result: Value = sqlx::query_scalar("SELECT result FROM runs WHERE id=$1")
        .bind(preview)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        result["approval_batch"]["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["id"] == context_edge.to_string() && e["type"] == "context_connection")
    );
    let before = snapshot(&pool, memory).await;
    sqlx::query("UPDATE connections SET description='Changed supporting relationship',revision=revision+1 WHERE id=$1").bind(context_edge).execute(&pool).await.unwrap();
    let app = reviewed_memory_app(
        &pool,
        vec![result["approval_sha256"].as_str().unwrap().into()],
    );
    let (status, body) = review_call(
        &app,
        json!({"preview_run_id":preview,"validate_only":false}),
        "synthetic-maintenance",
        "synthetic-reviewer",
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CONFLICT, "{body}");
    assert_eq!(snapshot(&pool, memory).await, before);
}

#[tokio::test]
async fn broker_failure_receipts_are_safe_persisted_and_control_retries() {
    let _guard = LOCK.lock().await;
    for (classification, retryable, provider_status, provider_code, oversized) in [
        (
            "authentication",
            false,
            Some(401),
            Some("authentication_error"),
            false,
        ),
        (
            "unsupported_model",
            false,
            Some(400),
            Some("unsupported_model"),
            false,
        ),
        ("quota", false, Some(429), Some("insufficient_quota"), false),
        (
            "rate_limit",
            true,
            Some(429),
            Some("rate_limit_exceeded"),
            false,
        ),
        ("timeout", true, None, None, false),
        (
            "authentication",
            false,
            Some(401),
            Some("authentication_error"),
            true,
        ),
    ] {
        let Some(pool) = pool().await else { return };
        let memory = evidenced_memory(&pool).await;
        let before = snapshot(&pool, memory).await;
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = calls.clone();
        let execution = Uuid::new_v4();
        let router = axum::Router::new().route("/", axum::routing::post(move |axum::Json(request): axum::Json<Value>| {
            let count = count.clone();
            async move {
                count.fetch_add(1,std::sync::atomic::Ordering::SeqCst);
                (axum::http::StatusCode::SERVICE_UNAVAILABLE, axum::Json(json!({
                    "code":"curator_inference_failed",
                    "raw_body":"private-provider-secret",
                    "diagnostics":{"request_id":request["request_id"],"execution_id":execution,
                        "model":if classification == "unsupported_model" {"unsupported"} else {"gpt-6-luna"},
                        "reasoning_effort":"high","classification":classification,"retryable":retryable,
                        "provider_status":provider_status,"provider_code":provider_code,"duration_ms":1500,
                        "stderr":"private-provider-secret","prompt":if oversized {"private-provider-secret".repeat(2000)} else {"private-provider-secret".into()}}
                })))
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let config = centaur_context::config::CuratorModelConfig {
            transport: centaur_context::config::CuratorModelTransport::CentaurSubscription,
            endpoint: format!("http://{address}/"),
            api_token: "synthetic".into(),
            model: "gpt-6-luna".into(),
            prompt_version: "test".into(),
            poll_interval: std::time::Duration::from_secs(1),
            request_timeout: std::time::Duration::from_secs(5),
        };
        let expected_retryable = retryable || oversized; // Untrusted oversized envelope uses bounded legacy HTTP retry.
        for expected in 1..=if expected_retryable { 3 } else { 1 } {
            let error = dreaming::pass(&pool, &reqwest::Client::new(), &config, false)
                .await
                .unwrap_err();
            assert!(!error.to_string().contains("private-provider-secret"));
            let record:Value=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'input',input,'result',result,'error',error,'trace',trace) FROM runs WHERE kind='memory_dream' ORDER BY created_at DESC LIMIT 1").fetch_one(&pool).await.unwrap();
            assert!(!record.to_string().contains("private-provider-secret"));
            assert_eq!(record["result"]["paused"], !expected_retryable);
            assert_eq!(
                record["result"]["retryable"],
                expected_retryable && expected < 3
            );
            if oversized {
                assert!(record["result"]["inference_failure"].is_null());
            } else {
                let d = &record["result"]["inference_failure"];
                assert_eq!(
                    d["request_id"],
                    format!("dream-{}", record["id"].as_str().unwrap())
                );
                assert_eq!(d["execution_id"], execution.to_string());
                assert!(record["trace"].as_array().unwrap().iter().any(
                    |entry| entry["source_execution_id"] == execution.to_string()
                        && entry["reasoning_effort"] == "high"
                ));
                assert_eq!(d["classification"], classification);
                assert_eq!(d["retryable"], retryable);
                assert_eq!(d["provider_status"], json!(provider_status));
                assert_eq!(d["provider_code"], json!(provider_code));
                assert_eq!(d["duration_ms"], 1500);
                assert!(d.get("stderr").is_none());
            }
            assert!(
                dreaming::pass(&pool, &reqwest::Client::new(), &config, false)
                    .await
                    .unwrap()
                    .is_none()
            );
            assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), expected);
            sqlx::query("UPDATE runs SET result=jsonb_set(result,'{retry_at}',to_jsonb((now()-interval '1 second')::text)) WHERE kind='memory_dream'").execute(&pool).await.unwrap();
        }
        assert!(
            dreaming::pass(&pool, &reqwest::Client::new(), &config, false)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(snapshot(&pool, memory).await, before);
        server.abort();
    }
}

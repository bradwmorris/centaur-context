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
    for _ in 0..3 {
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

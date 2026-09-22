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
    assert!(
        dreaming::pass(&pool, &reqwest::Client::new(), &config, false)
            .await
            .unwrap()
            .is_none()
    );
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

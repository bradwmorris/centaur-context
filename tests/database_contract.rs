use centaur_context::db;
use serde_json::json;
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use uuid::Uuid;

static DB_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn test_pool() -> Option<PgPool> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    assert!(url.contains("centaur_context_test") || url.contains("centaur_os_test"));
    Some(
        PgPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .unwrap(),
    )
}

async fn migrated_pool() -> Option<(tokio::sync::MutexGuard<'static, ()>, PgPool)> {
    let pool = test_pool().await?;
    let guard = DB_TEST_LOCK.lock().await;
    db::migrate(&pool).await.unwrap();
    Some((guard, pool))
}

#[tokio::test]
async fn canonical_schema_has_exactly_fifteen_application_tables() {
    let Some((_guard, pool)) = migrated_pool().await else {
        return;
    };
    let tables: Vec<String> = sqlx::query_scalar("SELECT table_name FROM information_schema.tables WHERE table_schema='public' AND table_type='BASE TABLE' AND table_name <> '_sqlx_migrations' ORDER BY table_name")
        .fetch_all(&pool).await.unwrap();
    assert_eq!(
        tables,
        vec![
            "artifacts",
            "chat_messages",
            "chats",
            "connections",
            "embeddings",
            "entities",
            "memories",
            "notes",
            "object_events",
            "objects",
            "runs",
            "sources",
            "tasks",
            "themes",
            "users"
        ]
    );
}

#[tokio::test]
async fn users_embed_multiple_provider_identities() {
    let Some((_guard, pool)) = migrated_pool().await else {
        return;
    };
    let id = Uuid::new_v4();
    let identity_suffix = id.simple().to_string();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,'user','Ada','Test user','system','test','system','test','{}')")
        .bind(id).execute(&mut *tx).await.unwrap();
    let identities = json!([
        {"id":Uuid::new_v4(),"provider":"slack","workspace_id":"w","provider_user_id":format!("u-{identity_suffix}"),"display_name":"Ada"},
        {"id":Uuid::new_v4(),"provider":"github","workspace_id":"org","provider_user_id":format!("ada-{identity_suffix}"),"display_name":"Ada"}
    ]);
    sqlx::query("INSERT INTO users(object_id,user_kind,identities) VALUES($1,'human',$2)")
        .bind(id)
        .bind(&identities)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let stored: serde_json::Value =
        sqlx::query_scalar("SELECT identities FROM users WHERE object_id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored.as_array().unwrap().len(), 2);

    let second_id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,'user','Second User','A second test user used to prove provider identity uniqueness.','system','test','system','test','{}')")
        .bind(second_id).execute(&mut *tx).await.unwrap();
    assert!(
        sqlx::query("INSERT INTO users(object_id,user_kind,identities) VALUES($1,'human',$2)")
            .bind(second_id)
            .bind(&identities)
            .execute(&mut *tx)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn artifacts_attach_to_any_object_and_are_immutable() {
    let Some((_guard, pool)) = migrated_pool().await else {
        return;
    };
    let object_id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,'task','Task','Test task','system','test','system','test','{}')")
        .bind(object_id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO tasks(object_id,status,priority,agent_suitable) VALUES($1,'todo','medium',true)").bind(object_id).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    let artifact_id = Uuid::new_v4();
    sqlx::query("INSERT INTO artifacts(id,object_id,kind,title,content,media_type,sha256,size_bytes,metadata) VALUES($1,$2,'transcript','Interview','hello','text/plain',$3,5,'{}')")
        .bind(artifact_id).bind(object_id).bind("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824").execute(&pool).await.unwrap();
    assert!(
        sqlx::query("UPDATE artifacts SET title='changed' WHERE id=$1")
            .bind(artifact_id)
            .execute(&pool)
            .await
            .is_err()
    );
    let window = db::get_artifact_window_by_id(&pool, artifact_id, 1, 3)
        .await
        .unwrap();
    assert_eq!(window.text, "ell");
    assert_eq!(window.next_offset, Some(4));
}

#[tokio::test]
async fn one_run_owns_trace_result_and_mutation_events() {
    let Some((_guard, pool)) = migrated_pool().await else {
        return;
    };
    let run_id = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,input,trace,result) VALUES($1,'curator','completed','centaur_agent','curator',$2,'{}','[{\"type\":\"retrieval\"}]','{\"summary\":\"done\"}')")
        .bind(run_id).bind(format!("run-{run_id}")).execute(&pool).await.unwrap();
    let object_id = Uuid::new_v4();
    let state = json!({"id":object_id,"kind":"memory","title":"Fact","description":"Durable fact","revision":1});
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,'memory','Fact','Durable fact','centaur_agent','curator','centaur_agent','curator','{}')")
        .bind(object_id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO memories(object_id,happened_at) VALUES($1,now())")
        .bind(object_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    sqlx::query("INSERT INTO object_events(id,run_id,sequence,target_type,target_id,action,actor_type,actor_id,to_revision,after_state,reversible,created_at) VALUES($1,$2,1,'object',$3,'create','centaur_agent','curator',1,$4,true,now())")
        .bind(Uuid::new_v4()).bind(run_id).bind(object_id).bind(state).execute(&pool).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM object_events WHERE run_id=$1")
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert!(
        sqlx::query("UPDATE object_events SET action='updated' WHERE run_id=$1")
            .bind(run_id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE runs SET input='{\"changed\":true}' WHERE id=$1")
            .bind(run_id)
            .execute(&pool)
            .await
            .is_err()
    );
    let child_id = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,parent_run_id,kind,status,actor_type,actor_id,idempotency_key,input,trace,result) VALUES($1,$2,'curator_undo','completed','human','reviewer',$3,'{}','[]','{}')")
        .bind(child_id).bind(run_id).bind(format!("child-{child_id}")).execute(&pool).await.unwrap();
    assert!(
        sqlx::query("UPDATE runs SET parent_run_id=$2 WHERE id=$1")
            .bind(run_id)
            .bind(child_id)
            .execute(&pool)
            .await
            .is_err()
    );
    let columns: Vec<String> = sqlx::query("SELECT column_name FROM information_schema.columns WHERE table_schema='public' AND table_name='runs'").fetch_all(&pool).await.unwrap().into_iter().map(|row| row.get(0)).collect();
    assert!(!columns.iter().any(|column| column == "changes"));
}

#[tokio::test]
async fn embeddings_are_jobs_and_results_in_one_table() {
    let Some((_guard, pool)) = migrated_pool().await else {
        return;
    };
    let object_id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,'memory','Memory','Embedding target','system','test','system','test','{}')")
        .bind(object_id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO memories(object_id,happened_at) VALUES($1,now())")
        .bind(object_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    sqlx::query("INSERT INTO embeddings(object_id,model,dimensions,source_hash,format_version,input_mode,status) VALUES($1,'text-embedding-3-small',3,$2,'centaur-object-v1','shared','pending')")
        .bind(object_id).bind("a".repeat(32)).execute(&pool).await.unwrap();
    sqlx::query("UPDATE embeddings SET status='completed', embedding='[0.1,0.2,0.3]'::vector, completed_at=now() WHERE object_id=$1 AND model='text-embedding-3-small'")
        .bind(object_id).execute(&pool).await.unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM embeddings WHERE object_id=$1")
        .bind(object_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "completed");
    assert!(
        sqlx::query("UPDATE embeddings SET status='pending' WHERE object_id=$1")
            .bind(object_id)
            .execute(&pool)
            .await
            .is_err()
    );
}

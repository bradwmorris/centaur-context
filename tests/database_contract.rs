use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use centaur_context::{
    api::{AppState, human_router, note_write_router},
    db,
};
use http_body_util::BodyExt;
use serde_json::json;
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use std::path::PathBuf;
use tower::ServiceExt;
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

#[tokio::test]
async fn object_lists_use_stable_recent_and_active_connection_density_ordering() {
    let Some((_guard, pool)) = migrated_pool().await else {
        return;
    };
    let token = format!("density{}", Uuid::new_v4().simple());
    let zero = Uuid::new_v4();
    let one = Uuid::new_v4();
    let two = Uuid::new_v4();
    let outside = Uuid::new_v4();
    let indexes: Vec<String> = sqlx::query_scalar("SELECT indexname FROM pg_indexes WHERE schemaname='public' AND indexname = ANY($1) ORDER BY indexname")
        .bind(vec!["connections_active_source_idx".to_owned(), "connections_active_target_idx".to_owned(), "objects_active_created_idx".to_owned(), "objects_active_kind_created_idx".to_owned()])
        .fetch_all(&pool).await.unwrap();
    assert_eq!(indexes.len(), 4);
    let mut tx = pool.begin().await.unwrap();
    for (id, title, created_at) in [
        (zero, format!("{token} zero"), "2099-01-03T00:00:00Z"),
        (one, format!("{token} one"), "2099-01-02T00:00:00Z"),
        (two, format!("{token} two"), "2099-01-01T00:00:00Z"),
        (
            outside,
            "density ordering endpoint".to_owned(),
            "2098-01-01T00:00:00Z",
        ),
    ] {
        sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance,created_at,updated_at) VALUES($1,'entity',$2,'List ordering contract','system','list-test','system','list-test','{}',$3::timestamptz,$3::timestamptz)")
            .bind(id).bind(title).bind(created_at).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO entities(object_id,entity_kind) VALUES($1,'concept')")
            .bind(id)
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    for (source, kind, archived) in [
        (one, "related_to", false),
        (two, "related_to", false),
        (two, "involves", false),
        (zero, "related_to", true),
    ] {
        sqlx::query("INSERT INTO connections(id,source_object_id,kind,target_object_id,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance,archived_at) VALUES($1,$2,$3,$4,'List density edge','system','list-test','system','list-test','{}',CASE WHEN $5 THEN now() ELSE NULL END)")
            .bind(Uuid::new_v4()).bind(source).bind(kind).bind(outside).bind(archived).execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap();

    let filter = |sort, cursor, limit| db::ObjectListFilter {
        query: Some(token.clone()),
        kind: Some("entity".to_owned()),
        lifecycle: Some("active".to_owned()),
        cursor,
        limit,
        sort,
        text_search_config: centaur_context::config::TextSearchConfig::SIMPLE,
    };
    let recent = db::list_objects(&pool, filter(db::ListSort::Recent, None, 10))
        .await
        .unwrap();
    assert_eq!(
        recent.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![zero, one, two]
    );
    sqlx::query("UPDATE objects SET updated_at='2100-01-01T00:00:00Z' WHERE id=$1")
        .bind(two)
        .execute(&pool)
        .await
        .unwrap();
    let unchanged = db::list_objects(&pool, filter(db::ListSort::Recent, None, 10))
        .await
        .unwrap();
    assert_eq!(
        unchanged.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![zero, one, two]
    );

    let first = db::list_objects(&pool, filter(db::ListSort::Connections, None, 2))
        .await
        .unwrap();
    assert_eq!(
        first.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![two, one]
    );
    let second = db::list_objects(&pool, filter(db::ListSort::Connections, Some(one), 2))
        .await
        .unwrap();
    assert_eq!(
        second.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![zero]
    );
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
    let tables: Vec<String> = sqlx::query_scalar("SELECT t.table_name FROM information_schema.tables t WHERE t.table_schema='public' AND t.table_type='BASE TABLE' AND t.table_name <> '_sqlx_migrations' AND NOT EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace JOIN pg_depend d ON d.objid=c.oid AND d.deptype='e' JOIN pg_extension e ON e.oid=d.refobjid WHERE n.nspname=t.table_schema AND c.relname=t.table_name) ORDER BY t.table_name")
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
async fn context_subtypes_include_themes_without_null_decode_failures() {
    let Some((_guard, pool)) = migrated_pool().await else {
        return;
    };
    let object_id = Uuid::new_v4();
    let slug = format!("theme-{}", object_id.simple());
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,'theme','Test theme','A theme returned by shared Context retrieval.','system','test','system','test','{}')")
        .bind(object_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO themes(object_id,slug) VALUES($1,$2)")
        .bind(object_id)
        .bind(&slug)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let subtypes = db::context_subtypes(&pool, &[object_id], None)
        .await
        .unwrap();
    assert_eq!(subtypes[&object_id], json!({"kind":"theme","slug":slug}));
}

#[tokio::test]
async fn narrow_write_listener_creates_and_replays_one_open_task() {
    let Some((_guard, pool)) = migrated_pool().await else {
        return;
    };
    let chat_id = Uuid::new_v4();
    let source_id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,'chat','Slack conversation','A Slack conversation requesting a follow-up task.','system','fixture','system','fixture','{}')")
        .bind(chat_id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO chats(object_id) VALUES($1)")
        .bind(chat_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,'source','Research source','A Source requiring a follow-up task.','system','fixture','system','fixture','{}')")
        .bind(source_id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO sources(object_id,source_kind) VALUES($1,'video')")
        .bind(source_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let token = "w".repeat(32);
    let app = note_write_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: centaur_context::config::TextSearchConfig::SIMPLE,
        },
        token.clone(),
    );
    let fixture = Uuid::new_v4().simple().to_string();
    let body = json!({
        "title":"Follow up on research",
        "description":"A bounded follow-up Task created through the narrow write listener.",
        "priority":"medium",
        "brief_markdown":"Review the captured evidence.",
        "provenance":{"source_type":"human"},
        "originating_chat_object_id":chat_id,
        "derived_from_source_object_ids":[source_id]
    });
    let request = || {
        Request::builder()
            .method("POST")
            .uri("/api/v2/tasks")
            .header("authorization", format!("Bearer {token}"))
            .header("x-centaur-principal-id", "researcher")
            .header("x-centaur-thread-key", "slack:T:C:thread")
            .header("idempotency-key", format!("task-{fixture}"))
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    };

    let created = app.clone().oneshot(request()).await.unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created: serde_json::Value =
        serde_json::from_slice(&created.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let object_id = created["data"]["object_id"].as_str().unwrap();
    assert_eq!(created["data"]["status"], "todo");

    let connections: Vec<(Uuid, String, Uuid)> = sqlx::query_as(
        "SELECT source_object_id,kind,target_object_id FROM connections WHERE (source_object_id=$1 OR target_object_id=$1) AND archived_at IS NULL ORDER BY kind",
    )
    .bind(Uuid::parse_str(object_id).unwrap())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        connections,
        vec![
            (chat_id, "about".into(), Uuid::parse_str(object_id).unwrap()),
            (
                Uuid::parse_str(object_id).unwrap(),
                "derived_from".into(),
                source_id
            ),
        ]
    );

    sqlx::query("DELETE FROM connections WHERE source_object_id=$1 AND kind='derived_from'")
        .bind(Uuid::parse_str(object_id).unwrap())
        .execute(&pool)
        .await
        .unwrap();

    let replayed = app.oneshot(request()).await.unwrap();
    assert_eq!(replayed.status(), StatusCode::CREATED);
    let replayed: serde_json::Value =
        serde_json::from_slice(&replayed.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(replayed["data"]["object_id"], object_id);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM tasks WHERE object_id=$1")
        .bind(Uuid::parse_str(object_id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let connection_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM connections WHERE (source_object_id=$1 OR target_object_id=$1) AND archived_at IS NULL",
    )
    .bind(Uuid::parse_str(object_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(connection_count, 2);
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
    sqlx::query("INSERT INTO artifacts(id,object_id,kind,title,content,media_type,sha256,size_bytes,capture_outcome,expected_size_bytes,metadata) VALUES($1,$2,'transcript','Interview','hello','text/plain',$3,5,'complete',5,'{}')")
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
        .bind(object_id).bind("a".repeat(64)).execute(&pool).await.unwrap();
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

#[tokio::test]
async fn connection_graph_is_complete_stable_and_active_only() {
    let Some((_guard, pool)) = migrated_pool().await else {
        return;
    };
    let source_id = Uuid::new_v4();
    let target_id = Uuid::new_v4();
    let archived_id = Uuid::new_v4();
    let active_connection_id = Uuid::new_v4();
    let archived_connection_id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    for (id, title, archived) in [
        (source_id, "Graph source", false),
        (target_id, "Graph target", false),
        (archived_id, "Archived graph node", true),
    ] {
        sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance,archived_at) VALUES($1,'memory',$2,'A graph contract fixture with enough context for validation.','system','graph-test','system','graph-test','{}',CASE WHEN $3 THEN now() ELSE NULL END)")
            .bind(id)
            .bind(title)
            .bind(archived)
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("INSERT INTO memories(object_id,happened_at) VALUES($1,now())")
            .bind(id)
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO connections(id,source_object_id,kind,target_object_id,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,$2,'related_to',$3,'The graph source is related to the graph target.','system','graph-test','system','graph-test','{}')")
        .bind(active_connection_id)
        .bind(source_id)
        .bind(target_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO connections(id,source_object_id,kind,target_object_id,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance,archived_at) VALUES($1,$2,'related_to',$3,'This archived edge must not appear in the active graph.','system','graph-test','system','graph-test','{}',now())")
        .bind(archived_connection_id)
        .bind(source_id)
        .bind(archived_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let first = db::connection_graph(&pool).await.unwrap();
    let second = db::connection_graph(&pool).await.unwrap();
    assert_eq!(first.fingerprint, second.fingerprint);
    assert_eq!(first.node_count, first.nodes.len());
    assert_eq!(first.connection_count, first.edges.len());
    assert!(first.nodes.iter().any(|node| node.id == source_id));
    assert!(first.nodes.iter().any(|node| node.id == target_id));
    assert!(!first.nodes.iter().any(|node| node.id == archived_id));
    assert!(
        first
            .edges
            .iter()
            .any(|edge| edge.id == active_connection_id)
    );
    assert!(
        !first
            .edges
            .iter()
            .any(|edge| edge.id == archived_connection_id)
    );
    assert!(first.edges.iter().all(|edge| {
        first
            .nodes
            .iter()
            .any(|node| node.id == edge.source_object_id)
            && first
                .nodes
                .iter()
                .any(|node| node.id == edge.target_object_id)
    }));

    let router = human_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: centaur_context::config::TextSearchConfig::SIMPLE,
        },
        PathBuf::from("web/dist"),
        PathBuf::from("identity-assets"),
    );
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v2/connection-graph")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-cache");
    let etag = response.headers()["etag"].clone();
    let payload: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(payload["data"]["fingerprint"], first.fingerprint);
    let unchanged = router
        .oneshot(
            Request::builder()
                .uri("/api/v2/connection-graph")
                .header("if-none-match", etag)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unchanged.status(), StatusCode::NOT_MODIFIED);
}

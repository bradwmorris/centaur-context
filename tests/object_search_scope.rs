use axum::{Json, Router, routing::post};
use centaur_context::{
    config::{EmbeddingConfig, EmbeddingInputMode, TextSearchConfig},
    db, embeddings, search,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;
use tokio::net::TcpListener;
use uuid::Uuid;

async fn test_pool() -> Option<PgPool> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    assert!(url.contains("centaur_context_test"));
    Some(
        PgPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .unwrap(),
    )
}

async fn insert_object(pool: &PgPool, id: Uuid, kind: &str, title: &str, description: &str) {
    let mut transaction = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,$2,$3,$4,'system','object-search-test','system','object-search-test')")
        .bind(id)
        .bind(kind)
        .bind(title)
        .bind(description)
        .execute(&mut *transaction)
        .await
        .unwrap();
    match kind {
        "source" => {
            sqlx::query("INSERT INTO sources(object_id,source_kind) VALUES($1,'paper')")
                .bind(id)
                .execute(&mut *transaction)
                .await
                .unwrap();
        }
        "chat" => {
            sqlx::query("INSERT INTO chats(object_id) VALUES($1)")
                .bind(id)
                .execute(&mut *transaction)
                .await
                .unwrap();
        }
        _ => unreachable!(),
    }
    transaction.commit().await.unwrap();
}

#[tokio::test]
async fn object_discovery_excludes_artifact_candidates_in_search_and_context() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping Object search scope contract: TEST_DATABASE_URL is not set");
        return;
    };
    db::migrate(&pool).await.unwrap();

    let suffix = Uuid::new_v4().simple().to_string();
    let artifact_marker = format!("artifactonly{suffix}");
    let metadata_marker = format!("metadataonly{suffix}");
    let model = format!("object-search-{suffix}");
    let source_a = Uuid::new_v4();
    let source_b = Uuid::new_v4();
    let stale_source = Uuid::new_v4();
    let chat_id = Uuid::new_v4();

    insert_object(
        &pool,
        source_a,
        "source",
        &format!("Primary {metadata_marker}"),
        "The nearest metadata-only semantic candidate.",
    )
    .await;
    insert_object(
        &pool,
        source_b,
        "source",
        "Secondary metadata candidate",
        "A lower-ranked metadata-only semantic candidate.",
    )
    .await;
    insert_object(
        &pool,
        stale_source,
        "source",
        "Stale semantic candidate",
        "Its embedding source hash deliberately does not match current metadata.",
    )
    .await;
    insert_object(
        &pool,
        chat_id,
        "chat",
        "Object search test chat",
        "A disposable chat for exercising Context retrieval.",
    )
    .await;
    let artifact_id = Uuid::new_v4();
    let artifact_content = format!("This complete captured body contains {artifact_marker}.");
    let artifact_sha = format!("{:x}", Sha256::digest(artifact_content.as_bytes()));
    sqlx::query("INSERT INTO artifacts(id,object_id,kind,content,media_type,sha256,size_bytes,capture_outcome,semantic_indexing_enabled,metadata) VALUES($1,$2,'paper_text',$3,'text/plain',$4,$5,'complete',true,'{\"coverage\":\"complete\"}')")
        .bind(artifact_id)
        .bind(source_b)
        .bind(&artifact_content)
        .bind(artifact_sha)
        .bind(artifact_content.len() as i64)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE sources SET current_artifact_id=$2 WHERE object_id=$1")
        .bind(source_b)
        .bind(artifact_id)
        .execute(&pool)
        .await
        .unwrap();

    for (id, vector) in [(source_a, "[1,0,0]"), (source_b, "[0.8,0.6,0]")] {
        let source_hash: String = sqlx::query_scalar("SELECT object_embedding_source_hash($2,kind,title,description) FROM objects WHERE id=$1")
            .bind(id)
            .bind(embeddings::OBJECT_EMBEDDING_FORMAT)
            .fetch_one(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO embeddings(object_id,model,dimensions,source_hash,format_version,input_mode,status,embedding,completed_at) VALUES($1,$2,3,$3,$4,'shared','completed',$5::vector,now())")
            .bind(id)
            .bind(&model)
            .bind(source_hash)
            .bind(embeddings::OBJECT_EMBEDDING_FORMAT)
            .bind(vector)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO embeddings(object_id,model,dimensions,source_hash,format_version,input_mode,status,embedding,completed_at) VALUES($1,$2,3,$3,$4,'shared','completed','[1,0,0]'::vector,now())")
        .bind(stale_source)
        .bind(&model)
        .bind("0".repeat(64))
        .bind(embeddings::OBJECT_EMBEDDING_FORMAT)
        .execute(&pool)
        .await
        .unwrap();

    let artifact_source_hash = format!(
        "{:x}",
        Sha256::digest(
            embeddings::format_artifact_document("Secondary metadata candidate", &artifact_content)
                .as_bytes()
        )
    );
    sqlx::query("INSERT INTO embeddings(object_id,artifact_id,chunk_index,start_offset,end_offset,model,dimensions,source_hash,format_version,input_mode,status,embedding,completed_at) VALUES($1,$2,0,0,$3,$4,3,$5,$6,'shared','completed','[1,0,0]'::vector,now())")
        .bind(source_b)
        .bind(artifact_id)
        .bind(artifact_content.chars().count() as i32)
        .bind(&model)
        .bind(artifact_source_hash)
        .bind(embeddings::ARTIFACT_EMBEDDING_FORMAT)
        .execute(&pool)
        .await
        .unwrap();

    let lexical = search::search(
        &pool,
        None,
        TextSearchConfig::SIMPLE,
        &artifact_marker,
        Some("source"),
        10,
    )
    .await
    .unwrap();
    assert!(lexical.objects.is_empty());

    let metadata = search::search(
        &pool,
        None,
        TextSearchConfig::SIMPLE,
        &metadata_marker,
        Some("source"),
        10,
    )
    .await
    .unwrap();
    assert_eq!(metadata.objects[0].id, source_a);
    assert!(
        metadata
            .objects
            .iter()
            .all(|object| object.evidence.is_none())
    );

    let context = search::context(
        &pool,
        None,
        TextSearchConfig::SIMPLE,
        &artifact_marker,
        Some("source"),
        chat_id,
        10,
    )
    .await
    .unwrap();
    assert!(context.objects.iter().all(|object| object.id != source_b));
    assert!(
        context
            .general_context_objects
            .iter()
            .all(|object| object.id != source_b)
    );
    assert!(
        context
            .objects
            .iter()
            .all(|object| object.evidence.is_none())
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route(
                "/embed",
                post(|| async { Json(json!({"data":[{"embedding":[1.0,0.0,0.0]}]})) }),
            ),
        )
        .await
        .unwrap();
    });
    let client = embeddings::EmbeddingClient::new(&EmbeddingConfig {
        endpoint: format!("http://{address}/embed"),
        api_token: "synthetic-token".into(),
        model: model.clone(),
        dimensions: 3,
        input_mode: EmbeddingInputMode::Shared,
        poll_interval: Duration::from_secs(1),
    })
    .unwrap();
    let direct_artifact_candidates = db::artifact_semantic_candidates(
        &pool,
        &[1.0, 0.0, 0.0],
        &model,
        3,
        embeddings::ARTIFACT_EMBEDDING_FORMAT,
        "shared",
        Some("source"),
        10,
        false,
    )
    .await
    .unwrap();
    assert_eq!(direct_artifact_candidates[0].object.id, source_b);

    let semantic = search::search(
        &pool,
        Some(&client),
        TextSearchConfig::SIMPLE,
        &format!("queryonly{suffix}"),
        Some("source"),
        10,
    )
    .await
    .unwrap();
    assert_eq!(semantic.retrieval, "hybrid");
    assert_eq!(semantic.objects[0].id, source_a);
    assert!(
        semantic
            .objects
            .iter()
            .all(|object| object.id != stale_source)
    );
    assert!(
        semantic
            .objects
            .iter()
            .all(|object| object.evidence.is_none())
    );

    let semantic_context = search::context(
        &pool,
        Some(&client),
        TextSearchConfig::SIMPLE,
        &format!("contextqueryonly{suffix}"),
        Some("source"),
        chat_id,
        10,
    )
    .await
    .unwrap();
    let source_a_rank = semantic_context
        .objects
        .iter()
        .position(|object| object.id == source_a)
        .unwrap();
    let source_b_rank = semantic_context
        .objects
        .iter()
        .position(|object| object.id == source_b)
        .unwrap();
    assert!(source_a_rank < source_b_rank);
    assert!(
        semantic_context
            .objects
            .iter()
            .all(|object| object.evidence.is_none())
    );
}

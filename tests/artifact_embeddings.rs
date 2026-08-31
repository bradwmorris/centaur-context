use centaur_context::{db, embeddings};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

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
async fn complete_current_artifacts_queue_chunks_and_return_exact_evidence() {
    let Some(pool) = test_pool().await else {
        eprintln!("skipping Artifact embeddings contract: TEST_DATABASE_URL is not set");
        return;
    };
    db::migrate(&pool).await.unwrap();
    let object_id = Uuid::new_v4();
    let mut setup = pool.begin().await.unwrap();
    sqlx::query(
        r#"INSERT INTO objects
           (id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id)
           VALUES ($1,'source','Chunked evidence','A complete synthetic Source for Artifact retrieval.',
                   'system','test','system','test')"#,
    )
    .bind(object_id)
    .execute(&mut *setup)
    .await
    .unwrap();
    sqlx::query("INSERT INTO sources (object_id,source_kind) VALUES ($1,'paper')")
        .bind(object_id)
        .execute(&mut *setup)
        .await
        .unwrap();
    setup.commit().await.unwrap();

    let incomplete_id = Uuid::new_v4();
    let incomplete = "partial material";
    sqlx::query(
        r#"INSERT INTO artifacts
           (id,object_id,kind,content,media_type,sha256,size_bytes,capture_outcome,capture_reason)
           VALUES ($1,$2,'paper_text',$3,'text/plain',$4,$5,'incomplete','only an excerpt was available')"#,
    )
    .bind(incomplete_id)
    .bind(object_id)
    .bind(incomplete)
    .bind(format!("{:x}", Sha256::digest(incomplete.as_bytes())))
    .bind(incomplete.len() as i64)
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        sqlx::query("UPDATE sources SET current_artifact_id=$2 WHERE object_id=$1")
            .bind(object_id)
            .bind(incomplete_id)
            .execute(&pool)
            .await
            .is_err()
    );

    let artifact_id = Uuid::new_v4();
    let content = format!(
        "{}\n\nThe hidden retrieval fact is heliotrope-cassowary.",
        "A complete paper paragraph. ".repeat(260)
    );
    let sha256 = format!("{:x}", Sha256::digest(content.as_bytes()));
    sqlx::query(
        r#"INSERT INTO artifacts
           (id,object_id,kind,content,media_type,sha256,size_bytes,capture_outcome,expected_size_bytes,
            metadata)
           VALUES ($1,$2,'paper_text',$3,'text/plain',$4,$5,'complete',$5,
                   '{"extraction_method":"test","extraction_version":"1","coverage":"complete"}')"#,
    )
    .bind(artifact_id)
    .bind(object_id)
    .bind(&content)
    .bind(&sha256)
    .bind(content.len() as i64)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE sources SET current_artifact_id=$2 WHERE object_id=$1")
        .bind(object_id)
        .bind(artifact_id)
        .execute(&pool)
        .await
        .unwrap();

    let source = db::artifact_embedding_sources(&pool)
        .await
        .unwrap()
        .into_iter()
        .find(|source| source.artifact_id == artifact_id)
        .unwrap();
    let chunks = embeddings::artifact_chunks(&source.title, &source.content)
        .into_iter()
        .enumerate()
        .map(
            |(index, (start_offset, end_offset))| db::ArtifactEmbeddingChunk {
                chunk_index: index as i32,
                start_offset,
                end_offset,
                source_hash: format!(
                    "{:x}",
                    Sha256::digest(
                        embeddings::format_artifact_document(
                            &source.title,
                            &source
                                .content
                                .chars()
                                .skip(start_offset as usize)
                                .take((end_offset - start_offset) as usize)
                                .collect::<String>(),
                        )
                        .as_bytes()
                    )
                ),
            },
        )
        .collect::<Vec<_>>();
    assert!(chunks.len() > 1);
    db::queue_artifact_embedding_chunks(
        &pool,
        &source,
        &chunks,
        "test-model",
        3,
        embeddings::ARTIFACT_EMBEDDING_FORMAT,
        "shared",
    )
    .await
    .unwrap();
    let queued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM embeddings WHERE artifact_id=$1 AND status='pending'",
    )
    .bind(artifact_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(queued, chunks.len() as i64);
    sqlx::query("UPDATE embeddings SET available_at=now()+interval '1 day' WHERE artifact_id<>$1")
        .bind(artifact_id)
        .execute(&pool)
        .await
        .unwrap();

    let job = db::claim_embedding_job(&pool, "test-model", 3, "shared")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.artifact_id, Some(artifact_id));
    db::complete_embedding_job(&pool, &job, &[0.1, 0.2, 0.3])
        .await
        .unwrap();
    let semantic = db::artifact_semantic_candidates(
        &pool,
        &[0.1, 0.2, 0.3],
        "test-model",
        3,
        embeddings::ARTIFACT_EMBEDDING_FORMAT,
        "shared",
        Some("source"),
        10,
        false,
    )
    .await
    .unwrap();
    assert!(semantic.iter().any(|candidate| {
        candidate.object.id == object_id
            && candidate
                .evidence
                .as_ref()
                .is_some_and(|evidence| evidence.artifact_id == artifact_id)
    }));

    let candidates = db::artifact_full_text_candidates(
        &pool,
        "absent heliotrope-cassowary",
        Some("source"),
        10,
        false,
    )
    .await
    .unwrap();
    let candidate = candidates
        .into_iter()
        .find(|candidate| candidate.object.id == object_id)
        .unwrap();
    let evidence = candidate.evidence.unwrap();
    assert_eq!(evidence.artifact_id, artifact_id);
    assert_eq!(evidence.capture_outcome, "complete");
    assert_eq!(
        evidence.excerpt,
        content
            .chars()
            .skip(evidence.start_offset as usize)
            .take((evidence.end_offset - evidence.start_offset) as usize)
            .collect::<String>()
    );
    assert!(evidence.excerpt.contains("heliotrope-cassowary"));

    let status = db::embedding_status(&pool, Some(("test-model", 3, "shared")))
        .await
        .unwrap();
    assert!(
        status["coverage"]["completed_artifact_chunks"]
            .as_i64()
            .unwrap()
            >= 1
    );
    assert!(
        status["coverage"]["indexed_current_artifacts"]
            .as_i64()
            .unwrap()
            >= 1
    );
    assert!(status["oldest_age_seconds"].is_number());
}

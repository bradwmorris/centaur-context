use sqlx::PgPool;
use uuid::Uuid;

pub async fn task_owner(pool: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,'user','Synthetic task owner','A disposable assigned User for task contract tests.','system','task-test','system','task-test')")
        .bind(id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO users(object_id,user_kind,identities) VALUES($1,'agent','[]')")
        .bind(id)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    id
}

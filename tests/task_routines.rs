mod support;
use centaur_context::{db, domain::ActorContext};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use uuid::Uuid;

#[tokio::test]
async fn schedules_claims_pause_and_results_are_independent_of_due_dates() {
    let Ok(url) = std::env::var("TEST_DATABASE_URL") else {
        return;
    };
    assert!(url.contains("centaur_context_test"));
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    db::migrate(&pool).await.unwrap();
    let actor = ActorContext::human();
    let owner = support::task_owner(&pool).await;
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,'task','Synthetic Routine','Check a synthetic fixture without side effects.','human','test','human','test')").bind(id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO tasks(object_id,status,priority,owner_object_id,agent_suitable,due_at,brief_markdown) VALUES($1,'todo','medium',$2,true,now()-interval '1 day','Project: research\n\nCheck the synthetic fixture.')").bind(id).bind(owner).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    assert!(
        db::routine_claim_due(&pool)
            .await
            .unwrap()
            .iter()
            .all(|r| r["task_id"] != json!(id))
    );
    let schedule = db::RoutineSchedule {
        timezone: "Australia/Sydney".into(),
        every_minutes: Some(1),
        local_time: None,
        weekdays: vec![],
    };
    let make = |enabled, confirmed, rev| db::RoutineConfigure {
        expected_revision: rev,
        schedule: schedule.clone(),
        enabled,
        confirmed,
    };
    assert!(
        db::routine_configure(&pool, &actor, id, make(true, false, 1))
            .await
            .is_err()
    );
    db::routine_configure(&pool, &actor, id, make(true, true, 1))
        .await
        .unwrap();
    assert!(
        db::routine_configure(&pool, &actor, id, make(true, true, 1))
            .await
            .is_err()
    );
    sqlx::query("UPDATE task_routines SET next_run_at=now()-interval '1 minute' WHERE task_id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let (a, b) = tokio::join!(db::routine_claim_due(&pool), db::routine_claim_due(&pool));
    let a = a.unwrap();
    let b = b.unwrap();
    let run = a
        .iter()
        .chain(b.iter())
        .find(|r| r["task_id"] == json!(id))
        .unwrap();
    let run_id: Uuid = serde_json::from_value(run["id"].clone()).unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM task_routine_runs WHERE task_id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let update = |status: &str, thread: Option<&str>| db::RoutineRunUpdate {
        status: status.into(),
        execution_thread: thread.map(str::to_owned),
        execution_url: thread.map(|_| "https://example.test/thread".into()),
        result: Some("Synthetic result".into()),
    };
    db::routine_run_update(
        &pool,
        None,
        run_id,
        update("running", Some("slack:test:run")),
    )
    .await
    .unwrap();
    let mut worker = ActorContext::system("worker");
    worker.actor_type = "centaur_agent";
    worker.is_agent = true;
    worker.centaur_thread_key = Some("slack:test:wrong".into());
    assert!(
        db::routine_run_update(&pool, Some(&worker), run_id, update("completed", None))
            .await
            .is_err()
    );
    worker.centaur_thread_key = Some("slack:test:run".into());
    db::routine_run_update(&pool, Some(&worker), run_id, update("completed", None))
        .await
        .unwrap();
    assert_eq!(
        db::routine_run_update(&pool, None, run_id, update("review", None))
            .await
            .unwrap()["status"],
        "completed"
    );
    assert_eq!(db::get_task(&pool, id).await.unwrap().status, "todo");
    sqlx::query("UPDATE task_routines SET next_run_at=now()-interval '1 minute' WHERE task_id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let second = db::routine_claim_due(&pool).await.unwrap();
    assert!(
        second
            .iter()
            .any(|r| r["task_id"] == json!(id) && r["id"] != json!(run_id))
    );
    sqlx::query(
        "UPDATE objects SET revision=revision+1,title='Changed synthetic Routine' WHERE id=$1",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    let detail = db::routine_read(&pool, id).await.unwrap();
    assert_eq!(detail["routine"]["enabled"], false);
    db::routine_claim_due(&pool).await.unwrap();
    let active:i64=sqlx::query_scalar("SELECT count(*) FROM task_routine_runs WHERE task_id=$1 AND status IN ('pending','running')").bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(active, 0);
    let wall = db::RoutineSchedule {
        timezone: "Australia/Sydney".into(),
        every_minutes: None,
        local_time: Some("02:30".into()),
        weekdays: vec![7],
    };
    let parse = |s| OffsetDateTime::parse(s, &Rfc3339).unwrap();
    assert_eq!(
        db::routine_next(&pool, &wall, parse("2026-10-03T00:00:00Z"))
            .await
            .unwrap(),
        parse("2026-10-03T16:30:00Z")
    );
    assert_eq!(
        db::routine_next(&pool, &wall, parse("2026-04-04T00:00:00Z"))
            .await
            .unwrap(),
        parse("2026-04-04T16:30:00Z")
    );
    let invalid = db::RoutineSchedule {
        timezone: "Not/AZone".into(),
        ..wall
    };
    assert!(
        db::routine_next(&pool, &invalid, OffsetDateTime::now_utc())
            .await
            .is_err()
    );
}

//! Canonical schedules and occurrence claims; transport and credentials stay outside Context.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutineSchedule {
    pub timezone: String,
    pub every_minutes: Option<i32>,
    pub local_time: Option<String>,
    #[serde(default)]
    pub weekdays: Vec<i32>,
}

impl RoutineSchedule {
    pub fn validate(&self) -> Result<(), DbError> {
        let interval = self
            .every_minutes
            .is_some_and(|m| (1..=525600).contains(&m));
        let clock = self.local_time.as_ref().is_some_and(|s| {
            let b = s.as_bytes();
            b.len() == 5
                && b.iter().all(u8::is_ascii)
                && b[2] == b':'
                && s[..2].parse::<u8>().is_ok_and(|h| h < 24)
                && s[3..].parse::<u8>().is_ok_and(|m| m < 60)
        });
        if self.timezone.is_empty()
            || self.timezone.len() > 100
            || !((interval && self.local_time.is_none() && self.weekdays.is_empty())
                || (self.every_minutes.is_none()
                    && clock
                    && !self.weekdays.is_empty()
                    && self.weekdays.len() <= 7
                    && self.weekdays.iter().all(|d| (1..=7).contains(d))))
        {
            return Err(DbError::Invalid("Use every_minutes (1–525600), or local_time HH:MM and weekdays (Monday=1…Sunday=7), plus an IANA timezone".into()));
        }
        Ok(())
    }
}

pub async fn routine_next(
    pool: &PgPool,
    schedule: &RoutineSchedule,
    after: OffsetDateTime,
) -> Result<OffsetDateTime, DbError> {
    schedule.validate()?;
    let valid: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_timezone_names WHERE name=$1)")
            .bind(&schedule.timezone)
            .fetch_one(pool)
            .await?;
    if !valid {
        return Err(DbError::Invalid("Unknown timezone".into()));
    }
    if let Some(minutes) = schedule.every_minutes {
        return Ok(after + time::Duration::minutes(i64::from(minutes)));
    }
    let next: Option<OffsetDateTime> = sqlx::query_scalar(
        r#"
        SELECT min(occurrence) FROM (
          SELECT (d::date + $3::time) AT TIME ZONE $2 AS occurrence
          FROM generate_series(($1::timestamptz AT TIME ZONE $2)::date,
            ($1::timestamptz AT TIME ZONE $2)::date + 8, interval '1 day') d
          WHERE extract(isodow FROM d)::int = ANY($4)
        ) candidates WHERE occurrence > $1"#,
    )
    .bind(after)
    .bind(&schedule.timezone)
    .bind(&schedule.local_time)
    .bind(&schedule.weekdays)
    .fetch_one(pool)
    .await?;
    next.ok_or_else(|| DbError::Invalid("Schedule has no next occurrence".into()))
}

pub async fn routine_read(pool: &PgPool, id: Uuid) -> Result<Value, DbError> {
    get_task(pool, id).await?;
    let config: Option<Value> =
        sqlx::query_scalar("SELECT to_jsonb(r) FROM task_routines r WHERE task_id=$1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    let runs: Vec<Value> = sqlx::query_scalar("SELECT to_jsonb(r) FROM task_routine_runs r WHERE task_id=$1 ORDER BY created_at DESC,id LIMIT 50")
        .bind(id).fetch_all(pool).await?;
    Ok(json!({"routine":config,"runs":runs}))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutineConfigure {
    pub expected_revision: i64,
    pub schedule: RoutineSchedule,
    pub enabled: bool,
    #[serde(default)]
    pub confirmed: bool,
}

pub async fn routine_configure(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    input: RoutineConfigure,
) -> Result<Value, DbError> {
    if input.enabled && !input.confirmed {
        return Err(DbError::Invalid(
            "Explicit confirmation is required to enable recurring execution".into(),
        ));
    }
    let next = routine_next(pool, &input.schedule, OffsetDateTime::now_utc()).await?;
    let task = get_task(pool, id).await?;
    if actor.is_agent && task.protected {
        return Err(DbError::Invalid(
            "Protected task cannot be reconfigured by an agent".into(),
        ));
    }
    if task.lifecycle != "active"
        || (input.enabled
            && (!task.agent_suitable
                || task.owner_object_id.is_none()
                || task.due_at.is_none()
                || task
                    .brief_markdown
                    .as_ref()
                    .is_none_or(|b| b.trim().is_empty())
                || !["todo", "review"].contains(&task.status.as_str())))
    {
        return Err(DbError::Invalid("Enable requires an active, Ready or Review, agent-suitable task with owner, review date and execution brief".into()));
    }
    let mut tx = pool.begin().await?;
    let revision: Option<i64> = sqlx::query_scalar("UPDATE objects SET revision=revision+1,updated_by_type=$3,updated_by_id=$4,updated_at=now() WHERE id=$1 AND revision=$2 RETURNING revision")
        .bind(id).bind(input.expected_revision).bind(actor.actor_type).bind(&actor.actor_id).fetch_optional(&mut *tx).await?;
    let revision = revision.ok_or(DbError::Conflict)?;
    sqlx::query("INSERT INTO task_routines(task_id,schedule,enabled,definition_revision,next_run_at) VALUES($1,$2,$3,$4,$5) ON CONFLICT(task_id) DO UPDATE SET schedule=$2,enabled=$3,definition_revision=$4,next_run_at=$5,updated_at=now()")
        .bind(id).bind(json!(input.schedule)).bind(input.enabled).bind(revision).bind(input.enabled.then_some(next)).execute(&mut *tx).await?;
    insert_event(&mut tx,actor,"task",id,id,"updated",None,Some(input.expected_revision),revision,
        json!({"routine_schedule":input.schedule,"routine_enabled":input.enabled,"confirmed":input.confirmed})).await?;
    tx.commit().await?;
    routine_read(pool, id).await
}

/// Pending claims are returned again until the transport acknowledges them.
/// A unique active occurrence and a row lock prevent duplicate/overlapping starts.
pub async fn routine_claim_due(pool: &PgPool) -> Result<Vec<Value>, DbError> {
    let now = OffsetDateTime::now_utc();
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE task_routine_runs run SET status='skipped',result='Routine paused or definition changed before dispatch',completed_at=now() FROM task_routines r,objects o WHERE run.task_id=r.task_id AND o.id=r.task_id AND run.status='pending' AND (NOT r.enabled OR r.definition_revision<>run.definition_revision OR o.revision<>run.definition_revision OR o.archived_at IS NOT NULL)").execute(&mut *tx).await?;
    let due: Vec<(Uuid, Value, i64, OffsetDateTime)> = sqlx::query_as(
        r#"
        SELECT r.task_id,r.schedule,r.definition_revision,r.next_run_at FROM task_routines r
        JOIN tasks t ON t.object_id=r.task_id JOIN objects o ON o.id=r.task_id
        WHERE r.enabled AND r.next_run_at<=$1 AND o.archived_at IS NULL
          AND o.revision=r.definition_revision AND t.status IN ('todo','review')
        ORDER BY r.next_run_at,r.task_id LIMIT 25 FOR UPDATE OF r SKIP LOCKED"#,
    )
    .bind(now)
    .fetch_all(&mut *tx)
    .await?;
    for (task_id, schedule, revision, scheduled_for) in due {
        let schedule: RoutineSchedule =
            serde_json::from_value(schedule).map_err(|e| DbError::Invalid(e.to_string()))?;
        let next = routine_next(pool, &schedule, now).await?;
        sqlx::query("UPDATE task_routines SET next_run_at=$2 WHERE task_id=$1")
            .bind(task_id)
            .bind(next)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO task_routine_runs(id,task_id,scheduled_for,definition_revision) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING")
            .bind(Uuid::new_v4()).bind(task_id).bind(scheduled_for).bind(revision).execute(&mut *tx).await?;
    }
    let pending = sqlx::query_scalar("SELECT to_jsonb(r) FROM task_routine_runs r WHERE status IN ('pending','running') ORDER BY created_at LIMIT 100")
        .fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(pending)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutineRunUpdate {
    pub status: String,
    pub execution_thread: Option<String>,
    pub execution_url: Option<String>,
    pub result: Option<String>,
}

pub async fn routine_run_update(
    pool: &PgPool,
    actor: Option<&ActorContext>,
    id: Uuid,
    input: RoutineRunUpdate,
) -> Result<Value, DbError> {
    let mut tx = pool.begin().await?;
    let run: Value =
        sqlx::query_scalar("SELECT to_jsonb(r) FROM task_routine_runs r WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(DbError::NotFound)?;
    let old = run["status"].as_str().unwrap_or_default();
    if actor.is_none() && ["review", "completed", "blocked", "skipped"].contains(&old) {
        return Ok(run);
    }
    if let Some(actor) = actor {
        if actor.actor_type != "human"
            && (run["execution_thread"].as_str().is_none()
                || run["execution_thread"].as_str() != actor.centaur_thread_key.as_deref())
        {
            return Err(DbError::Invalid(
                "Only the assigned execution thread may report this occurrence".into(),
            ));
        }
        if input.execution_thread.is_some() || input.execution_url.is_some() {
            return Err(DbError::Invalid(
                "Only the trusted dispatcher can bind an execution".into(),
            ));
        }
    }
    let terminal = ["review", "completed", "blocked", "skipped"].contains(&input.status.as_str());
    if !(terminal || (actor.is_none() && input.status == "running"))
        || (!matches!(old, "pending" | "running")
            && !(old == "review"
                && actor.is_some_and(|a| a.actor_type == "human")
                && input.status == "completed"))
    {
        if old == input.status {
            return Ok(run);
        }
        return Err(DbError::Invalid("Invalid occurrence transition".into()));
    }
    if input.result.as_ref().is_some_and(|v| v.len() > 20000)
        || (input.status == "blocked" && input.result.as_ref().is_none_or(|v| v.trim().is_empty()))
    {
        return Err(DbError::Invalid(
            "Result must be at most 20000 bytes; a blocked run requires a reason".into(),
        ));
    }
    if input.status == "running" {
        let task_id: Uuid = serde_json::from_value(run["task_id"].clone())
            .map_err(|e| DbError::Invalid(e.to_string()))?;
        let valid: bool = sqlx::query_scalar(r#"SELECT EXISTS(SELECT 1 FROM task_routines r JOIN objects o ON o.id=r.task_id JOIN tasks t ON t.object_id=r.task_id
            WHERE r.task_id=$1 AND r.enabled AND r.definition_revision=$2 AND o.revision=$2 AND o.archived_at IS NULL AND t.status IN ('todo','review'))"#)
            .bind(task_id).bind(run["definition_revision"].as_i64()).fetch_one(&mut *tx).await?;
        if !valid {
            return Err(DbError::Invalid(
                "Routine is paused or its definition changed".into(),
            ));
        }
        if input.execution_thread.as_ref().is_none_or(|s| s.is_empty())
            || input
                .execution_url
                .as_ref()
                .is_none_or(|s| !s.starts_with("https://"))
        {
            return Err(DbError::Invalid(
                "Execution thread and HTTPS link are required".into(),
            ));
        }
    }
    let updated: Value = sqlx::query_scalar(r#"UPDATE task_routine_runs SET status=$2,
        execution_thread=coalesce($3,execution_thread),execution_url=coalesce($4,execution_url),result=coalesce($5,result),
        completed_at=CASE WHEN $6 THEN now() ELSE NULL END WHERE id=$1 RETURNING to_jsonb(task_routine_runs)"#)
        .bind(id).bind(&input.status).bind(&input.execution_thread).bind(&input.execution_url).bind(&input.result).bind(terminal)
        .fetch_one(&mut *tx).await?;
    let event_actor = actor
        .cloned()
        .unwrap_or_else(|| ActorContext::system("routine-dispatcher"));
    let task_id: Uuid = serde_json::from_value(run["task_id"].clone())
        .map_err(|e| DbError::Invalid(e.to_string()))?;
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM objects WHERE id=$1")
        .bind(task_id)
        .fetch_one(&mut *tx)
        .await?;
    insert_event(
        &mut tx,
        &event_actor,
        "task",
        task_id,
        task_id,
        "updated",
        None,
        Some(revision),
        revision,
        json!({"routine_run_id":id,"before":run,"after":updated}),
    )
    .await?;
    tx.commit().await?;
    Ok(updated)
}

pub async fn routine_run_read(pool: &PgPool, id: Uuid) -> Result<Value, DbError> {
    sqlx::query_scalar("SELECT to_jsonb(r) || jsonb_build_object('eligible',c.enabled AND c.definition_revision=r.definition_revision AND o.revision=r.definition_revision AND o.archived_at IS NULL AND t.status IN ('todo','review')) FROM task_routine_runs r JOIN task_routines c ON c.task_id=r.task_id JOIN objects o ON o.id=r.task_id JOIN tasks t ON t.object_id=r.task_id WHERE r.id=$1")
        .bind(id).fetch_optional(pool).await?.ok_or(DbError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schedule_is_one_clear_choice() {
        let mut s = RoutineSchedule {
            timezone: "UTC".into(),
            every_minutes: Some(5),
            local_time: None,
            weekdays: vec![],
        };
        assert!(s.validate().is_ok());
        s.local_time = Some("09:00".into());
        assert!(s.validate().is_err());
        s.every_minutes = None;
        s.weekdays = vec![1, 5];
        assert!(s.validate().is_ok());
        s.local_time = Some("24:00".into());
        assert!(s.validate().is_err());
        s.local_time = Some("a😀".into());
        assert!(s.validate().is_err());
    }
}

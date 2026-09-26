//! Selective outcome capture from committed events. No model and no agent write capability.
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{db, domain::ActorContext};

const CAPTURE_ACTOR: &str = "context-memory-capture";

/// Bounded ledger traversal, deliberately using NOT EXISTS instead of a timestamp
/// watermark: an older transaction can commit after a newer event was processed.
/// The unique run key is both the checkpoint and retry/deduplication identity.
pub async fn capture_outcomes(pool: &PgPool) -> Result<usize, db::DbError> {
    let mut tx = pool.begin().await?;
    let claimed: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(7818001)")
        .fetch_one(&mut *tx)
        .await?;
    if !claimed {
        return Ok(0);
    }
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,result,completed_at) VALUES($1,'memory_capture_start','completed','system',$2,'start','{}',now()) ON CONFLICT(kind,idempotency_key) DO NOTHING")
        .bind(Uuid::new_v4()).bind(CAPTURE_ACTOR).execute(&mut *tx).await?;
    let events: Vec<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(e) FROM object_events e JOIN runs r ON r.id=e.run_id \
         WHERE e.target_type='object' AND (e.action='created' OR (e.action IN ('updated','task_status_changed') AND e.after_state->>'kind'='task')) \
         AND e.after_state->>'kind' IN ('task','source','note') AND r.status='completed' \
         AND e.created_at >= (SELECT created_at FROM runs WHERE kind='memory_capture_start' AND idempotency_key='start') \
         AND NOT EXISTS(SELECT 1 FROM runs done WHERE done.kind='memory_capture' AND done.idempotency_key=e.id::text) \
         ORDER BY e.created_at,e.id LIMIT 25")
        .fetch_all(&mut *tx).await?;
    let mut count = 0;
    for event in events {
        let event_id = id(&event, "id")?;
        let target = id(&event, "target_id")?;
        let run_id = Uuid::new_v4();
        sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,input,result,completed_at) VALUES($1,'memory_capture','completed','system',$2,$3,$4,'{}',now())")
            .bind(run_id).bind(CAPTURE_ACTOR).bind(event_id.to_string())
            .bind(json!({"event_id":event_id,"target_id":target})).execute(&mut *tx).await?;
        let object = &event["after_state"];
        let title = object["title"].as_str().unwrap_or_default();
        let kind = object["kind"].as_str().unwrap_or_default();
        let active: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM objects WHERE id=$1 AND archived_at IS NULL)",
        )
        .bind(target)
        .fetch_one(&mut *tx)
        .await?;
        // Explicitly synthetic/imported traffic is not new human activity. Unknown
        // or legacy snapshots are safely skipped rather than fabricated.
        let milestone = task_milestone(&event);
        if !active
            || !eligible_outcome(object)
            || (event["action"] != "created" && milestone.is_none())
        {
            sqlx::query("UPDATE runs SET result=$2 WHERE id=$1")
                .bind(run_id)
                .bind(if active && eligible_outcome(object) && event["action"] != "created" {json!({"deferred":"Task event has no new committed result reference or supported review/completion milestone; preserve the event and wait for a later evidenced Task update"})} else {json!({"skipped":"ineligible, synthetic, imported or archived"})})
                .execute(&mut *tx)
                .await?;
            continue;
        }
        let actor_id = event["actor_id"].as_str().unwrap_or("unknown");
        let actor_type = event["actor_type"].as_str().unwrap_or("system");
        // Only exact authenticated principal/identity matches, never title search
        // or an inferred initiating human from a mixed conversation.
        let identities: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT DISTINCT o.id,o.title FROM objects o JOIN users u ON u.object_id=o.id \
             LEFT JOIN LATERAL jsonb_array_elements(u.identities) e ON true \
             WHERE o.archived_at IS NULL AND (o.id::text=$1 OR \
               (e->>'provider'='centaur' AND e->>'provider_user_id'=$1)) LIMIT 2",
        )
        .bind(actor_id)
        .fetch_all(&mut *tx)
        .await?;
        let actor_name = if identities.len() == 1 {
            identities[0].1.chars().take(100).collect::<String>()
        } else if actor_type == "human" {
            "A user".into()
        } else if actor_id == "codex" {
            "Codex".into()
        } else {
            "An agent".into()
        };
        let verb = milestone
            .as_ref()
            .map(|(verb, _)| *verb)
            .unwrap_or(if kind == "task" { "created" } else { "added" });
        let memory_id = Uuid::new_v4();
        let memory_title = format!(
            "{}: {}",
            milestone
                .as_ref()
                .map(|(_, label)| *label)
                .unwrap_or("Added"),
            title.chars().take(260).collect::<String>()
        );
        let description = if milestone.is_some() {
            format!("{actor_name} {verb}: {title}.")
        } else {
            format!("{actor_name} {verb} a {kind}: {title}.")
        };
        let description = crate::domain::object_description(&memory_title, description)?;
        let provenance = json!({"source_type":"context_memory_event","source_event_id":event_id,
            "source_run_id":event["run_id"],"actor_type":actor_type,"actor_id":actor_id,"task_object_id":if kind=="task" {Some(target)} else {None},"event_action":event["action"]});
        sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,'memory',$2,$3,'system',$4,'system',$4,$5)")
            .bind(memory_id).bind(memory_title).bind(description).bind(CAPTURE_ACTOR).bind(provenance).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO memories(object_id,happened_at) VALUES($1,$2::text::timestamptz)")
            .bind(memory_id)
            .bind(event["created_at"].as_str().unwrap_or_default())
            .execute(&mut *tx)
            .await?;
        journal(&mut tx, run_id, 1, "object", memory_id, None, 1).await?;
        let mut links = vec![(
            target,
            "about",
            format!(
                "Records the committed {} event for this {kind}.",
                event["action"].as_str().unwrap_or("change")
            ),
        )];
        if identities.len() == 1 {
            links.push((
                identities[0].0,
                "involves",
                "This User is the authenticated writer of the recorded event.".into(),
            ));
        }
        for (i, (target, relation, description)) in links.into_iter().enumerate() {
            let link = Uuid::new_v4();
            sqlx::query("INSERT INTO connections(id,source_object_id,kind,target_object_id,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,$2,$3,$4,$5,'system',$6,'system',$6,$7)")
                .bind(link).bind(memory_id).bind(relation).bind(target).bind(description).bind(CAPTURE_ACTOR)
                .bind(json!({"source_type":"context_memory_event","source_event_id":event_id})).execute(&mut *tx).await?;
            journal(&mut tx, run_id, i as i64 + 2, "connection", link, None, 1).await?;
        }
        sqlx::query("UPDATE runs SET primary_object_id=$2,result=$3 WHERE id=$1")
            .bind(run_id)
            .bind(memory_id)
            .bind(json!({"memory_id":memory_id,"event_id":event_id}))
            .execute(&mut *tx)
            .await?;
        count += 1;
    }
    tx.commit().await?;
    Ok(count)
}

/// A committed transition proves the recorded milestone, not deployment or acceptance.
/// Preserve its complete snapshot as evidence; unchanged status/unsupported states do not emit.
fn task_milestone(event: &Value) -> Option<(&'static str, &'static str)> {
    let after = &event["after_state"];
    if after["kind"] != "task" || event["action"] == "created" {
        return None;
    }
    let status = after["subtype"]["status"].as_str()?;
    let brief = after["subtype"]["brief_markdown"].as_str()?;
    // Outcome transitions without a committed result reference are deferred.
    let prior = event["before_state"]["subtype"]["brief_markdown"]
        .as_str()
        .unwrap_or_default();
    let new_reference = brief.split_whitespace().any(|word| {
        (word.contains("https://") || word.contains("http://")) && !prior.contains(word)
    });
    if !new_reference {
        return None;
    }
    let unchanged_status = event["before_state"]["subtype"]["status"] == status;
    match status {
        "review" | "done" if unchanged_status => Some((
            "recorded new result evidence for the task",
            "Result evidence recorded",
        )),
        "review" => Some(("submitted for review the task", "Submitted for review")),
        "done" => Some(("recorded completion of the task", "Completion recorded")),
        _ => None,
    }
}

fn eligible_outcome(object: &Value) -> bool {
    let title = object["title"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    if title.is_empty()
        || title.starts_with("test ")
        || title.starts_with("test:")
        || title.starts_with("synthetic ")
        || title.starts_with("fixture ")
    {
        return false;
    }
    let source = object["provenance"]["source_type"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase();
    !["test", "fixture", "import", "migration"]
        .iter()
        .any(|v| source.contains(v))
}

fn id(value: &Value, key: &str) -> Result<Uuid, db::DbError> {
    value[key]
        .as_str()
        .and_then(|v| Uuid::parse_str(v).ok())
        .ok_or_else(|| db::DbError::Invalid(format!("event is missing {key}")))
}

pub(crate) async fn journal(
    tx: &mut Transaction<'_, Postgres>,
    run: Uuid,
    sequence: i64,
    kind: &str,
    target: Uuid,
    before: Option<Value>,
    revision: i64,
) -> Result<(), db::DbError> {
    let actor = ActorContext::system(CAPTURE_ACTOR);
    db::insert_event_for_run_with_before(
        tx,
        run,
        sequence,
        &actor,
        kind,
        target,
        target,
        if before.is_some() {
            "updated"
        } else {
            "created"
        },
        None,
        before.as_ref().map(|_| revision - 1),
        revision,
        before,
    )
    .await?;
    Ok(())
}

pub async fn run_capture_worker(pool: PgPool, enabled: bool) {
    if !enabled {
        std::future::pending::<()>().await;
    }
    let mut timer = tokio::time::interval(std::time::Duration::from_secs(5));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        timer.tick().await;
        if let Err(error) = capture_outcomes(&pool).await {
            tracing::warn!(%error,"memory outcome capture failed; retrying next tick");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn task_milestones_require_changed_status_and_committed_evidence() {
        let mut event = json!({"action":"updated","before_state":{"subtype":{"status":"doing"}},"after_state":{"kind":"task","subtype":{"status":"review","brief_markdown":"Result: https://example.invalid/pull/1"}}});
        assert_eq!(task_milestone(&event).unwrap().1, "Submitted for review");
        event["before_state"]["subtype"]["status"] = json!("review");
        assert_eq!(
            task_milestone(&event).unwrap().1,
            "Result evidence recorded"
        );
        event["before_state"]["subtype"]["brief_markdown"] =
            event["after_state"]["subtype"]["brief_markdown"].clone();
        assert!(task_milestone(&event).is_none());
        event["before_state"]["subtype"]["status"] = json!("doing");
        event["after_state"]["subtype"]["brief_markdown"] = json!("No committed evidence");
        assert!(task_milestone(&event).is_none());
    }

    #[test]
    fn explicit_test_and_import_events_are_not_activity() {
        for title in ["TEST — saved source", "Synthetic example", "Fixture user"] {
            assert!(!eligible_outcome(&json!({"title":title})));
        }
        assert!(!eligible_outcome(
            &json!({"title":"Real source","provenance":{"source_type":"legacy_import"}})
        ));
        assert!(eligible_outcome(
            &json!({"title":"Evaluate memory capture","provenance":{"source_type":"owner_requested_task"}})
        ));
    }
}

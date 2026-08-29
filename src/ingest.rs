use std::{collections::BTreeSet, sync::Arc, time::Duration as StdDuration};

use axum::{
    Json, Router,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use subtle::ConstantTimeEq;
use time::{Duration, OffsetDateTime};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::{
    api::{ApiError, AppState},
    db::{self, DbError},
    domain::{ActorContext, ValidationError, allowed, optional_text, required_text},
};

const INGESTOR_ACTOR_ID: &str = "chat-ingestor";
const MAX_MESSAGES_PER_REQUEST: usize = 500;

#[derive(Clone, Debug)]
pub struct ApprovedSlackSurfaces {
    entries: Arc<BTreeSet<(String, String)>>,
}

impl ApprovedSlackSurfaces {
    pub fn parse(value: &str) -> Result<Self, String> {
        let mut entries = BTreeSet::new();
        for raw in value.split(',') {
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            let (workspace_id, channel_id) = raw.split_once(':').ok_or_else(|| {
                format!("approved Slack surface {raw:?} must be workspace_id:channel_id")
            })?;
            let workspace_id = workspace_id.trim();
            let channel_id = channel_id.trim();
            if workspace_id.is_empty() || channel_id.is_empty() {
                return Err(format!(
                    "approved Slack surface {raw:?} must contain non-empty IDs"
                ));
            }
            entries.insert((workspace_id.to_owned(), channel_id.to_owned()));
        }
        if entries.is_empty() {
            return Err("APPROVED_SLACK_SURFACES must contain at least one surface".to_owned());
        }
        Ok(Self {
            entries: Arc::new(entries),
        })
    }

    pub fn contains(&self, workspace_id: &str, channel_id: &str) -> bool {
        self.entries
            .contains(&(workspace_id.to_owned(), channel_id.to_owned()))
    }
}

#[derive(Clone)]
struct IngestState {
    pool: PgPool,
    approved_surfaces: ApprovedSlackSurfaces,
}

#[derive(Clone)]
struct IngestAuth {
    token: Arc<String>,
}

pub fn router(state: AppState, token: String, approved_surfaces: ApprovedSlackSurfaces) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .route(
            "/api/v1/ingest/slack/interactions",
            post(ingest_slack_interaction),
        )
        .with_state(IngestState {
            pool: state.pool,
            approved_surfaces,
        })
        .layer(middleware::from_fn_with_state(
            IngestAuth {
                token: Arc::new(token),
            },
            ingest_auth,
        ))
        .layer(TraceLayer::new_for_http())
}

async fn ingest_auth(
    State(auth): State<IngestAuth>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let bearer = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(ApiError::Unauthorized)?;
    let expected = auth.token.as_bytes();
    let supplied = bearer.as_bytes();
    if expected.len() != supplied.len() || expected.ct_eq(supplied).unwrap_u8() != 1 {
        return Err(ApiError::Unauthorized);
    }
    Ok(next.run(request).await)
}

async fn health() -> Json<Value> {
    Json(json!({"ok": true}))
}

async fn ready(State(state): State<IngestState>) -> Result<Json<Value>, ApiError> {
    db::ready(&state.pool).await?;
    Ok(Json(json!({"ok": true, "ready": true})))
}

#[derive(Clone, Debug, Deserialize)]
pub struct SlackSenderInput {
    pub provider_user_id: String,
    pub display_name: String,
    pub user_kind: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SlackMessageInput {
    pub provider_message_id: String,
    pub sender: SlackSenderInput,
    pub content: String,
    #[serde(with = "time::serde::rfc3339")]
    pub source_created_at: OffsetDateTime,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SlackInteractionInput {
    pub workspace_id: String,
    pub channel_id: String,
    pub thread_id: String,
    pub surface_kind: String,
    pub channel_name: Option<String>,
    pub title: Option<String>,
    pub messages: Vec<SlackMessageInput>,
    #[serde(default)]
    pub interaction_finished: bool,
}

#[derive(Clone, Debug)]
pub struct ValidatedSlackSender {
    provider_user_id: String,
    display_name: String,
    user_kind: String,
    avatar_url: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ValidatedSlackMessage {
    provider_message_id: String,
    sender: ValidatedSlackSender,
    content: String,
    source_created_at: OffsetDateTime,
}

#[derive(Clone, Debug)]
pub struct ValidatedSlackInteraction {
    workspace_id: String,
    channel_id: String,
    thread_id: String,
    surface_kind: String,
    channel_name: Option<String>,
    title: Option<String>,
    messages: Vec<ValidatedSlackMessage>,
    interaction_finished: bool,
}

impl SlackInteractionInput {
    pub fn validate(self) -> Result<ValidatedSlackInteraction, ValidationError> {
        if self.messages.is_empty() {
            return Err(ValidationError::Required("messages"));
        }
        if self.messages.len() > MAX_MESSAGES_PER_REQUEST {
            return Err(ValidationError::TooLong {
                field: "messages",
                max: MAX_MESSAGES_PER_REQUEST,
            });
        }
        let mut seen = BTreeSet::new();
        let mut messages = Vec::with_capacity(self.messages.len());
        for message in self.messages {
            let provider_message_id =
                required_text(message.provider_message_id, "provider_message_id", 300)?;
            if !seen.insert(provider_message_id.clone()) {
                return Err(ValidationError::Unsupported {
                    field: "duplicate provider_message_id",
                    value: provider_message_id,
                });
            }
            messages.push(ValidatedSlackMessage {
                provider_message_id,
                sender: ValidatedSlackSender {
                    provider_user_id: required_text(
                        message.sender.provider_user_id,
                        "sender.provider_user_id",
                        300,
                    )?,
                    display_name: required_text(
                        message.sender.display_name,
                        "sender.display_name",
                        300,
                    )?,
                    user_kind: allowed(
                        message.sender.user_kind,
                        "sender.user_kind",
                        &["human", "agent"],
                    )?,
                    avatar_url: validate_avatar_url(message.sender.avatar_url)?,
                },
                content: required_text(message.content, "content", 20_000)?,
                source_created_at: message.source_created_at,
            });
        }
        messages.sort_by(|a, b| {
            a.source_created_at
                .cmp(&b.source_created_at)
                .then_with(|| a.provider_message_id.cmp(&b.provider_message_id))
        });
        Ok(ValidatedSlackInteraction {
            workspace_id: required_text(self.workspace_id, "workspace_id", 300)?,
            channel_id: required_text(self.channel_id, "channel_id", 300)?,
            thread_id: required_text(self.thread_id, "thread_id", 300)?,
            surface_kind: allowed(self.surface_kind, "surface_kind", &["channel", "dm"])?,
            channel_name: optional_text(self.channel_name, "channel_name", 300)?,
            title: optional_text(self.title, "title", 300)?,
            messages,
            interaction_finished: self.interaction_finished,
        })
    }
}

fn validate_avatar_url(value: Option<String>) -> Result<Option<String>, ValidationError> {
    let value = optional_text(value, "sender.avatar_url", 2048)?;
    if value
        .as_deref()
        .is_some_and(|url| !url.starts_with("https://") && !url.starts_with("http://"))
    {
        return Err(ValidationError::Unsupported {
            field: "sender.avatar_url",
            value: value.unwrap_or_default(),
        });
    }
    Ok(value)
}

#[derive(Clone, Debug, Serialize)]
pub struct IngestResult {
    pub chat_object_id: Uuid,
    pub participant_object_ids: Vec<Uuid>,
    pub inserted_message_count: usize,
    pub duplicate_message_count: usize,
    pub curator_run_id: Option<Uuid>,
    pub interaction_state: &'static str,
}

async fn ingest_slack_interaction(
    State(state): State<IngestState>,
    Json(input): Json<SlackInteractionInput>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let input = input.validate()?;
    if !state
        .approved_surfaces
        .contains(&input.workspace_id, &input.channel_id)
    {
        return Err(ApiError::Forbidden(
            "This Slack surface is not approved for ingestion.".to_owned(),
        ));
    }
    let result = ingest(&state.pool, input).await?;
    Ok((StatusCode::ACCEPTED, Json(json!({"data": result}))))
}

pub async fn ingest(
    pool: &PgPool,
    input: ValidatedSlackInteraction,
) -> Result<IngestResult, DbError> {
    let actor = ActorContext::system(INGESTOR_ACTOR_ID);
    let mut tx = pool.begin().await?;
    advisory_lock(
        &mut tx,
        &format!(
            "slack-chat:{}:{}:{}",
            input.workspace_id, input.channel_id, input.thread_id
        ),
    )
    .await?;
    let chat_object_id = get_or_create_chat(&mut tx, &actor, &input).await?;
    let mut participants = BTreeSet::new();
    let mut inserted_message_count = 0usize;
    let mut last_ingested_message_id = None;

    for message in &input.messages {
        let user_object_id =
            get_or_create_user(&mut tx, &actor, &input.workspace_id, &message.sender).await?;
        participants.insert(user_object_id);
        ensure_participant_connection(
            &mut tx,
            &actor,
            chat_object_id,
            user_object_id,
            &message.sender.display_name,
        )
        .await?;
        if let Some(message_id) =
            insert_message(&mut tx, &actor, chat_object_id, user_object_id, message).await?
        {
            inserted_message_count += 1;
            last_ingested_message_id = Some(message_id);
        }
    }

    let latest_message_at = input
        .messages
        .iter()
        .map(|message| message.source_created_at)
        .max()
        .expect("validated interactions contain messages");
    sqlx::query(
        r#"UPDATE chats
           SET last_message_at=GREATEST(COALESCE(last_message_at, $2), $2),
               channel_name=COALESCE($3, channel_name),
               last_ingested_message_id=COALESCE($4,last_ingested_message_id),
               updated_at=now()
           WHERE object_id=$1"#,
    )
    .bind(chat_object_id)
    .bind(latest_message_at)
    .bind(&input.channel_name)
    .bind(last_ingested_message_id)
    .execute(&mut *tx)
    .await?;

    let curator_run_id = if input.interaction_finished {
        queue_next_window(&mut tx, &actor, chat_object_id, "explicit_finish").await?
    } else {
        None
    };
    tx.commit().await?;

    Ok(IngestResult {
        chat_object_id,
        participant_object_ids: participants.into_iter().collect(),
        inserted_message_count,
        duplicate_message_count: input.messages.len() - inserted_message_count,
        curator_run_id,
        interaction_state: if input.interaction_finished {
            "finished"
        } else {
            "open"
        },
    })
}

async fn advisory_lock(tx: &mut Transaction<'_, Postgres>, key: &str) -> Result<(), DbError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(key)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn get_or_create_chat(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    input: &ValidatedSlackInteraction,
) -> Result<Uuid, DbError> {
    if let Some(id) = sqlx::query_scalar(
        r#"SELECT object_id FROM chats
           WHERE provider='slack' AND workspace_id=$1 AND channel_id=$2 AND thread_id=$3"#,
    )
    .bind(&input.workspace_id)
    .bind(&input.channel_id)
    .bind(&input.thread_id)
    .fetch_optional(&mut **tx)
    .await?
    {
        return Ok(id);
    }

    let id = Uuid::new_v4();
    let title = input.title.clone().unwrap_or_else(|| {
        match (input.surface_kind.as_str(), input.channel_name.as_deref()) {
            ("channel", Some(name)) => format!("#{name} Slack conversation"),
            ("channel", None) => "Slack channel conversation".to_owned(),
            ("dm", _) => "Slack direct-message conversation".to_owned(),
            _ => unreachable!(),
        }
    });
    let description = match (input.surface_kind.as_str(), input.channel_name.as_deref()) {
        ("channel", Some(name)) => format!("A Slack conversation in the #{name} channel."),
        ("channel", None) => "A conversation in an approved Slack channel.".to_owned(),
        ("dm", _) => "A direct-message conversation with a Centaur agent on Slack.".to_owned(),
        _ => unreachable!(),
    };
    sqlx::query(
        r#"INSERT INTO objects
           (id,kind,title,description,created_by_type,created_by_id,
            updated_by_type,updated_by_id,provenance)
           VALUES ($1,'chat',$2,$3,$4,$5,$4,$5,$6)"#,
    )
    .bind(id)
    .bind(&title)
    .bind(&description)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(json!({
        "source_type": "slack",
        "source_ref": format!("{}:{}:{}", input.workspace_id, input.channel_id, input.thread_id)
    }))
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        r#"INSERT INTO chats
           (object_id,provider,workspace_id,channel_id,thread_id,surface_kind,channel_name)
           VALUES ($1,'slack',$2,$3,$4,$5,$6)"#,
    )
    .bind(id)
    .bind(&input.workspace_id)
    .bind(&input.channel_id)
    .bind(&input.thread_id)
    .bind(&input.surface_kind)
    .bind(&input.channel_name)
    .execute(&mut **tx)
    .await?;
    insert_event(
        tx,
        actor,
        "object",
        id,
        id,
        "created",
        Some(&format!(
            "slack-chat:{}:{}:{}",
            input.workspace_id, input.channel_id, input.thread_id
        )),
        json!({"kind": "chat", "title": title}),
    )
    .await?;
    Ok(id)
}

async fn get_or_create_user(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    workspace_id: &str,
    sender: &ValidatedSlackSender,
) -> Result<Uuid, DbError> {
    advisory_lock(
        tx,
        &format!("slack-user:{workspace_id}:{}", sender.provider_user_id),
    )
    .await?;
    if let Some((id, existing_kind)) = sqlx::query_as::<_, (Uuid, String)>(
        r#"SELECT e.user_object_id,u.user_kind
           FROM external_identities e JOIN users u ON u.object_id=e.user_object_id
           WHERE e.provider='slack' AND e.workspace_id=$1 AND e.provider_user_id=$2"#,
    )
    .bind(workspace_id)
    .bind(&sender.provider_user_id)
    .fetch_optional(&mut **tx)
    .await?
    {
        if existing_kind != sender.user_kind {
            return Err(DbError::Validation(ValidationError::Unsupported {
                field: "sender.user_kind for existing Slack identity",
                value: sender.user_kind.clone(),
            }));
        }
        sqlx::query(
            "UPDATE external_identities SET display_name=$2,avatar_url=COALESCE($3,avatar_url),updated_at=now() WHERE user_object_id=$1 AND provider='slack'",
        )
        .bind(id)
        .bind(&sender.display_name)
        .bind(&sender.avatar_url)
        .execute(&mut **tx)
        .await?;
        return Ok(id);
    }

    let id = Uuid::new_v4();
    let description = if sender.user_kind == "human" {
        format!("A human Slack user named {}.", sender.display_name)
    } else {
        format!("A Centaur agent on Slack named {}.", sender.display_name)
    };
    sqlx::query(
        r#"INSERT INTO objects
           (id,kind,title,description,created_by_type,created_by_id,
            updated_by_type,updated_by_id,provenance)
           VALUES ($1,'user',$2,$3,$4,$5,$4,$5,$6)"#,
    )
    .bind(id)
    .bind(&sender.display_name)
    .bind(description)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(json!({
        "source_type": "slack",
        "source_ref": format!("{workspace_id}:{}", sender.provider_user_id)
    }))
    .execute(&mut **tx)
    .await?;
    sqlx::query("INSERT INTO users (object_id,user_kind) VALUES ($1,$2)")
        .bind(id)
        .bind(&sender.user_kind)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        r#"INSERT INTO external_identities
           (id,user_object_id,provider,workspace_id,provider_user_id,display_name,avatar_url)
           VALUES ($1,$2,'slack',$3,$4,$5,$6)"#,
    )
    .bind(Uuid::new_v4())
    .bind(id)
    .bind(workspace_id)
    .bind(&sender.provider_user_id)
    .bind(&sender.display_name)
    .bind(&sender.avatar_url)
    .execute(&mut **tx)
    .await?;
    insert_event(
        tx,
        actor,
        "object",
        id,
        id,
        "created",
        Some(&format!(
            "slack-user:{workspace_id}:{}",
            sender.provider_user_id
        )),
        json!({"kind": "user", "user_kind": sender.user_kind}),
    )
    .await?;
    Ok(id)
}

async fn ensure_participant_connection(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    chat_object_id: Uuid,
    user_object_id: Uuid,
    display_name: &str,
) -> Result<(), DbError> {
    let id = Uuid::new_v4();
    let inserted: Option<Uuid> = sqlx::query_scalar(
        r#"INSERT INTO connections
           (id,source_object_id,kind,target_object_id,description,
            created_by_type,created_by_id,updated_by_type,updated_by_id,provenance)
           VALUES ($1,$2,'involves',$3,$4,$5,$6,$5,$6,$7)
           ON CONFLICT (source_object_id,kind,target_object_id)
               WHERE archived_at IS NULL DO NOTHING
           RETURNING id"#,
    )
    .bind(id)
    .bind(chat_object_id)
    .bind(user_object_id)
    .bind(format!("This Slack conversation includes {display_name}."))
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(json!({"source_type": "slack_ingestion"}))
    .fetch_optional(&mut **tx)
    .await?;
    if inserted.is_some() {
        insert_event(
            tx,
            actor,
            "connection",
            id,
            chat_object_id,
            "connected",
            Some(&format!(
                "slack-participant:{chat_object_id}:{user_object_id}"
            )),
            json!({"kind": "involves", "target_object_id": user_object_id}),
        )
        .await?;
    }
    Ok(())
}

async fn insert_message(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    chat_object_id: Uuid,
    user_object_id: Uuid,
    message: &ValidatedSlackMessage,
) -> Result<Option<Uuid>, DbError> {
    let id = Uuid::new_v4();
    let inserted: Option<Uuid> = sqlx::query_scalar(
        r#"INSERT INTO chat_messages
           (id,chat_object_id,provider_message_id,sender_user_object_id,content,source_created_at)
           VALUES ($1,$2,$3,$4,$5,$6)
           ON CONFLICT (chat_object_id,provider_message_id) DO NOTHING
           RETURNING id"#,
    )
    .bind(id)
    .bind(chat_object_id)
    .bind(&message.provider_message_id)
    .bind(user_object_id)
    .bind(&message.content)
    .bind(message.source_created_at)
    .fetch_optional(&mut **tx)
    .await?;
    if inserted.is_none() {
        return Ok(None);
    }
    insert_event(
        tx,
        actor,
        "chat_message",
        id,
        chat_object_id,
        "message_ingested",
        Some(&format!(
            "slack-message:{chat_object_id}:{}",
            message.provider_message_id
        )),
        json!({
            "provider_message_id": message.provider_message_id,
            "sender_user_object_id": user_object_id
        }),
    )
    .await?;
    Ok(Some(id))
}

async fn queue_next_window(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    chat_object_id: Uuid,
    trigger: &str,
) -> Result<Option<Uuid>, DbError> {
    let messages: Vec<(Uuid,)> = sqlx::query_as(
        r#"SELECT m.id
           FROM chat_messages m
           WHERE m.chat_object_id=$1
             AND m.ingested_sequence > COALESCE(
                 (SELECT previous.ingested_sequence FROM chat_messages previous
                  JOIN chats c ON c.last_queued_message_id=previous.id
                  WHERE c.object_id=$1), 0)
           ORDER BY m.ingested_sequence"#,
    )
    .bind(chat_object_id)
    .fetch_all(&mut **tx)
    .await?;
    let Some((first_message_id,)) = messages.first().copied() else {
        return Ok(None);
    };
    let (last_message_id,) = messages.last().copied().expect("non-empty message window");
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO curator_runs
           (id,chat_object_id,first_message_id,last_message_id,trigger,message_count,idempotency_key)
           VALUES ($1,$2,$3,$4,$5,$6,$7)"#,
    )
    .bind(id)
    .bind(chat_object_id)
    .bind(first_message_id)
    .bind(last_message_id)
    .bind(trigger)
    .bind(i32::try_from(messages.len()).expect("message request is bounded"))
    .bind(format!(
        "curator-window:{chat_object_id}:{last_message_id}"
    ))
    .execute(&mut **tx)
    .await?;
    sqlx::query("UPDATE chats SET last_queued_message_id=$2,updated_at=now() WHERE object_id=$1")
        .bind(chat_object_id)
        .bind(last_message_id)
        .execute(&mut **tx)
        .await?;
    insert_event(
        tx,
        actor,
        "curator_run",
        id,
        chat_object_id,
        "curator_queued",
        Some(&format!(
            "curator-window:{chat_object_id}:{last_message_id}"
        )),
        json!({
            "trigger": trigger,
            "first_message_id": first_message_id,
            "last_message_id": last_message_id,
            "message_count": messages.len()
        }),
    )
    .await?;
    Ok(Some(id))
}

pub async fn queue_inactive_interactions(
    pool: &PgPool,
    inactivity: StdDuration,
) -> Result<usize, DbError> {
    let inactivity = Duration::try_from(inactivity).map_err(|_| {
        DbError::Validation(ValidationError::Unsupported {
            field: "inactivity duration",
            value: "out of range".to_owned(),
        })
    })?;
    let cutoff = OffsetDateTime::now_utc() - inactivity;
    let actor = ActorContext::system(INGESTOR_ACTOR_ID);
    let mut tx = pool.begin().await?;
    let chats: Vec<(Uuid,)> = sqlx::query_as(
        r#"SELECT c.object_id
           FROM chats c
           WHERE c.provider='slack' AND c.last_message_at <= $1
             AND EXISTS (
                 SELECT 1 FROM chat_messages m
                 WHERE m.chat_object_id=c.object_id
                   AND m.ingested_sequence > COALESCE(
                       (SELECT previous.ingested_sequence FROM chat_messages previous
                        WHERE previous.id=c.last_queued_message_id), 0)
             )
           ORDER BY c.last_message_at,c.object_id
           LIMIT 100 FOR UPDATE SKIP LOCKED"#,
    )
    .bind(cutoff)
    .fetch_all(&mut *tx)
    .await?;
    let mut queued = 0usize;
    for (chat_object_id,) in chats {
        if queue_next_window(&mut tx, &actor, chat_object_id, "inactivity")
            .await?
            .is_some()
        {
            queued += 1;
        }
    }
    tx.commit().await?;
    Ok(queued)
}

#[allow(clippy::too_many_arguments)]
async fn insert_event(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    entity_type: &str,
    entity_id: Uuid,
    object_id: Uuid,
    action: &str,
    idempotency_key: Option<&str>,
    changes: Value,
) -> Result<(), DbError> {
    sqlx::query(
        r#"INSERT INTO object_events
           (id,entity_type,entity_id,object_id,action,actor_type,actor_id,
            centaur_thread_key,centaur_execution_id,idempotency_key,to_revision,changes)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,1,$11)"#,
    )
    .bind(Uuid::new_v4())
    .bind(entity_type)
    .bind(entity_id)
    .bind(object_id)
    .bind(action)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(&actor.centaur_thread_key)
    .bind(&actor.centaur_execution_id)
    .bind(idempotency_key)
    .bind(changes)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approved_surfaces_are_exact() {
        let approved = ApprovedSlackSurfaces::parse("T1:C1, T1:D2").unwrap();
        assert!(approved.contains("T1", "C1"));
        assert!(approved.contains("T1", "D2"));
        assert!(!approved.contains("T1", "C2"));
        assert!(!approved.contains("T2", "C1"));
    }

    #[test]
    fn approved_surfaces_reject_wildcards_and_bad_entries() {
        assert!(ApprovedSlackSurfaces::parse("").is_err());
        assert!(ApprovedSlackSurfaces::parse("T1").is_err());
        assert!(ApprovedSlackSurfaces::parse(":C1").is_err());
    }

    #[test]
    fn provider_avatar_references_require_http_urls() {
        assert_eq!(
            validate_avatar_url(Some(" https://example.test/avatar.png ".to_owned())).unwrap(),
            Some("https://example.test/avatar.png".to_owned())
        );
        assert!(validate_avatar_url(Some("data:image/png;base64,secret".to_owned())).is_err());
        assert!(validate_avatar_url(Some("javascript:alert(1)".to_owned())).is_err());
        assert_eq!(validate_avatar_url(None).unwrap(), None);
    }
}

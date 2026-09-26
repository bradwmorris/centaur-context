//! Opt-in Codex transport. Host configuration, not model arguments, owns identity.
use std::{collections::HashSet, net::SocketAddr, sync::Arc};

use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Request, State},
    http::header,
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use subtle::ConstantTimeEq;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    api::{self, ApiError, AppState},
    db::{self, DbError},
    domain::ActorContext,
};

#[derive(Clone)]
pub struct CodexConfig {
    pub addr: SocketAddr,
    pub capture_token: String,
    pub tool_token: String,
    pub host_id: Uuid,
    pub human_id: Uuid,
    pub repositories: HashSet<String>,
    pub curate: bool,
}

#[derive(Clone)]
struct Session {
    id: Uuid,
    repository: String,
    config: Arc<CodexConfig>,
}

pub fn router(state: AppState, config: CodexConfig) -> Router {
    Router::new()
        .route("/api/v2/codex/capture", post(capture))
        .route("/api/v2/codex/session", get(session_status))
        .route("/api/v2/contract", get(api::read_contract))
        .route(
            "/api/v2/audit/objects",
            get(crate::maintenance::audit_objects),
        )
        .route(
            "/api/v2/audit/connections",
            get(crate::maintenance::audit_connections),
        )
        .route("/api/v2/search", post(api::universal_search))
        .route("/api/v2/read", post(api::universal_read))
        .route("/api/v2/apply", post(api::universal_apply))
        .layer(DefaultBodyLimit::max(512 * 1024))
        .layer(middleware::from_fn_with_state(
            Arc::new(config),
            authenticate,
        ))
        .with_state(state)
}

async fn authenticate(
    State(config): State<Arc<CodexConfig>>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let session = authenticate_request(&config, &request)?;
    request.extensions_mut().insert(ActorContext {
        actor_type: "centaur_agent",
        actor_id: format!("codex:{}", config.host_id),
        centaur_thread_key: Some(format!(
            "codex:{}:{}:{}",
            config.host_id, session.repository, session.id
        )),
        centaur_execution_id: None,
        is_agent: true,
    });
    request.extensions_mut().insert(session);
    Ok(next.run(request).await)
}

fn authenticate_request(config: &Arc<CodexConfig>, request: &Request) -> Result<Session, ApiError> {
    let supplied = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(ApiError::Unauthorized)?;
    let expected = if request.uri().path() == "/api/v2/codex/capture" {
        &config.capture_token
    } else {
        &config.tool_token
    };
    if supplied.len() != expected.len()
        || supplied.as_bytes().ct_eq(expected.as_bytes()).unwrap_u8() != 1
    {
        return Err(ApiError::Unauthorized);
    }
    let header = |name| request.headers().get(name).and_then(|v| v.to_str().ok());
    let id = header("x-codex-session-id")
        .and_then(|v| Uuid::parse_str(v).ok())
        .ok_or_else(|| ApiError::BadRequest("X-Codex-Session-Id must be a UUID".into()))?;
    let repository = header("x-codex-repository").unwrap_or_default().to_owned();
    if !config.repositories.contains(&repository) {
        return Err(ApiError::Forbidden(
            "Repository is not bound to this Context instance".into(),
        ));
    }
    Ok(Session {
        id,
        repository,
        config: config.clone(),
    })
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureBatch {
    pub version: u32,
    pub batch_id: Uuid,
    pub title: String,
    pub messages: Vec<CapturedMessage>,
    pub finished_turn_id: Option<Uuid>,
    #[serde(default)]
    pub git_receipts: Vec<crate::codex_outcomes::GitReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapturedMessage {
    pub id: String,
    pub turn_id: Uuid,
    pub role: String,
    pub content: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

fn invalid(message: &str) -> ApiError {
    ApiError::BadRequest(message.to_owned())
}

async fn capture(
    State(state): State<AppState>,
    Extension(session): Extension<Session>,
    Json(input): Json<CaptureBatch>,
) -> Result<Json<Value>, ApiError> {
    if input.version != 1 || input.messages.len() > 100 || input.git_receipts.len() > 1 {
        return Err(invalid(
            "Capture version must be 1 and batches contain at most 100 messages",
        ));
    }
    crate::domain::required_text(input.title.clone(), "title", 300)?;
    if input.coverage.as_deref().is_some_and(|v| {
        !matches!(
            v,
            "registered"
                | "unavailable"
                | "partial"
                | "text_messages"
                | "text_only_attachments_omitted"
        )
    }) {
        return Err(invalid("Unsupported capture coverage"));
    }
    for receipt in &input.git_receipts {
        crate::codex_outcomes::verify(receipt)?;
    }
    let mut ids = HashSet::new();
    for message in &input.messages {
        crate::domain::required_text(message.id.clone(), "message.id", 180)?;
        crate::domain::required_text(message.content.clone(), "message.content", 20_000)?;
        if !matches!(message.role.as_str(), "human" | "agent") || !ids.insert(&message.id) {
            return Err(invalid(
                "Message roles must be human/agent and IDs unique within a batch",
            ));
        }
    }
    let hash = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&input).expect("serializable input"))
    );
    let key = format!(
        "{}:{}:{}",
        session.config.host_id, session.id, input.batch_id
    );
    let actor = ActorContext::system(format!("codex-capture:{}", session.config.host_id));
    let mut tx = state.pool.begin().await.map_err(DbError::from)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("codex:{}:{}", session.config.host_id, session.id))
        .execute(&mut *tx)
        .await
        .map_err(DbError::from)?;
    let prior: Option<(Value, Value)> = sqlx::query_as(
        "SELECT input,result FROM runs WHERE kind='codex_interaction' AND idempotency_key=$1",
    )
    .bind(&key)
    .fetch_optional(&mut *tx)
    .await
    .map_err(DbError::from)?;
    if let Some((prior, result)) = prior {
        if prior["payload_sha256"] != hash || prior["repository"] != session.repository {
            return Err(ApiError::IdempotencyConflict);
        }
        return Ok(Json(json!({"data":result})));
    }
    let human_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users u JOIN objects o ON o.id=u.object_id WHERE u.object_id=$1 AND u.user_kind='human' AND o.archived_at IS NULL)")
        .bind(session.config.human_id).fetch_one(&mut *tx).await.map_err(DbError::from)?;
    if !human_exists {
        return Err(invalid("Configured canonical human User is unavailable"));
    }
    let run_id = Uuid::new_v4();
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,input,started_at) VALUES($1,'codex_interaction','running','system',$2,$3,$4,now())")
        .bind(run_id).bind(&actor.actor_id).bind(&key)
        .bind(json!({"payload_sha256":hash,"host_id":session.config.host_id,"session_id":session.id,"repository":session.repository,"finished_turn_id":input.finished_turn_id}))
        .execute(&mut *tx).await.map_err(DbError::from)?;
    crate::runs::set_context(&mut tx, run_id).await?;
    let chat_id = ensure_chat(&mut tx, &actor, run_id, &session, &input.title).await?;
    let agent_id = ensure_agent(&mut tx, &actor, run_id, &session).await?;
    sqlx::query("UPDATE runs SET chat_object_id=$2,primary_object_id=$2 WHERE id=$1")
        .bind(run_id)
        .bind(chat_id)
        .execute(&mut *tx)
        .await
        .map_err(DbError::from)?;
    for participant in [session.config.human_id, agent_id] {
        crate::ingest::ensure_participant_connection(
            &mut tx,
            &actor,
            run_id,
            chat_id,
            participant,
            if participant == agent_id {
                "Codex"
            } else {
                "the configured human"
            },
        )
        .await?;
    }
    let mut inserted = 0;
    for message in &input.messages {
        let sender = if message.role == "human" {
            session.config.human_id
        } else {
            agent_id
        };
        let message_id = format!("{}:{}", message.turn_id, message.id);
        let prior: Option<(Uuid,String,OffsetDateTime)> = sqlx::query_as("SELECT sender_user_object_id,content,source_created_at FROM chat_messages WHERE chat_object_id=$1 AND provider_message_id=$2")
            .bind(chat_id).bind(&message_id).fetch_optional(&mut *tx).await.map_err(DbError::from)?;
        if let Some(prior) = prior {
            if prior != (sender, message.content.clone(), message.created_at) {
                return Err(ApiError::IdempotencyConflict);
            }
            continue;
        }
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO chat_messages(id,chat_object_id,provider_message_id,sender_user_object_id,content,source_created_at) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(id).bind(chat_id).bind(&message_id).bind(sender).bind(&message.content).bind(message.created_at)
            .execute(&mut *tx).await.map_err(DbError::from)?;
        crate::runs::append_trace(&mut tx,run_id,"message_ingested",json!({"message_id":id,"provider_message_id":message_id,"turn_id":message.turn_id,"sender_user_object_id":sender})).await?;
        inserted += 1;
    }
    sqlx::query("UPDATE chats SET latest_source_message_at=(SELECT max(source_created_at) FROM chat_messages WHERE chat_object_id=$1),processing_updated_at=now() WHERE object_id=$1")
        .bind(chat_id).execute(&mut *tx).await.map_err(DbError::from)?;
    // Receipt-only batches must not downgrade established coverage.
    let before = db::target_snapshot(&mut tx, "object", chat_id).await?;
    let coverage = input.coverage.clone().unwrap_or_else(|| {
        if inserted > 0 {
            "partial".into()
        } else {
            before["provenance"]["capture_coverage"]
                .as_str()
                .unwrap_or("registered")
                .to_owned()
        }
    });
    // A provider receipt describes only the observed activation window, never all history.
    if before["provenance"]["capture_coverage"] != coverage {
        sqlx::query("UPDATE objects SET provenance=provenance || jsonb_build_object('capture_coverage',$2::text,'capture_scope','visible text from activation','capture_recovery_action',CASE WHEN $2 IN ('registered','unavailable','partial') THEN 'Restore the original transcript and verify its session identity and coverage before recovery.' ELSE 'Attachments and pre-activation history are outside capture coverage.' END),revision=revision+1,updated_at=now() WHERE id=$1")
            .bind(chat_id).bind(&coverage).execute(&mut *tx).await.map_err(DbError::from)?;
        let sequence: i64 = sqlx::query_scalar(
            "SELECT COALESCE(max(sequence),0)::bigint+1 FROM object_events WHERE run_id=$1",
        )
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(DbError::from)?;
        let revision = before["revision"].as_i64().unwrap_or(1);
        db::insert_event_for_run_with_before(
            &mut tx,
            run_id,
            sequence,
            &actor,
            "object",
            chat_id,
            chat_id,
            "updated",
            None,
            Some(revision),
            revision + 1,
            Some(before),
        )
        .await?;
    }
    let mut curator_run_ids = Vec::new();
    // Incoming batches contain <=100 messages. Drain at most two bounded windows
    // on finish; while running, queue full windows so evidence cannot grow unbounded.
    if session.config.curate {
        for _ in 0..2 {
            let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM chat_messages m WHERE m.chat_object_id=$1 AND m.ingestion_sequence>COALESCE((SELECT previous.ingestion_sequence FROM chat_messages previous JOIN chats c ON c.curation_queued_through_message_id=previous.id WHERE c.object_id=$1),0)")
                .bind(chat_id).fetch_one(&mut *tx).await.map_err(DbError::from)?;
            if pending == 0 || (input.finished_turn_id.is_none() && pending < 100) {
                break;
            }
            if let Some(id) = crate::ingest::queue_next_window(
                &mut tx,
                &actor,
                run_id,
                chat_id,
                "explicit_finish",
                None,
            )
            .await?
            {
                curator_run_ids.push(id);
            }
        }
    }
    let curator_run_id = curator_run_ids.first().copied();
    let mut outcome_memories = Vec::new();
    if session.config.curate {
        for receipt in &input.git_receipts {
            if let Some(id) = crate::codex_outcomes::capture(
                &mut tx,
                &actor,
                session.config.host_id,
                &session.repository,
                chat_id,
                receipt,
            )
            .await?
            {
                outcome_memories.push(id);
            }
        }
    }
    let result = json!({"capture_coverage":coverage,"outcome_memory_ids":outcome_memories,"run_id":run_id,"chat_object_id":chat_id,"inserted_messages":inserted,"curator_run_id":curator_run_id,"curator_run_ids":curator_run_ids,"curation_enabled":session.config.curate});
    sqlx::query("UPDATE runs SET status='completed',result=$2,completed_at=now() WHERE id=$1")
        .bind(run_id)
        .bind(&result)
        .execute(&mut *tx)
        .await
        .map_err(DbError::from)?;
    tx.commit().await.map_err(DbError::from)?;
    Ok(Json(json!({"data":result})))
}

async fn ensure_chat(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    run: Uuid,
    session: &Session,
    title: &str,
) -> Result<Uuid, DbError> {
    let prior: Option<(Uuid,String,bool)> = sqlx::query_as("SELECT c.object_id,c.channel_id,o.archived_at IS NOT NULL FROM chats c JOIN objects o ON o.id=c.object_id WHERE c.provider='codex' AND c.workspace_id=$1 AND c.thread_id=$2")
        .bind(session.config.host_id.to_string()).bind(session.id.to_string()).fetch_optional(&mut **tx).await?;
    if let Some((id, repo, archived)) = prior {
        if repo != session.repository || archived {
            return Err(DbError::Invalid(
                "Codex session target is immutable and must be active".into(),
            ));
        }
        return Ok(id);
    }
    let id = Uuid::new_v4();
    let description = format!(
        "Codex session registered for {}. Conversation content and coverage have not yet been established.",
        session.repository
    );
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,'chat',$2,$3,'system',$4,'system',$4,$5)")
        .bind(id).bind(title).bind(description).bind(&actor.actor_id)
        .bind(json!({"source_type":"codex","host_id":session.config.host_id,"session_id":session.id,"repository":session.repository,"capture_coverage":"registered"}))
        .execute(&mut **tx).await?;
    sqlx::query("INSERT INTO chats(object_id,provider,workspace_id,channel_id,thread_id,surface_kind) VALUES($1,'codex',$2,$3,$4,'desktop')")
        .bind(id).bind(session.config.host_id.to_string()).bind(&session.repository).bind(session.id.to_string()).execute(&mut **tx).await?;
    journal(tx, actor, run, "object", id).await?;
    Ok(id)
}

async fn ensure_agent(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    run: Uuid,
    session: &Session,
) -> Result<Uuid, DbError> {
    let id = Uuid::new_v5(&session.config.host_id, b"centaur-context-codex-agent");
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(id.to_string())
        .execute(&mut **tx)
        .await?;
    let existing: Option<(String,bool)> = sqlx::query_as("SELECT u.user_kind,o.archived_at IS NOT NULL FROM users u JOIN objects o ON o.id=u.object_id WHERE u.object_id=$1")
        .bind(id).fetch_optional(&mut **tx).await?;
    if let Some((kind, archived)) = existing {
        if kind != "agent" || archived {
            return Err(DbError::Invalid(
                "Codex agent identity must be active".into(),
            ));
        }
        return Ok(id);
    }
    sqlx::query("INSERT INTO objects(id,kind,title,description,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance) VALUES($1,'user','Codex','The Codex agent on the configured desktop host.','system',$2,'system',$2,$3)")
        .bind(id).bind(&actor.actor_id).bind(json!({"source_type":"codex","host_id":session.config.host_id})).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO users(object_id,user_kind,identities) VALUES($1,'agent',$2)")
        .bind(id).bind(json!([{"id":Uuid::new_v4(),"provider":"centaur","provider_user_id":format!("codex:{}",session.config.host_id)}])).execute(&mut **tx).await?;
    journal(tx, actor, run, "object", id).await?;
    Ok(id)
}

pub(crate) async fn journal(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    run: Uuid,
    kind: &str,
    id: Uuid,
) -> Result<(), DbError> {
    let sequence: i64 = sqlx::query_scalar(
        "SELECT COALESCE(max(sequence),0)::bigint+1 FROM object_events WHERE run_id=$1",
    )
    .bind(run)
    .fetch_one(&mut **tx)
    .await?;
    db::insert_event_for_run_with_before(
        tx, run, sequence, actor, kind, id, id, "created", None, None, 1, None,
    )
    .await?;
    Ok(())
}

async fn session_status(
    State(state): State<AppState>,
    Extension(session): Extension<Session>,
) -> Result<Json<Value>, ApiError> {
    let chat: Option<Uuid>=sqlx::query_scalar("SELECT c.object_id FROM chats c JOIN objects o ON o.id=c.object_id WHERE c.provider='codex' AND c.workspace_id=$1 AND c.channel_id=$2 AND c.thread_id=$3 AND o.archived_at IS NULL")
        .bind(session.config.host_id.to_string()).bind(&session.repository).bind(session.id.to_string()).fetch_optional(&state.pool).await.map_err(DbError::from)?;
    let latest: Option<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'status',status,'error',error) FROM runs WHERE kind='curator' AND chat_object_id=$1 ORDER BY created_at DESC,id DESC LIMIT 1")
        .bind(chat).fetch_optional(&state.pool).await.map_err(DbError::from)?;
    let capture: Option<Value> = sqlx::query_scalar("SELECT jsonb_build_object('coverage',COALESCE(o.provenance->>'capture_coverage',CASE WHEN EXISTS(SELECT 1 FROM chat_messages m WHERE m.chat_object_id=o.id) THEN 'partial' ELSE 'registered' END),'message_count',(SELECT count(*) FROM chat_messages m WHERE m.chat_object_id=o.id),'scope','visible text from activation','recovery_action',o.provenance->>'capture_recovery_action') FROM objects o WHERE o.id=$1")
        .bind(chat).fetch_optional(&state.pool).await.map_err(DbError::from)?;
    Ok(Json(
        json!({"data":{"chat_object_id":chat,"capture":capture,"curation_enabled":session.config.curate,"latest_curation":latest}}),
    ))
}

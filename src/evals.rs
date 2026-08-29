use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::db::DbError;

pub const VERDICTS: &[&str] = &["unreviewed", "pass", "mixed", "fail"];

#[derive(Clone, Debug, Deserialize, Default)]
pub struct EvalFilter {
    pub kind: Option<String>,
    pub status: Option<String>,
    pub verdict: Option<String>,
    pub component: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub execution_type: Option<String>,
    pub auth_mode: Option<String>,
    pub billing_mode: Option<String>,
    pub object_id: Option<Uuid>,
    pub from: Option<OffsetDateTime>,
    pub to: Option<OffsetDateTime>,
    pub before: Option<OffsetDateTime>,
    pub limit: i64,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct EvalSummary {
    pub id: Uuid,
    pub kind: String,
    pub status: String,
    pub actor_type: String,
    pub actor_id: String,
    pub chat_object_id: Option<Uuid>,
    pub curator_run_id: Option<Uuid>,
    pub summary: String,
    pub error_summary: Option<String>,
    pub verdict: String,
    pub notes: Option<String>,
    pub annotated_by: Option<String>,
    pub annotation_revision: i64,
    pub affected_object_count: i64,
    pub total_tokens: i64,
    pub estimated_micro_usd: Option<i64>,
    pub chatgpt_credit_microunits: Option<i64>,
    pub usage_sources: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub completed_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct TraceEntry {
    pub id: Uuid,
    pub eval_id: Uuid,
    pub sequence: i64,
    pub entry_type: String,
    pub component: Option<String>,
    pub provider: Option<String>,
    pub model_id: Option<String>,
    pub display_tier: Option<String>,
    pub execution_type: Option<String>,
    pub auth_mode: Option<String>,
    pub upstream_service: Option<String>,
    pub billing_mode: Option<String>,
    pub reasoning_effort: Option<String>,
    pub service_tier: Option<String>,
    pub source_thread_id: Option<String>,
    pub source_execution_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub usage_status: String,
    pub usage_missing_reason: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_creation_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub estimated_micro_usd: Option<i64>,
    pub chatgpt_credit_microunits: Option<i64>,
    pub api_equivalent_micro_usd: Option<i64>,
    pub rate_card_version: Option<String>,
    pub pricing_snapshot: Option<Value>,
    pub facts: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct EvalObject {
    pub object_id: Uuid,
    pub role: String,
    pub kind: String,
    pub title: String,
    pub lifecycle: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct EvalDetail {
    pub eval: EvalSummary,
    pub trace: Vec<TraceEntry>,
    pub objects: Vec<EvalObject>,
}

const SUMMARY_SELECT: &str = r#"SELECT e.id,e.kind,e.status,e.actor_type,e.actor_id,e.chat_object_id,e.curator_run_id,
 e.summary,e.error_summary,e.verdict,e.notes,e.annotated_by,e.annotation_revision,e.created_at,e.updated_at,e.completed_at,
 (SELECT count(DISTINCT eo.object_id) FROM eval_objects eo WHERE eo.eval_id=e.id) AS affected_object_count,
 COALESCE((SELECT sum(COALESCE(t.total_tokens,0)) FROM eval_trace_entries t WHERE t.eval_id=e.id AND t.entry_type='model_attempt'),0)::bigint AS total_tokens,
 (SELECT sum(t.estimated_micro_usd)::bigint FROM eval_trace_entries t WHERE t.eval_id=e.id AND t.entry_type='model_attempt') AS estimated_micro_usd,
 (SELECT sum(t.chatgpt_credit_microunits)::bigint FROM eval_trace_entries t WHERE t.eval_id=e.id AND t.entry_type='model_attempt') AS chatgpt_credit_microunits,
 COALESCE((SELECT jsonb_agg(source ORDER BY source->>'provider',source->>'model_id') FROM (
   SELECT DISTINCT jsonb_build_object('component',t.component,'provider',t.provider,'model_id',t.model_id,'display_tier',t.display_tier,
     'execution_type',t.execution_type,'auth_mode',t.auth_mode,'billing_mode',t.billing_mode,'usage_status',t.usage_status) AS source
   FROM eval_trace_entries t WHERE t.eval_id=e.id AND t.entry_type='model_attempt'
 ) sources),'[]'::jsonb) AS usage_sources FROM evals e"#;

pub async fn list(pool: &PgPool, filter: EvalFilter) -> Result<Vec<EvalSummary>, DbError> {
    let mut query = QueryBuilder::<Postgres>::new(SUMMARY_SELECT);
    query.push(" WHERE true");
    if let Some(value) = filter.kind {
        query.push(" AND e.kind=").push_bind(value);
    }
    if let Some(value) = filter.status {
        query.push(" AND e.status=").push_bind(value);
    }
    if let Some(value) = filter.verdict {
        query.push(" AND e.verdict=").push_bind(value);
    }
    if let Some(value) = filter.from {
        query.push(" AND e.created_at>=").push_bind(value);
    }
    if let Some(value) = filter.to {
        query.push(" AND e.created_at<=").push_bind(value);
    }
    if let Some(value) = filter.before {
        query.push(" AND e.created_at<").push_bind(value);
    }
    if let Some(value) = filter.object_id {
        query
            .push(" AND EXISTS(SELECT 1 FROM eval_objects f WHERE f.eval_id=e.id AND f.object_id=")
            .push_bind(value)
            .push(")");
    }
    for (column, value) in [
        ("component", filter.component),
        ("provider", filter.provider),
        ("model_id", filter.model),
        ("execution_type", filter.execution_type),
        ("auth_mode", filter.auth_mode),
        ("billing_mode", filter.billing_mode),
    ] {
        if let Some(value) = value {
            query
                .push(" AND EXISTS(SELECT 1 FROM eval_trace_entries f WHERE f.eval_id=e.id AND f.")
                .push(column)
                .push("=")
                .push_bind(value)
                .push(")");
        }
    }
    query
        .push(" ORDER BY e.created_at DESC,e.id DESC LIMIT ")
        .push_bind(filter.limit.clamp(1, 100));
    Ok(query.build_query_as().fetch_all(pool).await?)
}

pub async fn detail(pool: &PgPool, id: Uuid) -> Result<EvalDetail, DbError> {
    let eval = sqlx::query_as::<_, EvalSummary>(&format!("{SUMMARY_SELECT} WHERE e.id=$1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)?;
    let trace = sqlx::query_as::<_, TraceEntry>(
        "SELECT * FROM eval_trace_entries WHERE eval_id=$1 ORDER BY sequence",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    let objects = sqlx::query_as::<_, EvalObject>(
        "SELECT eo.object_id,eo.role,o.kind,o.title,o.lifecycle FROM eval_objects eo JOIN objects o ON o.id=eo.object_id WHERE eo.eval_id=$1 ORDER BY eo.created_at,eo.object_id,eo.role")
        .bind(id).fetch_all(pool).await?;
    Ok(EvalDetail {
        eval,
        trace,
        objects,
    })
}

pub async fn annotate(
    pool: &PgPool,
    id: Uuid,
    verdict: &str,
    notes: Option<&str>,
    annotator: &str,
    expected_revision: i64,
) -> Result<EvalSummary, DbError> {
    let updated = sqlx::query_scalar::<_, Uuid>(
        "UPDATE evals SET verdict=$2,notes=$3,annotated_by=$4,annotated_at=now(),annotation_revision=annotation_revision+1,updated_at=now() WHERE id=$1 AND annotation_revision=$5 RETURNING id")
        .bind(id).bind(verdict).bind(notes).bind(annotator).bind(expected_revision)
        .fetch_optional(pool).await?.ok_or(DbError::Conflict)?;
    Ok(detail(pool, updated).await?.eval)
}

pub async fn set_context(tx: &mut Transaction<'_, Postgres>, eval_id: Uuid) -> Result<(), DbError> {
    sqlx::query("SELECT centaur_context_set_eval_context($1)")
        .bind(eval_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub async fn open_slack_interaction(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: &str,
    channel_id: &str,
    thread_id: &str,
) -> Result<(Uuid, bool), DbError> {
    let key = format!("slack-open:{workspace_id}:{channel_id}:{thread_id}");
    if let Some(eval_id) =
        sqlx::query_scalar("SELECT id FROM evals WHERE idempotency_key=$1 FOR UPDATE")
            .bind(&key)
            .fetch_optional(&mut **tx)
            .await?
    {
        set_context(tx, eval_id).await?;
        return Ok((eval_id, false));
    }
    let id = Uuid::new_v4();
    let eval_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO evals (id,kind,status,actor_type,actor_id,summary,idempotency_key)
           VALUES ($1,'slack_interaction','open','system','chat-ingestor','Slack interaction awaiting curation',$2)
           RETURNING id"#,
    )
    .bind(id)
    .bind(key)
    .fetch_one(&mut **tx)
    .await?;
    set_context(tx, eval_id).await?;
    Ok((eval_id, true))
}

pub async fn attach_slack_chat(
    tx: &mut Transaction<'_, Postgres>,
    eval_id: Uuid,
    chat_object_id: Uuid,
) -> Result<(), DbError> {
    sqlx::query("UPDATE evals SET chat_object_id=$2,summary='Slack interaction for Chat ' || $2::text,updated_at=now() WHERE id=$1")
        .bind(eval_id).bind(chat_object_id).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO eval_objects (eval_id,object_id,role) VALUES ($1,$2,'consulted') ON CONFLICT DO NOTHING")
        .bind(eval_id).bind(chat_object_id).execute(&mut **tx).await?;
    Ok(())
}

pub async fn link_object(
    tx: &mut Transaction<'_, Postgres>,
    eval_id: Uuid,
    object_id: Uuid,
    role: &str,
) -> Result<(), DbError> {
    sqlx::query("INSERT INTO eval_objects (eval_id,object_id,role) VALUES ($1,$2,$3) ON CONFLICT DO NOTHING")
        .bind(eval_id).bind(object_id).bind(role).execute(&mut **tx).await?;
    Ok(())
}

pub async fn append_trace(
    tx: &mut Transaction<'_, Postgres>,
    eval_id: Uuid,
    entry_type: &str,
    facts: Value,
) -> Result<(), DbError> {
    sqlx::query("SELECT centaur_context_append_trace($1,$2,$3)")
        .bind(eval_id)
        .bind(entry_type)
        .bind(facts)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub async fn attach_curator_run(
    tx: &mut Transaction<'_, Postgres>,
    eval_id: Uuid,
    run_id: Uuid,
) -> Result<(), DbError> {
    sqlx::query("UPDATE evals SET curator_run_id=$2,status='running',idempotency_key='curator-run:' || $2::text,updated_at=now() WHERE id=$1")
        .bind(eval_id).bind(run_id).execute(&mut **tx).await?;
    append_trace(
        tx,
        eval_id,
        "commit",
        serde_json::json!({"phase":"curator_queued","curator_run_id":run_id}),
    )
    .await
}

pub async fn resume_slack_eval(
    tx: &mut Transaction<'_, Postgres>,
    chat_object_id: Uuid,
) -> Result<Option<Uuid>, DbError> {
    let eval_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM evals WHERE kind='slack_interaction' AND chat_object_id=$1 AND curator_run_id IS NULL AND status='open' ORDER BY created_at LIMIT 1 FOR UPDATE",
    ).bind(chat_object_id).fetch_optional(&mut **tx).await?;
    if let Some(id) = eval_id {
        set_context(tx, id).await?;
    }
    Ok(eval_id)
}

pub async fn set_curator_context(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
) -> Result<Option<Uuid>, DbError> {
    let eval_id: Option<Uuid> = sqlx::query_scalar("SELECT id FROM evals WHERE curator_run_id=$1")
        .bind(run_id)
        .fetch_optional(&mut **tx)
        .await?;
    if let Some(id) = eval_id {
        set_context(tx, id).await?;
        sqlx::query("UPDATE evals SET status='running',error_summary=NULL,completed_at=NULL,updated_at=now() WHERE id=$1 AND status NOT IN ('completed','reversed')")
            .bind(id).execute(&mut **tx).await?;
    }
    Ok(eval_id)
}

pub async fn finish_curator_eval(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    change_count: i32,
) -> Result<(), DbError> {
    if let Some(eval_id) = set_curator_context(tx, run_id).await? {
        append_trace(
            tx,
            eval_id,
            "commit",
            serde_json::json!({"curator_run_id":run_id,"change_count":change_count}),
        )
        .await?;
        sqlx::query(
            "UPDATE evals SET status='completed',error_summary=NULL,completed_at=now(),updated_at=now() WHERE id=$1",
        )
        .bind(eval_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub async fn fail_curator_eval(pool: &PgPool, run_id: Uuid, message: &str) -> Result<(), DbError> {
    let mut tx = pool.begin().await?;
    if let Some(eval_id) = set_curator_context(&mut tx, run_id).await? {
        let summary = message.chars().take(4000).collect::<String>();
        append_trace(
            &mut tx,
            eval_id,
            "failure",
            serde_json::json!({"error":summary}),
        )
        .await?;
        sqlx::query("UPDATE evals SET status='failed',error_summary=$2,completed_at=now(),updated_at=now() WHERE id=$1")
            .bind(eval_id).bind(summary).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn append_curator_trace(
    pool: &PgPool,
    run_id: Uuid,
    entry_type: &str,
    facts: Value,
) -> Result<(), DbError> {
    let mut tx = pool.begin().await?;
    if let Some(eval_id) = set_curator_context(&mut tx, run_id).await? {
        append_trace(&mut tx, eval_id, entry_type, facts).await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn link_curator_candidates(
    pool: &PgPool,
    run_id: Uuid,
    object_ids: &[Uuid],
) -> Result<(), DbError> {
    let mut tx = pool.begin().await?;
    if let Some(eval_id) = set_curator_context(&mut tx, run_id).await? {
        for object_id in object_ids {
            link_object(&mut tx, eval_id, *object_id, "consulted").await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

pub async fn reverse_curator_eval(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    reversed_change_count: usize,
) -> Result<(), DbError> {
    if let Some(eval_id) = set_curator_context(tx, run_id).await? {
        append_trace(tx, eval_id, "reversal", serde_json::json!({"curator_run_id":run_id,"reversed_change_count":reversed_change_count})).await?;
        sqlx::query("UPDATE evals SET status='reversed',completed_at=COALESCE(completed_at,now()),updated_at=now() WHERE id=$1")
            .bind(eval_id).execute(&mut **tx).await?;
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize)]
pub struct NormalizedUsage {
    pub eval_id: Uuid,
    pub component: String,
    pub provider: String,
    pub model_id: String,
    pub display_tier: Option<String>,
    pub execution_type: String,
    pub auth_mode: String,
    pub upstream_service: String,
    pub billing_mode: String,
    pub reasoning_effort: Option<String>,
    pub service_tier: Option<String>,
    pub source_thread_id: Option<String>,
    pub source_execution_id: String,
    pub source_turn_id: Option<String>,
    pub usage_status: String,
    pub usage_missing_reason: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_creation_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub estimated_micro_usd: Option<i64>,
    pub chatgpt_credit_microunits: Option<i64>,
    pub api_equivalent_micro_usd: Option<i64>,
    pub rate_card_version: Option<String>,
    pub pricing_snapshot: Option<Value>,
}

impl NormalizedUsage {
    pub fn validate(&self) -> Result<(), String> {
        for (field, value, max) in [
            ("component", self.component.as_str(), 100),
            ("provider", self.provider.as_str(), 100),
            ("model_id", self.model_id.as_str(), 200),
            ("upstream_service", self.upstream_service.as_str(), 200),
            (
                "source_execution_id",
                self.source_execution_id.as_str(),
                300,
            ),
        ] {
            if value.trim().is_empty() || value.len() > max {
                return Err(format!("{field} must contain 1 to {max} characters"));
            }
        }
        if !["codex_harness", "direct_api", "embedding", "other"]
            .contains(&self.execution_type.as_str())
        {
            return Err("execution_type is unsupported".into());
        }
        if ![
            "chatgpt_subscription",
            "api_key",
            "not_applicable",
            "unknown",
        ]
        .contains(&self.auth_mode.as_str())
        {
            return Err("auth_mode is unsupported".into());
        }
        if ![
            "subscription_allowance",
            "chatgpt_credits",
            "metered_api",
            "not_applicable",
            "unknown",
        ]
        .contains(&self.billing_mode.as_str())
        {
            return Err("billing_mode is unsupported".into());
        }
        if !["reported", "partial", "unavailable", "not_applicable"]
            .contains(&self.usage_status.as_str())
        {
            return Err("usage_status is unsupported".into());
        }
        if self.usage_status == "reported"
            && self.input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.total_tokens.is_none()
        {
            return Err("reported usage requires at least one token count".into());
        }
        if self.usage_status == "not_applicable"
            && [
                self.input_tokens,
                self.output_tokens,
                self.cache_creation_tokens,
                self.cache_read_tokens,
                self.reasoning_tokens,
                self.total_tokens,
                self.estimated_micro_usd,
                self.chatgpt_credit_microunits,
                self.api_equivalent_micro_usd,
            ]
            .iter()
            .any(Option::is_some)
        {
            return Err("not_applicable usage cannot include tokens or charges".into());
        }
        for (field, value) in [
            ("input_tokens", self.input_tokens),
            ("output_tokens", self.output_tokens),
            ("cache_creation_tokens", self.cache_creation_tokens),
            ("cache_read_tokens", self.cache_read_tokens),
            ("reasoning_tokens", self.reasoning_tokens),
            ("total_tokens", self.total_tokens),
            ("estimated_micro_usd", self.estimated_micro_usd),
            ("chatgpt_credit_microunits", self.chatgpt_credit_microunits),
            ("api_equivalent_micro_usd", self.api_equivalent_micro_usd),
        ] {
            if value.is_some_and(|number| number < 0) {
                return Err(format!("{field} must not be negative"));
            }
        }
        if let (Some(input), Some(output), Some(total)) =
            (self.input_tokens, self.output_tokens, self.total_tokens)
            && input
                .checked_add(output)
                .is_none_or(|minimum| total < minimum)
        {
            return Err("total_tokens must include input_tokens and output_tokens".into());
        }
        if self.billing_mode == "metered_api" {
            match (&self.rate_card_version, &self.pricing_snapshot) {
                (Some(_), Some(snapshot)) => {
                    let calculated = metered_micro_usd(self, snapshot)?;
                    if self.estimated_micro_usd != Some(calculated) {
                        return Err(
                            "estimated_micro_usd does not match the saved pricing snapshot".into(),
                        );
                    }
                }
                (None, None) if self.estimated_micro_usd.is_none() => {}
                _ => {
                    return Err(
                        "metered pricing provenance must be entirely present or explicitly unavailable"
                            .into(),
                    );
                }
            }
        }
        if matches!(
            self.billing_mode.as_str(),
            "subscription_allowance" | "chatgpt_credits"
        ) && self.estimated_micro_usd.is_some()
        {
            return Err("subscription usage cannot report per-trace billed USD".into());
        }
        if matches!(self.usage_status.as_str(), "partial" | "unavailable")
            && self
                .usage_missing_reason
                .as_deref()
                .is_none_or(str::is_empty)
        {
            return Err("partial or unavailable usage requires usage_missing_reason".into());
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct MeteredRateCard {
    input_micro_usd_per_million: i64,
    output_micro_usd_per_million: i64,
    #[serde(default)]
    cache_creation_micro_usd_per_million: i64,
    #[serde(default)]
    cache_read_micro_usd_per_million: i64,
    #[serde(default)]
    reasoning_micro_usd_per_million: i64,
}

pub fn metered_micro_usd(input: &NormalizedUsage, snapshot: &Value) -> Result<i64, String> {
    let rates: MeteredRateCard = serde_json::from_value(snapshot.clone())
        .map_err(|_| "pricing_snapshot has an invalid metered rate card".to_owned())?;
    let cache_creation = input.cache_creation_tokens.unwrap_or(0);
    let cache_read = input.cache_read_tokens.unwrap_or(0);
    let reasoning = input.reasoning_tokens.unwrap_or(0);
    let uncached_input = input
        .input_tokens
        .unwrap_or(0)
        .checked_sub(cache_creation)
        .and_then(|value| value.checked_sub(cache_read))
        .filter(|value| *value >= 0)
        .ok_or_else(|| "cache token categories exceed input_tokens".to_owned())?;
    let standard_output = input
        .output_tokens
        .unwrap_or(0)
        .checked_sub(reasoning)
        .filter(|value| *value >= 0)
        .ok_or_else(|| "reasoning_tokens exceeds output_tokens".to_owned())?;
    let terms = [
        (uncached_input, rates.input_micro_usd_per_million),
        (standard_output, rates.output_micro_usd_per_million),
        (cache_creation, rates.cache_creation_micro_usd_per_million),
        (cache_read, rates.cache_read_micro_usd_per_million),
        (reasoning, rates.reasoning_micro_usd_per_million),
    ];
    let numerator = terms.into_iter().try_fold(0_i128, |sum, (tokens, rate)| {
        if rate < 0 {
            return Err("pricing rates must not be negative".to_owned());
        }
        sum.checked_add(i128::from(tokens) * i128::from(rate))
            .ok_or_else(|| "pricing arithmetic overflow".to_owned())
    })?;
    i64::try_from((numerator + 500_000) / 1_000_000)
        .map_err(|_| "pricing arithmetic overflow".to_owned())
}

pub async fn record_usage(pool: &PgPool, input: &NormalizedUsage) -> Result<Uuid, DbError> {
    let mut tx = pool.begin().await?;
    let id = record_usage_in_tx(&mut tx, input).await?;
    tx.commit().await?;
    Ok(id)
}

pub async fn record_usage_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    input: &NormalizedUsage,
) -> Result<Uuid, DbError> {
    let sequence: i64 = sqlx::query_scalar("UPDATE evals SET next_sequence=next_sequence+1,updated_at=now() WHERE id=$1 RETURNING next_sequence")
        .bind(input.eval_id).fetch_optional(&mut **tx).await?.ok_or(DbError::NotFound)?;
    let id = Uuid::new_v4();
    let inserted = sqlx::query_scalar::<_, Uuid>(r#"INSERT INTO eval_trace_entries
      (id,eval_id,sequence,entry_type,component,provider,model_id,display_tier,execution_type,auth_mode,upstream_service,billing_mode,
       reasoning_effort,service_tier,source_thread_id,source_execution_id,source_turn_id,usage_status,usage_missing_reason,
       input_tokens,output_tokens,cache_creation_tokens,cache_read_tokens,reasoning_tokens,total_tokens,estimated_micro_usd,
       chatgpt_credit_microunits,api_equivalent_micro_usd,rate_card_version,pricing_snapshot,facts)
      VALUES ($1,$2,$3,'model_attempt',$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27,$28,$29,'{}')
      ON CONFLICT DO NOTHING RETURNING id"#)
        .bind(id).bind(input.eval_id).bind(sequence).bind(&input.component).bind(&input.provider).bind(&input.model_id)
        .bind(&input.display_tier).bind(&input.execution_type).bind(&input.auth_mode).bind(&input.upstream_service).bind(&input.billing_mode)
        .bind(&input.reasoning_effort).bind(&input.service_tier).bind(&input.source_thread_id).bind(&input.source_execution_id)
        .bind(&input.source_turn_id).bind(&input.usage_status).bind(&input.usage_missing_reason).bind(input.input_tokens).bind(input.output_tokens)
        .bind(input.cache_creation_tokens).bind(input.cache_read_tokens).bind(input.reasoning_tokens).bind(input.total_tokens)
        .bind(input.estimated_micro_usd).bind(input.chatgpt_credit_microunits).bind(input.api_equivalent_micro_usd)
        .bind(&input.rate_card_version).bind(&input.pricing_snapshot).fetch_optional(&mut **tx).await?;
    let id = if let Some(inserted) = inserted {
        inserted
    } else {
        sqlx::query_scalar("SELECT id FROM eval_trace_entries WHERE eval_id=$1 AND entry_type='model_attempt' AND component=$2 AND source_execution_id=$3 AND COALESCE(source_turn_id,'')=COALESCE($4,'')")
            .bind(input.eval_id).bind(&input.component).bind(&input.source_execution_id).bind(&input.source_turn_id)
            .fetch_one(&mut **tx).await?
    };
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn usage() -> NormalizedUsage {
        NormalizedUsage {
            eval_id: Uuid::nil(),
            component: "agent".into(),
            provider: "openai".into(),
            model_id: "fixture".into(),
            display_tier: None,
            execution_type: "codex_harness".into(),
            auth_mode: "api_key".into(),
            upstream_service: "api.openai.com".into(),
            billing_mode: "metered_api".into(),
            reasoning_effort: None,
            service_tier: None,
            source_thread_id: None,
            source_execution_id: "execution-1".into(),
            source_turn_id: None,
            usage_status: "reported".into(),
            usage_missing_reason: None,
            input_tokens: Some(1_000),
            output_tokens: Some(500),
            cache_creation_tokens: Some(100),
            cache_read_tokens: Some(200),
            reasoning_tokens: Some(50),
            total_tokens: Some(1_500),
            estimated_micro_usd: Some(4_800),
            chatgpt_credit_microunits: None,
            api_equivalent_micro_usd: None,
            rate_card_version: Some("fixture-v1".into()),
            pricing_snapshot: Some(json!({
                "input_micro_usd_per_million": 2_000_000,
                "output_micro_usd_per_million": 6_000_000,
                "cache_creation_micro_usd_per_million": 2_000_000,
                "cache_read_micro_usd_per_million": 1_000_000,
                "reasoning_micro_usd_per_million": 6_000_000
            })),
        }
    }

    #[test]
    fn metered_cost_does_not_double_count_cached_or_reasoning_tokens() {
        let input = usage();
        assert_eq!(
            metered_micro_usd(&input, input.pricing_snapshot.as_ref().unwrap()).unwrap(),
            4_800
        );
        assert!(input.validate().is_ok());
    }

    #[test]
    fn subscription_usage_never_claims_billed_usd() {
        let mut input = usage();
        input.billing_mode = "subscription_allowance".into();
        input.estimated_micro_usd = Some(0);
        assert!(
            input
                .validate()
                .unwrap_err()
                .contains("per-trace billed USD")
        );
    }

    #[test]
    fn incomplete_usage_requires_a_reason() {
        let mut input = usage();
        input.billing_mode = "unknown".into();
        input.pricing_snapshot = None;
        input.rate_card_version = None;
        input.estimated_micro_usd = None;
        input.usage_status = "partial".into();
        assert!(
            input
                .validate()
                .unwrap_err()
                .contains("usage_missing_reason")
        );
    }
}

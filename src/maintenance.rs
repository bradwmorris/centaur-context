//! Owner-reviewed maintenance on the existing intake listener.
use std::{collections::HashSet, sync::Arc};

use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Query, Request, State},
    http::header,
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::{
    api::AppState,
    domain::ActorContext,
    intake::IntakeError,
    universal::{self, ApplyOperation, ApplyRequest, ObjectReference},
};

#[derive(Clone)]
pub struct MaintenanceConfig {
    pub api_token: String,
    pub allowed_principal: String,
    pub approved_request_hashes: HashSet<String>,
}

#[derive(Clone)]
pub(crate) struct MaintenanceState {
    pub(crate) app: AppState,
    pub(crate) config: Arc<MaintenanceConfig>,
}

pub(crate) fn router(app: AppState, config: MaintenanceConfig) -> Router {
    let state = MaintenanceState {
        app: app.clone(),
        config: Arc::new(config),
    };
    Router::new()
        .route("/api/v2/maintenance/apply", post(apply))
        .route(
            "/api/v2/maintenance/tables",
            get(crate::reviewed_purge::catalog),
        )
        .route(
            "/api/v2/maintenance/table-rows",
            get(crate::reviewed_purge::audit_rows),
        )
        .route(
            "/api/v2/maintenance/purge",
            post(crate::reviewed_purge::purge),
        )
        .route("/api/v2/maintenance/objects", get(objects))
        .route("/api/v2/maintenance/connections", get(connections))
        .with_state(state.clone())
        .merge(crate::api::maintenance_read_router(app))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(state, authenticate))
}

async fn authenticate(
    State(state): State<MaintenanceState>,
    mut request: Request,
    next: Next,
) -> Result<Response, IntakeError> {
    let bearer = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(IntakeError::Unauthorized)?;
    let expected = state.config.api_token.as_bytes();
    if expected.len() != bearer.len() || expected.ct_eq(bearer.as_bytes()).unwrap_u8() != 1 {
        return Err(IntakeError::Unauthorized);
    }
    let principal = crate::intake::required_header(request.headers(), "x-centaur-principal-id")?;
    if principal != state.config.allowed_principal {
        return Err(IntakeError::Forbidden(
            "maintenance principal is not authorized".into(),
        ));
    }
    let thread_key = crate::intake::required_header(request.headers(), "x-centaur-thread-key")?;
    let execution_id = crate::intake::optional_header(request.headers(), "x-centaur-execution-id")?;
    request.extensions_mut().insert(ActorContext {
        actor_type: "centaur_agent",
        actor_id: principal,
        centaur_thread_key: Some(thread_key),
        centaur_execution_id: execution_id,
        is_agent: true,
    });
    Ok(next.run(request).await)
}

/// The server supplies this normalized digest during validation. No caller-supplied
/// approval or authority flag can authorize a commit.
pub fn approval_hash(request: &ApplyRequest) -> Result<String, crate::db::DbError> {
    let mut normalized = request.clone();
    normalized.validate_only = false;
    universal::request_hash(&normalized)
}

fn validate_scope(request: &ApplyRequest) -> Result<(), IntakeError> {
    if request.chat_object_id.is_some() {
        return Err(IntakeError::BadRequest(
            "maintenance cannot create implicit Chat relationships".into(),
        ));
    }
    for operation in &request.operations {
        match operation {
            ApplyOperation::UpdateDescription { .. } => {}
            ApplyOperation::PromoteSourceArtifact { .. } => {}
            ApplyOperation::UpdateObject { changes, .. } => {
                const FIELDS: &[&str] = &[
                    "title",
                    "description",
                    "content",
                    "content_format",
                    "intent",
                    "canonical_uri",
                ];
                if changes.keys().any(|key| !FIELDS.contains(&key.as_str())) {
                    return Err(IntakeError::BadRequest("maintenance allows only research text, intent and source identity corrections".into()));
                }
            }
            ApplyOperation::ArchiveObject { .. }
            | ApplyOperation::UpdateConnection { .. }
            | ApplyOperation::ArchiveConnection { .. } => {}
            ApplyOperation::CreateConnection {
                source: ObjectReference::Id { .. },
                target: ObjectReference::Id { .. },
                ..
            }
            | ApplyOperation::AppendArtifact {
                object: ObjectReference::Id { .. },
                ..
            } => {}
            _ => {
                return Err(IntakeError::BadRequest(
                    "maintenance requires explicit existing IDs and cannot create Objects".into(),
                ));
            }
        }
    }
    Ok(())
}

async fn apply(
    State(state): State<MaintenanceState>,
    Extension(actor): Extension<ActorContext>,
    Json(request): Json<ApplyRequest>,
) -> Result<Json<Value>, IntakeError> {
    validate_scope(&request)?;
    let hash = approval_hash(&request)?;
    if !request.validate_only && !state.config.approved_request_hashes.contains(&hash) {
        return Err(IntakeError::Forbidden(
            "exact maintenance request has not been approved".into(),
        ));
    }
    let result = universal::apply_maintenance(&state.app.pool, &actor, request).await?;
    Ok(Json(json!({"data":result,"approval_sha256":hash})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InventoryQuery {
    cursor: Option<Uuid>,
    limit: Option<i64>,
    lifecycle: Option<String>,
}

async fn objects(
    State(state): State<MaintenanceState>,
    Query(query): Query<InventoryQuery>,
) -> Result<Json<Value>, IntakeError> {
    inventory(&state.app, query, false).await
}
async fn connections(
    State(state): State<MaintenanceState>,
    Query(query): Query<InventoryQuery>,
) -> Result<Json<Value>, IntakeError> {
    inventory(&state.app, query, true).await
}
async fn inventory(
    app: &AppState,
    query: InventoryQuery,
    connections: bool,
) -> Result<Json<Value>, IntakeError> {
    let limit = query.limit.unwrap_or(100);
    if !(1..=200).contains(&limit)
        || !matches!(
            query.lifecycle.as_deref(),
            None | Some("active" | "archived")
        )
    {
        return Err(IntakeError::BadRequest(
            "inventory requires limit 1..200 and optional active/archived lifecycle".into(),
        ));
    }
    // Only these two internal constants may enter the query text.
    let table = if connections {
        "connections"
    } else {
        "objects"
    };
    let sql = format!(
        "SELECT to_jsonb(t) FROM {table} t WHERE ($1::uuid IS NULL OR id>$1) AND ($2::text IS NULL OR ($2='active' AND archived_at IS NULL) OR ($2='archived' AND archived_at IS NOT NULL)) ORDER BY id LIMIT $3"
    );
    let mut rows: Vec<Value> = sqlx::query_scalar(&sql)
        .bind(query.cursor)
        .bind(query.lifecycle)
        .bind(limit + 1)
        .fetch_all(&app.pool)
        .await?;
    let more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let cursor = if more {
        rows.last().map(|row| row["id"].clone())
    } else {
        None
    };
    Ok(Json(json!({"data":rows,"next_cursor":cursor})))
}

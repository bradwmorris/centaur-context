//! Small controlled vocabulary with the same attributed Run history as Object writes.
use crate::{
    db::DbError,
    domain::{ActorContext, required_text},
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub async fn catalogue(pool: &PgPool) -> Result<Value, DbError> {
    let rows: Vec<Value> =
        sqlx::query_scalar("SELECT to_jsonb(c) FROM entity_categories c ORDER BY slug,id")
            .fetch_all(pool)
            .await?;
    let hash = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&rows).expect("JSON values"))
    );
    Ok(json!({"categories":rows,"revision":hash}))
}
fn text(fields: &Map<String, Value>, key: &'static str, max: usize) -> Result<String, DbError> {
    Ok(required_text(
        fields
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| DbError::Invalid(format!("{key} must be text")))?
            .to_owned(),
        key,
        max,
    )?)
}
pub async fn mutate(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    id: Uuid,
    revision: Option<i64>,
    action: &str,
    changes: &Map<String, Value>,
) -> Result<Value, DbError> {
    for key in changes.keys() {
        if !["slug", "label", "definition", "aliases", "legacy_kind"].contains(&key.as_str()) {
            return Err(DbError::Invalid(format!("unknown category field {key}")));
        }
    }
    let mut fields = if let Some(revision) = revision {
        let value: Value = sqlx::query_scalar(
            "SELECT to_jsonb(c) FROM entity_categories c WHERE id=$1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(DbError::NotFound)?;
        if value["revision"].as_i64() != Some(revision) {
            return Err(DbError::Conflict);
        }
        if changes.contains_key("slug") {
            return Err(DbError::Invalid("category slug is immutable".into()));
        }
        value.as_object().cloned().expect("category row")
    } else {
        Map::new()
    };
    fields.extend(changes.clone());
    let label = text(&fields, "label", 100)?;
    let definition = text(&fields, "definition", 1000)?;
    let slug = text(&fields, "slug", 100)?;
    if !valid_slug(&slug) {
        return Err(DbError::Invalid(
            "slug must be normalized lowercase words separated by hyphens".into(),
        ));
    }
    let legacy = text(&fields, "legacy_kind", 32)?;
    crate::domain::allowed(
        legacy.clone(),
        "legacy_kind",
        &[
            "person",
            "organization",
            "product",
            "project",
            "publication",
            "place",
            "concept",
            "other",
        ],
    )?;
    let aliases: Vec<String> =
        serde_json::from_value(fields.get("aliases").cloned().unwrap_or(json!([])))
            .map_err(|_| DbError::Invalid("aliases must be a list of text".into()))?;
    validate_names(&label, &aliases)?;
    if revision.is_none() {
        sqlx::query("INSERT INTO entity_categories(id,slug,label,definition,aliases,legacy_kind,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$7,$8)").bind(id).bind(slug).bind(label).bind(definition).bind(aliases).bind(legacy).bind(actor.actor_type).bind(&actor.actor_id).execute(&mut **tx).await?;
    } else {
        sqlx::query("UPDATE entity_categories SET label=$2,definition=$3,aliases=$4,legacy_kind=$5,revision=revision+1,updated_by_type=$6,updated_by_id=$7,updated_at=now(),archived_at=CASE WHEN $8='archive' THEN now() WHEN $8='restore' THEN NULL ELSE archived_at END WHERE id=$1")
        .bind(id).bind(label).bind(definition).bind(aliases).bind(legacy).bind(actor.actor_type).bind(&actor.actor_id).bind(action).execute(&mut **tx).await?;
    }
    crate::db::target_snapshot(tx, "entity_category", id).await
}
fn valid_slug(s: &str) -> bool {
    s.len() <= 100
        && s.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && s.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}
fn normalized_name(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
fn validate_names(label: &str, aliases: &[String]) -> Result<(), DbError> {
    if aliases.len() > 20 {
        return Err(DbError::Invalid("at most 20 category aliases".into()));
    }
    let mut seen = std::collections::BTreeSet::new();
    for name in std::iter::once(label).chain(aliases.iter().map(String::as_str)) {
        if name.trim().is_empty()
            || name.chars().count() > 100
            || !seen.insert(normalized_name(name))
        {
            return Err(DbError::Invalid("category label and aliases must be distinct nonempty names of at most 100 characters".into()));
        }
    }
    Ok(())
}
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityFilter {
    pub category_ids: Vec<Uuid>,
    #[serde(default = "any_match", rename = "match")]
    pub match_mode: String,
    pub cursor: Option<Uuid>,
}
fn any_match() -> String {
    "any".into()
}
pub async fn filtered(
    pool: &PgPool,
    query: &str,
    filter: EntityFilter,
    limit: i64,
) -> Result<Value, DbError> {
    if filter.category_ids.is_empty()
        || filter.category_ids.len() > 32
        || !["any", "all"].contains(&filter.match_mode.as_str())
    {
        return Err(DbError::Invalid(
            "entity_filters require 1–32 category IDs and match any or all".into(),
        ));
    }
    let count = filter
        .category_ids
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    if count != filter.category_ids.len() {
        return Err(DbError::Invalid("duplicate category filter IDs".into()));
    }
    let mut rows:Vec<Value>=sqlx::query_scalar("SELECT to_jsonb(o) FROM objects o WHERE o.kind='entity' AND o.archived_at IS NULL AND ($1='' OR strpos(lower(o.title || ' ' || o.description),lower($1))>0) AND ($2::uuid IS NULL OR o.id>$2) AND (SELECT count(*) FROM entity_category_assignments a WHERE a.entity_object_id=o.id AND a.category_id=ANY($3)) >= $4 ORDER BY o.id LIMIT $5")
        .bind(query).bind(filter.cursor).bind(filter.category_ids).bind(if filter.match_mode=="all" {count as i64} else {1}).bind(limit+1).fetch_all(pool).await?;
    let has_more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let cursor = if has_more {
        rows.last().and_then(|r| r.get("id")).cloned()
    } else {
        None
    };
    Ok(
        json!({"objects":rows,"next_cursor":cursor,"order":"id_asc","contract_version":crate::contract::version(),"tool_version":crate::contract::tool_version()}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names() {
        assert!(valid_slug("cost-model"));
        assert!(!valid_slug("Cost Model"));
        assert!(!valid_slug("model--x"));
        assert!(validate_names("Model", &[" MODEL ".into()]).is_err());
        assert!(validate_names("Model", &["Estimator".into()]).is_ok());
    }
}

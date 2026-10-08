//! Canonical classification writes; legacy writers are also guarded by database triggers.
use crate::db::DbError;
use serde_json::{Map, Value};
use sqlx::{Postgres, Transaction};
use std::collections::BTreeSet;
use uuid::Uuid;

pub async fn write(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    fields: &Map<String, Value>,
    create: bool,
) -> Result<(), DbError> {
    let legacy = fields
        .get("entity_kind")
        .map(|v| {
            v.as_str()
                .ok_or_else(|| DbError::Invalid("entity_kind must be text".into()))
        })
        .transpose()?;
    let canonical =
        fields.contains_key("category_ids") || fields.contains_key("primary_category_id");
    if !canonical {
        let legacy = legacy.ok_or_else(|| {
            DbError::Invalid(
                "provide category_ids and primary_category_id, or legacy entity_kind".into(),
            )
        })?;
        let query = if create {
            "INSERT INTO entities(object_id,entity_kind) VALUES($1,$2)"
        } else {
            "UPDATE entities SET entity_kind=$2 WHERE object_id=$1"
        };
        sqlx::query(query)
            .bind(id)
            .bind(legacy)
            .execute(&mut **tx)
            .await?;
        return Ok(());
    }
    let current: Option<Value> = if create {
        None
    } else {
        sqlx::query_scalar("SELECT jsonb_build_object('primary_category_id',primary_category_id,'category_ids',(SELECT jsonb_agg(category_id ORDER BY category_id) FROM entity_category_assignments WHERE entity_object_id=$1)) FROM entities WHERE object_id=$1").bind(id).fetch_optional(&mut **tx).await?
    };
    let category_values = fields
        .get("category_ids")
        .or_else(|| current.as_ref().and_then(|c| c.get("category_ids")))
        .ok_or_else(|| DbError::Invalid("category_ids is required".into()))?;
    let ids: Vec<Uuid> = serde_json::from_value(category_values.clone())
        .map_err(|_| DbError::Invalid("category_ids must be UUIDs".into()))?;
    let primary: Uuid = serde_json::from_value(
        fields
            .get("primary_category_id")
            .or_else(|| current.as_ref().and_then(|c| c.get("primary_category_id")))
            .cloned()
            .ok_or_else(|| DbError::Invalid("primary_category_id is required".into()))?,
    )
    .map_err(|_| DbError::Invalid("primary_category_id must be a UUID".into()))?;
    validate_membership(&ids, primary)?;
    let categories:Vec<(Uuid,String, bool)>=sqlx::query_as("SELECT id,legacy_kind,archived_at IS NOT NULL FROM entity_categories WHERE id=ANY($1) ORDER BY id FOR SHARE").bind(&ids).fetch_all(&mut **tx).await?;
    if categories.len() != ids.len() {
        return Err(DbError::Invalid(
            "unknown category ID; refresh entity_categories".into(),
        ));
    }
    let old: Vec<Uuid> = if create {
        vec![]
    } else {
        sqlx::query_scalar(
            "SELECT category_id FROM entity_category_assignments WHERE entity_object_id=$1",
        )
        .bind(id)
        .fetch_all(&mut **tx)
        .await?
    };
    if categories
        .iter()
        .any(|(id, _, archived)| *archived && !old.contains(id))
    {
        return Err(DbError::Invalid(
            "cannot newly assign an archived category".into(),
        ));
    }
    let projection = &categories
        .iter()
        .find(|(id, _, _)| *id == primary)
        .expect("validated primary")
        .1;
    if legacy.is_some_and(|l| l != projection) {
        return Err(DbError::Invalid(
            "entity_kind conflicts with primary category legacy_kind".into(),
        ));
    }
    let query = if create {
        "INSERT INTO entities(object_id,entity_kind,primary_category_id) VALUES($1,$2,$3)"
    } else {
        "UPDATE entities SET entity_kind=$2,primary_category_id=$3 WHERE object_id=$1"
    };
    sqlx::query(query)
        .bind(id)
        .bind(projection)
        .bind(primary)
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM entity_category_assignments WHERE entity_object_id=$1 AND NOT(category_id=ANY($2))").bind(id).bind(&ids).execute(&mut **tx).await?;
    for category in ids {
        if !old.contains(&category) && !(create && category == primary) {
            sqlx::query("INSERT INTO entity_category_assignments VALUES($1,$2)")
                .bind(id)
                .bind(category)
                .execute(&mut **tx)
                .await?;
        }
    }
    Ok(())
}
fn validate_membership(ids: &[Uuid], primary: Uuid) -> Result<(), DbError> {
    if ids.is_empty() || ids.len() > 32 || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
        return Err(DbError::Invalid(
            "category_ids must contain 1–32 distinct IDs".into(),
        ));
    }
    if !ids.contains(&primary) {
        return Err(DbError::Invalid(
            "primary_category_id must belong to category_ids".into(),
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn primary_and_duplicates() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        assert!(validate_membership(&[a, b], a).is_ok());
        assert!(validate_membership(&[a, a], a).is_err());
        assert!(validate_membership(&[a], b).is_err());
        assert!(validate_membership(&[], a).is_err());
    }
}

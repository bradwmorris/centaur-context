use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{FromRow, PgPool, Row};
use thiserror::Error;

const DEFAULT_PAGE_SIZE: i64 = 50;
const MAX_PAGE_SIZE: i64 = 100;

#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("schema inspection is unavailable for database {0}")]
    UnsafeDatabase(String),
    #[error("table is not registered for schema inspection")]
    UnknownTable,
    #[error("row cursor is invalid")]
    InvalidCursor,
    #[error("row cursor belongs to an older schema version")]
    StaleCursor,
    #[error("focused row lookup is invalid")]
    InvalidFocus,
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Serialize)]
pub struct SchemaSnapshot {
    pub fingerprint: String,
    pub tables: Vec<SchemaTable>,
    pub foreign_keys: Vec<SchemaForeignKey>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SchemaTable {
    pub name: String,
    pub classification: String,
    pub estimated_row_count: i64,
    pub columns: Vec<SchemaColumn>,
    pub constraints: Vec<SchemaConstraint>,
}

#[derive(Clone, Debug, FromRow, Serialize)]
pub struct SchemaColumn {
    pub name: String,
    pub ordinal: i16,
    pub data_type: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub identity: bool,
    pub generated: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SchemaConstraint {
    pub name: String,
    pub kind: String,
    pub columns: Vec<String>,
    pub definition: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SchemaForeignKey {
    pub name: String,
    pub source_table: String,
    pub source_columns: Vec<String>,
    pub target_table: String,
    pub target_columns: Vec<String>,
    pub one_to_one_subtype: bool,
    pub nullable: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SchemaRowPage {
    pub schema_fingerprint: String,
    pub table: String,
    pub rows: Vec<Value>,
    pub next_cursor: Option<String>,
    pub page_size: i64,
}

#[derive(Debug, FromRow)]
struct TableRow {
    name: String,
    estimated_row_count: i64,
}

#[derive(Debug, FromRow)]
struct ConstraintRow {
    table_name: String,
    name: String,
    kind: String,
    columns: Vec<String>,
    definition: String,
    target_table: Option<String>,
    target_columns: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RowCursor {
    table: String,
    fingerprint: String,
    keys: Vec<String>,
}

pub async fn inspect_schema(pool: &PgPool) -> Result<SchemaSnapshot, SchemaError> {
    validate_database(pool).await?;

    let table_rows = sqlx::query_as::<_, TableRow>(
        r#"
        SELECT r.table_name AS name,
               greatest(c.reltuples::bigint, 0) AS estimated_row_count
        FROM schema_visualizer_tables r
        JOIN pg_namespace n ON n.nspname = 'public'
        JOIN pg_class c ON c.relnamespace = n.oid
                       AND c.relname = r.table_name
                       AND c.relkind IN ('r', 'p')
        ORDER BY CASE WHEN r.table_name = 'objects' THEN 0 ELSE 1 END,
                 r.table_name
        "#,
    )
    .fetch_all(pool)
    .await?;

    let columns = column_rows(pool).await?;

    let constraint_rows = sqlx::query_as::<_, ConstraintRow>(
        r#"
        SELECT r.table_name,
               con.conname AS name,
               CASE con.contype
                   WHEN 'p' THEN 'primary_key'
                   WHEN 'f' THEN 'foreign_key'
                   WHEN 'u' THEN 'unique'
                   WHEN 'c' THEN 'check'
                   ELSE con.contype::text
               END AS kind,
               ARRAY(
                   SELECT a.attname
                   FROM unnest(con.conkey) WITH ORDINALITY AS key(attnum, position)
                   JOIN pg_attribute a ON a.attrelid = con.conrelid
                                      AND a.attnum = key.attnum
                   ORDER BY key.position
               ) AS columns,
               pg_get_constraintdef(con.oid, true) AS definition,
               target.relname AS target_table,
               CASE WHEN con.confrelid = 0 THEN ARRAY[]::text[] ELSE ARRAY(
                   SELECT a.attname
                   FROM unnest(con.confkey) WITH ORDINALITY AS key(attnum, position)
                   JOIN pg_attribute a ON a.attrelid = con.confrelid
                                      AND a.attnum = key.attnum
                   ORDER BY key.position
               ) END AS target_columns
        FROM schema_visualizer_tables r
        JOIN pg_namespace n ON n.nspname = 'public'
        JOIN pg_class c ON c.relnamespace = n.oid AND c.relname = r.table_name
        JOIN pg_constraint con ON con.conrelid = c.oid
        LEFT JOIN pg_class target ON target.oid = con.confrelid
        WHERE con.contype IN ('p', 'f', 'u', 'c')
        ORDER BY r.table_name, con.contype, con.conname
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut tables = Vec::with_capacity(table_rows.len());
    for table in table_rows {
        let table_columns = columns
            .iter()
            .filter(|(name, _)| name == &table.name)
            .map(|(_, column)| column.clone())
            .collect::<Vec<_>>();
        let table_constraints = constraint_rows
            .iter()
            .filter(|constraint| constraint.table_name == table.name)
            .map(|constraint| SchemaConstraint {
                name: constraint.name.clone(),
                kind: constraint.kind.clone(),
                columns: constraint.columns.clone(),
                definition: constraint.definition.clone(),
            })
            .collect::<Vec<_>>();
        let classification = if table.name == "objects" {
            "canonical"
        } else if is_subtype(&table.name, &table_constraints, &constraint_rows) {
            "subtype"
        } else {
            "supporting"
        };
        tables.push(SchemaTable {
            name: table.name,
            classification: classification.to_owned(),
            estimated_row_count: table.estimated_row_count,
            columns: table_columns,
            constraints: table_constraints,
        });
    }

    let registered = tables
        .iter()
        .map(|table| table.name.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut foreign_keys = Vec::new();
    for constraint in &constraint_rows {
        let Some(target_table) = constraint.target_table.as_deref() else {
            continue;
        };
        if constraint.kind != "foreign_key" || !registered.contains(target_table) {
            continue;
        }
        let source = tables
            .iter()
            .find(|table| table.name == constraint.table_name)
            .expect("constraint source table is registered");
        let nullable = constraint.columns.iter().any(|name| {
            source
                .columns
                .iter()
                .find(|column| column.name == *name)
                .is_some_and(|column| column.nullable)
        });
        foreign_keys.push(SchemaForeignKey {
            name: constraint.name.clone(),
            source_table: constraint.table_name.clone(),
            source_columns: constraint.columns.clone(),
            target_table: target_table.to_owned(),
            target_columns: constraint.target_columns.clone(),
            one_to_one_subtype: source.classification == "subtype" && target_table == "objects",
            nullable,
        });
    }
    foreign_keys.sort_by(|left, right| {
        (&left.source_table, &left.name).cmp(&(&right.source_table, &right.name))
    });

    let fingerprint = fingerprint(&tables, &foreign_keys)?;
    Ok(SchemaSnapshot {
        fingerprint,
        tables,
        foreign_keys,
    })
}

async fn column_rows(pool: &PgPool) -> Result<Vec<(String, SchemaColumn)>, sqlx::Error> {
    #[derive(FromRow)]
    struct ColumnRow {
        table_name: String,
        name: String,
        ordinal: i16,
        data_type: String,
        nullable: bool,
        default: Option<String>,
        identity: bool,
        generated: bool,
    }
    let rows = sqlx::query_as::<_, ColumnRow>(
        r#"
        SELECT r.table_name,
               a.attname AS name,
               a.attnum::smallint AS ordinal,
               pg_catalog.format_type(a.atttypid, a.atttypmod) AS data_type,
               NOT a.attnotnull AS nullable,
               pg_get_expr(d.adbin, d.adrelid) AS default,
               a.attidentity <> '' AS identity,
               a.attgenerated <> '' AS generated
        FROM schema_visualizer_tables r
        JOIN pg_namespace n ON n.nspname = 'public'
        JOIN pg_class c ON c.relnamespace = n.oid AND c.relname = r.table_name
        JOIN pg_attribute a ON a.attrelid = c.oid
                           AND a.attnum > 0
                           AND NOT a.attisdropped
        LEFT JOIN pg_attrdef d ON d.adrelid = c.oid AND d.adnum = a.attnum
        ORDER BY r.table_name, a.attnum
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.table_name,
                SchemaColumn {
                    name: row.name,
                    ordinal: row.ordinal,
                    data_type: row.data_type,
                    nullable: row.nullable,
                    default: row.default,
                    identity: row.identity,
                    generated: row.generated,
                },
            )
        })
        .collect())
}

pub async fn read_rows(
    pool: &PgPool,
    table_name: &str,
    limit: Option<i64>,
    cursor: Option<&str>,
    focus: Option<(&str, &str)>,
) -> Result<SchemaRowPage, SchemaError> {
    let snapshot = inspect_schema(pool).await?;
    let table = snapshot
        .tables
        .iter()
        .find(|table| table.name == table_name)
        .ok_or(SchemaError::UnknownTable)?;
    let primary_key = table
        .constraints
        .iter()
        .find(|constraint| constraint.kind == "primary_key")
        .map(|constraint| constraint.columns.clone())
        .filter(|columns| !columns.is_empty())
        .ok_or(SchemaError::UnknownTable)?;
    let page_size = limit.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE);
    let cursor = cursor.map(decode_cursor).transpose()?;
    if cursor.is_some() && focus.is_some() {
        return Err(SchemaError::InvalidFocus);
    }
    if let Some(cursor) = cursor.as_ref() {
        if cursor.table != table_name || cursor.keys.len() != primary_key.len() {
            return Err(SchemaError::InvalidCursor);
        }
        if cursor.fingerprint != snapshot.fingerprint {
            return Err(SchemaError::StaleCursor);
        }
    }

    let quoted_table = quote_identifier(table_name);
    let key_expressions = primary_key
        .iter()
        .map(|column| format!("t.{}::text", quote_identifier(column)))
        .collect::<Vec<_>>();
    let mut sql = format!(
        "SELECT (SELECT jsonb_object_agg(key, CASE WHEN value = 'null'::jsonb THEN 'null'::jsonb ELSE to_jsonb(value #>> '{{}}') END) FROM jsonb_each(to_jsonb(t))) AS data, jsonb_build_array({}) AS cursor_values FROM public.{} t",
        key_expressions.join(", "),
        quoted_table
    );
    if let Some((focus_column, _)) = focus {
        if !table
            .columns
            .iter()
            .any(|column| column.name == focus_column)
        {
            return Err(SchemaError::InvalidFocus);
        }
        sql.push_str(&format!(
            " WHERE t.{}::text = $1",
            quote_identifier(focus_column)
        ));
    } else if cursor.is_some() {
        if key_expressions.len() == 1 {
            sql.push_str(&format!(" WHERE {} > $1", key_expressions[0]));
        } else {
            let parameters = (1..=key_expressions.len())
                .map(|index| format!("${index}"))
                .collect::<Vec<_>>();
            sql.push_str(&format!(
                " WHERE ({}) > ({})",
                key_expressions.join(", "),
                parameters.join(", ")
            ));
        }
    }
    sql.push_str(&format!(
        " ORDER BY {} LIMIT {}",
        key_expressions.join(", "),
        page_size + 1
    ));

    let mut query = sqlx::query(&sql);
    if let Some((_, focus_value)) = focus {
        query = query.bind(focus_value);
    } else if let Some(cursor) = cursor.as_ref() {
        for key in &cursor.keys {
            query = query.bind(key);
        }
    }
    let result_rows = query.fetch_all(pool).await?;
    let has_more = result_rows.len() as i64 > page_size;
    let visible_rows = result_rows
        .iter()
        .take(page_size as usize)
        .collect::<Vec<_>>();
    let rows = visible_rows
        .iter()
        .map(|row| row.try_get::<Value, _>("data"))
        .collect::<Result<Vec<_>, _>>()?;
    let next_cursor = if has_more {
        let last = visible_rows
            .last()
            .expect("a page with more rows is non-empty");
        let keys = last
            .try_get::<Value, _>("cursor_values")?
            .as_array()
            .ok_or(SchemaError::InvalidCursor)?
            .iter()
            .map(|value| value.as_str().unwrap_or_default().to_owned())
            .collect();
        Some(encode_cursor(&RowCursor {
            table: table_name.to_owned(),
            fingerprint: snapshot.fingerprint.clone(),
            keys,
        })?)
    } else {
        None
    };

    Ok(SchemaRowPage {
        schema_fingerprint: snapshot.fingerprint,
        table: table_name.to_owned(),
        rows,
        next_cursor,
        page_size,
    })
}

async fn validate_database(pool: &PgPool) -> Result<(), SchemaError> {
    let database = sqlx::query_scalar::<_, String>("SELECT current_database()")
        .fetch_one(pool)
        .await?;
    let allowed = database == "centaur_context"
        || database == "centaur_os"
        || database.contains("centaur_context_test")
        || database.contains("centaur_os_test");
    if allowed {
        Ok(())
    } else {
        Err(SchemaError::UnsafeDatabase(database))
    }
}

fn is_subtype(
    table_name: &str,
    constraints: &[SchemaConstraint],
    all_constraints: &[ConstraintRow],
) -> bool {
    let Some(primary_key) = constraints
        .iter()
        .find(|constraint| constraint.kind == "primary_key")
    else {
        return false;
    };
    all_constraints.iter().any(|constraint| {
        constraint.table_name == table_name
            && constraint.kind == "foreign_key"
            && constraint.columns.starts_with(&primary_key.columns)
            && constraint.columns.iter().any(|name| name == "object_kind")
            && constraint.target_table.as_deref() == Some("objects")
            && constraint
                .target_columns
                .first()
                .is_some_and(|name| name == "id")
            && constraint.target_columns.iter().any(|name| name == "kind")
    })
}

fn fingerprint(
    tables: &[SchemaTable],
    foreign_keys: &[SchemaForeignKey],
) -> Result<String, serde_json::Error> {
    let normalized = json!({
        "tables": tables.iter().map(|table| json!({
            "name": table.name,
            "classification": table.classification,
            "columns": table.columns,
            "constraints": table.constraints,
        })).collect::<Vec<_>>(),
        "foreign_keys": foreign_keys,
    });
    let bytes = serde_json::to_vec(&normalized)?;
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(format!("{hash:016x}"))
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn encode_cursor(cursor: &RowCursor) -> Result<String, serde_json::Error> {
    Ok(serde_json::to_vec(cursor)?
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn decode_cursor(encoded: &str) -> Result<RowCursor, SchemaError> {
    if !encoded.len().is_multiple_of(2) {
        return Err(SchemaError::InvalidCursor);
    }
    let bytes = (0..encoded.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&encoded[index..index + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| SchemaError::InvalidCursor)?;
    serde_json::from_slice(&bytes).map_err(|_| SchemaError::InvalidCursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_round_trip_is_opaque_and_lossless() {
        let cursor = RowCursor {
            table: "eval_objects".to_owned(),
            fingerprint: "abc123".to_owned(),
            keys: vec!["first".to_owned(), "second".to_owned()],
        };
        let encoded = encode_cursor(&cursor).unwrap();
        assert!(!encoded.contains("eval_objects"));
        let decoded = decode_cursor(&encoded).unwrap();
        assert_eq!(decoded.table, cursor.table);
        assert_eq!(decoded.fingerprint, cursor.fingerprint);
        assert_eq!(decoded.keys, cursor.keys);
    }

    #[test]
    fn malformed_cursor_is_rejected() {
        assert!(matches!(
            decode_cursor("not-hex"),
            Err(SchemaError::InvalidCursor)
        ));
    }

    #[test]
    fn identifiers_are_always_quoted() {
        assert_eq!(quote_identifier("objects"), "\"objects\"");
        assert_eq!(quote_identifier("a\"b"), "\"a\"\"b\"");
    }
}

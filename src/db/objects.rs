//! Canonical Object and Theme persistence plus database lifecycle checks.

use super::*;

pub async fn migrate(pool: &PgPool) -> Result<(), DbError> {
    let database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(pool)
        .await?;
    if !allowed_database_name(&database) {
        return Err(DbError::Sqlx(sqlx::Error::Configuration(
            format!("refusing migrations against unexpected database {database:?}").into(),
        )));
    }
    sqlx::migrate!()
        .run(pool)
        .await
        .map_err(|error| sqlx::Error::Migrate(Box::new(error)))?;
    Ok(())
}

pub(crate) fn allowed_database_name(database: &str) -> bool {
    database == "centaur_context"
        || database.starts_with("centaur_context_")
        || database == "centaur_os"
        || database.starts_with("centaur_os_")
}

pub async fn ready(pool: &PgPool) -> Result<(), DbError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT to_regclass('public.objects') IS NOT NULL AND to_regclass('public.object_events') IS NOT NULL",
    )
    .fetch_one(pool)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(DbError::NotFound)
    }
}

fn push_active_connection_count(query: &mut QueryBuilder<'_, Postgres>, object_id: &str) {
    query
        .push("((SELECT count(*) FROM connections list_source_connection WHERE list_source_connection.archived_at IS NULL AND list_source_connection.source_object_id=")
        .push(object_id)
        .push(")+(SELECT count(*) FROM connections list_target_connection WHERE list_target_connection.archived_at IS NULL AND list_target_connection.target_object_id=")
        .push(object_id)
        .push("))");
}

pub(super) fn push_object_list_cursor(
    query: &mut QueryBuilder<'_, Postgres>,
    cursor: Uuid,
    sort: &ListSort,
) {
    query.push(" AND (");
    if matches!(sort, ListSort::Connections) {
        push_active_connection_count(query, "o.id");
        query.push(",");
    }
    query.push("o.created_at,o.id) ");
    query.push(if matches!(sort, ListSort::Oldest) {
        "> (SELECT "
    } else {
        "< (SELECT "
    });
    if matches!(sort, ListSort::Connections) {
        push_active_connection_count(query, "cursor_object.id");
        query.push(",");
    }
    query
        .push("cursor_object.created_at,cursor_object.id FROM objects cursor_object WHERE cursor_object.id=")
        .push_bind(cursor)
        .push(")");
}

pub(super) fn push_object_list_order(query: &mut QueryBuilder<'_, Postgres>, sort: &ListSort) {
    query.push(" ORDER BY ");
    if matches!(sort, ListSort::Connections) {
        push_active_connection_count(query, "o.id");
        query.push(" DESC,");
    }
    if matches!(sort, ListSort::Oldest) {
        query.push("o.created_at ASC,o.id ASC");
    } else {
        query.push("o.created_at DESC,o.id DESC");
    }
}

pub async fn list_objects(pool: &PgPool, filter: ObjectListFilter) -> Result<Vec<Object>, DbError> {
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT o.id,o.kind,o.title,o.description,o.protected,CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,o.created_by_type,o.created_by_id,o.updated_by_type,o.updated_by_id,o.provenance,o.created_at,o.updated_at,o.archived_at FROM objects o WHERE true",
    );
    if let Some(kind) = filter.kind {
        query.push(" AND o.kind = ").push_bind(kind);
    }
    if let Some(cursor) = filter.cursor {
        push_object_list_cursor(&mut query, cursor, &filter.sort);
    }
    if let Some(lifecycle) = filter.lifecycle {
        if lifecycle == "active" {
            query.push(" AND o.archived_at IS NULL");
        } else {
            query.push(" AND o.archived_at IS NOT NULL");
        }
    }
    if let Some(search) = filter.query {
        query.push(" AND ");
        if filter.text_search_config == crate::config::TextSearchConfig::SIMPLE {
            query.push("o.search_document");
        } else {
            query
                .push("(setweight(to_tsvector(")
                .push_bind(filter.text_search_config.as_str())
                .push("::regconfig, coalesce(o.title,'')), 'A') || setweight(to_tsvector(")
                .push_bind(filter.text_search_config.as_str())
                .push("::regconfig, coalesce(o.description,'')), 'B'))");
        }
        query
            .push(" @@ websearch_to_tsquery(")
            .push_bind(filter.text_search_config.as_str())
            .push("::regconfig, ")
            .push_bind(search)
            .push(")");
    }
    push_object_list_order(&mut query, &filter.sort);
    query.push(" LIMIT ").push_bind(filter.limit);
    Ok(query.build_query_as().fetch_all(pool).await?)
}

pub async fn get_object(pool: &PgPool, id: Uuid) -> Result<Object, DbError> {
    sqlx::query_as(
        "SELECT id,kind,title,description,protected,CASE WHEN archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,revision,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance,created_at,updated_at,archived_at FROM objects WHERE id = $1",
    )
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
}

const SOURCE_SELECT: &str = r#"SELECT o.id AS object_id,o.title,o.description,CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,
       o.provenance,o.protected,s.source_kind,s.canonical_uri,s.byline,s.publisher,
       s.published_at,s.published_at_precision,s.last_accessed_at,s.original_language,
       s.original_media_type,s.original_artifact_reference,s.current_artifact_id,
       o.created_at,o.updated_at
FROM sources s JOIN objects o ON o.id=s.object_id"#;

pub async fn get_source(pool: &PgPool, id: Uuid) -> Result<Source, DbError> {
    sqlx::query_as(&format!("{SOURCE_SELECT} WHERE o.id=$1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn get_note(pool: &PgPool, id: Uuid) -> Result<Note, DbError> {
    sqlx::query_as(
        r#"SELECT o.id AS object_id,o.title,o.description,CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,
        o.provenance,o.protected,n.content,n.content_format,n.intent,n.source_artifact_id,n.source_locator,o.created_at,o.updated_at
        FROM notes n JOIN objects o ON o.id=n.object_id WHERE o.id=$1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(DbError::NotFound)
}

const THEME_SELECT: &str = r#"SELECT o.id AS object_id,o.title,o.description,t.slug,
       CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,o.provenance,o.protected,o.created_at,o.updated_at
FROM themes t JOIN objects o ON o.id=t.object_id"#;

pub async fn list_themes(
    pool: &PgPool,
    sort: ListSort,
    cursor: Option<Uuid>,
    limit: i64,
) -> Result<Vec<Theme>, DbError> {
    let mut query =
        QueryBuilder::<Postgres>::new(format!("{THEME_SELECT} WHERE o.archived_at IS NULL"));
    if let Some(cursor) = cursor {
        push_object_list_cursor(&mut query, cursor, &sort);
    }
    push_object_list_order(&mut query, &sort);
    query.push(" LIMIT ").push_bind(limit);
    Ok(query.build_query_as().fetch_all(pool).await?)
}

pub async fn get_theme(pool: &PgPool, id: Uuid) -> Result<Theme, DbError> {
    sqlx::query_as(&format!("{THEME_SELECT} WHERE o.id=$1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn get_theme_by_slug(pool: &PgPool, slug: &str) -> Result<Theme, DbError> {
    sqlx::query_as(&format!("{THEME_SELECT} WHERE t.slug=$1"))
        .bind(slug)
        .fetch_optional(pool)
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn create_theme(
    pool: &PgPool,
    actor: &ActorContext,
    mut input: NewTheme,
    idempotency_key: &str,
) -> Result<Theme, DbError> {
    if let Some(id) = idempotent_entity(pool, actor, idempotency_key).await? {
        return get_theme(pool, id).await;
    }
    validate_object_description(&input.title, &input.description)?;
    input.slug = theme_slug(input.slug)?;
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"INSERT INTO objects
        (id,kind,title,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id,provenance)
        VALUES ($1,'theme',$2,$3,$4,$5,$6,$5,$6,$7)"#,
    )
    .bind(id)
    .bind(&input.title)
    .bind(&input.description)
    .bind(input.protected)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(&input.provenance)
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO themes (object_id,slug) VALUES ($1,$2)")
        .bind(id)
        .bind(&input.slug)
        .execute(&mut *tx)
        .await?;
    insert_event(
        &mut tx,
        actor,
        "object",
        id,
        id,
        "created",
        Some(idempotency_key),
        None,
        1,
        json!({"kind":"theme","title":input.title,"slug":input.slug,"protected":input.protected}),
    )
    .await?;
    tx.commit().await?;
    get_theme(pool, id).await
}

pub async fn list_theme_objects(
    pool: &PgPool,
    theme_id: Uuid,
    kind: Option<&str>,
    limit: i64,
) -> Result<Vec<Object>, DbError> {
    let theme = get_theme(pool, theme_id).await?;
    if theme.lifecycle != "active" {
        return Err(DbError::NotFound);
    }
    let mut query = QueryBuilder::<Postgres>::new(
        r#"SELECT o.* FROM connections c
        JOIN objects o ON o.id=c.source_object_id
        WHERE c.kind='themed' AND c.archived_at IS NULL
          AND c.target_object_id="#,
    );
    query.push_bind(theme_id).push(" AND o.archived_at IS NULL");
    if let Some(kind) = kind {
        query.push(" AND o.kind=").push_bind(kind);
    }
    query
        .push(" ORDER BY o.updated_at DESC,o.id LIMIT ")
        .push_bind(limit);
    Ok(query.build_query_as().fetch_all(pool).await?)
}

pub async fn create_object(
    pool: &PgPool,
    actor: &ActorContext,
    input: NewObject,
    idempotency_key: &str,
) -> Result<Object, DbError> {
    if let Some(id) = idempotent_entity(pool, actor, idempotency_key).await? {
        return get_object(pool, id).await;
    }
    validate_object_description(&input.title, &input.description)?;
    match input.kind.as_str() {
        "chat" if input.entity_kind.is_none() && input.happened_at.is_none() => {}
        "entity" if input.entity_kind.is_some() && input.happened_at.is_none() => {}
        "memory" if input.entity_kind.is_none() && input.happened_at.is_some() => {}
        "chat" | "entity" | "memory" => {
            return Err(DbError::Invalid(
                "typed Object creation fields do not match its kind".into(),
            ));
        }
        _ => {
            return Err(DbError::Invalid(
                "use the typed creation contract for this Object kind".into(),
            ));
        }
    }
    let id = Uuid::new_v4();
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"INSERT INTO objects
           (id, kind, title, description, created_by_type, created_by_id, updated_by_type, updated_by_id, provenance)
           VALUES ($1,$2,$3,$4,$5,$6,$5,$6,$7)"#,
    )
    .bind(id)
    .bind(&input.kind)
    .bind(&input.title)
    .bind(&input.description)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .bind(&input.provenance)
    .execute(&mut *tx)
    .await?;
    insert_object_subtype(&mut tx, id, &input).await?;
    insert_event(
        &mut tx,
        actor,
        "object",
        id,
        id,
        "created",
        Some(idempotency_key),
        None,
        1,
        json!({"kind": input.kind, "title": input.title}),
    )
    .await?;
    tx.commit().await?;
    get_object(pool, id).await
}

async fn insert_object_subtype(
    tx: &mut Transaction<'_, Postgres>,
    object_id: Uuid,
    input: &NewObject,
) -> Result<(), DbError> {
    match input.kind.as_str() {
        "chat" => {
            sqlx::query("INSERT INTO chats (object_id) VALUES ($1)")
                .bind(object_id)
                .execute(&mut **tx)
                .await?;
        }
        "entity" => {
            sqlx::query("INSERT INTO entities (object_id,entity_kind) VALUES ($1,$2)")
                .bind(object_id)
                .bind(input.entity_kind.as_deref().expect("validated entity kind"))
                .execute(&mut **tx)
                .await?;
        }
        "memory" => {
            sqlx::query("INSERT INTO memories (object_id,happened_at) VALUES ($1,$2)")
                .bind(object_id)
                .bind(input.happened_at.expect("validated Memory time"))
                .execute(&mut **tx)
                .await?;
        }
        _ => unreachable!("Object kind validated before subtype insertion"),
    }
    Ok(())
}

pub async fn update_object(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    expected_revision: i64,
    changes: ObjectChanges,
    idempotency_key: Option<&str>,
) -> Result<Object, DbError> {
    if let Some(key) = idempotency_key
        && let Some(existing_id) = idempotent_entity(pool, actor, key).await?
    {
        return get_object(pool, existing_id).await;
    }
    let current = get_object(pool, id).await?;
    let title = changes.title.unwrap_or_else(|| current.title.clone());
    let description = changes
        .description
        .unwrap_or_else(|| current.description.clone());
    validate_object_description(&title, &description)?;
    let provenance = changes
        .provenance
        .unwrap_or_else(|| current.provenance.clone());
    let protected = changes.protected.unwrap_or(current.protected);
    let lifecycle = if changes.archive {
        "archived"
    } else {
        &current.lifecycle
    };
    let archived_at = if changes.archive {
        Some(OffsetDateTime::now_utc())
    } else {
        current.archived_at
    };
    let mut tx = pool.begin().await?;
    let updated: Option<Object> = sqlx::query_as(
        r#"UPDATE objects SET title=$3, description=$4, provenance=$5, protected=$6,
           archived_at=$7, revision=revision+1, updated_by_type=$8, updated_by_id=$9,
           updated_at=now() WHERE id=$1 AND revision=$2
           RETURNING id,kind,title,description,protected,
             CASE WHEN archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,
             revision,created_by_type,created_by_id,updated_by_type,updated_by_id,
             provenance,created_at,updated_at,archived_at"#,
    )
    .bind(id)
    .bind(expected_revision)
    .bind(&title)
    .bind(&description)
    .bind(&provenance)
    .bind(protected)
    .bind(archived_at)
    .bind(actor.actor_type)
    .bind(&actor.actor_id)
    .fetch_optional(&mut *tx)
    .await?;
    let updated = updated.ok_or(DbError::Conflict)?;
    insert_event(
        &mut tx,
        actor,
        "object",
        id,
        id,
        if changes.archive {
            "archived"
        } else {
            "updated"
        },
        idempotency_key,
        Some(expected_revision),
        updated.revision,
        json!({"title": title, "description_changed": description != current.description, "protected": protected, "lifecycle": lifecycle}),
    )
    .await?;
    tx.commit().await?;
    Ok(updated)
}

#[cfg(test)]
mod rename_compatibility_tests {
    use super::allowed_database_name;

    #[test]
    fn accepts_canonical_and_legacy_database_names_only() {
        for allowed in [
            "centaur_context",
            "centaur_context_test_issue_10",
            "centaur_context_enyu",
            "centaur_os",
            "centaur_os_test_upgrade",
            "centaur_os_enyu",
        ] {
            assert!(
                allowed_database_name(allowed),
                "expected {allowed} to be accepted"
            );
        }
        for rejected in [
            "postgres",
            "ai_v2",
            "centaur_contextual",
            "other_centaur_context_test",
            "centaur_test",
        ] {
            assert!(
                !allowed_database_name(rejected),
                "expected {rejected} to be rejected"
            );
        }
    }
}

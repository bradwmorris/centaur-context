//! Event, Chat-message, User, identity, and visual read models.

use super::*;

pub async fn list_events(pool: &PgPool, object_id: Uuid) -> Result<Vec<ObjectEvent>, DbError> {
    Ok(sqlx::query_as(
        "SELECT * FROM object_events WHERE target_type='object' AND target_id=$1 ORDER BY created_at DESC,id DESC LIMIT 100",
    )
    .bind(object_id)
    .fetch_all(pool)
    .await?)
}

pub async fn list_chat_messages(
    pool: &PgPool,
    chat_object_id: Uuid,
) -> Result<Vec<ChatMessage>, DbError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM chats WHERE object_id=$1)")
        .bind(chat_object_id)
        .fetch_one(pool)
        .await?;
    if !exists {
        return Err(DbError::NotFound);
    }
    Ok(sqlx::query_as(
        r#"SELECT m.id,m.chat_object_id,m.provider_message_id,m.sender_user_object_id,
                  o.title AS sender_title,u.user_kind AS sender_kind,m.content,
                  m.source_created_at,m.ingestion_sequence,m.ingested_at
           FROM chat_messages m
           JOIN users u ON u.object_id=m.sender_user_object_id
           JOIN objects o ON o.id=u.object_id
           WHERE m.chat_object_id=$1
           ORDER BY m.source_created_at,m.provider_message_id"#,
    )
    .bind(chat_object_id)
    .fetch_all(pool)
    .await?)
}

pub async fn list_users(pool: &PgPool, limit: i64) -> Result<Vec<User>, DbError> {
    Ok(sqlx::query_as(
        r#"SELECT o.id AS object_id,o.title,o.description,CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,o.provenance,
                  u.user_kind,u.identities,o.created_at,o.updated_at
           FROM users u JOIN objects o ON o.id=u.object_id
           WHERE o.archived_at IS NULL ORDER BY o.updated_at DESC,o.id LIMIT $1"#,
    )
    .bind(limit.clamp(1, 100))
    .fetch_all(pool)
    .await?)
}

pub async fn get_user(pool: &PgPool, id: Uuid) -> Result<User, DbError> {
    sqlx::query_as(
        r#"SELECT o.id AS object_id,o.title,o.description,CASE WHEN o.archived_at IS NULL THEN 'active' ELSE 'archived' END AS lifecycle,o.revision,o.provenance,
                  u.user_kind,u.identities,o.created_at,o.updated_at
           FROM users u JOIN objects o ON o.id=u.object_id WHERE o.id=$1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(DbError::NotFound)
}

pub async fn list_user_identities(
    pool: &PgPool,
    user_object_id: Uuid,
) -> Result<Vec<ExternalIdentity>, DbError> {
    let user = get_user(pool, user_object_id).await?;
    serde_json::from_value(user.identities)
        .map_err(|error| DbError::Invalid(format!("invalid embedded User identities: {error}")))
}

pub async fn list_object_visuals(pool: &PgPool) -> Result<Vec<ObjectVisual>, DbError> {
    let sources: Vec<ObjectVisualSource> = sqlx::query_as(
        r#"SELECT o.id AS object_id,
                  CASE WHEN
                    lower(COALESCE(o.provenance->>'source_type',''))='slack'
                    OR EXISTS (
                      SELECT 1 FROM chats ch
                      WHERE ch.object_id=o.id AND ch.provider='slack'
                    )
                    OR EXISTS (
                      SELECT 1
                      FROM connections c
                      JOIN chats ch ON ch.object_id=CASE
                        WHEN c.source_object_id=o.id THEN c.target_object_id
                        ELSE c.source_object_id
                      END
                      WHERE c.archived_at IS NULL AND c.kind='derived_from'
                        AND (c.source_object_id=o.id OR c.target_object_id=o.id)
                        AND ch.provider='slack'
                    )
                    OR EXISTS (
                      SELECT 1
                      FROM jsonb_array_elements_text(
                        CASE WHEN jsonb_typeof(o.provenance->'supporting_message_ids')='array'
                          THEN o.provenance->'supporting_message_ids' ELSE '[]'::jsonb END
                      ) message_ref
                      JOIN chat_messages m ON m.id=message_ref.value::uuid
                      JOIN chats ch ON ch.object_id=m.chat_object_id
                      WHERE ch.provider='slack'
                    )
                  THEN 'slack'::text ELSE NULL::text END AS source_provider
           FROM objects o
           WHERE o.archived_at IS NULL
           ORDER BY o.updated_at DESC,o.id"#,
    )
    .fetch_all(pool)
    .await?;

    let attributions: Vec<UserAttribution> = sqlx::query_as(
        r#"WITH attribution AS (
             SELECT u.object_id,u.object_id AS user_object_id,'identity'::text AS role
             FROM users u
             UNION
             SELECT t.object_id,t.owner_object_id,'owner'::text
             FROM tasks t WHERE t.owner_object_id IS NOT NULL
             UNION
             SELECT c.source_object_id,u.object_id,'participant'::text
             FROM connections c JOIN users u ON u.object_id=c.target_object_id
             WHERE c.kind='involves' AND c.archived_at IS NULL
             UNION
             SELECT c.target_object_id,u.object_id,'participant'::text
             FROM connections c JOIN users u ON u.object_id=c.source_object_id
             WHERE c.kind='involves' AND c.archived_at IS NULL
             UNION
             SELECT o.id,m.sender_user_object_id,'source author'::text
             FROM objects o
             JOIN LATERAL jsonb_array_elements_text(
               CASE WHEN jsonb_typeof(o.provenance->'supporting_message_ids')='array'
                 THEN o.provenance->'supporting_message_ids' ELSE '[]'::jsonb END
             ) message_ref ON true
             JOIN chat_messages m ON m.id=message_ref.value::uuid
           )
           SELECT a.object_id,a.user_object_id,uo.title,u.user_kind,a.role,
                  avatar.avatar_url,
                  CASE WHEN avatar.avatar_asset_sha256 IS NOT NULL THEN
                    '/api/v2/identity-assets/' || avatar.avatar_asset_sha256 || '/' || avatar.avatar_asset_filename
                  ELSE NULL END AS avatar_asset_url
           FROM attribution a
           JOIN users u ON u.object_id=a.user_object_id
           JOIN objects uo ON uo.id=u.object_id
           LEFT JOIN LATERAL (
             SELECT e.value->>'avatar_url' AS avatar_url,
                    e.value->>'avatar_asset_sha256' AS avatar_asset_sha256,
                    e.value->>'avatar_asset_filename' AS avatar_asset_filename
             FROM jsonb_array_elements(u.identities) e(value)
             WHERE e.value->>'avatar_url' IS NOT NULL
                OR e.value->>'avatar_asset_sha256' IS NOT NULL
             ORDER BY (e.value->>'avatar_asset_sha256' IS NOT NULL) DESC,
                      (e.value->>'provider'='slack') DESC,
                      (e.value->>'profile_refreshed_at') DESC NULLS LAST LIMIT 1
           ) avatar ON true
           ORDER BY a.object_id,
             CASE a.role WHEN 'owner' THEN 1 WHEN 'source author' THEN 2
               WHEN 'participant' THEN 3 ELSE 4 END,
             uo.title,a.user_object_id"#,
    )
    .fetch_all(pool)
    .await?;

    let mut users_by_object = std::collections::HashMap::<Uuid, Vec<UserAttribution>>::new();
    for attribution in attributions {
        users_by_object
            .entry(attribution.object_id)
            .or_default()
            .push(attribution);
    }
    Ok(sources
        .into_iter()
        .map(|source| ObjectVisual {
            object_id: source.object_id,
            source_provider: source.source_provider,
            users: users_by_object
                .remove(&source.object_id)
                .unwrap_or_default(),
        })
        .collect())
}

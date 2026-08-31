-- Canonical cleanup: remove redundant state, make subtype meaning explicit, and
-- align queue/content field names with the concepts they actually represent.

-- Objects: archived_at is the single persisted lifecycle source of truth.
DROP INDEX objects_list_idx;
ALTER TABLE objects
    DROP CONSTRAINT objects_check,
    DROP CONSTRAINT objects_lifecycle_check,
    DROP COLUMN lifecycle;
CREATE INDEX objects_list_idx
    ON objects (kind, updated_at DESC, id)
    WHERE archived_at IS NULL;
CREATE INDEX objects_archived_list_idx
    ON objects (kind, archived_at DESC, id)
    WHERE archived_at IS NOT NULL;

CREATE OR REPLACE FUNCTION centaur_context_trace_object_mutation() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE target_eval_id uuid;
DECLARE action_role text;
DECLARE trace_type text;
DECLARE actor_type_value text;
DECLARE actor_id_value text;
BEGIN
    actor_type_value := CASE WHEN TG_OP='INSERT' THEN NEW.created_by_type ELSE NEW.updated_by_type END;
    actor_id_value := CASE WHEN TG_OP='INSERT' THEN NEW.created_by_id ELSE NEW.updated_by_id END;
    target_eval_id := centaur_context_ensure_mutation_eval(actor_type_value, actor_id_value);
    action_role := CASE
        WHEN TG_OP='INSERT' THEN 'created'
        WHEN NEW.archived_at IS NOT NULL AND OLD.archived_at IS NULL THEN 'archived'
        ELSE 'updated'
    END;
    trace_type := 'object_' || action_role;
    INSERT INTO eval_objects (eval_id,object_id,role)
    VALUES (target_eval_id,NEW.id,action_role) ON CONFLICT DO NOTHING;
    PERFORM centaur_context_append_trace(target_eval_id,trace_type,
        jsonb_build_object('object_id',NEW.id,'kind',NEW.kind,'from_revision',
            CASE WHEN TG_OP='INSERT' THEN NULL ELSE OLD.revision END,
            'to_revision',NEW.revision));
    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION enforce_themed_connection()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    source_kind text;
    source_archived_at timestamptz;
    target_kind text;
    target_archived_at timestamptz;
BEGIN
    IF NEW.kind <> 'themed' THEN
        RETURN NEW;
    END IF;
    SELECT kind, archived_at INTO source_kind, source_archived_at
    FROM objects WHERE id = NEW.source_object_id;
    SELECT kind, archived_at INTO target_kind, target_archived_at
    FROM objects WHERE id = NEW.target_object_id;
    IF source_kind = 'theme' THEN
        RAISE EXCEPTION 'Theme Objects cannot be themed while Theme hierarchy is unsupported';
    END IF;
    IF source_archived_at IS NOT NULL THEN
        RAISE EXCEPTION 'themed Connection source must be active';
    END IF;
    IF target_kind <> 'theme' OR target_archived_at IS NOT NULL THEN
        RAISE EXCEPTION 'themed Connection target must be an active Theme';
    END IF;
    RETURN NEW;
END;
$$;

-- Canonical subtypes use their parent Object timestamps.
ALTER TABLE users DROP COLUMN created_at, DROP COLUMN updated_at;
ALTER TABLE memories DROP COLUMN created_at, DROP COLUMN updated_at;
ALTER TABLE notes DROP COLUMN created_at, DROP COLUMN updated_at;
ALTER TABLE themes DROP COLUMN created_at, DROP COLUMN updated_at;

-- Entities receive an explicit controlled classification. Legacy migration
-- provenance is used only where it already states the accepted classification.
ALTER TABLE entities ADD COLUMN entity_kind text;
UPDATE entities e
SET entity_kind = CASE
    WHEN o.provenance->>'note' ~* '^Accepted (active )?person Entity' THEN 'person'
    WHEN o.provenance->>'note' ~* '^Accepted (active )?organization Entity' THEN 'organization'
    WHEN o.provenance->>'note' ~* '^Accepted (active )?product Entity' THEN 'product'
    WHEN o.provenance->>'note' ~* '^Accepted (active )?(publication|podcast) Entity' THEN 'publication'
    WHEN o.title IN ('Dan Robinson','Georgios Konstantopoulos','Alpin Yukseloglu','Justin Wang','Matt Huang') THEN 'person'
    WHEN o.title IN ('Nous Research','Vana','Paradigm','Harmonic') THEN 'organization'
    WHEN o.title = 'Andromeda' THEN 'product'
    -- Synthetic Enyu Ops acceptance fixture; it is archived by the approved
    -- cleanup manifest after migration but still needs a valid subtype value.
    WHEN o.id = '3426a296-5230-473a-a2cc-7ede7384ad2e' THEN 'other'
    WHEN o.provenance->>'note' ~* '^Accepted (active )?other Entity' THEN 'other'
    ELSE NULL
END
FROM objects o WHERE o.id=e.object_id;
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM entities WHERE entity_kind IS NULL) THEN
        RAISE EXCEPTION 'entity_kind cannot be inferred for every legacy Entity; classify those rows before migration';
    END IF;
END $$;
ALTER TABLE entities
    ALTER COLUMN entity_kind SET NOT NULL,
    ADD CONSTRAINT entities_entity_kind_check CHECK (entity_kind IN (
        'person','organization','product','project','publication','place','concept','other'
    )),
    DROP COLUMN created_at,
    DROP COLUMN updated_at;

-- Tasks use an explicit workflow and carry operational blockers and briefs.
DROP INDEX tasks_list_idx;
ALTER TABLE tasks DROP CONSTRAINT tasks_status_check;
ALTER TABLE tasks RENAME COLUMN agent_eligible TO agent_suitable;
ALTER TABLE tasks
    ADD COLUMN blocked_reason text,
    ADD COLUMN completed_at timestamptz,
    ADD COLUMN github_issue_url text,
    ADD COLUMN brief_markdown text;
UPDATE tasks t
SET blocked_reason = CASE
        WHEN position('blocked because ' IN lower(o.description)) > 0
            THEN btrim(regexp_replace(o.description, '^.*blocked because ', '', 'i'))
        ELSE NULL
    END
FROM objects o WHERE o.id=t.object_id AND t.status='blocked';
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM tasks
        WHERE status='blocked'
          AND (blocked_reason IS NULL OR btrim(blocked_reason)='')
    ) THEN
        RAISE EXCEPTION 'blocked_reason cannot be inferred for every legacy blocked Task; supply those reasons before migration';
    END IF;
END $$;
UPDATE tasks t SET completed_at=o.updated_at
FROM objects o WHERE o.id=t.object_id AND t.status='done';
ALTER TABLE tasks
    ADD CONSTRAINT tasks_status_check CHECK (status IN (
        'backlog','todo','doing','review','done','blocked'
    )),
    ADD CONSTRAINT tasks_blocked_reason_check CHECK (
        (status='blocked' AND blocked_reason IS NOT NULL
            AND char_length(btrim(blocked_reason)) BETWEEN 1 AND 2000)
        OR (status<>'blocked' AND blocked_reason IS NULL)
    ),
    ADD CONSTRAINT tasks_completed_at_check CHECK (
        (status='done') = (completed_at IS NOT NULL)
    ),
    ADD CONSTRAINT tasks_github_issue_url_check CHECK (
        github_issue_url IS NULL OR (
            char_length(github_issue_url) <= 2000
            AND github_issue_url ~ '^https://github\.com/[^/]+/[^/]+/issues/[1-9][0-9]*$'
        )
    ),
    ADD CONSTRAINT tasks_brief_markdown_check CHECK (
        brief_markdown IS NULL OR char_length(btrim(brief_markdown)) BETWEEN 1 AND 100000
    ),
    DROP COLUMN created_at,
    DROP COLUMN updated_at;
CREATE INDEX tasks_list_idx
    ON tasks (status, agent_suitable, object_id);

-- Chat processing fields name their precise cursor and clock semantics.
DROP INDEX chats_inactivity_queue_idx;
ALTER TABLE chats
    DROP CONSTRAINT chats_last_ingested_message_fk,
    DROP CONSTRAINT chats_last_queued_message_fk,
    DROP CONSTRAINT chats_last_curated_message_fk,
    DROP COLUMN last_ingested_message_id,
    DROP COLUMN created_at;
ALTER TABLE chats RENAME COLUMN updated_at TO processing_updated_at;
ALTER TABLE chats RENAME COLUMN last_message_at TO latest_source_message_at;
ALTER TABLE chats RENAME COLUMN last_queued_message_id TO curation_queued_through_message_id;
ALTER TABLE chats RENAME COLUMN last_curated_message_id TO curated_through_message_id;
ALTER TABLE chat_messages RENAME COLUMN ingested_sequence TO ingestion_sequence;
ALTER TABLE chat_messages ADD CONSTRAINT chat_messages_chat_id_unique UNIQUE (chat_object_id,id);
ALTER TABLE chats
    ADD CONSTRAINT chats_curation_queued_message_fk
        FOREIGN KEY (object_id,curation_queued_through_message_id)
        REFERENCES chat_messages(chat_object_id,id) ON DELETE RESTRICT,
    ADD CONSTRAINT chats_curated_message_fk
        FOREIGN KEY (object_id,curated_through_message_id)
        REFERENCES chat_messages(chat_object_id,id) ON DELETE RESTRICT;
CREATE INDEX chats_inactivity_queue_idx
    ON chats (latest_source_message_at, object_id)
    WHERE latest_source_message_at IS NOT NULL;

-- Curator queue time and message-window ownership are explicit.
ALTER TABLE curator_runs
    DROP CONSTRAINT curator_runs_first_message_id_fkey,
    DROP CONSTRAINT curator_runs_last_message_id_fkey;
ALTER TABLE curator_runs RENAME COLUMN created_at TO queued_at;
DROP INDEX curator_runs_queue_idx;
CREATE INDEX curator_runs_queue_idx ON curator_runs (status,queued_at,id);
ALTER TABLE curator_runs
    ADD CONSTRAINT curator_runs_first_message_fk
        FOREIGN KEY (chat_object_id,first_message_id)
        REFERENCES chat_messages(chat_object_id,id) ON DELETE RESTRICT,
    ADD CONSTRAINT curator_runs_last_message_fk
        FOREIGN KEY (chat_object_id,last_message_id)
        REFERENCES chat_messages(chat_object_id,id) ON DELETE RESTRICT;

-- Memories must record an evidenced event time explicitly.
ALTER TABLE memories ALTER COLUMN happened_at DROP DEFAULT;
CREATE INDEX memories_happened_idx ON memories (happened_at DESC,object_id);

-- Source identity remains separate from immutable captured content.
ALTER TABLE sources DROP CONSTRAINT sources_source_kind_check;
UPDATE sources SET source_kind='podcast_episode' WHERE source_kind='podcast';
ALTER TABLE sources RENAME COLUMN accessed_at TO last_accessed_at;
ALTER TABLE sources RENAME COLUMN language TO original_language;
ALTER TABLE sources RENAME COLUMN media_type TO original_media_type;
ALTER TABLE sources RENAME COLUMN artifact_reference TO original_artifact_reference;
ALTER TABLE sources DROP COLUMN content_hash;
ALTER TABLE sources
    ADD COLUMN published_at_precision text,
    ADD CONSTRAINT sources_source_kind_check CHECK (source_kind IN (
        'article','paper','podcast_episode','video','book','report','document',
        'dataset','web_page','social_post','other'
    ));
UPDATE sources SET published_at_precision = CASE
    WHEN published_at IS NULL THEN NULL
    WHEN published_at = date_trunc('day',published_at AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'
        THEN 'day'
    ELSE 'instant'
END;
ALTER TABLE sources
    ADD CONSTRAINT sources_published_precision_check CHECK (
        (published_at IS NULL AND published_at_precision IS NULL)
        OR (published_at IS NOT NULL AND published_at_precision IN ('instant','day','month','year'))
    ),
    DROP COLUMN created_at,
    DROP COLUMN updated_at;

ALTER TABLE source_contents RENAME COLUMN content_hash TO content_sha256;
ALTER TABLE source_contents RENAME COLUMN artifact_reference TO capture_artifact_reference;
ALTER TABLE source_contents RENAME COLUMN created_at TO recorded_at;
ALTER TABLE source_contents
    ADD COLUMN coverage text NOT NULL DEFAULT 'unknown',
    ADD COLUMN captured_at timestamptz,
    ADD CONSTRAINT source_contents_coverage_check CHECK (coverage IN ('complete','partial','unknown'));
ALTER TABLE source_contents ALTER COLUMN coverage DROP DEFAULT;
-- This remains non-unique until the exact legacy duplicate deletion manifest is
-- approved. Runtime writers already return an existing same-Source hash rather
-- than creating another version.
CREATE INDEX source_contents_source_sha256_idx
    ON source_contents (source_object_id,content_sha256);

-- Both independent indexes duplicate the leading columns of existing unique
-- btree indexes and add write cost without supporting a different access path.
DROP INDEX object_embeddings_object_idx;
DROP INDEX eval_trace_entries_eval_idx;

COMMENT ON COLUMN objects.archived_at IS
    'NULL means active; a timestamp means the Object was archived at that time.';
COMMENT ON COLUMN entities.entity_kind IS
    'Explicit ontology classification for the named subject represented by this Entity.';
COMMENT ON COLUMN chats.processing_updated_at IS
    'Latest ingestion, queue, or curation checkpoint change for this Chat.';
COMMENT ON COLUMN source_contents.recorded_at IS
    'Time Centaur Context stored this immutable content version.';

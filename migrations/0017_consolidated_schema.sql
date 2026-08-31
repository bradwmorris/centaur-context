-- Consolidate supporting state into human-maintainable Users, Artifacts, Runs,
-- Embeddings, and one authoritative Object/Connection event ledger.

-- Stop legacy Eval triggers from producing rows while this transaction migrates.
DROP TRIGGER IF EXISTS objects_trace_eval ON objects;
DROP TRIGGER IF EXISTS connections_trace_eval ON connections;

-- Users keep their provider identities together while preserving stable IDs.
ALTER TABLE users ADD COLUMN identities jsonb NOT NULL DEFAULT '[]'::jsonb;
UPDATE users u
SET identities = COALESCE((
    SELECT jsonb_agg(jsonb_build_object(
        'id', e.id,
        'provider', e.provider,
        'workspace_id', e.workspace_id,
        'provider_user_id', e.provider_user_id,
        'display_name', e.display_name,
        'avatar_url', e.avatar_url,
        'avatar_asset_sha256', e.avatar_asset_sha256,
        'avatar_asset_filename', e.avatar_asset_filename,
        'avatar_provenance', e.avatar_provenance,
        'profile_refreshed_at', e.profile_refreshed_at
    ) ORDER BY e.provider,e.workspace_id,e.provider_user_id,e.id)
    FROM external_identities e WHERE e.user_object_id=u.object_id
), '[]'::jsonb);
ALTER TABLE users ADD CONSTRAINT users_identities_array_check
    CHECK (jsonb_typeof(identities)='array' AND pg_column_size(identities) <= 262144);

CREATE OR REPLACE FUNCTION validate_user_identities() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE identity jsonb;
DECLARE identity_key text;
BEGIN
    IF jsonb_typeof(NEW.identities) <> 'array' THEN
        RAISE EXCEPTION 'User identities must be a JSON array';
    END IF;
    IF EXISTS (
        SELECT 1 FROM jsonb_array_elements(NEW.identities) value
        WHERE jsonb_typeof(value) <> 'object'
           OR btrim(COALESCE(value->>'provider',''))=''
           OR btrim(COALESCE(value->>'provider_user_id',''))=''
           OR btrim(COALESCE(value->>'id',''))=''
    ) THEN
        RAISE EXCEPTION 'each User identity requires id, provider, and provider_user_id';
    END IF;
    IF EXISTS (
        SELECT 1 FROM jsonb_array_elements(NEW.identities) value
        GROUP BY value->>'provider',COALESCE(value->>'workspace_id',''),value->>'provider_user_id'
        HAVING count(*) > 1
    ) THEN
        RAISE EXCEPTION 'duplicate provider identity within User %',NEW.object_id;
    END IF;
    FOR identity IN SELECT value FROM jsonb_array_elements(NEW.identities) value LOOP
        identity_key := (identity->>'provider') || E'\n' ||
            COALESCE(identity->>'workspace_id','') || E'\n' ||
            (identity->>'provider_user_id');
        PERFORM pg_advisory_xact_lock(hashtextextended(identity_key,0));
        IF EXISTS (
            SELECT 1 FROM users other
            CROSS JOIN LATERAL jsonb_array_elements(other.identities) other_identity
            WHERE other.object_id <> NEW.object_id
              AND other_identity->>'provider'=identity->>'provider'
              AND COALESCE(other_identity->>'workspace_id','')=COALESCE(identity->>'workspace_id','')
              AND other_identity->>'provider_user_id'=identity->>'provider_user_id'
        ) THEN
            RAISE EXCEPTION 'provider identity is already assigned to another User';
        END IF;
    END LOOP;
    RETURN NEW;
END $$;
CREATE TRIGGER users_validate_identities
BEFORE INSERT OR UPDATE OF identities ON users
FOR EACH ROW EXECUTE FUNCTION validate_user_identities();
CREATE INDEX users_identities_gin_idx ON users USING gin (identities jsonb_path_ops);

-- Artifacts generalize immutable Source content to supporting material for any Object.
CREATE TABLE artifacts (
    id uuid PRIMARY KEY,
    object_id uuid NOT NULL REFERENCES objects(id) ON DELETE RESTRICT,
    kind text NOT NULL CHECK (char_length(btrim(kind)) BETWEEN 1 AND 100),
    title text CHECK (title IS NULL OR char_length(btrim(title)) BETWEEN 1 AND 500),
    content text,
    uri text CHECK (uri IS NULL OR char_length(btrim(uri)) BETWEEN 1 AND 2000),
    media_type text CHECK (media_type IS NULL OR char_length(btrim(media_type)) BETWEEN 1 AND 255),
    language text CHECK (language IS NULL OR char_length(btrim(language)) BETWEEN 1 AND 35),
    sha256 text NOT NULL CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    size_bytes bigint NOT NULL CHECK (size_bytes > 0),
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(metadata)='object' AND pg_column_size(metadata) <= 262144),
    supersedes_artifact_id uuid REFERENCES artifacts(id) ON DELETE RESTRICT,
    captured_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (object_id,id),
    CHECK (content IS NOT NULL OR uri IS NOT NULL),
    CHECK (content IS NULL OR size_bytes=octet_length(content)),
    CHECK (supersedes_artifact_id IS NULL OR supersedes_artifact_id<>id)
);
INSERT INTO artifacts (
    id,object_id,kind,content,uri,language,sha256,size_bytes,metadata,
    supersedes_artifact_id,captured_at,created_at
)
SELECT sc.id,sc.source_object_id,sc.content_kind,sc.normalized_text,
       sc.capture_artifact_reference,sc.language,sc.content_sha256,sc.size_bytes,
       jsonb_strip_nulls(jsonb_build_object(
           'legacy_version',sc.version,
           'extraction_method',sc.extraction_method,
           'extraction_version',sc.extraction_version,
           'coverage',sc.coverage,
           'locators',sc.locators,
           'recorded_at',sc.recorded_at
       )),
       lag(sc.id) OVER (PARTITION BY sc.source_object_id ORDER BY sc.version),
       sc.captured_at,sc.recorded_at
FROM source_contents sc;

CREATE OR REPLACE FUNCTION preserve_artifact() RETURNS trigger
LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'Artifacts are immutable'; END $$;
CREATE TRIGGER artifacts_are_immutable
BEFORE UPDATE OR DELETE ON artifacts FOR EACH ROW EXECUTE FUNCTION preserve_artifact();
CREATE INDEX artifacts_object_idx ON artifacts (object_id,created_at DESC,id);
CREATE INDEX artifacts_text_search_idx ON artifacts
    USING gin (to_tsvector('simple',COALESCE(content,''))) WHERE content IS NOT NULL;

ALTER TABLE sources DROP CONSTRAINT sources_current_content_fk;
ALTER TABLE sources RENAME COLUMN current_content_id TO current_artifact_id;
ALTER TABLE sources ADD CONSTRAINT sources_current_artifact_fk
    FOREIGN KEY (object_id,current_artifact_id)
    REFERENCES artifacts(object_id,id) ON DELETE RESTRICT;

-- One Run owns execution, review, usage, and orchestration state.
CREATE TABLE runs (
    id uuid PRIMARY KEY,
    parent_run_id uuid REFERENCES runs(id) ON DELETE RESTRICT,
    kind text NOT NULL CHECK (char_length(btrim(kind)) BETWEEN 1 AND 100),
    status text NOT NULL CHECK (char_length(btrim(status)) BETWEEN 1 AND 100),
    actor_type text NOT NULL CHECK (actor_type IN ('human','centaur_agent','system')),
    actor_id text NOT NULL CHECK (char_length(btrim(actor_id)) BETWEEN 1 AND 300),
    chat_object_id uuid REFERENCES chats(object_id) ON DELETE RESTRICT,
    idempotency_key text NOT NULL CHECK (char_length(btrim(idempotency_key)) BETWEEN 1 AND 500),
    input jsonb NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(input)='object' AND pg_column_size(input) <= 1048576),
    trace jsonb NOT NULL DEFAULT '[]'::jsonb
        CHECK (jsonb_typeof(trace)='array' AND pg_column_size(trace) <= 8388608),
    result jsonb NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(result)='object' AND pg_column_size(result) <= 1048576),
    consulted_object_ids uuid[] NOT NULL DEFAULT '{}',
    error text CHECK (error IS NULL OR char_length(error) <= 10000),
    verdict text NOT NULL DEFAULT 'unreviewed'
        CHECK (verdict IN ('unreviewed','pass','mixed','fail')),
    review_notes text CHECK (review_notes IS NULL OR char_length(review_notes) <= 4000),
    reviewed_by text CHECK (reviewed_by IS NULL OR btrim(reviewed_by)<>''),
    reviewed_at timestamptz,
    available_at timestamptz,
    started_at timestamptz,
    completed_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (kind,idempotency_key),
    CHECK (parent_run_id IS NULL OR parent_run_id<>id),
    CHECK ((reviewed_by IS NULL)=(reviewed_at IS NULL)),
    CHECK (
        (kind='external_action' AND status IN (
            'reserved','previewed','approved','attempting','reconciliation_required',
            'accepted','delivered','suppressed','failed'
        )) OR
        (kind='curator' AND status IN ('queued','running','completed','failed','reversed')) OR
        (kind='curator_undo' AND status IN ('running','completed','failed')) OR
        (kind='intake' AND status IN ('running','completed','failed')) OR
        (kind='slack_interaction' AND status IN ('open','running','completed','failed','reversed')) OR
        (kind IN ('human_mutation','system_mutation','mutation','legacy_import')
            AND status IN ('open','running','completed','failed','reversed'))
    )
);

CREATE OR REPLACE FUNCTION validate_run_references() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE candidate uuid;
DECLARE cursor_id uuid;
BEGIN
    FOREACH candidate IN ARRAY NEW.consulted_object_ids LOOP
        IF NOT EXISTS (SELECT 1 FROM objects WHERE id=candidate) THEN
            RAISE EXCEPTION 'consulted Object % does not exist',candidate;
        END IF;
    END LOOP;
    IF NEW.parent_run_id IS NOT NULL THEN
        cursor_id := NEW.parent_run_id;
        LOOP
            IF cursor_id=NEW.id THEN RAISE EXCEPTION 'Run parent cycle'; END IF;
            SELECT parent_run_id INTO cursor_id FROM runs WHERE id=cursor_id;
            EXIT WHEN cursor_id IS NULL;
        END LOOP;
    END IF;
    IF NEW.chat_object_id IS NOT NULL AND NEW.input ? 'first_message_id' THEN
        IF NOT NEW.input ? 'last_message_id'
           OR NOT EXISTS (SELECT 1 FROM chat_messages WHERE chat_object_id=NEW.chat_object_id AND id=(NEW.input->>'first_message_id')::uuid)
           OR NOT EXISTS (SELECT 1 FROM chat_messages WHERE chat_object_id=NEW.chat_object_id AND id=(NEW.input->>'last_message_id')::uuid)
        THEN RAISE EXCEPTION 'Run message window must belong to its Chat'; END IF;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER runs_validate_references
BEFORE INSERT OR UPDATE OF parent_run_id,chat_object_id,input,consulted_object_ids ON runs
FOR EACH ROW EXECUTE FUNCTION validate_run_references();

CREATE OR REPLACE FUNCTION preserve_run_identity_input() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE trace_index integer;
BEGIN
    IF OLD.id<>NEW.id OR OLD.kind<>NEW.kind OR OLD.actor_type<>NEW.actor_type
       OR OLD.actor_id<>NEW.actor_id OR OLD.idempotency_key<>NEW.idempotency_key
       OR OLD.input<>NEW.input OR OLD.created_at<>NEW.created_at THEN
        RAISE EXCEPTION 'Run identity and input are immutable';
    END IF;
    IF jsonb_array_length(NEW.trace) < jsonb_array_length(OLD.trace) THEN
        RAISE EXCEPTION 'Run trace is append-only';
    END IF;
    IF jsonb_array_length(OLD.trace) > 0 THEN
        FOR trace_index IN 0..jsonb_array_length(OLD.trace)-1 LOOP
            IF NEW.trace->trace_index IS DISTINCT FROM OLD.trace->trace_index THEN
                RAISE EXCEPTION 'Run trace is append-only';
            END IF;
        END LOOP;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER runs_preserve_identity_input
BEFORE UPDATE ON runs FOR EACH ROW EXECUTE FUNCTION preserve_run_identity_input();

WITH eval_rows AS (
    SELECT e.*,
           cr.id AS linked_curator_id,
           cr.trigger AS curator_trigger,cr.status AS curator_status,
           cr.first_message_id,cr.last_message_id,cr.message_count,
           cr.queued_at,cr.started_at AS curator_started_at,
           cr.completed_at AS curator_completed_at,cr.reversed_at,
           cr.error_message,cr.attempts,cr.available_at,cr.model,cr.prompt_version,
           cr.proposed_plan,cr.committed_plan,cr.result AS curator_result,
           action.object_id AS external_action_id,
           action.provider AS external_action_provider,
           action.action_kind AS external_action_kind,
           action.external_key AS external_action_key,
           action.metadata AS external_action_metadata,
           action.state AS external_action_state
    FROM evals e
    LEFT JOIN curator_runs cr ON cr.id=e.curator_run_id
    LEFT JOIN LATERAL (
        SELECT (array_agg(DISTINCT eo.object_id))[1] AS object_id,
               min(xa.provider) AS provider,min(xa.action_kind) AS action_kind,
               min(xa.external_key) AS external_key,
               min(xa.state) AS state,
               jsonb_agg(DISTINCT xa.metadata)->0 AS metadata
        FROM eval_objects eo JOIN external_actions xa ON xa.object_id=eo.object_id
        WHERE eo.eval_id=e.id
        HAVING count(DISTINCT eo.object_id)=1
    ) action ON true
)
INSERT INTO runs (
    id,kind,status,actor_type,actor_id,chat_object_id,idempotency_key,input,trace,result,
    consulted_object_ids,error,verdict,review_notes,reviewed_by,reviewed_at,
    available_at,started_at,completed_at,created_at,updated_at
)
SELECT COALESCE(linked_curator_id,external_action_id,id),
       CASE WHEN linked_curator_id IS NOT NULL THEN 'curator'
            WHEN external_action_id IS NOT NULL THEN 'external_action' ELSE kind END,
       CASE WHEN linked_curator_id IS NOT NULL THEN curator_status
            WHEN external_action_id IS NOT NULL THEN external_action_state
            ELSE status END,
       actor_type,actor_id,chat_object_id,
       CASE WHEN linked_curator_id IS NOT NULL THEN 'curator:'||linked_curator_id::text
            WHEN external_action_id IS NOT NULL THEN 'external-action:'||external_action_id::text
            ELSE idempotency_key END,
       jsonb_strip_nulls(jsonb_build_object(
           'legacy_eval_id',id,'summary',summary,'trigger',curator_trigger,
           'first_message_id',first_message_id,'last_message_id',last_message_id,
           'message_count',message_count,'model',model,'prompt_version',prompt_version,
           'proposed_plan',proposed_plan,'committed_plan',committed_plan,
           'annotation_revision',annotation_revision,
           'provider',external_action_provider,'action_kind',external_action_kind,
           'external_key',external_action_key,'metadata',external_action_metadata
       )),
       COALESCE((SELECT jsonb_agg(to_jsonb(t)-'eval_id' ORDER BY t.sequence)
                 FROM eval_trace_entries t WHERE t.eval_id=eval_rows.id),'[]'::jsonb),
       jsonb_strip_nulls(jsonb_build_object(
           'summary',summary,'legacy_result',curator_result,
           'affected_object_ids',(SELECT COALESCE(jsonb_agg(DISTINCT eo.object_id),'[]'::jsonb)
                                  FROM eval_objects eo WHERE eo.eval_id=eval_rows.id
                                    AND eo.role NOT IN ('consulted','participant'))
       )),
       ARRAY(SELECT DISTINCT eo.object_id FROM eval_objects eo
             WHERE eo.eval_id=eval_rows.id AND eo.role IN ('consulted','participant')
             ORDER BY eo.object_id),
       COALESCE(error_message,error_summary),verdict,notes,annotated_by,annotated_at,
       COALESCE(available_at,started_at),COALESCE(curator_started_at,started_at),
       COALESCE(curator_completed_at,completed_at,reversed_at),created_at,updated_at
FROM eval_rows;

-- External Actions without a uniquely linked Eval still become Runs.
INSERT INTO runs (id,kind,status,actor_type,actor_id,idempotency_key,input,result,created_at,updated_at)
SELECT xa.object_id,'external_action',xa.state,o.created_by_type,o.created_by_id,
       'external-action:'||xa.object_id::text,
       jsonb_build_object('provider',xa.provider,'action_kind',xa.action_kind,
                          'external_key',xa.external_key,'metadata',xa.metadata),
       jsonb_build_object('state',xa.state),xa.created_at,xa.updated_at
FROM external_actions xa JOIN objects o ON o.id=xa.object_id
ON CONFLICT (id) DO UPDATE SET
    status=EXCLUDED.status,
    result=runs.result || EXCLUDED.result,
    updated_at=EXCLUDED.updated_at;

-- External Action events were execution history rather than canonical Object
-- mutations. Preserve them, including their idempotency keys, in Run trace.
UPDATE runs r
SET trace = r.trace || history.entries
FROM (
    SELECT e.object_id AS run_id,
           jsonb_agg(jsonb_build_object(
               'id',e.id,'entry_type','external_action_event',
               'event_type',e.changes->>'event_type',
               'idempotency_key',e.idempotency_key,
               'metadata',COALESCE(e.changes->'metadata','{}'::jsonb),
               'actor_type',e.actor_type,'actor_id',e.actor_id,
               'created_at',e.created_at
           ) ORDER BY e.created_at,e.id) AS entries
    FROM object_events e
    WHERE e.entity_type='external_action'
    GROUP BY e.object_id
) history
WHERE r.id=history.run_id AND r.kind='external_action';
CREATE UNIQUE INDEX runs_external_action_key_idx ON runs
    ((input->>'provider'),(input->>'action_kind'),(input->>'external_key'))
    WHERE kind='external_action';
CREATE INDEX runs_queue_idx ON runs (status,available_at,created_at,id);
CREATE INDEX runs_parent_idx ON runs (parent_run_id,created_at,id) WHERE parent_run_id IS NOT NULL;
CREATE INDEX runs_consulted_gin_idx ON runs USING gin (consulted_object_ids);

-- A fixed legacy Run owns old events that cannot be tied to an exact execution.
INSERT INTO runs (id,kind,status,actor_type,actor_id,idempotency_key,input,result,completed_at)
VALUES ('00000000-0000-0000-0000-000000000017','legacy_import','completed','system',
        'schema-migration-17','schema-migration-17:legacy-events',
        '{"source":"schema_16_object_events"}'::jsonb,
        '{"summary":"Historical events without exact Run linkage"}'::jsonb,now())
ON CONFLICT (id) DO NOTHING;

CREATE TABLE object_events_v17 (
    id uuid PRIMARY KEY,
    run_id uuid NOT NULL REFERENCES runs(id) ON DELETE RESTRICT,
    sequence integer NOT NULL CHECK (sequence > 0),
    target_type text NOT NULL CHECK (target_type IN ('object','connection')),
    target_id uuid NOT NULL,
    action text NOT NULL CHECK (char_length(btrim(action)) BETWEEN 1 AND 100),
    actor_type text NOT NULL CHECK (actor_type IN ('human','centaur_agent','system')),
    actor_id text NOT NULL CHECK (char_length(btrim(actor_id)) BETWEEN 1 AND 300),
    idempotency_key text,
    from_revision bigint,
    to_revision bigint NOT NULL CHECK (to_revision > 0),
    before_state jsonb,
    after_state jsonb NOT NULL CHECK (jsonb_typeof(after_state)='object'),
    reversible boolean NOT NULL,
    created_at timestamptz NOT NULL,
    UNIQUE (run_id,sequence)
);

WITH mapped AS (
    SELECT e.*,
           CASE
             WHEN e.idempotency_key ~ '^curator:[0-9a-f-]{36}:'
               THEN split_part(e.idempotency_key,':',2)::uuid
             ELSE '00000000-0000-0000-0000-000000000017'::uuid
           END AS mapped_run_id,
           crc.sequence AS change_sequence,crc.before_state,crc.after_state,
           row_number() OVER (ORDER BY e.created_at,e.id) AS legacy_sequence
    FROM object_events e
    LEFT JOIN curator_run_changes crc
      ON e.idempotency_key LIKE 'curator:'||crc.curator_run_id::text||':%'
     AND e.entity_id=crc.entity_id AND e.to_revision=crc.after_revision
    WHERE e.entity_type IN ('object','task','connection')
      AND e.action IN ('created','updated','archived','connected','task_status_changed')
)
INSERT INTO object_events_v17 (
    id,run_id,sequence,target_type,target_id,action,actor_type,actor_id,idempotency_key,
    from_revision,to_revision,before_state,after_state,reversible,created_at
)
SELECT id,mapped_run_id,
       COALESCE(change_sequence,legacy_sequence::integer),
       CASE WHEN entity_type='connection' THEN 'connection' ELSE 'object' END,
       CASE WHEN entity_type='connection' THEN entity_id ELSE object_id END,
       CASE WHEN action='connected' THEN 'created' ELSE action END,
       actor_type,actor_id,idempotency_key,from_revision,to_revision,before_state,
       COALESCE(after_state,changes,'{}'::jsonb),change_sequence IS NOT NULL,created_at
FROM mapped;

CREATE OR REPLACE FUNCTION preserve_object_event() RETURNS trigger
LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'Object Events are immutable'; END $$;
CREATE TRIGGER object_events_v17_are_immutable
BEFORE UPDATE OR DELETE ON object_events_v17 FOR EACH ROW EXECUTE FUNCTION preserve_object_event();
CREATE UNIQUE INDEX object_events_v17_idempotency_idx
    ON object_events_v17 (actor_type,actor_id,idempotency_key)
    WHERE idempotency_key IS NOT NULL;
CREATE INDEX object_events_v17_target_idx
    ON object_events_v17 (target_type,target_id,created_at DESC,id);

-- One row now carries both embedding queue state and completed vector output.
CREATE TABLE embeddings (
    object_id uuid NOT NULL REFERENCES objects(id) ON DELETE CASCADE,
    model text NOT NULL CHECK (char_length(btrim(model)) BETWEEN 1 AND 300),
    dimensions integer NOT NULL CHECK (dimensions BETWEEN 1 AND 2000),
    source_hash text NOT NULL CHECK (char_length(source_hash)=32),
    format_version text NOT NULL CHECK (char_length(btrim(format_version)) BETWEEN 1 AND 100),
    input_mode text NOT NULL CHECK (input_mode IN ('shared','search_document')),
    status text NOT NULL CHECK (status IN ('pending','running','failed','completed')),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    available_at timestamptz NOT NULL DEFAULT now(),
    started_at timestamptz,
    completed_at timestamptz,
    last_error text,
    embedding vector,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (object_id,model),
    CHECK ((status='completed')=(embedding IS NOT NULL)),
    CHECK (embedding IS NULL OR vector_dims(embedding)=dimensions),
    CHECK ((status='running')=(started_at IS NOT NULL)),
    CHECK ((status='completed')=(completed_at IS NOT NULL))
);
INSERT INTO embeddings (
    object_id,model,dimensions,source_hash,format_version,input_mode,status,attempts,
    available_at,started_at,completed_at,last_error,embedding,created_at,updated_at
)
SELECT COALESCE(e.object_id,j.object_id),COALESCE(e.model,'__unconfigured__'),
       COALESCE(e.dimensions,1),COALESCE(e.source_hash,j.source_hash),
       COALESCE(e.format_version,j.format_version),COALESCE(e.input_mode,j.input_mode),
       CASE WHEN e.object_id IS NOT NULL THEN 'completed' ELSE j.status END,
       COALESCE(j.attempts,0),COALESCE(j.available_at,e.embedded_at),j.started_at,
       e.embedded_at,j.last_error,e.embedding,COALESCE(j.created_at,e.embedded_at),
       COALESCE(j.updated_at,e.embedded_at)
FROM object_embedding_jobs j FULL JOIN object_embeddings e ON e.object_id=j.object_id
WHERE COALESCE(e.object_id,j.object_id) NOT IN (SELECT object_id FROM external_actions);
CREATE INDEX embeddings_claim_idx ON embeddings (available_at,updated_at,object_id)
    WHERE status IN ('pending','failed');
-- The configured dimension is deployment-specific. RD 3 owns the eventual
-- dimension-specific ANN index; the initial consolidated store remains exact.

-- Reconciliation guards: abort this transaction rather than drop a fact that
-- was not mapped exactly once to its consolidated owner.
DO $$
DECLARE old_count bigint;
DECLARE new_count bigint;
BEGIN
    SELECT count(*) INTO old_count FROM external_identities;
    SELECT COALESCE(sum(jsonb_array_length(identities)),0) INTO new_count FROM users;
    IF old_count<>new_count THEN
        RAISE EXCEPTION 'identity reconciliation failed: % old, % embedded',old_count,new_count;
    END IF;

    SELECT count(*) INTO old_count FROM source_contents;
    SELECT count(*) INTO new_count FROM artifacts;
    IF old_count<>new_count OR EXISTS (
        SELECT 1 FROM source_contents sc
        LEFT JOIN artifacts a ON a.id=sc.id AND a.object_id=sc.source_object_id
          AND a.content IS NOT DISTINCT FROM sc.normalized_text
          AND a.sha256=sc.content_sha256 AND a.size_bytes=sc.size_bytes
        WHERE a.id IS NULL
    ) THEN RAISE EXCEPTION 'Artifact reconciliation failed'; END IF;

    IF EXISTS (
        SELECT 1 FROM evals e
        WHERE NOT EXISTS (
            SELECT 1 FROM runs r WHERE r.input->>'legacy_eval_id'=e.id::text
        )
    ) THEN RAISE EXCEPTION 'Eval-to-Run reconciliation failed'; END IF;

    IF EXISTS (
        SELECT 1 FROM curator_runs cr
        WHERE NOT EXISTS (SELECT 1 FROM runs r WHERE r.id=cr.id AND r.kind='curator')
    ) THEN RAISE EXCEPTION 'Curator Run reconciliation failed'; END IF;

    IF EXISTS (
        SELECT 1 FROM external_actions xa
        WHERE NOT EXISTS (SELECT 1 FROM runs r WHERE r.id=xa.object_id AND r.kind='external_action')
    ) THEN RAISE EXCEPTION 'External Action reconciliation failed'; END IF;

    IF EXISTS (
        SELECT 1 FROM curator_run_changes crc
        WHERE (SELECT count(*) FROM object_events e
               WHERE e.idempotency_key LIKE 'curator:'||crc.curator_run_id::text||':%'
                 AND e.entity_id=crc.entity_id AND e.to_revision=crc.after_revision)<>1
    ) THEN RAISE EXCEPTION 'Curator change-to-event reconciliation failed'; END IF;

    IF (SELECT count(*) FROM object_events_v17)<>(
        SELECT count(*) FROM object_events e
        WHERE e.entity_type IN ('object','task','connection')
          AND e.action IN ('created','updated','archived','connected','task_status_changed')
    ) THEN RAISE EXCEPTION 'mutation event reconciliation failed'; END IF;

    IF EXISTS (
        SELECT 1 FROM object_events e
        WHERE NOT (
            (e.entity_type IN ('object','task','connection') AND
             e.action IN ('created','updated','archived','connected','task_status_changed')) OR
            (e.entity_type='source_content' AND e.action='content_version_created' AND
             EXISTS (SELECT 1 FROM artifacts a WHERE a.id=e.entity_id)) OR
            (e.entity_type='chat_message' AND e.action='message_ingested' AND
             EXISTS (SELECT 1 FROM chat_messages m WHERE m.id=e.entity_id)) OR
            (e.entity_type='curator_run' AND e.action IN (
                'curator_queued','curator_started','curator_committed','curator_failed','curator_undone'
             ) AND EXISTS (SELECT 1 FROM runs r WHERE r.id=e.entity_id AND r.kind='curator')) OR
            (e.entity_type='external_action' AND e.action='external_action_event' AND
             EXISTS (SELECT 1 FROM runs r
                     CROSS JOIN LATERAL jsonb_array_elements(r.trace) entry
                     WHERE r.id=e.object_id AND r.kind='external_action'
                       AND entry->>'id'=e.id::text))
        )
    ) THEN RAISE EXCEPTION 'unclassified legacy Object Event would be lost'; END IF;

    SELECT count(DISTINCT COALESCE(e.object_id,j.object_id)) INTO old_count
    FROM object_embedding_jobs j FULL JOIN object_embeddings e ON e.object_id=j.object_id
    WHERE COALESCE(e.object_id,j.object_id) NOT IN (SELECT object_id FROM external_actions);
    SELECT count(*) INTO new_count FROM embeddings;
    IF old_count<>new_count THEN
        RAISE EXCEPTION 'Embedding reconciliation failed: % old targets, % rows',old_count,new_count;
    END IF;
END $$;

-- Remove old SQL machinery and supporting tables only after data is copied.
DROP FUNCTION IF EXISTS centaur_context_trace_object_mutation() CASCADE;
DROP FUNCTION IF EXISTS centaur_context_trace_connection_mutation() CASCADE;
DROP FUNCTION IF EXISTS centaur_context_append_trace(uuid,text,jsonb) CASCADE;
DROP FUNCTION IF EXISTS centaur_context_ensure_mutation_eval(text,text) CASCADE;
DROP FUNCTION IF EXISTS centaur_context_active_eval() CASCADE;
DROP FUNCTION IF EXISTS centaur_context_set_eval_context(uuid) CASCADE;
DROP TRIGGER IF EXISTS source_contents_are_immutable ON source_contents;
DROP FUNCTION IF EXISTS preserve_source_content() CASCADE;
DROP TRIGGER IF EXISTS theme_proposals_preserve_decision ON theme_proposals;
DROP TRIGGER IF EXISTS theme_proposals_prevent_delete ON theme_proposals;
DROP FUNCTION IF EXISTS preserve_theme_proposal_decision() CASCADE;
DROP FUNCTION IF EXISTS prevent_theme_proposal_delete() CASCADE;
DROP TRIGGER IF EXISTS objects_queue_embedding ON objects;
DROP FUNCTION IF EXISTS queue_object_embedding() CASCADE;

DROP TABLE curator_run_changes;
DROP TABLE eval_trace_entries;
DROP TABLE eval_objects;
DROP TABLE evals;
DROP TABLE curator_runs;
DROP TABLE theme_proposals;
DROP TABLE principal_permissions;
DROP TABLE external_identities;
DROP TABLE source_contents;
DROP TABLE object_embedding_jobs;
DROP TABLE object_embeddings;

DROP TRIGGER IF EXISTS external_actions_preserve_subtype ON external_actions;
DROP TABLE external_actions;

-- External Actions are Runs, no longer canonical Objects.
DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM objects o WHERE o.kind='external_action'
          AND EXISTS (SELECT 1 FROM connections c WHERE c.source_object_id=o.id OR c.target_object_id=o.id)
    ) THEN RAISE EXCEPTION 'External Action Objects still have Connections'; END IF;
END $$;

DROP TABLE object_events;
ALTER TABLE object_events_v17 RENAME TO object_events;
ALTER INDEX object_events_v17_pkey RENAME TO object_events_pkey;
ALTER INDEX object_events_v17_idempotency_idx RENAME TO object_events_idempotency_idx;
ALTER INDEX object_events_v17_target_idx RENAME TO object_events_target_idx;
ALTER TRIGGER object_events_v17_are_immutable ON object_events RENAME TO object_events_are_immutable;

DELETE FROM objects WHERE kind='external_action';
ALTER TABLE objects DROP CONSTRAINT objects_kind_check;
ALTER TABLE objects ADD CONSTRAINT objects_kind_check CHECK (
    kind IN ('task','chat','user','entity','memory','source','note','theme')
);

CREATE OR REPLACE FUNCTION enforce_object_subtype() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.kind='task' AND NOT EXISTS (SELECT 1 FROM tasks WHERE object_id=NEW.id) THEN
        RAISE EXCEPTION 'task Object % requires a tasks subtype row',NEW.id;
    ELSIF NEW.kind='chat' AND NOT EXISTS (SELECT 1 FROM chats WHERE object_id=NEW.id) THEN
        RAISE EXCEPTION 'chat Object % requires a chats subtype row',NEW.id;
    ELSIF NEW.kind='user' AND NOT EXISTS (SELECT 1 FROM users WHERE object_id=NEW.id) THEN
        RAISE EXCEPTION 'user Object % requires a users subtype row',NEW.id;
    ELSIF NEW.kind='entity' AND NOT EXISTS (SELECT 1 FROM entities WHERE object_id=NEW.id) THEN
        RAISE EXCEPTION 'entity Object % requires an entities subtype row',NEW.id;
    ELSIF NEW.kind='memory' AND NOT EXISTS (SELECT 1 FROM memories WHERE object_id=NEW.id) THEN
        RAISE EXCEPTION 'memory Object % requires a memories subtype row',NEW.id;
    ELSIF NEW.kind='source' AND NOT EXISTS (SELECT 1 FROM sources WHERE object_id=NEW.id) THEN
        RAISE EXCEPTION 'source Object % requires a sources subtype row',NEW.id;
    ELSIF NEW.kind='note' AND NOT EXISTS (SELECT 1 FROM notes WHERE object_id=NEW.id) THEN
        RAISE EXCEPTION 'note Object % requires a notes subtype row',NEW.id;
    ELSIF NEW.kind='theme' AND NOT EXISTS (SELECT 1 FROM themes WHERE object_id=NEW.id) THEN
        RAISE EXCEPTION 'theme Object % requires a themes subtype row',NEW.id;
    END IF;
    RETURN NEW;
END $$;

CREATE OR REPLACE FUNCTION invalidate_object_embeddings() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE desired_hash text;
BEGIN
    desired_hash := object_embedding_source_hash('centaur-object-v1',NEW.kind,NEW.title,NEW.description);
    UPDATE embeddings SET source_hash=desired_hash,status='pending',attempts=0,
        available_at=now(),started_at=NULL,completed_at=NULL,last_error=NULL,
        embedding=NULL,updated_at=now()
    WHERE object_id=NEW.id AND source_hash IS DISTINCT FROM desired_hash;
    RETURN NEW;
END $$;
CREATE TRIGGER objects_invalidate_embeddings
AFTER UPDATE OF kind,title,description ON objects
FOR EACH ROW EXECUTE FUNCTION invalidate_object_embeddings();

DROP TABLE schema_visualizer_tables;
CREATE VIEW schema_visualizer_tables(table_name) AS VALUES
    ('objects'),('connections'),('tasks'),('chats'),('chat_messages'),('users'),
    ('entities'),('memories'),('sources'),('notes'),('themes'),('artifacts'),
    ('runs'),('embeddings'),('object_events');

COMMENT ON TABLE artifacts IS 'Immutable supporting content or external artifact attached to any Object.';
COMMENT ON TABLE runs IS 'Execution, orchestration, trace, result, and human review state.';
COMMENT ON TABLE embeddings IS 'Combined queue and completed vector state for Object embeddings.';
COMMENT ON TABLE object_events IS 'Immutable authoritative Object and Connection mutation/reversal history.';

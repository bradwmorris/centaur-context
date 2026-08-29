CREATE TABLE evals (
    id uuid PRIMARY KEY,
    kind text NOT NULL CHECK (kind IN ('slack_interaction','human_mutation','system_mutation','legacy_import')),
    status text NOT NULL DEFAULT 'open' CHECK (status IN ('open','running','completed','failed','reversed')),
    actor_type text NOT NULL CHECK (actor_type IN ('human','centaur_agent','system')),
    actor_id text NOT NULL CHECK (btrim(actor_id) <> ''),
    chat_object_id uuid REFERENCES objects(id) ON DELETE RESTRICT,
    curator_run_id uuid UNIQUE REFERENCES curator_runs(id) ON DELETE RESTRICT,
    summary text NOT NULL CHECK (btrim(summary) <> '' AND char_length(summary) <= 1000),
    error_summary text CHECK (error_summary IS NULL OR char_length(error_summary) <= 4000),
    idempotency_key text NOT NULL UNIQUE CHECK (btrim(idempotency_key) <> ''),
    verdict text NOT NULL DEFAULT 'unreviewed' CHECK (verdict IN ('unreviewed','pass','mixed','fail')),
    notes text CHECK (notes IS NULL OR char_length(notes) <= 4000),
    annotated_by text CHECK (annotated_by IS NULL OR btrim(annotated_by) <> ''),
    annotated_at timestamptz,
    annotation_revision bigint NOT NULL DEFAULT 0 CHECK (annotation_revision >= 0),
    next_sequence bigint NOT NULL DEFAULT 0 CHECK (next_sequence >= 0),
    started_at timestamptz NOT NULL DEFAULT now(),
    completed_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((annotated_by IS NULL) = (annotated_at IS NULL))
);

CREATE INDEX evals_dashboard_idx ON evals (created_at DESC, id DESC);
CREATE INDEX evals_filters_idx ON evals (kind, status, verdict, created_at DESC);
CREATE INDEX evals_chat_idx ON evals (chat_object_id, created_at DESC) WHERE chat_object_id IS NOT NULL;

CREATE TABLE eval_trace_entries (
    id uuid PRIMARY KEY,
    eval_id uuid NOT NULL REFERENCES evals(id) ON DELETE CASCADE,
    sequence bigint NOT NULL CHECK (sequence > 0),
    entry_type text NOT NULL CHECK (entry_type IN (
        'message_ingested','model_attempt','validation_repair','object_created','object_updated',
        'object_archived','connection_created','connection_updated','connection_archived',
        'commit','failure','reversal','legacy_import'
    )),
    component text,
    provider text,
    model_id text,
    display_tier text,
    execution_type text CHECK (execution_type IS NULL OR execution_type IN ('codex_harness','direct_api','embedding','other')),
    auth_mode text CHECK (auth_mode IS NULL OR auth_mode IN ('chatgpt_subscription','api_key','not_applicable','unknown')),
    upstream_service text,
    billing_mode text CHECK (billing_mode IS NULL OR billing_mode IN ('subscription_allowance','chatgpt_credits','metered_api','not_applicable','unknown')),
    reasoning_effort text,
    service_tier text,
    source_thread_id text,
    source_execution_id text,
    source_turn_id text,
    usage_status text NOT NULL DEFAULT 'not_applicable' CHECK (usage_status IN ('reported','partial','unavailable','not_applicable')),
    usage_missing_reason text,
    input_tokens bigint CHECK (input_tokens IS NULL OR input_tokens >= 0),
    output_tokens bigint CHECK (output_tokens IS NULL OR output_tokens >= 0),
    cache_creation_tokens bigint CHECK (cache_creation_tokens IS NULL OR cache_creation_tokens >= 0),
    cache_read_tokens bigint CHECK (cache_read_tokens IS NULL OR cache_read_tokens >= 0),
    reasoning_tokens bigint CHECK (reasoning_tokens IS NULL OR reasoning_tokens >= 0),
    total_tokens bigint CHECK (total_tokens IS NULL OR total_tokens >= 0),
    estimated_micro_usd bigint CHECK (estimated_micro_usd IS NULL OR estimated_micro_usd >= 0),
    chatgpt_credit_microunits bigint CHECK (chatgpt_credit_microunits IS NULL OR chatgpt_credit_microunits >= 0),
    api_equivalent_micro_usd bigint CHECK (api_equivalent_micro_usd IS NULL OR api_equivalent_micro_usd >= 0),
    rate_card_version text,
    pricing_snapshot jsonb,
    facts jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(facts) = 'object' AND pg_column_size(facts) <= 32768),
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (eval_id, sequence)
);

CREATE INDEX eval_trace_entries_eval_idx ON eval_trace_entries (eval_id, sequence);
CREATE INDEX eval_trace_entries_usage_filters_idx ON eval_trace_entries (component, provider, model_id, execution_type, auth_mode, billing_mode)
    WHERE entry_type = 'model_attempt';
CREATE UNIQUE INDEX eval_trace_entries_source_attempt_unique_idx
    ON eval_trace_entries (eval_id,component,source_execution_id,COALESCE(source_turn_id,''))
    WHERE entry_type='model_attempt' AND source_execution_id IS NOT NULL;

CREATE TABLE eval_objects (
    eval_id uuid NOT NULL REFERENCES evals(id) ON DELETE CASCADE,
    object_id uuid NOT NULL REFERENCES objects(id) ON DELETE RESTRICT,
    role text NOT NULL CHECK (role IN ('created','updated','archived','connected','consulted','participant')),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (eval_id, object_id, role)
);

CREATE INDEX eval_objects_object_idx ON eval_objects (object_id, eval_id);

CREATE FUNCTION centaur_context_set_eval_context(target_eval_id uuid) RETURNS void
LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM evals WHERE id = target_eval_id) THEN
        RAISE EXCEPTION 'unknown eval context %', target_eval_id;
    END IF;
    PERFORM set_config('centaur_context.eval_id', target_eval_id::text, true);
END;
$$;

CREATE FUNCTION centaur_context_active_eval() RETURNS uuid
LANGUAGE plpgsql STABLE AS $$
DECLARE raw_value text;
BEGIN
    raw_value := current_setting('centaur_context.eval_id', true);
    IF raw_value IS NULL OR raw_value = '' THEN RETURN NULL; END IF;
    RETURN raw_value::uuid;
EXCEPTION WHEN invalid_text_representation THEN
    RAISE EXCEPTION 'invalid centaur_context.eval_id setting';
END;
$$;

CREATE FUNCTION centaur_context_ensure_mutation_eval(actor_type_value text, actor_id_value text) RETURNS uuid
LANGUAGE plpgsql AS $$
DECLARE target_eval_id uuid;
DECLARE eval_kind text;
BEGIN
    target_eval_id := centaur_context_active_eval();
    IF target_eval_id IS NOT NULL THEN RETURN target_eval_id; END IF;
    target_eval_id := gen_random_uuid();
    eval_kind := CASE WHEN actor_type_value = 'human' THEN 'human_mutation' ELSE 'system_mutation' END;
    INSERT INTO evals (id,kind,status,actor_type,actor_id,summary,idempotency_key,completed_at)
    VALUES (target_eval_id,eval_kind,'completed',actor_type_value,actor_id_value,
            'Standalone ' || replace(eval_kind, '_', ' '),
            'standalone:' || txid_current()::text || ':' || target_eval_id::text,now());
    PERFORM centaur_context_set_eval_context(target_eval_id);
    RETURN target_eval_id;
END;
$$;

CREATE FUNCTION centaur_context_append_trace(
    target_eval_id uuid,
    target_entry_type text,
    target_facts jsonb
) RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE next_value bigint;
BEGIN
    UPDATE evals SET next_sequence=next_sequence+1,updated_at=now()
    WHERE id=target_eval_id RETURNING next_sequence INTO next_value;
    IF next_value IS NULL THEN RAISE EXCEPTION 'unknown eval %', target_eval_id; END IF;
    INSERT INTO eval_trace_entries (id,eval_id,sequence,entry_type,facts)
    VALUES (gen_random_uuid(),target_eval_id,next_value,target_entry_type,target_facts);
    RETURN next_value;
END;
$$;

CREATE FUNCTION centaur_context_trace_object_mutation() RETURNS trigger
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
    action_role := CASE WHEN TG_OP='INSERT' THEN 'created' WHEN NEW.lifecycle='archived' AND OLD.lifecycle <> 'archived' THEN 'archived' ELSE 'updated' END;
    trace_type := 'object_' || action_role;
    INSERT INTO eval_objects (eval_id,object_id,role) VALUES (target_eval_id,NEW.id,action_role) ON CONFLICT DO NOTHING;
    PERFORM centaur_context_append_trace(target_eval_id,trace_type,
        jsonb_build_object('object_id',NEW.id,'kind',NEW.kind,'from_revision',CASE WHEN TG_OP='INSERT' THEN NULL ELSE OLD.revision END,'to_revision',NEW.revision));
    RETURN NEW;
END;
$$;

CREATE TRIGGER objects_trace_eval
AFTER INSERT OR UPDATE ON objects
FOR EACH ROW EXECUTE FUNCTION centaur_context_trace_object_mutation();

CREATE FUNCTION centaur_context_trace_connection_mutation() RETURNS trigger
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
    action_role := CASE WHEN TG_OP='INSERT' THEN 'connected' WHEN NEW.archived_at IS NOT NULL AND OLD.archived_at IS NULL THEN 'archived' ELSE 'updated' END;
    trace_type := CASE action_role WHEN 'connected' THEN 'connection_created' WHEN 'archived' THEN 'connection_archived' ELSE 'connection_updated' END;
    INSERT INTO eval_objects (eval_id,object_id,role) VALUES (target_eval_id,NEW.source_object_id,'connected') ON CONFLICT DO NOTHING;
    INSERT INTO eval_objects (eval_id,object_id,role) VALUES (target_eval_id,NEW.target_object_id,'connected') ON CONFLICT DO NOTHING;
    PERFORM centaur_context_append_trace(target_eval_id,trace_type,
        jsonb_build_object('connection_id',NEW.id,'source_object_id',NEW.source_object_id,'target_object_id',NEW.target_object_id,'kind',NEW.kind,'from_revision',CASE WHEN TG_OP='INSERT' THEN NULL ELSE OLD.revision END,'to_revision',NEW.revision));
    RETURN NEW;
END;
$$;

CREATE TRIGGER connections_trace_eval
AFTER INSERT OR UPDATE ON connections
FOR EACH ROW EXECUTE FUNCTION centaur_context_trace_connection_mutation();

WITH legacy AS (
    INSERT INTO evals (id,kind,status,actor_type,actor_id,summary,idempotency_key,completed_at)
    VALUES (gen_random_uuid(),'legacy_import','completed','system','migration-0009',
            'Legacy Objects present before interaction eval tracing','legacy-import:0009',now())
    RETURNING id
), linked AS (
    INSERT INTO eval_objects (eval_id,object_id,role)
    SELECT legacy.id,objects.id,'consulted' FROM legacy CROSS JOIN objects
    RETURNING eval_id
)
INSERT INTO eval_trace_entries (id,eval_id,sequence,entry_type,usage_status,facts)
SELECT gen_random_uuid(),legacy.id,1,'legacy_import','unavailable',
       jsonb_build_object('object_count',(SELECT count(*) FROM objects),'historical_usage','unknown')
FROM legacy;

UPDATE evals SET next_sequence=1 WHERE idempotency_key='legacy-import:0009';

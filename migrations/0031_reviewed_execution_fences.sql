-- Durable tombstones are maintenance bookkeeping, not deletable fixture content.
-- No FK to Runs/Chats: fences must survive their separately reviewed purge.
CREATE TABLE maintenance_execution_fences (
    run_id uuid PRIMARY KEY,
    run_kind text NOT NULL CHECK (run_kind='slack_interaction'),
    idempotency_key text NOT NULL,
    chat_object_id uuid NOT NULL,
    provider text NOT NULL CHECK (provider='slack'),
    workspace_id text NOT NULL,
    channel_id text NOT NULL,
    thread_id text NOT NULL,
    owner_evidence jsonb NOT NULL,
    related_run_identities jsonb NOT NULL,
    principal_id text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(run_kind,idempotency_key)
);
CREATE INDEX maintenance_execution_fences_chat_idx ON maintenance_execution_fences(chat_object_id);
CREATE INDEX maintenance_execution_fences_provider_idx ON maintenance_execution_fences(provider,workspace_id,channel_id,thread_id);

ALTER TABLE runs DROP CONSTRAINT runs_kind_status_check;
ALTER TABLE runs ADD CONSTRAINT runs_kind_status_check CHECK (
    (kind='external_action' AND status IN ('reserved','previewed','approved','attempting','reconciliation_required','accepted','delivered','suppressed','failed')) OR
    (kind IN ('memory_capture_start','memory_capture','memory_dream','memory_undo') AND status IN ('running','completed','preview','failed','reversed')) OR
    (kind='curator' AND status IN ('queued','running','completed','failed','reversed')) OR
    (kind='curator_undo' AND status IN ('running','completed','failed')) OR
    (kind='intake' AND status IN ('running','completed','failed')) OR
    (kind='workflow' AND status IN ('running','completed','failed')) OR
    (kind='codex_interaction' AND status IN ('open','running','completed','failed','reversed')) OR
    (kind='slack_interaction' AND status IN ('open','running','completed','failed','reversed','cancelled')) OR
    (kind IN ('human_mutation','system_mutation','mutation','legacy_import') AND status IN ('open','running','completed','failed','reversed'))
);

-- Exact provider keys, including legacy unqualified and modern bot-qualified forms.
CREATE FUNCTION context_thread_is_fenced(thread_key text) RETURNS boolean LANGUAGE sql STABLE AS $$
    SELECT EXISTS(SELECT 1 FROM maintenance_execution_fences f WHERE f.provider='slack'
        AND split_part(thread_key,':',1)='slack' AND split_part(thread_key,':',2)=f.workspace_id
        AND ((cardinality(string_to_array(thread_key,':'))=5 AND split_part(thread_key,':',3)<>'' AND split_part(thread_key,':',4)=f.channel_id AND split_part(thread_key,':',5)=f.thread_id)
          OR (cardinality(string_to_array(thread_key,':'))=4 AND split_part(thread_key,':',3)=f.channel_id AND split_part(thread_key,':',4)=f.thread_id)))
$$;
CREATE FUNCTION context_run_is_fenced(target uuid) RETURNS boolean LANGUAGE sql STABLE AS $$
    WITH RECURSIVE ancestors(id,parent_run_id,chat_object_id,input) AS (
        SELECT id,parent_run_id,chat_object_id,input FROM runs WHERE id=target
        UNION
        SELECT r.id,r.parent_run_id,r.chat_object_id,r.input FROM runs r JOIN ancestors a ON r.id=a.parent_run_id
    )
    SELECT EXISTS(SELECT 1 FROM maintenance_execution_fences f WHERE f.run_id=target
        OR EXISTS(SELECT 1 FROM jsonb_array_elements(f.related_run_identities) i WHERE i->>'id'=target::text)
        OR EXISTS(SELECT 1 FROM ancestors a WHERE a.id=f.run_id OR a.chat_object_id=f.chat_object_id))
        OR EXISTS(SELECT 1 FROM ancestors a WHERE context_thread_is_fenced(a.input->>'centaur_thread_key') OR context_thread_is_fenced(a.input->>'source_thread_id'))
$$;
CREATE FUNCTION guard_maintenance_execution_fence() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE allowed boolean := false;
BEGIN
    -- Coordinate ordinary writers with the owner's exclusive reconciliation lock.
    LOCK TABLE maintenance_execution_fences IN SHARE MODE;
    IF TG_TABLE_NAME='runs' THEN
        IF TG_OP='UPDATE' AND to_regclass('pg_temp.context_reviewed_reconciliation') IS NOT NULL THEN
            EXECUTE 'SELECT EXISTS(SELECT 1 FROM pg_temp.context_reviewed_reconciliation WHERE run_id=$1)' INTO allowed USING OLD.id;
            IF allowed AND ((to_jsonb(NEW)-ARRAY['chat_object_id','updated_at'])=(to_jsonb(OLD)-ARRAY['chat_object_id','updated_at']) AND NEW.chat_object_id IS NULL
                OR (to_jsonb(NEW)-ARRAY['status','completed_at','updated_at'])=(to_jsonb(OLD)-ARRAY['status','completed_at','updated_at']) AND NEW.status='cancelled') THEN RETURN NEW; END IF;
        END IF;
        IF context_run_is_fenced(NEW.id) OR context_run_is_fenced(NEW.parent_run_id)
            OR context_thread_is_fenced(NEW.input->>'centaur_thread_key') OR context_thread_is_fenced(NEW.input->>'source_thread_id')
            OR EXISTS(SELECT 1 FROM jsonb_path_query(NEW.trace,'$.**.source_thread_id') k WHERE context_thread_is_fenced(k #>> '{}'))
            OR EXISTS(SELECT 1 FROM maintenance_execution_fences f WHERE (f.run_kind=NEW.kind AND f.idempotency_key=NEW.idempotency_key) OR f.chat_object_id=NEW.chat_object_id
                OR EXISTS(SELECT 1 FROM jsonb_array_elements(f.related_run_identities) i WHERE i->>'kind'=NEW.kind AND i->>'idempotency_key'=NEW.idempotency_key)
                OR (NEW.kind='slack_interaction' AND f.workspace_id=NEW.input->>'workspace_id' AND f.channel_id=NEW.input->>'channel_id' AND f.thread_id=NEW.input->>'thread_id')) THEN
            RAISE EXCEPTION 'execution identity is permanently fenced by reviewed maintenance';
        END IF;
    ELSIF TG_TABLE_NAME='chats' THEN
        IF TG_OP='UPDATE' AND to_regclass('pg_temp.context_reviewed_purge') IS NOT NULL THEN
            EXECUTE 'SELECT EXISTS(SELECT 1 FROM pg_temp.context_reviewed_purge WHERE table_name=''chats'' AND row_id=$1)' INTO allowed USING OLD.object_id;
            IF allowed AND NEW.curation_queued_through_message_id IS NULL AND NEW.curated_through_message_id IS NULL
                AND (to_jsonb(NEW)-ARRAY['curation_queued_through_message_id','curated_through_message_id'])=(to_jsonb(OLD)-ARRAY['curation_queued_through_message_id','curated_through_message_id']) THEN RETURN NEW; END IF;
        END IF;
        IF EXISTS(SELECT 1 FROM maintenance_execution_fences f WHERE f.chat_object_id=NEW.object_id OR (f.provider=NEW.provider AND f.workspace_id=NEW.workspace_id AND f.channel_id=NEW.channel_id AND f.thread_id=NEW.thread_id)) THEN RAISE EXCEPTION 'provider Chat is permanently fenced by reviewed maintenance'; END IF;
    ELSIF TG_TABLE_NAME='chat_messages' THEN
        IF EXISTS(SELECT 1 FROM maintenance_execution_fences WHERE chat_object_id=NEW.chat_object_id) THEN RAISE EXCEPTION 'provider Chat is permanently fenced by reviewed maintenance'; END IF;
    ELSIF TG_TABLE_NAME='object_events' THEN
        IF context_run_is_fenced(NEW.run_id) THEN RAISE EXCEPTION 'execution identity is permanently fenced by reviewed maintenance'; END IF;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER runs_execution_fence BEFORE INSERT OR UPDATE ON runs FOR EACH ROW EXECUTE FUNCTION guard_maintenance_execution_fence();
CREATE TRIGGER chats_execution_fence BEFORE INSERT OR UPDATE ON chats FOR EACH ROW EXECUTE FUNCTION guard_maintenance_execution_fence();
CREATE TRIGGER chat_messages_execution_fence BEFORE INSERT OR UPDATE ON chat_messages FOR EACH ROW EXECUTE FUNCTION guard_maintenance_execution_fence();
CREATE TRIGGER object_events_execution_fence BEFORE INSERT ON object_events FOR EACH ROW EXECUTE FUNCTION guard_maintenance_execution_fence();

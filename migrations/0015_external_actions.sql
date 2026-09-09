ALTER TABLE objects DROP CONSTRAINT objects_kind_check;
ALTER TABLE objects ADD CONSTRAINT objects_kind_check
    CHECK (kind IN (
        'task', 'chat', 'user', 'entity', 'memory', 'source', 'note', 'theme',
        'external_action'
    ));

CREATE TABLE external_actions (
    object_id uuid PRIMARY KEY,
    object_kind text NOT NULL DEFAULT 'external_action' CHECK (object_kind = 'external_action'),
    provider text NOT NULL CHECK (char_length(btrim(provider)) BETWEEN 1 AND 80),
    action_kind text NOT NULL CHECK (char_length(btrim(action_kind)) BETWEEN 1 AND 80),
    external_key text NOT NULL CHECK (char_length(btrim(external_key)) BETWEEN 1 AND 128),
    state text NOT NULL DEFAULT 'reserved' CHECK (state IN (
        'reserved', 'previewed', 'approved', 'attempting', 'accepted', 'delivered',
        'suppressed', 'failed', 'reconciliation_required'
    )),
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metadata) = 'object'),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (object_id, object_kind) REFERENCES objects(id, kind) ON DELETE RESTRICT,
    UNIQUE (provider, action_kind, external_key)
);

CREATE INDEX external_actions_state_idx
    ON external_actions (state, updated_at DESC, object_id);

CREATE CONSTRAINT TRIGGER external_actions_preserve_subtype
AFTER DELETE OR UPDATE OF object_id ON external_actions
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION prevent_canonical_subtype_removal();

ALTER TABLE object_events DROP CONSTRAINT object_events_entity_type_check;
ALTER TABLE object_events
    ADD CONSTRAINT object_events_entity_type_check
    CHECK (entity_type IN (
        'object', 'connection', 'task', 'source_content', 'chat_message',
        'curator_run', 'external_action'
    ));

ALTER TABLE object_events DROP CONSTRAINT object_events_action_check;
ALTER TABLE object_events
    ADD CONSTRAINT object_events_action_check
    CHECK (action IN (
        'created', 'updated', 'archived', 'connected', 'task_status_changed',
        'content_version_created', 'message_ingested', 'curator_queued', 'curator_started',
        'curator_committed', 'curator_failed', 'curator_undone', 'external_action_event'
    ));

INSERT INTO schema_visualizer_tables (table_name) VALUES ('external_actions');

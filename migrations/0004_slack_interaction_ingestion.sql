ALTER TABLE chats
    ADD COLUMN provider text,
    ADD COLUMN workspace_id text,
    ADD COLUMN channel_id text,
    ADD COLUMN thread_id text,
    ADD COLUMN surface_kind text,
    ADD COLUMN channel_name text,
    ADD COLUMN last_message_at timestamptz,
    ADD COLUMN last_ingested_message_id uuid,
    ADD COLUMN last_queued_message_id uuid,
    ADD COLUMN last_curated_message_id uuid;

ALTER TABLE chats
    ADD CONSTRAINT chats_provider_identity_check CHECK (
        (provider IS NULL AND workspace_id IS NULL AND channel_id IS NULL
            AND thread_id IS NULL AND surface_kind IS NULL)
        OR
        (char_length(btrim(provider)) BETWEEN 1 AND 100
            AND char_length(btrim(workspace_id)) BETWEEN 1 AND 300
            AND char_length(btrim(channel_id)) BETWEEN 1 AND 300
            AND char_length(btrim(thread_id)) BETWEEN 1 AND 300
            AND surface_kind IN ('channel', 'dm'))
    );

CREATE UNIQUE INDEX chats_provider_thread_unique_idx
    ON chats (provider, workspace_id, channel_id, thread_id)
    WHERE provider IS NOT NULL;
CREATE INDEX chats_inactivity_queue_idx
    ON chats (last_message_at, object_id)
    WHERE last_message_at IS NOT NULL;

CREATE TABLE chat_messages (
    id uuid PRIMARY KEY,
    chat_object_id uuid NOT NULL REFERENCES chats(object_id) ON DELETE RESTRICT,
    provider_message_id text NOT NULL
        CHECK (char_length(btrim(provider_message_id)) BETWEEN 1 AND 300),
    sender_user_object_id uuid NOT NULL REFERENCES users(object_id) ON DELETE RESTRICT,
    content text NOT NULL CHECK (char_length(btrim(content)) BETWEEN 1 AND 20000),
    source_created_at timestamptz NOT NULL,
    ingested_sequence bigint GENERATED ALWAYS AS IDENTITY UNIQUE,
    ingested_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (chat_object_id, provider_message_id)
);
CREATE INDEX chat_messages_order_idx
    ON chat_messages (chat_object_id, source_created_at, provider_message_id);
CREATE INDEX chat_messages_ingested_order_idx
    ON chat_messages (chat_object_id, ingested_sequence);

CREATE TABLE curator_runs (
    id uuid PRIMARY KEY,
    chat_object_id uuid NOT NULL REFERENCES chats(object_id) ON DELETE RESTRICT,
    first_message_id uuid NOT NULL REFERENCES chat_messages(id) ON DELETE RESTRICT,
    last_message_id uuid NOT NULL REFERENCES chat_messages(id) ON DELETE RESTRICT,
    trigger text NOT NULL CHECK (trigger IN ('explicit_finish', 'inactivity')),
    status text NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'running', 'completed', 'failed', 'reversed')),
    message_count integer NOT NULL CHECK (message_count > 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    started_at timestamptz,
    completed_at timestamptz,
    reversed_at timestamptz,
    error_message text,
    UNIQUE (chat_object_id, last_message_id),
    CHECK (status <> 'running' OR started_at IS NOT NULL),
    CHECK (status NOT IN ('completed', 'failed') OR completed_at IS NOT NULL),
    CHECK (status <> 'reversed' OR reversed_at IS NOT NULL)
);
CREATE INDEX curator_runs_queue_idx
    ON curator_runs (status, created_at, id);

ALTER TABLE chats
    ADD CONSTRAINT chats_last_ingested_message_fk
        FOREIGN KEY (last_ingested_message_id) REFERENCES chat_messages(id) ON DELETE RESTRICT,
    ADD CONSTRAINT chats_last_queued_message_fk
        FOREIGN KEY (last_queued_message_id) REFERENCES chat_messages(id) ON DELETE RESTRICT,
    ADD CONSTRAINT chats_last_curated_message_fk
        FOREIGN KEY (last_curated_message_id) REFERENCES chat_messages(id) ON DELETE RESTRICT;

ALTER TABLE object_events DROP CONSTRAINT object_events_entity_type_check;
ALTER TABLE object_events
    ADD CONSTRAINT object_events_entity_type_check
    CHECK (entity_type IN ('object', 'connection', 'task', 'source_content', 'chat_message', 'curator_run'));

ALTER TABLE object_events DROP CONSTRAINT object_events_action_check;
ALTER TABLE object_events
    ADD CONSTRAINT object_events_action_check
    CHECK (action IN (
        'created', 'updated', 'archived', 'connected', 'task_status_changed',
        'content_version_created', 'message_ingested', 'curator_queued'
    ));

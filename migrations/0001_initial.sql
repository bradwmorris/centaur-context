CREATE TABLE objects (
    id uuid PRIMARY KEY,
    kind text NOT NULL CHECK (kind IN ('note', 'source', 'decision', 'task')),
    title text NOT NULL CHECK (char_length(btrim(title)) BETWEEN 1 AND 300),
    body text NOT NULL DEFAULT '',
    lifecycle text NOT NULL DEFAULT 'active' CHECK (lifecycle IN ('active', 'archived')),
    revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_by_type text NOT NULL CHECK (created_by_type IN ('human', 'centaur_agent', 'system')),
    created_by_id text NOT NULL CHECK (char_length(btrim(created_by_id)) > 0),
    updated_by_type text NOT NULL CHECK (updated_by_type IN ('human', 'centaur_agent', 'system')),
    updated_by_id text NOT NULL CHECK (char_length(btrim(updated_by_id)) > 0),
    provenance jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(provenance) = 'object'),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    archived_at timestamptz,
    UNIQUE (id, kind),
    CHECK ((lifecycle = 'archived') = (archived_at IS NOT NULL))
);

CREATE INDEX objects_list_idx ON objects (kind, lifecycle, updated_at DESC, id);

CREATE TABLE connections (
    id uuid PRIMARY KEY,
    source_object_id uuid NOT NULL REFERENCES objects(id) ON DELETE RESTRICT,
    kind text NOT NULL CHECK (kind IN ('supports', 'depends_on', 'references', 'part_of', 'supersedes')),
    target_object_id uuid NOT NULL REFERENCES objects(id) ON DELETE RESTRICT,
    reason text NOT NULL CHECK (char_length(btrim(reason)) BETWEEN 1 AND 1000),
    revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
    created_by_type text NOT NULL CHECK (created_by_type IN ('human', 'centaur_agent', 'system')),
    created_by_id text NOT NULL CHECK (char_length(btrim(created_by_id)) > 0),
    updated_by_type text NOT NULL CHECK (updated_by_type IN ('human', 'centaur_agent', 'system')),
    updated_by_id text NOT NULL CHECK (char_length(btrim(updated_by_id)) > 0),
    provenance jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(provenance) = 'object'),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    archived_at timestamptz,
    CHECK (source_object_id <> target_object_id)
);

CREATE UNIQUE INDEX connections_active_unique_idx
    ON connections (source_object_id, kind, target_object_id)
    WHERE archived_at IS NULL;
CREATE INDEX connections_source_idx ON connections (source_object_id, updated_at DESC);
CREATE INDEX connections_target_idx ON connections (target_object_id, updated_at DESC);

CREATE TABLE tasks (
    object_id uuid PRIMARY KEY,
    object_kind text NOT NULL DEFAULT 'task' CHECK (object_kind = 'task'),
    status text NOT NULL DEFAULT 'todo' CHECK (status IN ('todo', 'doing', 'blocked', 'review', 'done')),
    owner_type text CHECK (owner_type IN ('human', 'centaur_agent')),
    owner_id text,
    agent_eligible boolean NOT NULL DEFAULT false,
    due_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (object_id, object_kind) REFERENCES objects(id, kind) ON DELETE RESTRICT,
    CHECK ((owner_type IS NULL) = (owner_id IS NULL)),
    CHECK (owner_id IS NULL OR char_length(btrim(owner_id)) > 0)
);

CREATE INDEX tasks_list_idx ON tasks (status, agent_eligible, updated_at DESC, object_id);

CREATE TABLE object_events (
    id uuid PRIMARY KEY,
    entity_type text NOT NULL CHECK (entity_type IN ('object', 'connection', 'task')),
    entity_id uuid NOT NULL,
    object_id uuid NOT NULL REFERENCES objects(id) ON DELETE RESTRICT,
    action text NOT NULL CHECK (action IN ('created', 'updated', 'archived', 'connected', 'task_status_changed')),
    actor_type text NOT NULL CHECK (actor_type IN ('human', 'centaur_agent', 'system')),
    actor_id text NOT NULL CHECK (char_length(btrim(actor_id)) > 0),
    centaur_thread_key text,
    centaur_execution_id text,
    idempotency_key text,
    from_revision bigint,
    to_revision bigint NOT NULL CHECK (to_revision > 0),
    changes jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(changes) = 'object'),
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK (actor_type <> 'centaur_agent' OR centaur_thread_key IS NOT NULL)
);

CREATE UNIQUE INDEX object_events_idempotency_idx
    ON object_events (actor_type, actor_id, idempotency_key)
    WHERE idempotency_key IS NOT NULL;
CREATE INDEX object_events_object_idx ON object_events (object_id, created_at DESC, id);

REVOKE CREATE ON SCHEMA public FROM PUBLIC;

ALTER TABLE objects DROP CONSTRAINT objects_kind_check;

ALTER TABLE objects RENAME COLUMN body TO description;
UPDATE objects SET description = title WHERE btrim(description) = '';
ALTER TABLE objects
    ADD CONSTRAINT objects_description_check
    CHECK (char_length(btrim(description)) > 0);
ALTER TABLE objects ADD COLUMN protected boolean NOT NULL DEFAULT false;

UPDATE objects
SET provenance = provenance || jsonb_build_object('migrated_from_kind', kind),
    kind = 'memory'
WHERE kind = 'decision';

INSERT INTO memories (object_id, object_kind, created_at, updated_at)
SELECT id, 'memory', created_at, updated_at
FROM objects
WHERE kind = 'memory'
ON CONFLICT (object_id) DO NOTHING;

ALTER TABLE objects
    ADD CONSTRAINT objects_kind_check
    CHECK (kind IN ('task', 'chat', 'user', 'entity', 'memory', 'source', 'note'));

ALTER TABLE memories
    ADD COLUMN happened_at timestamptz NOT NULL DEFAULT now();
UPDATE memories m
SET happened_at = o.updated_at
FROM objects o
WHERE o.id = m.object_id;

CREATE TABLE users (
    object_id uuid PRIMARY KEY,
    object_kind text NOT NULL DEFAULT 'user' CHECK (object_kind = 'user'),
    user_kind text NOT NULL CHECK (user_kind IN ('human', 'agent')),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (object_id, object_kind) REFERENCES objects(id, kind) ON DELETE RESTRICT
);

CREATE TABLE external_identities (
    id uuid PRIMARY KEY,
    user_object_id uuid NOT NULL REFERENCES users(object_id) ON DELETE RESTRICT,
    provider text NOT NULL CHECK (char_length(btrim(provider)) BETWEEN 1 AND 100),
    workspace_id text NOT NULL DEFAULT '',
    provider_user_id text NOT NULL CHECK (char_length(btrim(provider_user_id)) BETWEEN 1 AND 300),
    display_name text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (provider, workspace_id, provider_user_id)
);
CREATE INDEX external_identities_user_idx
    ON external_identities (user_object_id, provider);

ALTER TABLE connections DROP CONSTRAINT connections_kind_check;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM connections
        WHERE archived_at IS NULL
        GROUP BY source_object_id,
                 CASE kind
                     WHEN 'depends_on' THEN 'depends_on'
                     ELSE 'related_to'
                 END,
                 target_object_id
        HAVING count(*) > 1
    ) THEN
        RAISE EXCEPTION 'connection migration would collapse multiple active relationships; resolve them before migrating';
    END IF;
END
$$;

UPDATE connections
SET provenance = provenance || jsonb_build_object('migrated_from_kind', kind),
    kind = CASE kind
        WHEN 'depends_on' THEN 'depends_on'
        ELSE 'related_to'
    END;

ALTER TABLE connections RENAME COLUMN reason TO description;
ALTER TABLE connections ADD COLUMN protected boolean NOT NULL DEFAULT false;
ALTER TABLE connections
    ADD CONSTRAINT connections_kind_check
    CHECK (kind IN ('involves', 'about', 'related_to', 'depends_on', 'derived_from'));

ALTER TABLE tasks ADD COLUMN priority text NOT NULL DEFAULT 'medium';
ALTER TABLE tasks
    ADD CONSTRAINT tasks_priority_check
    CHECK (priority IN ('low', 'medium', 'high'));
ALTER TABLE tasks ADD COLUMN owner_object_id uuid REFERENCES users(object_id) ON DELETE RESTRICT;

CREATE TEMP TABLE owner_migration (
    owner_type text NOT NULL,
    owner_id text NOT NULL,
    object_id uuid NOT NULL,
    PRIMARY KEY (owner_type, owner_id)
) ON COMMIT DROP;

INSERT INTO owner_migration (owner_type, owner_id, object_id)
SELECT DISTINCT owner_type, owner_id, gen_random_uuid()
FROM tasks
WHERE owner_type IS NOT NULL AND owner_id IS NOT NULL;

INSERT INTO objects (
    id, kind, title, description, created_by_type, created_by_id,
    updated_by_type, updated_by_id, provenance
)
SELECT object_id,
       'user',
       owner_id,
       CASE owner_type
           WHEN 'human' THEN 'Human user migrated from an existing Task owner.'
           ELSE 'Agent user migrated from an existing Task owner.'
       END,
       'system',
       'schema-migration-3',
       'system',
       'schema-migration-3',
       jsonb_build_object(
           'source_type', 'schema_migration',
           'source_ref', '0003_canonical_graph_contract',
           'legacy_owner_type', owner_type,
           'legacy_owner_id', owner_id
       )
FROM owner_migration;

INSERT INTO users (object_id, user_kind)
SELECT object_id,
       CASE owner_type WHEN 'human' THEN 'human' ELSE 'agent' END
FROM owner_migration;

INSERT INTO external_identities (
    id, user_object_id, provider, workspace_id, provider_user_id, display_name
)
SELECT gen_random_uuid(),
       object_id,
       CASE owner_type
           WHEN 'human' THEN 'centaur_legacy_human'
           ELSE 'centaur_legacy_agent'
       END,
       '',
       owner_id,
       owner_id
FROM owner_migration;

UPDATE tasks t
SET owner_object_id = m.object_id
FROM owner_migration m
WHERE t.owner_type = m.owner_type AND t.owner_id = m.owner_id;

INSERT INTO object_events (
    id, entity_type, entity_id, object_id, action, actor_type, actor_id,
    idempotency_key, to_revision, changes
)
SELECT gen_random_uuid(),
       'object',
       object_id,
       object_id,
       'created',
       'system',
       'schema-migration-3',
       'migration-3-user-' || object_id::text,
       1,
       jsonb_build_object('kind', 'user', 'source', 'legacy_task_owner')
FROM owner_migration;

ALTER TABLE tasks DROP CONSTRAINT tasks_check;
ALTER TABLE tasks DROP COLUMN owner_type;
ALTER TABLE tasks DROP COLUMN owner_id;

CREATE OR REPLACE FUNCTION enforce_object_subtype()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.kind = 'task' AND NOT EXISTS (SELECT 1 FROM tasks WHERE object_id = NEW.id) THEN
        RAISE EXCEPTION 'task Object % requires a tasks subtype row', NEW.id;
    ELSIF NEW.kind = 'chat' AND NOT EXISTS (SELECT 1 FROM chats WHERE object_id = NEW.id) THEN
        RAISE EXCEPTION 'chat Object % requires a chats subtype row', NEW.id;
    ELSIF NEW.kind = 'user' AND NOT EXISTS (SELECT 1 FROM users WHERE object_id = NEW.id) THEN
        RAISE EXCEPTION 'user Object % requires a users subtype row', NEW.id;
    ELSIF NEW.kind = 'entity' AND NOT EXISTS (SELECT 1 FROM entities WHERE object_id = NEW.id) THEN
        RAISE EXCEPTION 'entity Object % requires an entities subtype row', NEW.id;
    ELSIF NEW.kind = 'memory' AND NOT EXISTS (SELECT 1 FROM memories WHERE object_id = NEW.id) THEN
        RAISE EXCEPTION 'memory Object % requires a memories subtype row', NEW.id;
    ELSIF NEW.kind = 'source' AND NOT EXISTS (SELECT 1 FROM sources WHERE object_id = NEW.id) THEN
        RAISE EXCEPTION 'source Object % requires a sources subtype row', NEW.id;
    ELSIF NEW.kind = 'note' AND NOT EXISTS (SELECT 1 FROM notes WHERE object_id = NEW.id) THEN
        RAISE EXCEPTION 'note Object % requires a notes subtype row', NEW.id;
    END IF;
    RETURN NEW;
END
$$;

CREATE CONSTRAINT TRIGGER objects_require_subtype
AFTER INSERT OR UPDATE OF kind ON objects
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION enforce_object_subtype();

CREATE OR REPLACE FUNCTION prevent_canonical_subtype_removal()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF EXISTS (SELECT 1 FROM objects WHERE id = OLD.object_id) THEN
        RAISE EXCEPTION 'cannot remove subtype for canonical Object %', OLD.object_id;
    END IF;
    RETURN OLD;
END
$$;

CREATE CONSTRAINT TRIGGER tasks_preserve_subtype
AFTER DELETE OR UPDATE OF object_id ON tasks
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION prevent_canonical_subtype_removal();
CREATE CONSTRAINT TRIGGER chats_preserve_subtype
AFTER DELETE OR UPDATE OF object_id ON chats
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION prevent_canonical_subtype_removal();
CREATE CONSTRAINT TRIGGER users_preserve_subtype
AFTER DELETE OR UPDATE OF object_id ON users
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION prevent_canonical_subtype_removal();
CREATE CONSTRAINT TRIGGER entities_preserve_subtype
AFTER DELETE OR UPDATE OF object_id ON entities
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION prevent_canonical_subtype_removal();
CREATE CONSTRAINT TRIGGER memories_preserve_subtype
AFTER DELETE OR UPDATE OF object_id ON memories
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION prevent_canonical_subtype_removal();
CREATE CONSTRAINT TRIGGER sources_preserve_subtype
AFTER DELETE OR UPDATE OF object_id ON sources
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION prevent_canonical_subtype_removal();
CREATE CONSTRAINT TRIGGER notes_preserve_subtype
AFTER DELETE OR UPDATE OF object_id ON notes
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION prevent_canonical_subtype_removal();

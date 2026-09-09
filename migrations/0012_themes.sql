ALTER TABLE objects DROP CONSTRAINT objects_kind_check;
ALTER TABLE objects
    ADD CONSTRAINT objects_kind_check
    CHECK (kind IN ('task', 'chat', 'user', 'entity', 'memory', 'source', 'note', 'theme'));

CREATE TABLE themes (
    object_id uuid PRIMARY KEY,
    object_kind text NOT NULL DEFAULT 'theme' CHECK (object_kind = 'theme'),
    slug text NOT NULL UNIQUE CHECK (
        char_length(slug) BETWEEN 1 AND 100
        AND slug ~ '^[a-z0-9]+(?:-[a-z0-9]+)*$'
    ),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (object_id, object_kind) REFERENCES objects(id, kind) ON DELETE RESTRICT
);

CREATE TABLE principal_permissions (
    principal_type text NOT NULL CHECK (principal_type IN ('human', 'centaur_agent')),
    principal_id text NOT NULL CHECK (char_length(btrim(principal_id)) BETWEEN 1 AND 300),
    permission text NOT NULL CHECK (permission = 'approve_themes'),
    granted_by text NOT NULL CHECK (char_length(btrim(granted_by)) BETWEEN 1 AND 300),
    granted_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (principal_type, principal_id, permission)
);

INSERT INTO principal_permissions (principal_type, principal_id, permission, granted_by)
VALUES ('human', 'local-human', 'approve_themes', 'schema-migration-12');

CREATE TABLE theme_proposals (
    id uuid PRIMARY KEY,
    title text NOT NULL CHECK (char_length(btrim(title)) BETWEEN 1 AND 300),
    slug text NOT NULL CHECK (
        char_length(slug) BETWEEN 1 AND 100
        AND slug ~ '^[a-z0-9]+(?:-[a-z0-9]+)*$'
    ),
    description text NOT NULL CHECK (char_length(btrim(description)) BETWEEN 1 AND 1000),
    rationale text NOT NULL CHECK (char_length(btrim(rationale)) BETWEEN 1 AND 2000),
    evidence jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(evidence) = 'object'),
    provenance jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(provenance) = 'object'),
    status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected')),
    proposed_by_type text NOT NULL CHECK (proposed_by_type = 'centaur_agent'),
    proposed_by_id text NOT NULL CHECK (char_length(btrim(proposed_by_id)) BETWEEN 1 AND 300),
    centaur_thread_key text NOT NULL CHECK (char_length(btrim(centaur_thread_key)) BETWEEN 1 AND 1000),
    centaur_execution_id text,
    idempotency_key text NOT NULL CHECK (char_length(btrim(idempotency_key)) BETWEEN 1 AND 300),
    decided_by_type text,
    decided_by_id text,
    decision_reason text CHECK (decision_reason IS NULL OR char_length(btrim(decision_reason)) BETWEEN 1 AND 1000),
    decided_at timestamptz,
    resulting_theme_object_id uuid REFERENCES themes(object_id) ON DELETE RESTRICT,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (
        (status = 'pending' AND decided_by_type IS NULL AND decided_by_id IS NULL
            AND decision_reason IS NULL AND decided_at IS NULL AND resulting_theme_object_id IS NULL)
        OR
        (status = 'approved' AND decided_by_type = 'human' AND decided_by_id IS NOT NULL
            AND decision_reason IS NOT NULL AND decided_at IS NOT NULL AND resulting_theme_object_id IS NOT NULL)
        OR
        (status = 'rejected' AND decided_by_type = 'human' AND decided_by_id IS NOT NULL
            AND decision_reason IS NOT NULL AND decided_at IS NOT NULL AND resulting_theme_object_id IS NULL)
    )
);

CREATE UNIQUE INDEX theme_proposals_idempotency_idx
    ON theme_proposals (proposed_by_type, proposed_by_id, idempotency_key);
CREATE INDEX theme_proposals_pending_idx
    ON theme_proposals (created_at, id)
    WHERE status = 'pending';

CREATE OR REPLACE FUNCTION preserve_theme_proposal_decision()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF OLD.status <> 'pending' THEN
        RAISE EXCEPTION 'decided Theme proposal % is immutable', OLD.id;
    END IF;
    RETURN NEW;
END
$$;

CREATE TRIGGER theme_proposals_preserve_decision
BEFORE UPDATE ON theme_proposals
FOR EACH ROW EXECUTE FUNCTION preserve_theme_proposal_decision();

CREATE OR REPLACE FUNCTION prevent_theme_proposal_delete()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'Theme proposals are an immutable audit record';
END
$$;

CREATE TRIGGER theme_proposals_prevent_delete
BEFORE DELETE ON theme_proposals
FOR EACH ROW EXECUTE FUNCTION prevent_theme_proposal_delete();

ALTER TABLE connections DROP CONSTRAINT connections_kind_check;
ALTER TABLE connections
    ADD CONSTRAINT connections_kind_check
    CHECK (kind IN ('involves', 'about', 'related_to', 'depends_on', 'derived_from', 'themed'));

CREATE OR REPLACE FUNCTION enforce_themed_connection()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    source_kind text;
    source_lifecycle text;
    target_kind text;
    target_lifecycle text;
BEGIN
    IF NEW.kind <> 'themed' THEN
        RETURN NEW;
    END IF;
    SELECT kind, lifecycle INTO source_kind, source_lifecycle
    FROM objects WHERE id = NEW.source_object_id;
    SELECT kind, lifecycle INTO target_kind, target_lifecycle
    FROM objects WHERE id = NEW.target_object_id;
    IF source_kind = 'theme' THEN
        RAISE EXCEPTION 'Theme Objects cannot be themed while Theme hierarchy is unsupported';
    END IF;
    IF source_lifecycle <> 'active' THEN
        RAISE EXCEPTION 'themed Connection source must be active';
    END IF;
    IF target_kind <> 'theme' OR target_lifecycle <> 'active' THEN
        RAISE EXCEPTION 'themed Connection target must be an active Theme';
    END IF;
    RETURN NEW;
END
$$;

CREATE TRIGGER connections_enforce_themed
BEFORE INSERT OR UPDATE OF source_object_id, kind, target_object_id ON connections
FOR EACH ROW EXECUTE FUNCTION enforce_themed_connection();

CREATE INDEX connections_active_themed_target_idx
    ON connections (target_object_id, source_object_id)
    WHERE kind = 'themed' AND archived_at IS NULL;

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
    ELSIF NEW.kind = 'theme' AND NOT EXISTS (SELECT 1 FROM themes WHERE object_id = NEW.id) THEN
        RAISE EXCEPTION 'theme Object % requires a themes subtype row', NEW.id;
    END IF;
    RETURN NEW;
END
$$;

CREATE CONSTRAINT TRIGGER themes_preserve_subtype
AFTER DELETE OR UPDATE OF object_id ON themes
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION prevent_canonical_subtype_removal();

INSERT INTO schema_visualizer_tables (table_name) VALUES
    ('themes'),
    ('theme_proposals'),
    ('principal_permissions');

COMMENT ON TABLE themes IS
    'One-to-one subtype for approved canonical research Theme Objects.';
COMMENT ON TABLE theme_proposals IS
    'Noncanonical agent proposals that become Theme Objects only after authorized human approval.';
COMMENT ON TABLE principal_permissions IS
    'Narrow local principal grants; only approve_themes is currently supported.';

-- Additive, transactional migration. Never re-infer historical classifications.
SET LOCAL lock_timeout = '10s';
SET LOCAL statement_timeout = '120s';
ALTER TABLE objects DROP CONSTRAINT objects_kind_check;
ALTER TABLE objects ADD CONSTRAINT objects_kind_check CHECK
    (kind IN ('task','chat','user','entity','memory','source','note','theme','event'));
CREATE TABLE real_world_events (
    object_id uuid PRIMARY KEY,
    object_kind text NOT NULL DEFAULT 'event' CHECK (object_kind='event'),
    starts_at text, starts_at_precision text,
    ends_at text, ends_at_precision text,
    timezone text,
    FOREIGN KEY (object_id,object_kind) REFERENCES objects(id,kind) ON DELETE RESTRICT,
    CHECK ((starts_at IS NULL) = (starts_at_precision IS NULL)),
    CHECK ((ends_at IS NULL) = (ends_at_precision IS NULL)),
    CHECK (starts_at_precision IN ('year','month','day','instant')),
    CHECK (ends_at_precision IN ('year','month','day','instant')),
    CHECK (starts_at_precision IS NULL OR ends_at_precision IS NULL OR starts_at_precision=ends_at_precision)
);
CREATE CONSTRAINT TRIGGER real_world_events_preserve_subtype
AFTER DELETE OR UPDATE OF object_id ON real_world_events DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION prevent_canonical_subtype_removal();
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
    ELSIF NEW.kind='event' AND NOT EXISTS (SELECT 1 FROM real_world_events WHERE object_id=NEW.id) THEN
        RAISE EXCEPTION 'event Object % requires a real_world_events subtype row',NEW.id;
    END IF;
    RETURN NEW;
END $$;

CREATE TABLE entity_categories (
    id uuid PRIMARY KEY,
    slug text NOT NULL UNIQUE CHECK (slug ~ '^[a-z][a-z0-9]*(-[a-z0-9]+)*$'),
    label text NOT NULL CHECK (char_length(btrim(label)) BETWEEN 1 AND 100),
    definition text NOT NULL CHECK (char_length(btrim(definition)) BETWEEN 1 AND 1000),
    aliases text[] NOT NULL DEFAULT '{}',
    legacy_kind text NOT NULL CHECK (legacy_kind IN ('person','organization','product','project','publication','place','concept','other')),
    revision bigint NOT NULL DEFAULT 1 CHECK (revision>0),
    archived_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(),
    created_by_type text NOT NULL, created_by_id text NOT NULL,
    updated_by_type text NOT NULL, updated_by_id text NOT NULL
);
-- One namespace for normalized labels and aliases, including archived definitions.
CREATE TABLE entity_category_names (
    name text PRIMARY KEY,
    category_id uuid NOT NULL REFERENCES entity_categories(id) ON DELETE RESTRICT
);
CREATE FUNCTION maintain_entity_category_names() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE n text;
BEGIN
    IF TG_OP='UPDATE' AND NEW.slug<>OLD.slug THEN
        RAISE EXCEPTION 'category slug is immutable' USING CONSTRAINT = 'entity_classification';
    END IF;
    DELETE FROM entity_category_names WHERE category_id=NEW.id;
    FOREACH n IN ARRAY array_prepend(NEW.label,NEW.aliases) LOOP
        n := lower(regexp_replace(btrim(n), '\s+', ' ', 'g'));
        IF n='' THEN RAISE EXCEPTION 'category names must not be empty' USING CONSTRAINT = 'entity_classification'; END IF;
        INSERT INTO entity_category_names(name,category_id) VALUES(n,NEW.id);
    END LOOP;
    RETURN NEW;
END $$;
CREATE TRIGGER entity_categories_names AFTER INSERT OR UPDATE ON entity_categories
FOR EACH ROW EXECUTE FUNCTION maintain_entity_category_names();
INSERT INTO entity_categories(id,slug,label,definition,legacy_kind,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES
('13f7a097-afbf-5a31-9955-c8df037c81b4','person','Person','A particular person; not their organization or work.','person','system','schema-38','system','schema-38'),
('4ada4721-8c22-5113-a67b-b2d586f8da66','organization','Organization','A named organization; not its products or members.','organization','system','schema-38','system','schema-38'),
('4362a584-073a-50cb-8f3b-6a28572b8225','product','Product','A named offering; not every model or concept is a product.','product','system','schema-38','system','schema-38'),
('12ff5921-5914-5a2d-94a1-d9188c1cd13c','project','Project','A particular coordinated endeavour; not a general topic.','project','system','schema-38','system','schema-38'),
('ec1a52ab-2951-53d4-8dff-461334ecd41a','publication','Publication','A named publication identity; captured documents remain Sources.','publication','system','schema-38','system','schema-38'),
('830d29e8-ed08-57aa-8d43-fb2c3d0b4ded','place','Place','A particular location; not an occurrence at that location.','place','system','schema-38','system','schema-38'),
('cfd45eec-9ac4-5138-b2b3-e66b0eb6d506','concept','Concept','An identifiable named concept; not every explanatory Note or Theme.','concept','system','schema-38','system','schema-38'),
('c23be2fd-5994-57c9-a718-2cab879ec78c','other','Other','An identifiable subject outside the defined categories.','other','system','schema-38','system','schema-38'),
('5b2cc51a-fa49-5489-a456-51c27c16da9f','model','Model','A named representation or estimator of a system; not its explanatory document.','concept','system','schema-38','system','schema-38'),
('94d3c10a-1137-53d4-ad9a-d9590e577889','benchmark','Benchmark','A named evaluation instrument; not the topic being evaluated.','concept','system','schema-38','system','schema-38'),
('61845718-7946-5b0a-8714-fafe4c681080','framework','Framework','A named structure for organizing reasoning; not every broad topic.','concept','system','schema-38','system','schema-38'),
('2b141124-4c80-514e-bdc9-9e72b896845f','method','Method','A named procedure; not one execution of that procedure.','concept','system','schema-38','system','schema-38'),
('123023f9-423b-52e7-85fa-336275c59826','teaching','Teaching','A named body of teaching; not an explanatory Note or publication.','concept','system','schema-38','system','schema-38'),
('a8ee4ead-eb13-5240-8b6a-b4c71228a2bf','fund','Fund','A particular pooled investment arrangement; not its provider, trustee or strategy document.','other','system','schema-38','system','schema-38');
ALTER TABLE entities ADD COLUMN primary_category_id uuid REFERENCES entity_categories(id) ON DELETE RESTRICT;
CREATE TABLE entity_category_assignments (
    entity_object_id uuid NOT NULL REFERENCES entities(object_id) ON DELETE RESTRICT,
    category_id uuid NOT NULL REFERENCES entity_categories(id) ON DELETE RESTRICT,
    PRIMARY KEY(entity_object_id,category_id)
);
CREATE INDEX entity_category_assignments_category_idx ON entity_category_assignments(category_id,entity_object_id);
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM entities e LEFT JOIN entity_categories c ON c.slug=e.entity_kind WHERE c.id IS NULL) THEN
        RAISE EXCEPTION 'unexpected legacy Entity classification; migration aborted' USING CONSTRAINT = 'entity_classification';
    END IF;
END $$;
UPDATE entities e SET primary_category_id=c.id FROM entity_categories c WHERE c.slug=e.entity_kind;
INSERT INTO entity_category_assignments SELECT object_id,primary_category_id FROM entities;
ALTER TABLE entities ALTER COLUMN primary_category_id SET NOT NULL;
-- Legacy writers share this compatibility boundary, including intake and networking.
CREATE FUNCTION entity_classification_compatibility() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE projected text; singleton boolean;
BEGIN
    IF TG_OP='INSERT' AND NEW.primary_category_id IS NULL THEN
        SELECT id INTO NEW.primary_category_id FROM entity_categories WHERE slug=NEW.entity_kind AND archived_at IS NULL;
    ELSIF TG_OP='UPDATE' AND NEW.entity_kind<>OLD.entity_kind AND NEW.primary_category_id=OLD.primary_category_id THEN
        SELECT count(*)=1 AND bool_and(c.slug=OLD.entity_kind) INTO singleton
        FROM entity_category_assignments a JOIN entity_categories c ON c.id=a.category_id WHERE a.entity_object_id=OLD.object_id;
        IF NOT singleton THEN
            RAISE EXCEPTION 'rich Entity classifications require category_ids and primary_category_id; legacy entity_kind cannot replace them' USING CONSTRAINT = 'entity_classification';
        END IF;
        SELECT id INTO NEW.primary_category_id FROM entity_categories WHERE slug=NEW.entity_kind AND archived_at IS NULL;
        DELETE FROM entity_category_assignments WHERE entity_object_id=OLD.object_id;
        INSERT INTO entity_category_assignments VALUES(OLD.object_id,NEW.primary_category_id);
    END IF;
    SELECT legacy_kind INTO projected FROM entity_categories WHERE id=NEW.primary_category_id;
    IF projected IS DISTINCT FROM NEW.entity_kind THEN
        RAISE EXCEPTION 'entity_kind must agree with primary category legacy_kind' USING CONSTRAINT = 'entity_classification';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER entities_classification_compatibility BEFORE INSERT OR UPDATE ON entities
FOR EACH ROW EXECUTE FUNCTION entity_classification_compatibility();
CREATE FUNCTION initialize_entity_classification() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO entity_category_assignments VALUES(NEW.object_id,NEW.primary_category_id) ON CONFLICT DO NOTHING;
    RETURN NEW;
END $$;
CREATE TRIGGER entities_initial_classification AFTER INSERT ON entities
FOR EACH ROW EXECUTE FUNCTION initialize_entity_classification();
CREATE FUNCTION enforce_entity_classification() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE entity_id uuid;
BEGIN
    IF TG_TABLE_NAME='entities' THEN entity_id:=NEW.object_id;
    ELSIF TG_OP='DELETE' THEN entity_id:=OLD.entity_object_id;
    ELSE entity_id:=NEW.entity_object_id;
    END IF;
    IF EXISTS (SELECT 1 FROM entities e JOIN entity_categories c ON c.id=e.primary_category_id
        WHERE e.object_id=entity_id AND (e.entity_kind<>c.legacy_kind OR NOT EXISTS
            (SELECT 1 FROM entity_category_assignments a WHERE a.entity_object_id=e.object_id AND a.category_id=e.primary_category_id))) THEN
        RAISE EXCEPTION 'primary category must be assigned and determine legacy entity_kind' USING CONSTRAINT = 'entity_classification';
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER entities_valid_classification AFTER INSERT OR UPDATE ON entities
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION enforce_entity_classification();
CREATE CONSTRAINT TRIGGER assignments_valid_classification AFTER INSERT OR UPDATE OR DELETE ON entity_category_assignments
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION enforce_entity_classification();
CREATE FUNCTION guard_category_assignment() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='UPDATE' AND NEW.entity_object_id<>OLD.entity_object_id THEN
        RAISE EXCEPTION 'move assignments with explicit delete and insert' USING CONSTRAINT = 'entity_classification';
    END IF;
    -- Lock catalogue row to serialize assignment against archive.
    PERFORM 1 FROM entity_categories WHERE id=NEW.category_id AND archived_at IS NULL FOR SHARE;
    IF NOT FOUND THEN RAISE EXCEPTION 'cannot assign an unknown or archived category' USING CONSTRAINT = 'entity_classification'; END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER assignments_active_category BEFORE INSERT OR UPDATE ON entity_category_assignments
FOR EACH ROW EXECUTE FUNCTION guard_category_assignment();
CREATE FUNCTION guard_category_archive() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.archived_at IS NOT NULL AND OLD.archived_at IS NULL AND EXISTS
        (SELECT 1 FROM entity_category_assignments a JOIN objects o ON o.id=a.entity_object_id
         WHERE a.category_id=NEW.id AND o.archived_at IS NULL) THEN
        RAISE EXCEPTION 'category is assigned to active Entities; reassign explicitly before archive' USING CONSTRAINT = 'entity_classification';
    END IF;
    IF NEW.legacy_kind<>OLD.legacy_kind AND EXISTS
        (SELECT 1 FROM entities WHERE primary_category_id=NEW.id) THEN
        RAISE EXCEPTION 'cannot change legacy projection while category is primary' USING CONSTRAINT = 'entity_classification';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER categories_safe_archive BEFORE UPDATE ON entity_categories
FOR EACH ROW EXECUTE FUNCTION guard_category_archive();
ALTER TABLE object_events DROP CONSTRAINT object_events_v17_target_type_check;
ALTER TABLE object_events ADD CONSTRAINT object_events_target_type_check CHECK(target_type IN ('object','connection','entity_category'));
COMMENT ON TABLE real_world_events IS 'Real-world occurrences, distinct from immutable mutation history in object_events.';
COMMENT ON COLUMN entities.entity_kind IS 'Compatibility projection of primary_category_id; not an independent classification.';
CREATE OR REPLACE VIEW schema_visualizer_tables(table_name) AS VALUES
    ('objects'),('connections'),('tasks'),('chats'),('chat_messages'),('users'),
    ('entities'),('memories'),('sources'),('notes'),('themes'),('artifacts'),
    ('runs'),('embeddings'),('object_events'),('real_world_events'),
    ('entity_categories'),('entity_category_names'),('entity_category_assignments');

CREATE EXTENSION IF NOT EXISTS vector;

ALTER TABLE objects
    ADD COLUMN search_document tsvector
    GENERATED ALWAYS AS (
        setweight(to_tsvector('english', coalesce(title, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(description, '')), 'B')
    ) STORED;

CREATE INDEX objects_search_document_idx
    ON objects USING gin (search_document);

CREATE TABLE object_embeddings (
    object_id uuid NOT NULL REFERENCES objects(id) ON DELETE CASCADE,
    model text NOT NULL CHECK (char_length(btrim(model)) BETWEEN 1 AND 300),
    dimensions integer NOT NULL CHECK (dimensions BETWEEN 1 AND 2000),
    source_hash text NOT NULL CHECK (char_length(source_hash) = 32),
    embedding vector NOT NULL,
    embedded_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (object_id, model),
    CHECK (vector_dims(embedding) = dimensions)
);

CREATE INDEX object_embeddings_object_idx
    ON object_embeddings (object_id);

CREATE TABLE object_embedding_jobs (
    object_id uuid PRIMARY KEY REFERENCES objects(id) ON DELETE CASCADE,
    source_hash text NOT NULL CHECK (char_length(source_hash) = 32),
    status text NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'running', 'failed')),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    available_at timestamptz NOT NULL DEFAULT now(),
    started_at timestamptz,
    last_error text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (status <> 'running' OR started_at IS NOT NULL)
);

CREATE INDEX object_embedding_jobs_claim_idx
    ON object_embedding_jobs (available_at, updated_at, object_id)
    WHERE status IN ('pending', 'failed');

CREATE OR REPLACE FUNCTION object_embedding_source_hash(
    object_kind text,
    object_title text,
    object_description text
)
RETURNS text
LANGUAGE sql
IMMUTABLE
PARALLEL SAFE
AS $$
    SELECT md5(object_kind || chr(10) || object_title || chr(10) || object_description)
$$;

CREATE OR REPLACE FUNCTION queue_object_embedding()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    desired_hash text;
BEGIN
    desired_hash := object_embedding_source_hash(NEW.kind, NEW.title, NEW.description);
    INSERT INTO object_embedding_jobs (object_id, source_hash)
    VALUES (NEW.id, desired_hash)
    ON CONFLICT (object_id) DO UPDATE
    SET source_hash = EXCLUDED.source_hash,
        status = 'pending',
        attempts = 0,
        available_at = now(),
        started_at = NULL,
        last_error = NULL,
        updated_at = now()
    WHERE object_embedding_jobs.source_hash IS DISTINCT FROM EXCLUDED.source_hash;
    RETURN NEW;
END
$$;

CREATE TRIGGER objects_queue_embedding
AFTER INSERT OR UPDATE OF kind, title, description ON objects
FOR EACH ROW EXECUTE FUNCTION queue_object_embedding();

INSERT INTO object_embedding_jobs (object_id, source_hash)
SELECT id, object_embedding_source_hash(kind, title, description)
FROM objects
ON CONFLICT (object_id) DO NOTHING;

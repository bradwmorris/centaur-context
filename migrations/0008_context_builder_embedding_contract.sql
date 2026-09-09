DROP INDEX objects_search_document_idx;

ALTER TABLE objects DROP COLUMN search_document;
ALTER TABLE objects
    ADD COLUMN search_document tsvector
    GENERATED ALWAYS AS (
        setweight(to_tsvector('simple', coalesce(title, '')), 'A') ||
        setweight(to_tsvector('simple', coalesce(description, '')), 'B')
    ) STORED;

CREATE INDEX objects_search_document_idx
    ON objects USING gin (search_document);

ALTER TABLE object_embeddings
    ADD COLUMN format_version text NOT NULL DEFAULT 'legacy-v0'
        CHECK (char_length(btrim(format_version)) BETWEEN 1 AND 100),
    ADD COLUMN input_mode text NOT NULL DEFAULT 'shared'
        CHECK (input_mode IN ('shared', 'search_document'));

ALTER TABLE object_embedding_jobs
    ADD COLUMN format_version text NOT NULL DEFAULT 'legacy-v0'
        CHECK (char_length(btrim(format_version)) BETWEEN 1 AND 100),
    ADD COLUMN input_mode text NOT NULL DEFAULT 'shared'
        CHECK (input_mode IN ('shared', 'search_document'));

CREATE OR REPLACE FUNCTION object_embedding_source_hash(
    embedding_format_version text,
    object_kind text,
    object_title text,
    object_description text
)
RETURNS text
LANGUAGE sql
IMMUTABLE
PARALLEL SAFE
AS $$
    SELECT md5(
        embedding_format_version || chr(10) ||
        'kind: ' || object_kind || chr(10) ||
        'title: ' || object_title || chr(10) ||
        'description: ' || object_description
    )
$$;

CREATE OR REPLACE FUNCTION queue_object_embedding()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    desired_format text := 'centaur-object-v1';
    desired_mode text := 'shared';
    desired_hash text;
BEGIN
    desired_hash := object_embedding_source_hash(
        desired_format, NEW.kind, NEW.title, NEW.description
    );
    INSERT INTO object_embedding_jobs
        (object_id, source_hash, format_version, input_mode)
    VALUES (NEW.id, desired_hash, desired_format, desired_mode)
    ON CONFLICT (object_id) DO UPDATE
    SET source_hash = EXCLUDED.source_hash,
        format_version = EXCLUDED.format_version,
        input_mode = EXCLUDED.input_mode,
        status = 'pending',
        attempts = 0,
        available_at = now(),
        started_at = NULL,
        last_error = NULL,
        updated_at = now()
    WHERE object_embedding_jobs.source_hash IS DISTINCT FROM EXCLUDED.source_hash
       OR object_embedding_jobs.format_version IS DISTINCT FROM EXCLUDED.format_version
       OR object_embedding_jobs.input_mode IS DISTINCT FROM EXCLUDED.input_mode;
    RETURN NEW;
END
$$;

DROP FUNCTION object_embedding_source_hash(text, text, text);

INSERT INTO object_embedding_jobs
    (object_id, source_hash, format_version, input_mode)
SELECT id,
       object_embedding_source_hash(
           'centaur-object-v1', kind, title, description
       ),
       'centaur-object-v1',
       'shared'
FROM objects
ON CONFLICT (object_id) DO UPDATE
SET source_hash = EXCLUDED.source_hash,
    format_version = EXCLUDED.format_version,
    input_mode = EXCLUDED.input_mode,
    status = 'pending',
    attempts = 0,
    available_at = now(),
    started_at = NULL,
    last_error = NULL,
    updated_at = now();

-- Make Artifact completeness explicit and let the consolidated embeddings table
-- carry both concise Object vectors and exact-offset Artifact chunk vectors.

ALTER TABLE artifacts
    ADD COLUMN capture_outcome text,
    ADD COLUMN capture_reason text,
    ADD COLUMN expected_size_bytes bigint,
    ADD COLUMN semantic_indexing_enabled boolean;

ALTER TABLE artifacts DISABLE TRIGGER artifacts_are_immutable;
UPDATE artifacts
SET capture_outcome = CASE
        WHEN content IS NOT NULL AND metadata->>'coverage' = 'complete' THEN 'complete'
        ELSE 'incomplete'
    END,
    capture_reason = CASE
        WHEN content IS NOT NULL AND metadata->>'coverage' = 'complete' THEN NULL
        ELSE 'legacy Artifact completeness was not established'
    END,
    semantic_indexing_enabled = false;
ALTER TABLE artifacts ENABLE TRIGGER artifacts_are_immutable;

ALTER TABLE artifacts
    ALTER COLUMN capture_outcome SET NOT NULL,
    ALTER COLUMN semantic_indexing_enabled SET DEFAULT true,
    ALTER COLUMN semantic_indexing_enabled SET NOT NULL,
    ADD CONSTRAINT artifacts_capture_outcome_check CHECK (capture_outcome IN (
        'complete','incomplete','unavailable','paywalled','disallowed','too_large','unsupported'
    )),
    ADD CONSTRAINT artifacts_capture_reason_check CHECK (
        (capture_outcome='complete' AND capture_reason IS NULL) OR
        (capture_outcome<>'complete' AND char_length(btrim(capture_reason)) BETWEEN 1 AND 1000)
    ),
    ADD CONSTRAINT artifacts_expected_size_check CHECK (
        expected_size_bytes IS NULL OR expected_size_bytes > 0
    ),
    ADD CONSTRAINT artifacts_complete_content_check CHECK (
        capture_outcome<>'complete' OR content IS NOT NULL
    );

-- Preserve legacy current pointers without overstating their completeness. The
-- trigger below governs every future current-Artifact assignment, while the
-- migration marks unproven historical captures incomplete and lexical-only.

CREATE OR REPLACE FUNCTION validate_current_artifact() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.current_artifact_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM artifacts a
        WHERE a.id=NEW.current_artifact_id
          AND a.object_id=NEW.object_id
          AND a.capture_outcome='complete'
          AND a.content IS NOT NULL
    ) THEN
        RAISE EXCEPTION 'a Source current Artifact must be a complete textual capture';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER sources_validate_current_artifact
BEFORE INSERT OR UPDATE OF current_artifact_id ON sources
FOR EACH ROW EXECUTE FUNCTION validate_current_artifact();

-- SHA-256 replaces the legacy 32-character MD5 source hash.
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
    SELECT encode(sha256(convert_to(
        embedding_format_version || chr(10) ||
        'kind: ' || object_kind || chr(10) ||
        'title: ' || object_title || chr(10) ||
        'description: ' || object_description,
        'UTF8'
    )), 'hex')
$$;

DROP INDEX embeddings_claim_idx;
ALTER TABLE embeddings DROP CONSTRAINT embeddings_pkey;
ALTER TABLE embeddings DROP CONSTRAINT embeddings_source_hash_check;
ALTER TABLE embeddings
    ADD COLUMN id uuid DEFAULT gen_random_uuid(),
    ADD COLUMN artifact_id uuid REFERENCES artifacts(id) ON DELETE CASCADE,
    ADD COLUMN chunk_index integer,
    ADD COLUMN start_offset integer,
    ADD COLUMN end_offset integer;
ALTER TABLE embeddings ADD CONSTRAINT embeddings_artifact_owner_fk
    FOREIGN KEY (object_id,artifact_id) REFERENCES artifacts(object_id,id) ON DELETE CASCADE;

UPDATE embeddings e
SET source_hash=object_embedding_source_hash(e.format_version,o.kind,o.title,o.description),
    status='pending',attempts=0,available_at=now(),started_at=NULL,completed_at=NULL,
    last_error=NULL,embedding=NULL,updated_at=now()
FROM objects o
WHERE o.id=e.object_id;

ALTER TABLE embeddings
    ALTER COLUMN id SET NOT NULL,
    ADD PRIMARY KEY (id),
    ADD CONSTRAINT embeddings_source_hash_check CHECK (source_hash ~ '^[0-9a-f]{64}$'),
    ADD CONSTRAINT embeddings_target_check CHECK (
        (artifact_id IS NULL AND chunk_index IS NULL AND start_offset IS NULL AND end_offset IS NULL)
        OR
        (artifact_id IS NOT NULL AND chunk_index >= 0 AND start_offset >= 0 AND end_offset > start_offset)
    );
CREATE UNIQUE INDEX embeddings_object_model_idx
    ON embeddings (object_id,model) WHERE artifact_id IS NULL;
CREATE UNIQUE INDEX embeddings_artifact_chunk_model_idx
    ON embeddings (artifact_id,model,chunk_index) WHERE artifact_id IS NOT NULL;
CREATE INDEX embeddings_claim_idx ON embeddings (available_at,updated_at,id)
    WHERE status IN ('pending','failed');
CREATE INDEX embeddings_artifact_idx ON embeddings (artifact_id,model,chunk_index)
    WHERE artifact_id IS NOT NULL;

CREATE OR REPLACE FUNCTION invalidate_object_embeddings() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE desired_hash text;
BEGIN
    desired_hash := object_embedding_source_hash('centaur-object-v1',NEW.kind,NEW.title,NEW.description);
    UPDATE embeddings SET source_hash=desired_hash,status='pending',attempts=0,
        available_at=now(),started_at=NULL,completed_at=NULL,last_error=NULL,
        embedding=NULL,updated_at=now()
    WHERE object_id=NEW.id AND artifact_id IS NULL
      AND source_hash IS DISTINCT FROM desired_hash;
    RETURN NEW;
END $$;

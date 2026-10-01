-- Only latest assertions contribute current derived text. Original evidence stays intact.
ALTER TABLE objects ADD COLUMN correction_text text NOT NULL DEFAULT '';
CREATE FUNCTION correction_embedding_description(description text, correction_text text)
RETURNS text LANGUAGE sql IMMUTABLE PARALLEL SAFE AS $$
SELECT description || CASE WHEN correction_text='' THEN '' ELSE E'\nCurrent correction assertions: ' || correction_text END
$$;
DROP INDEX objects_search_document_idx;
ALTER TABLE objects DROP COLUMN search_document;
ALTER TABLE objects ADD COLUMN search_document tsvector GENERATED ALWAYS AS (
    setweight(to_tsvector('simple',coalesce(title,'')),'A') ||
    setweight(to_tsvector('simple',coalesce(description,'')),'B') ||
    setweight(to_tsvector('simple',correction_text),'B')
) STORED;
CREATE INDEX objects_search_document_idx ON objects USING gin(search_document);
CREATE OR REPLACE FUNCTION invalidate_object_embeddings() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE desired_hash text;
BEGIN
    desired_hash := object_embedding_source_hash('centaur-object-v1',NEW.kind,NEW.title,correction_embedding_description(NEW.description,NEW.correction_text));
    UPDATE embeddings SET source_hash=desired_hash,status='pending',attempts=0,
        available_at=now(),started_at=NULL,completed_at=NULL,last_error=NULL,
        embedding=NULL,updated_at=now()
    WHERE object_id=NEW.id AND artifact_id IS NULL AND source_hash IS DISTINCT FROM desired_hash;
    RETURN NEW;
END $$;
DROP TRIGGER objects_invalidate_embeddings ON objects;
CREATE TRIGGER objects_invalidate_embeddings AFTER UPDATE OF kind,title,description,correction_text ON objects
FOR EACH ROW EXECUTE FUNCTION invalidate_object_embeddings();
CREATE FUNCTION refresh_correction_text() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    UPDATE objects SET correction_text=COALESCE((SELECT string_agg(representation::text,E'\n' ORDER BY target_type,target_id)
       FROM (SELECT DISTINCT ON(target_type,target_id) target_type,target_id,representation
             FROM evidence_corrections WHERE object_id=NEW.object_id ORDER BY target_type,target_id,object_revision DESC) latest
       WHERE representation<>'{}'::jsonb),'') WHERE id=NEW.object_id;
    RETURN NEW;
END $$;
CREATE TRIGGER evidence_correction_search AFTER INSERT ON evidence_corrections
FOR EACH ROW EXECUTE FUNCTION refresh_correction_text();

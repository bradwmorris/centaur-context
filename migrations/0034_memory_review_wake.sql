-- Notifications only wake the worker; committed records are the durable queue.
CREATE FUNCTION context_memory_review_notify() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.status = 'completed' AND NEW.kind NOT IN ('memory_dream','memory_undo')
     AND (TG_OP = 'INSERT' OR OLD.status IS DISTINCT FROM NEW.status) THEN
    PERFORM pg_notify('context_memory_review', '');
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER memory_review_run_completed AFTER INSERT OR UPDATE OF status ON runs
FOR EACH ROW EXECUTE FUNCTION context_memory_review_notify();

-- Human/API edits and source corrections do not all have a completed Run.
-- This trigger provides equivalent after-commit wake hints. Maintenance writes
-- are excluded; undo explicitly wakes so restored versions can be considered.
CREATE FUNCTION context_memory_review_change_notify() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.updated_by_id IS DISTINCT FROM 'context-memory-dream' THEN
    PERFORM pg_notify('context_memory_review', '');
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER memory_review_object_changed AFTER INSERT OR UPDATE ON objects
FOR EACH ROW EXECUTE FUNCTION context_memory_review_change_notify();
CREATE TRIGGER memory_review_connection_changed AFTER INSERT OR UPDATE ON connections
FOR EACH ROW EXECUTE FUNCTION context_memory_review_change_notify();

-- Include source revisions, graph changes and origin-chat message versions.
-- The digest contains no private payload. It invalidates a successful review
-- when supporting context changes without changing the Memory's own revision.
CREATE FUNCTION memory_review_fingerprint(memory_id uuid) RETURNS text LANGUAGE sql STABLE AS $$
 WITH direct AS (
   SELECT CASE WHEN source_object_id=memory_id THEN target_object_id ELSE source_object_id END AS id
   FROM connections WHERE archived_at IS NULL AND (source_object_id=memory_id OR target_object_id=memory_id)
 ), context_ids AS (
   SELECT memory_id AS id UNION SELECT id FROM direct
   UNION SELECT c.source_object_id FROM connections c JOIN direct d ON c.target_object_id=d.id WHERE c.archived_at IS NULL
   UNION SELECT c.target_object_id FROM connections c JOIN direct d ON c.source_object_id=d.id WHERE c.archived_at IS NULL
 )
 SELECT md5(jsonb_build_object(
   'objects',COALESCE((SELECT jsonb_agg(jsonb_build_array(o.id,o.revision) ORDER BY o.id) FROM objects o JOIN context_ids x ON x.id=o.id),'[]'::jsonb),
   'connections',COALESCE((SELECT jsonb_agg(jsonb_build_array(c.id,c.revision) ORDER BY c.id) FROM connections c WHERE c.source_object_id IN (SELECT id FROM direct UNION SELECT memory_id) OR c.target_object_id IN (SELECT id FROM direct UNION SELECT memory_id)),'[]'::jsonb),
   'messages',COALESCE((SELECT max(m.ingestion_sequence) FROM chat_messages m WHERE m.chat_object_id IN (SELECT id FROM context_ids)),0)
 )::text)
$$;

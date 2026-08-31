ALTER TABLE runs
    ADD COLUMN primary_object_id uuid REFERENCES objects(id) ON DELETE RESTRICT;

CREATE INDEX runs_primary_object_idx ON runs (primary_object_id,created_at DESC,id)
    WHERE primary_object_id IS NOT NULL;

UPDATE runs r
SET primary_object_id=(r.result->'object_ids'->>0)::uuid
WHERE r.primary_object_id IS NULL
  AND r.kind='intake'
  AND jsonb_typeof(r.result->'object_ids')='array'
  AND jsonb_array_length(r.result->'object_ids')=1
  AND (r.result->'object_ids'->>0) ~
      '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
  AND EXISTS (
      SELECT 1 FROM objects o WHERE o.id=(r.result->'object_ids'->>0)::uuid
  );

WITH origin_chats AS (
    SELECT DISTINCT ON (r.id) r.id run_id,c.source_object_id chat_object_id
    FROM runs r
    JOIN object_events e ON e.run_id=r.id AND e.target_type='connection'
    JOIN connections c ON c.id=e.target_id
    JOIN chats ch ON ch.object_id=c.source_object_id
    WHERE r.chat_object_id IS NULL
      AND r.primary_object_id IS NOT NULL
      AND c.kind='about'
      AND c.target_object_id=r.primary_object_id
      AND c.archived_at IS NULL
    ORDER BY r.id,e.sequence,c.id
)
UPDATE runs r
SET chat_object_id=origin_chats.chat_object_id
FROM origin_chats
WHERE r.id=origin_chats.run_id;

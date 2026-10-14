-- Additive capability only: activation is an explicit release step. Never sweep
-- or relabel existing Events/Memories as a migration side effect.
ALTER TABLE memories ALTER COLUMN happened_at DROP NOT NULL;

-- Only the Event and evidence it actually cites invalidate a completed decision.
-- Appended unrelated Chat messages and synthesized Memory writes are excluded.
CREATE FUNCTION event_memory_fingerprint(event_id uuid) RETURNS text LANGUAGE sql STABLE AS $$
 SELECT md5(jsonb_build_object(
   'event', (SELECT jsonb_build_array(o.id,o.revision,o.archived_at,o.protected) FROM objects o WHERE o.id=event_id),
   'evidence', COALESCE((
     SELECT jsonb_agg(jsonb_build_array(e.id,e.target_type,e.target_id,e.object_revision) ORDER BY e.id)
     FROM evidence_corrections e
     WHERE e.object_id=event_id OR e.target_id::text IN (
       SELECT jsonb_array_elements_text(COALESCE(o.provenance->'evidence_ids','[]'::jsonb)) FROM objects o WHERE o.id=event_id
     )
   ),'[]'::jsonb),
   'links', COALESCE((
     SELECT jsonb_agg(jsonb_build_array(c.id,c.revision,c.archived_at) ORDER BY c.id)
     FROM connections c WHERE c.source_object_id=event_id AND c.kind='derived_from'
   ),'[]'::jsonb)
 )::text)
$$;
CREATE INDEX runs_event_memory_policy_idx ON runs(kind,(result->>'policy_version'))
 WHERE kind IN ('event_memory_cutover','event_capture','memory_synthesis');

ALTER TABLE runs DROP CONSTRAINT runs_kind_status_check;
ALTER TABLE runs ADD CONSTRAINT runs_kind_status_check CHECK (
    (kind='external_action' AND status IN ('reserved','previewed','approved','attempting','reconciliation_required','accepted','delivered','suppressed','failed')) OR
    (kind IN ('memory_capture_start','memory_capture','memory_dream','memory_undo','event_memory_cutover','event_capture','memory_synthesis') AND status IN ('running','completed','preview','failed','reversed')) OR
    (kind='curator' AND status IN ('queued','running','completed','failed','reversed')) OR
    (kind='curator_undo' AND status IN ('running','completed','failed')) OR
    (kind='intake' AND status IN ('running','completed','failed')) OR
    (kind='workflow' AND status IN ('running','completed','failed')) OR
    (kind='codex_interaction' AND status IN ('open','running','completed','failed','reversed')) OR
    (kind='slack_interaction' AND status IN ('open','running','completed','failed','reversed','cancelled')) OR
    (kind IN ('human_mutation','system_mutation','mutation','legacy_import') AND status IN ('open','running','completed','failed','reversed'))
);

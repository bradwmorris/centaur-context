-- Represent complete durable workflows as parent Runs while retaining
-- fine-grained Context commits as child Runs.

ALTER TABLE runs DROP CONSTRAINT runs_check2;
ALTER TABLE runs ADD CONSTRAINT runs_kind_status_check CHECK (
    (kind='external_action' AND status IN (
        'reserved','previewed','approved','attempting','reconciliation_required',
        'accepted','delivered','suppressed','failed'
    )) OR
    (kind='curator' AND status IN ('queued','running','completed','failed','reversed')) OR
    (kind='curator_undo' AND status IN ('running','completed','failed')) OR
    (kind='intake' AND status IN ('running','completed','failed')) OR
    (kind='workflow' AND status IN ('running','completed','failed')) OR
    (kind='slack_interaction' AND status IN ('open','running','completed','failed','reversed')) OR
    (kind IN ('human_mutation','system_mutation','mutation','legacy_import')
        AND status IN ('open','running','completed','failed','reversed'))
);

-- Older Enyu Source-ingestion runs already carry their workflow UUID in the
-- immutable batch ID. Give those commits a lightweight parent so the UI can
-- explain their actual scope without inventing unavailable historical spans.
WITH candidates AS (
    SELECT r.*,
           substring(r.input->>'batch_id' from
             '^workflow:([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$')::uuid
             AS workflow_id
    FROM runs r
    WHERE r.kind='intake'
      AND r.input->>'batch_id' ~
        '^workflow:[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
), inserted AS (
    INSERT INTO runs (
        id,kind,status,actor_type,actor_id,chat_object_id,primary_object_id,
        idempotency_key,input,trace,result,started_at,completed_at,created_at,updated_at
    )
    SELECT workflow_id,'workflow','completed',actor_type,actor_id,chat_object_id,
           primary_object_id,'workflow:'||workflow_id::text,
           jsonb_build_object(
             'workflow_name','enyu_source_ingestion',
             'source_kind','unknown',
             'historical_trace_status','unavailable'
           ),
           jsonb_build_array(jsonb_build_object(
             'id',gen_random_uuid(),
             'entry_type','child_run',
             'name','Context commit',
             'status','completed',
             'started_at',started_at,
             'completed_at',completed_at,
             'duration_ms',0,
             'facts',jsonb_build_object(
               'summary','Committed the captured Source and its canonical connections.',
               'child_run_id',id,
               'historical_trace_status','unavailable'
             )
           )),
           result || jsonb_build_object(
             'summary','Completed Source ingestion; detailed historical workflow telemetry was not captured.',
             'child_run_id',id
           ),
           started_at,completed_at,created_at,updated_at
    FROM candidates
    WHERE workflow_id IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM runs existing WHERE existing.id=candidates.workflow_id)
    ON CONFLICT DO NOTHING
    RETURNING id
)
UPDATE runs child
SET parent_run_id=c.workflow_id,updated_at=GREATEST(child.updated_at,now())
FROM candidates c
JOIN runs parent ON parent.id=c.workflow_id AND parent.kind='workflow'
WHERE child.id=c.id AND child.parent_run_id IS NULL;


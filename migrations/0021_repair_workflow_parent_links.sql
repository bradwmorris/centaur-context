-- Migration 0020 initially created historical workflow parents and linked
-- children in one data-modifying CTE. PostgreSQL statement snapshots hid the
-- inserted parents from the sibling UPDATE. Repair databases that observed
-- that first form; this is a no-op for clean installs using corrected 0020.

WITH candidates AS (
    SELECT r.id,
           substring(r.input->>'batch_id' from
             '^workflow:([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$')::uuid
             AS workflow_id
    FROM runs r
    WHERE r.kind='intake'
      AND r.input->>'batch_id' ~
        '^workflow:[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
)
UPDATE runs child
SET parent_run_id=c.workflow_id,updated_at=GREATEST(child.updated_at,now())
FROM candidates c
JOIN runs parent ON parent.id=c.workflow_id AND parent.kind='workflow'
WHERE child.id=c.id AND child.parent_run_id IS NULL;

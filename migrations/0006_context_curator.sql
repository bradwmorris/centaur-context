ALTER TABLE curator_runs
    ADD COLUMN idempotency_key text,
    ADD COLUMN attempts integer NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    ADD COLUMN available_at timestamptz NOT NULL DEFAULT now(),
    ADD COLUMN lease_started_at timestamptz,
    ADD COLUMN worker_id text,
    ADD COLUMN model text,
    ADD COLUMN prompt_version text,
    ADD COLUMN proposed_plan jsonb,
    ADD COLUMN committed_plan jsonb,
    ADD COLUMN result jsonb;

UPDATE curator_runs
SET idempotency_key = 'curator-window:' || chat_object_id::text || ':' || last_message_id::text;

-- A pre-Phase-4 row cannot have a valid worker lease. Make any such stale
-- marker explicitly retryable before enforcing the lease invariant.
UPDATE curator_runs
SET status = 'failed',
    completed_at = COALESCE(completed_at, now()),
    error_message = COALESCE(error_message, 'requeued during Context Curator migration')
WHERE status = 'running';

ALTER TABLE curator_runs
    ALTER COLUMN idempotency_key SET NOT NULL,
    ADD CONSTRAINT curator_runs_idempotency_key_check
        CHECK (char_length(btrim(idempotency_key)) BETWEEN 1 AND 500),
    ADD CONSTRAINT curator_runs_worker_id_check
        CHECK (worker_id IS NULL OR char_length(btrim(worker_id)) BETWEEN 1 AND 300),
    ADD CONSTRAINT curator_runs_model_check
        CHECK (model IS NULL OR char_length(btrim(model)) BETWEEN 1 AND 300),
    ADD CONSTRAINT curator_runs_prompt_version_check
        CHECK (prompt_version IS NULL OR char_length(btrim(prompt_version)) BETWEEN 1 AND 300),
    ADD CONSTRAINT curator_runs_proposed_plan_check
        CHECK (proposed_plan IS NULL OR jsonb_typeof(proposed_plan) = 'object'),
    ADD CONSTRAINT curator_runs_committed_plan_check
        CHECK (committed_plan IS NULL OR jsonb_typeof(committed_plan) = 'object'),
    ADD CONSTRAINT curator_runs_result_check
        CHECK (result IS NULL OR jsonb_typeof(result) = 'object'),
    ADD CONSTRAINT curator_runs_lease_check
        CHECK ((status = 'running') = (lease_started_at IS NOT NULL AND worker_id IS NOT NULL));

CREATE UNIQUE INDEX curator_runs_idempotency_unique_idx
    ON curator_runs (idempotency_key);
CREATE INDEX curator_runs_claim_idx
    ON curator_runs (available_at, created_at, id)
    WHERE status IN ('queued', 'failed');

CREATE TABLE curator_run_changes (
    id uuid PRIMARY KEY,
    curator_run_id uuid NOT NULL REFERENCES curator_runs(id) ON DELETE RESTRICT,
    sequence integer NOT NULL CHECK (sequence > 0),
    entity_type text NOT NULL CHECK (entity_type IN ('object', 'connection')),
    entity_id uuid NOT NULL,
    action text NOT NULL CHECK (action IN ('created', 'updated')),
    before_state jsonb,
    after_state jsonb NOT NULL CHECK (jsonb_typeof(after_state) = 'object'),
    after_revision bigint NOT NULL CHECK (after_revision > 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    undone_at timestamptz,
    UNIQUE (curator_run_id, sequence),
    UNIQUE (curator_run_id, entity_type, entity_id),
    CHECK (before_state IS NULL OR jsonb_typeof(before_state) = 'object'),
    CHECK ((action = 'created') = (before_state IS NULL))
);
CREATE INDEX curator_run_changes_reverse_idx
    ON curator_run_changes (curator_run_id, sequence DESC);

ALTER TABLE object_events DROP CONSTRAINT object_events_action_check;
ALTER TABLE object_events
    ADD CONSTRAINT object_events_action_check
    CHECK (action IN (
        'created', 'updated', 'archived', 'connected', 'task_status_changed',
        'content_version_created', 'message_ingested', 'curator_queued', 'curator_started',
        'curator_committed', 'curator_failed', 'curator_undone'
    ));

-- Request-hash-bound replay records for the universal context_apply operation.
-- This table is additive and does not modify existing mutation keys.
CREATE TABLE context_apply_requests (
    principal_id text NOT NULL CHECK (char_length(btrim(principal_id)) BETWEEN 1 AND 300),
    idempotency_key text NOT NULL CHECK (char_length(btrim(idempotency_key)) BETWEEN 1 AND 300),
    request_hash text NOT NULL CHECK (char_length(request_hash)=64),
    contract_version text NOT NULL CHECK (char_length(btrim(contract_version)) BETWEEN 1 AND 50),
    run_id uuid REFERENCES runs(id) ON DELETE RESTRICT,
    response jsonb CHECK (response IS NULL OR jsonb_typeof(response)='object'),
    created_at timestamptz NOT NULL DEFAULT now(),
    completed_at timestamptz,
    PRIMARY KEY (principal_id,idempotency_key),
    CHECK ((run_id IS NULL)=(response IS NULL)),
    CHECK ((completed_at IS NULL)=(response IS NULL))
);

CREATE INDEX context_apply_requests_run_idx
    ON context_apply_requests (run_id) WHERE run_id IS NOT NULL;

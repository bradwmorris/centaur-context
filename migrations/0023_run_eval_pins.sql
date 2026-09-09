-- Keep golden eval examples on the existing Run rows they describe.
ALTER TABLE runs
    ADD COLUMN pinned boolean NOT NULL DEFAULT false;

CREATE INDEX runs_root_eval_order_idx
    ON runs (pinned DESC, created_at DESC, id DESC)
    WHERE parent_run_id IS NULL;

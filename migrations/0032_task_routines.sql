CREATE TABLE task_routines (
    task_id uuid PRIMARY KEY REFERENCES tasks(object_id),
    schedule jsonb NOT NULL,
    enabled boolean NOT NULL DEFAULT false,
    definition_revision bigint NOT NULL,
    next_run_at timestamptz,
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (enabled = (next_run_at IS NOT NULL))
);
CREATE TABLE task_routine_runs (
    id uuid PRIMARY KEY,
    task_id uuid NOT NULL REFERENCES task_routines(task_id),
    scheduled_for timestamptz NOT NULL,
    definition_revision bigint NOT NULL,
    status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','running','review','completed','blocked','skipped')),
    execution_thread text,
    execution_url text,
    result text,
    created_at timestamptz NOT NULL DEFAULT now(),
    completed_at timestamptz,
    UNIQUE(task_id,scheduled_for)
);
CREATE UNIQUE INDEX task_routine_one_active ON task_routine_runs(task_id) WHERE status IN ('pending','running');
CREATE INDEX task_routine_due ON task_routines(next_run_at) WHERE enabled;
-- Any edit invalidates approval of the old task definition. Configuration writes
-- increment the Object revision before re-approving the new exact revision.
CREATE FUNCTION pause_changed_task_routine() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    UPDATE task_routines SET enabled=false,next_run_at=NULL,updated_at=now()
    WHERE task_id=NEW.id AND enabled;
    RETURN NEW;
END $$;
CREATE TRIGGER task_routine_definition_changed AFTER UPDATE ON objects
FOR EACH ROW WHEN (NEW.kind='task' AND NEW.revision<>OLD.revision)
EXECUTE FUNCTION pause_changed_task_routine();

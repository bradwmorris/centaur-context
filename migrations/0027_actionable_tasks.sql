-- Existing tasks remain readable. New/claimed tasks must be actionable.
ALTER TABLE tasks ADD COLUMN work_kind text NOT NULL DEFAULT 'general'
  CHECK (work_kind IN ('general','code'));
ALTER TABLE tasks ADD COLUMN execution_actor_id text;

CREATE FUNCTION enforce_actionable_task() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE actor_kind text; actor_id text; require_ready boolean; creating boolean;
BEGIN
  creating := TG_OP = 'INSERT';
  SELECT updated_by_type, updated_by_id INTO actor_kind, actor_id
    FROM objects WHERE id = NEW.object_id;
  require_ready := creating OR (NEW.status = 'doing' AND OLD.status IS DISTINCT FROM 'doing');
  IF require_ready OR (NOT creating AND NEW.owner_object_id IS DISTINCT FROM OLD.owner_object_id) THEN
    IF NEW.owner_object_id IS NULL OR NOT EXISTS (
      SELECT 1 FROM users u JOIN objects o ON o.id=u.object_id
      WHERE u.object_id=NEW.owner_object_id AND o.archived_at IS NULL
    ) THEN
      RAISE EXCEPTION 'Task assigned user must be an active canonical User'
        USING ERRCODE='23514', CONSTRAINT='task_assigned_user';
    END IF;
  END IF;
  IF ((require_ready AND actor_kind <> 'human') OR
      (NOT creating AND OLD.due_at IS NOT NULL AND NEW.due_at IS NULL)) AND NEW.due_at IS NULL THEN
    RAISE EXCEPTION 'Task due_at is required for agent creation or execution and cannot be cleared'
      USING ERRCODE='23514', CONSTRAINT='task_due_at';
  END IF;
  IF (creating OR NEW.github_issue_url IS DISTINCT FROM OLD.github_issue_url OR
      NEW.work_kind IS DISTINCT FROM OLD.work_kind OR require_ready) AND
      ((NEW.work_kind='code' AND NEW.github_issue_url IS NULL) OR
       (NEW.github_issue_url IS NOT NULL AND NEW.github_issue_url !~ '^https://github[.]com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/issues/[1-9][0-9]*$')) THEN
    RAISE EXCEPTION 'Code Tasks require a canonical HTTPS GitHub issue URL'
      USING ERRCODE='23514', CONSTRAINT='task_github_issue';
  END IF;
  IF NOT creating AND OLD.status='doing' AND OLD.execution_actor_id IS NOT NULL AND
      OLD.execution_actor_id <> actor_id AND actor_kind <> 'human' THEN
    RAISE EXCEPTION 'Task is claimed by another actor; do not steal active work'
      USING ERRCODE='23514', CONSTRAINT='task_execution_claim';
  END IF;
  IF NEW.status='doing' AND (creating OR OLD.status <> 'doing') THEN
    IF NULLIF(btrim(NEW.brief_markdown),'') IS NULL THEN
      RAISE EXCEPTION 'Task brief must describe outcome, acceptance and next action before execution'
        USING ERRCODE='23514', CONSTRAINT='task_execution_brief';
    END IF;
    NEW.execution_actor_id := actor_id;
  ELSIF NEW.status <> 'doing' THEN
    NEW.execution_actor_id := NULL;
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER tasks_actionable BEFORE INSERT OR UPDATE ON tasks
  FOR EACH ROW EXECUTE FUNCTION enforce_actionable_task();
CREATE INDEX tasks_assigned_queue ON tasks(owner_object_id,status,priority,due_at,object_id);

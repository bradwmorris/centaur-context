export type ObjectKind = "note" | "source" | "decision" | "task" | "chat" | "entity" | "memory";
export type TaskStatus = "todo" | "doing" | "blocked" | "review" | "done";

export interface SharedObject {
  id: string;
  kind: ObjectKind;
  title: string;
  body: string;
  lifecycle: "active" | "archived";
  revision: number;
  created_by_type: string;
  created_by_id: string;
  updated_by_type: string;
  updated_by_id: string;
  provenance: Record<string, string>;
  created_at: string;
  updated_at: string;
  archived_at: string | null;
}

export interface Connection {
  id: string;
  source_object_id: string;
  kind: string;
  target_object_id: string;
  reason: string;
  revision: number;
  created_by_type: string;
  created_by_id: string;
  created_at: string;
}

export interface Task {
  id: string;
  title: string;
  body: string;
  lifecycle: "active" | "archived";
  revision: number;
  provenance: Record<string, string>;
  status: TaskStatus;
  owner_type: string | null;
  owner_id: string | null;
  agent_eligible: boolean;
  due_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface ObjectEvent {
  id: string;
  entity_type: string;
  entity_id: string;
  action: string;
  actor_type: string;
  actor_id: string;
  centaur_thread_key: string | null;
  centaur_execution_id: string | null;
  changes: Record<string, unknown>;
  created_at: string;
}

export type ObjectKind = "task" | "chat" | "user" | "entity" | "memory";
export type TaskStatus = "todo" | "doing" | "blocked" | "review" | "done";

export interface SharedObject {
  id: string;
  kind: ObjectKind;
  title: string;
  description: string;
  protected: boolean;
  lifecycle: "active" | "archived";
  revision: number;
  created_by_type: string;
  created_by_id: string;
  updated_by_type: string;
  updated_by_id: string;
  provenance: Record<string, unknown>;
  created_at: string;
  updated_at: string;
  archived_at: string | null;
}

export interface Connection {
  id: string;
  source_object_id: string;
  kind: string;
  target_object_id: string;
  description: string;
  protected: boolean;
  revision: number;
  created_by_type: string;
  created_by_id: string;
  updated_by_type: string;
  updated_by_id: string;
  provenance: Record<string, unknown>;
  created_at: string;
  updated_at: string;
  archived_at: string | null;
}

export interface Task {
  id: string;
  title: string;
  description: string;
  lifecycle: "active" | "archived";
  revision: number;
  provenance: Record<string, unknown>;
  protected: boolean;
  status: TaskStatus;
  priority: "low" | "medium" | "high";
  owner_object_id: string | null;
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

export interface ChatMessage {
  id: string;
  chat_object_id: string;
  provider_message_id: string;
  sender_user_object_id: string;
  sender_title: string;
  sender_kind: "human" | "agent";
  content: string;
  source_created_at: string;
  ingested_sequence: number;
  ingested_at: string;
}

export interface User {
  id: string;
  title: string;
  description: string;
  lifecycle: "active" | "archived";
  revision: number;
  provenance: Record<string, unknown>;
  user_kind: "human" | "agent";
  created_at: string;
  updated_at: string;
}

export interface ExternalIdentity {
  id: string;
  user_object_id: string;
  provider: string;
  workspace_id: string;
  provider_user_id: string;
  display_name: string | null;
  created_at: string;
  updated_at: string;
}

export interface CuratorRun {
  id: string;
  chat_object_id: string;
  first_message_id: string;
  last_message_id: string;
  trigger: "explicit_finish" | "inactivity";
  status: "queued" | "running" | "completed" | "failed" | "reversed";
  message_count: number;
  idempotency_key: string;
  attempts: number;
  worker_id: string | null;
  model: string | null;
  prompt_version: string | null;
  proposed_plan: Record<string, unknown> | null;
  committed_plan: Record<string, unknown> | null;
  result: Record<string, unknown> | null;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
  reversed_at: string | null;
  error_message: string | null;
}

export interface CuratorRunChange {
  id: string;
  sequence: number;
  entity_type: "object" | "connection";
  entity_id: string;
  action: "created" | "updated";
  before_state: Record<string, unknown> | null;
  after_state: Record<string, unknown>;
  after_revision: number;
  created_at: string;
  undone_at: string | null;
}

export interface CuratorRunDetail {
  run: CuratorRun;
  messages: ChatMessage[];
  changes: CuratorRunChange[];
}

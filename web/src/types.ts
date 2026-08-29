export type ObjectKind = "task" | "chat" | "user" | "entity" | "memory";
export type TaskStatus = "todo" | "doing" | "blocked" | "review" | "done";
export type SchemaClassification = "canonical" | "subtype" | "supporting";
export type SchemaViewMode = "map" | "structure" | "rows";

export interface SchemaColumn {
  name: string;
  ordinal: number;
  data_type: string;
  nullable: boolean;
  default: string | null;
  identity: boolean;
  generated: boolean;
}

export interface SchemaConstraint {
  name: string;
  kind: "primary_key" | "foreign_key" | "unique" | "check" | string;
  columns: string[];
  definition: string;
}

export interface SchemaTable {
  name: string;
  classification: SchemaClassification;
  estimated_row_count: number;
  columns: SchemaColumn[];
  constraints: SchemaConstraint[];
}

export interface SchemaForeignKey {
  name: string;
  source_table: string;
  source_columns: string[];
  target_table: string;
  target_columns: string[];
  one_to_one_subtype: boolean;
  nullable: boolean;
}

export interface SchemaSnapshot {
  fingerprint: string;
  tables: SchemaTable[];
  foreign_keys: SchemaForeignKey[];
}

export interface SchemaRowPage {
  schema_fingerprint: string;
  table: string;
  rows: Array<Record<string, string | null>>;
  next_cursor: string | null;
  page_size: number;
}

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
  object_id: string;
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
  object_id: string;
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
  object_id: string;
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
  avatar_url: string | null;
  created_at: string;
  updated_at: string;
}

export interface UserAttribution {
  object_id: string;
  user_object_id: string;
  title: string;
  user_kind: "human" | "agent";
  role: "identity" | "owner" | "participant" | "source author";
  avatar_url: string | null;
}

export interface ObjectVisual {
  object_id: string;
  source_provider: string | null;
  users: UserAttribution[];
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

export type EvalVerdict = "unreviewed" | "pass" | "mixed" | "fail";

export interface EvalUsageSource {
  component: string | null;
  provider: string | null;
  model_id: string | null;
  display_tier: string | null;
  execution_type: string | null;
  auth_mode: string | null;
  billing_mode: string | null;
  usage_status: string;
}

export interface EvalSummary {
  id: string;
  kind: "slack_interaction" | "human_mutation" | "system_mutation" | "legacy_import";
  status: "open" | "running" | "completed" | "failed" | "reversed";
  actor_type: string;
  actor_id: string;
  chat_object_id: string | null;
  curator_run_id: string | null;
  summary: string;
  error_summary: string | null;
  verdict: EvalVerdict;
  notes: string | null;
  annotated_by: string | null;
  annotation_revision: number;
  affected_object_count: number;
  total_tokens: number;
  estimated_micro_usd: number | null;
  chatgpt_credit_microunits: number | null;
  usage_sources: EvalUsageSource[];
  created_at: string;
  updated_at: string;
  completed_at: string | null;
}

export interface EvalTraceEntry {
  id: string;
  eval_id: string;
  sequence: number;
  entry_type: string;
  component: string | null;
  provider: string | null;
  model_id: string | null;
  display_tier: string | null;
  execution_type: string | null;
  auth_mode: string | null;
  upstream_service: string | null;
  billing_mode: string | null;
  reasoning_effort: string | null;
  service_tier: string | null;
  source_thread_id: string | null;
  source_execution_id: string | null;
  source_turn_id: string | null;
  usage_status: "reported" | "partial" | "unavailable" | "not_applicable";
  usage_missing_reason: string | null;
  input_tokens: number | null;
  output_tokens: number | null;
  cache_creation_tokens: number | null;
  cache_read_tokens: number | null;
  reasoning_tokens: number | null;
  total_tokens: number | null;
  estimated_micro_usd: number | null;
  chatgpt_credit_microunits: number | null;
  api_equivalent_micro_usd: number | null;
  rate_card_version: string | null;
  pricing_snapshot: Record<string, unknown> | null;
  facts: Record<string, unknown>;
  created_at: string;
}

export interface EvalObject {
  object_id: string;
  role: string;
  kind: ObjectKind;
  title: string;
  lifecycle: string;
}

export interface EvalDetail {
  eval: EvalSummary;
  trace: EvalTraceEntry[];
  objects: EvalObject[];
}

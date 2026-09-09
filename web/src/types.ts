export type ObjectKind = "task" | "chat" | "user" | "entity" | "memory" | "source" | "note" | "theme";
export type TaskStatus = "backlog" | "todo" | "doing" | "review" | "done" | "blocked";
export type SchemaClassification = "canonical" | "subtype" | "supporting";
export type SchemaViewMode = "map" | "rows";

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
  indexes: SchemaIndex[];
  triggers: SchemaTrigger[];
}

export interface SchemaIndex {
  name: string;
  unique: boolean;
  primary: boolean;
  constraint_backed: boolean;
  definition: string;
}

export interface SchemaTrigger {
  name: string;
  enabled: string;
  definition: string;
}

export interface SchemaColumnProfile {
  name: string;
  null_count: number;
  empty_count: number | null;
  distinct_count: number;
  default_count: number | null;
}

export interface SchemaProfile {
  schema_fingerprint: string;
  table: string;
  exact: boolean;
  row_count: number | null;
  columns: SchemaColumnProfile[];
  unavailable_reason: string | null;
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

export interface ConnectionGraphNode {
  id: string;
  kind: ObjectKind;
  title: string;
}

export interface ConnectionGraphEdge {
  id: string;
  source_object_id: string;
  target_object_id: string;
  kind: string;
  description: string;
}

export interface ConnectionGraphSnapshot {
  fingerprint: string;
  node_count: number;
  connection_count: number;
  nodes: ConnectionGraphNode[];
  edges: ConnectionGraphEdge[];
}

export interface Theme {
  object_id: string;
  title: string;
  description: string;
  slug: string;
  lifecycle: "active" | "archived";
  revision: number;
  provenance: Record<string, unknown>;
  protected: boolean;
  created_at: string;
  updated_at: string;
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
  agent_suitable: boolean;
  blocked_reason: string | null;
  due_at: string | null;
  completed_at: string | null;
  github_issue_url: string | null;
  brief_markdown: string | null;
  created_at: string;
  updated_at: string;
}

export type SourceKind = "article" | "paper" | "podcast_episode" | "video" | "book" | "report" | "document" | "dataset" | "web_page" | "social_post" | "other";

export interface Source {
  object_id: string;
  title: string;
  description: string;
  lifecycle: "active" | "archived";
  revision: number;
  provenance: Record<string, unknown>;
  protected: boolean;
  source_kind: SourceKind;
  canonical_uri: string | null;
  byline: string | null;
  publisher: string | null;
  published_at: string | null;
  published_at_precision: "instant" | "day" | "month" | "year" | null;
  last_accessed_at: string | null;
  original_language: string | null;
  original_media_type: string | null;
  original_artifact_reference: string | null;
  current_artifact_id: string | null;
  created_at: string;
  updated_at: string;
  excerpt?: string | null;
}

export interface SourcePage {
  items: Source[];
  next_cursor: string | null;
}

export interface Note {
  object_id: string;
  title: string;
  description: string;
  content: string;
  content_format: "plain_text" | "markdown";
  lifecycle: "active" | "archived";
  revision: number;
  provenance: Record<string, unknown>;
  protected: boolean;
  created_at: string;
  updated_at: string;
  excerpt?: string | null;
}

export interface NoteSummary {
  object_id: string;
  title: string;
  description: string;
  lifecycle: "active" | "archived";
  revision: number;
  content_format: "plain_text" | "markdown";
  excerpt: string;
  created_at: string;
  updated_at: string;
}

export interface NotePage {
  items: NoteSummary[];
  next_cursor: string | null;
}

export interface Artifact {
  id: string;
  object_id: string;
  kind: string;
  title: string | null;
  uri: string | null;
  media_type: string | null;
  language: string | null;
  sha256: string;
  size_bytes: number;
  capture_outcome: "complete" | "incomplete" | "unavailable" | "paywalled" | "disallowed" | "too_large" | "unsupported";
  capture_reason: string | null;
  expected_size_bytes: number | null;
  semantic_indexing_enabled: boolean;
  metadata: Record<string, unknown>;
  supersedes_artifact_id: string | null;
  captured_at: string | null;
  created_at: string;
}

export interface ArtifactWindow extends Artifact {
  text: string;
  offset: number;
  next_offset: number | null;
}

export interface EmbeddingStatus {
  configured: boolean;
  configuration: { model: string; dimensions: number; input_mode: string } | null;
  queue: {
    counts: Array<{ target: "object" | "artifact_chunk"; status: "pending" | "running" | "failed" | "terminal" | "completed"; count: number }>;
    oldest_available_at: string | null;
    oldest_age_seconds: number | null;
    coverage: {
      active_objects: number;
      current_complete_artifacts: number;
      artifact_embedding_eligible: number;
      completed_object_vectors: number;
      completed_artifact_chunks: number;
      indexed_current_artifacts: number;
      stale_rows: number;
    } | null;
  };
  fallback: "full_text";
}

export interface ObjectEvent {
  id: string;
  run_id: string;
  sequence: number;
  target_type: "object" | "connection";
  target_id: string;
  action: string;
  actor_type: string;
  actor_id: string;
  idempotency_key: string | null;
  from_revision: number | null;
  to_revision: number;
  before_state: Record<string, unknown> | null;
  after_state: Record<string, unknown>;
  reversible: boolean;
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
  ingestion_sequence: number;
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
  identities: ExternalIdentity[];
  created_at: string;
  updated_at: string;
}

export interface ExternalIdentity {
  id: string;
  provider: string;
  workspace_id: string;
  provider_user_id: string;
  display_name: string | null;
  avatar_url: string | null;
  avatar_asset_sha256: string | null;
  avatar_asset_filename: string | null;
  avatar_provenance: Record<string, unknown>;
  profile_refreshed_at: string | null;
}

export interface UserAttribution {
  object_id: string;
  user_object_id: string;
  title: string;
  user_kind: "human" | "agent";
  role: "identity" | "owner" | "participant" | "source author";
  avatar_url: string | null;
  avatar_asset_url: string | null;
}

export interface ObjectVisual {
  object_id: string;
  source_provider: string | null;
  users: UserAttribution[];
}

export type RunVerdict = "unreviewed" | "pass" | "mixed" | "fail";

export interface Run {
  id: string;
  parent_run_id: string | null;
  kind: string;
  status: string;
  actor_type: string;
  actor_id: string;
  chat_object_id: string | null;
  primary_object_id: string | null;
  idempotency_key: string;
  input: Record<string, unknown>;
  trace: Array<Record<string, unknown>>;
  result: Record<string, unknown>;
  consulted_object_ids: string[];
  error: string | null;
  verdict: RunVerdict;
  review_notes: string | null;
  pinned: boolean;
  reviewed_by: string | null;
  reviewed_at: string | null;
  available_at: string | null;
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface RunObject {
  object_id: string;
  role: string;
  kind: ObjectKind;
  title: string;
  lifecycle: string;
}

export interface RunDetail {
  run: Run;
  children: Run[];
  objects: RunObject[];
  events: ObjectEvent[];
}

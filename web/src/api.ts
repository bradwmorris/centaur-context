import type { ChatMessage, Connection, CuratorRun, CuratorRunDetail, EvalDetail, EvalSummary, EvalVerdict, ExternalIdentity, Note, NotePage, ObjectEvent, ObjectVisual, SchemaProfile, SchemaRowPage, SchemaSnapshot, SharedObject, Source, SourceContentVersion, SourceContentWindow, SourcePage, Task, Theme, ThemeProposal, User } from "./types";

interface Envelope<T> {
  data: T;
}

interface ErrorEnvelope {
  error?: { code?: string; message?: string };
}

export class ApiError extends Error {
  constructor(
    message: string,
    public readonly status: number,
    public readonly code?: string,
  ) {
    super(message);
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...init?.headers,
    },
  });
  const payload = (await response.json()) as Envelope<T> & ErrorEnvelope;
  if (!response.ok) {
    throw new ApiError(
      payload.error?.message ?? `Request failed (${response.status})`,
      response.status,
      payload.error?.code,
    );
  }
  return payload.data;
}

function write(method: "POST" | "PATCH", body: unknown): RequestInit {
  return {
    method,
    headers: { "Idempotency-Key": crypto.randomUUID() },
    body: JSON.stringify(body),
  };
}

export const api = {
  schema() {
    return request<SchemaSnapshot>("/api/v1/schema");
  },
  schemaRows(table: string, cursor?: string, focus?: { column: string; value: string }) {
    const params = new URLSearchParams({ limit: "50" });
    if (cursor) params.set("cursor", cursor);
    if (focus) {
      params.set("focus_column", focus.column);
      params.set("focus_value", focus.value);
    }
    return request<SchemaRowPage>(`/api/v1/schema/tables/${encodeURIComponent(table)}/rows?${params}`);
  },
  schemaProfile(table: string) {
    return request<SchemaProfile>(`/api/v1/schema/tables/${encodeURIComponent(table)}/profile`);
  },
  async objects(query = "", kind?: string) {
    const items: SharedObject[] = [];
    let cursor: string | undefined;
    do {
      const params = new URLSearchParams({ lifecycle: "active", limit: "500" });
      if (query.trim()) params.set("q", query.trim());
      if (kind) params.set("kind", kind);
      if (cursor) params.set("cursor", cursor);
      const page = await request<SharedObject[]>(`/api/v1/objects?${params}`);
      items.push(...page);
      cursor = kind && page.length === 500 ? page.at(-1)?.id : undefined;
    } while (cursor);
    return items;
  },
  object(id: string) {
    return request<SharedObject>(`/api/v1/objects/${id}`);
  },
  objectVisuals() {
    return request<ObjectVisual[]>("/api/v1/object-visuals");
  },
  createObject(body: {
    kind: string;
    title: string;
    description: string;
    provenance: Record<string, string>;
    entity_kind?: string;
    happened_at?: string;
  }) {
    return request<SharedObject>("/api/v1/objects", write("POST", body));
  },
  updateObject(id: string, body: Record<string, unknown>) {
    return request<SharedObject>(`/api/v1/objects/${id}`, write("PATCH", body));
  },
  connections(id: string) {
    return request<Connection[]>(`/api/v1/objects/${id}/connections`);
  },
  connection(id: string) {
    return request<Connection>(`/api/v1/connections/${id}`);
  },
  createConnection(body: Record<string, unknown>) {
    return request<Connection>("/api/v1/connections", write("POST", body));
  },
  updateConnection(id: string, body: Record<string, unknown>) {
    return request<Connection>(`/api/v1/connections/${id}`, write("PATCH", body));
  },
  events(id: string) {
    return request<ObjectEvent[]>(`/api/v1/objects/${id}/events`);
  },
  chatMessages(id: string) {
    return request<ChatMessage[]>(`/api/v1/chats/${id}/messages`);
  },
  users() {
    return request<User[]>("/api/v1/users");
  },
  user(id: string) {
    return request<User>(`/api/v1/users/${id}`);
  },
  userIdentities(id: string) {
    return request<ExternalIdentity[]>(`/api/v1/users/${id}/identities`);
  },
  curatorRuns() {
    return request<CuratorRun[]>("/api/v1/curator-runs");
  },
  curatorRun(id: string) {
    return request<CuratorRunDetail>(`/api/v1/curator-runs/${id}`);
  },
  undoCuratorRun(id: string) {
    return request<Record<string, unknown>>(`/api/v1/curator-runs/${id}/undo`, write("POST", {}));
  },
  evals(filters: Record<string, string> = {}) {
    const params = new URLSearchParams(filters);
    params.set("limit", "100");
    return request<EvalSummary[]>(`/api/v1/evals?${params}`);
  },
  eval(id: string) {
    return request<EvalDetail>(`/api/v1/evals/${id}`);
  },
  annotateEval(id: string, body: { verdict: EvalVerdict; notes: string | null; expected_revision: number }) {
    return request<EvalSummary>(`/api/v1/evals/${id}/annotation`, write("PATCH", body));
  },
  tasks() {
    return request<Task[]>("/api/v1/tasks");
  },
  task(id: string) {
    return request<Task>(`/api/v1/tasks/${id}`);
  },
  createTask(body: Record<string, unknown>) {
    return request<Task>("/api/v1/tasks", write("POST", body));
  },
  updateTask(id: string, body: Record<string, unknown>) {
    return request<Task>(`/api/v1/tasks/${id}`, write("PATCH", body));
  },
  sources(query = "") {
    const params = new URLSearchParams({ limit: "100" });
    if (query.trim()) params.set("q", query.trim());
    return request<SourcePage>(`/api/v1/sources?${params}`);
  },
  source(id: string) {
    return request<Source>(`/api/v1/sources/${id}`);
  },
  createSource(body: Record<string, unknown>) {
    return request<Source>("/api/v1/sources", write("POST", body));
  },
  updateSource(id: string, body: Record<string, unknown>) {
    return request<Source>(`/api/v1/sources/${id}`, write("PATCH", body));
  },
  sourceContents(id: string) {
    return request<SourceContentVersion[]>(`/api/v1/sources/${id}/contents`);
  },
  createSourceContent(id: string, body: Record<string, unknown>) {
    return request<SourceContentVersion>(`/api/v1/sources/${id}/contents`, write("POST", body));
  },
  sourceContent(id: string, version: number, offset = 0, limit = 8_000) {
    const params = new URLSearchParams({ version: String(version), offset: String(offset), limit: String(limit) });
    return request<SourceContentWindow>(`/api/v1/sources/${id}/content?${params}`);
  },
  notes(query = "") {
    const params = new URLSearchParams({ limit: "100" });
    if (query.trim()) params.set("q", query.trim());
    return request<NotePage>(`/api/v1/notes?${params}`);
  },
  note(id: string) {
    return request<Note>(`/api/v1/notes/${id}`);
  },
  updateNote(id: string, body: Record<string, unknown>) {
    return request<Note>(`/api/v1/notes/${id}`, write("PATCH", body));
  },
  createNote(body: Record<string, unknown>) {
    return request<Note>("/api/v1/notes", write("POST", body));
  },
  themes() {
    return request<Theme[]>("/api/v1/themes");
  },
  theme(id: string) {
    return request<Theme>(`/api/v1/themes/${id}`);
  },
  themeObjects(id: string) {
    return request<SharedObject[]>(`/api/v1/themes/${id}/objects?limit=100`);
  },
  createTheme(body: Record<string, unknown>) {
    return request<Theme>("/api/v1/themes", write("POST", body));
  },
  themeProposals(status = "pending") {
    return request<ThemeProposal[]>(`/api/v1/theme-proposals?status=${encodeURIComponent(status)}`);
  },
  approveThemeProposal(id: string, decisionReason: string) {
    return request<Theme>(`/api/v1/theme-proposals/${id}/approve`, write("POST", { decision_reason: decisionReason }));
  },
  rejectThemeProposal(id: string, decisionReason: string) {
    return request<ThemeProposal>(`/api/v1/theme-proposals/${id}/reject`, write("POST", { decision_reason: decisionReason }));
  },
};

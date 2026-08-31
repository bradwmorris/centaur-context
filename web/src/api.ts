import type { Artifact, ArtifactWindow, ChatMessage, Connection, ExternalIdentity, Note, NotePage, ObjectEvent, ObjectVisual, Run, RunDetail, RunVerdict, SchemaProfile, SchemaRowPage, SchemaSnapshot, SharedObject, Source, SourcePage, Task, Theme, User } from "./types";

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
    return request<SchemaSnapshot>("/api/v2/schema");
  },
  schemaRows(table: string, cursor?: string, focus?: { column: string; value: string }) {
    const params = new URLSearchParams({ limit: "50" });
    if (cursor) params.set("cursor", cursor);
    if (focus) {
      params.set("focus_column", focus.column);
      params.set("focus_value", focus.value);
    }
    return request<SchemaRowPage>(`/api/v2/schema/tables/${encodeURIComponent(table)}/rows?${params}`);
  },
  schemaProfile(table: string) {
    return request<SchemaProfile>(`/api/v2/schema/tables/${encodeURIComponent(table)}/profile`);
  },
  async objects(query = "", kind?: string) {
    const items: SharedObject[] = [];
    let cursor: string | undefined;
    do {
      const params = new URLSearchParams({ lifecycle: "active", limit: "500" });
      if (query.trim()) params.set("q", query.trim());
      if (kind) params.set("kind", kind);
      if (cursor) params.set("cursor", cursor);
      const page = await request<SharedObject[]>(`/api/v2/objects?${params}`);
      items.push(...page);
      cursor = kind && page.length === 500 ? page.at(-1)?.id : undefined;
    } while (cursor);
    return items;
  },
  object(id: string) {
    return request<SharedObject>(`/api/v2/objects/${id}`);
  },
  objectVisuals() {
    return request<ObjectVisual[]>("/api/v2/object-visuals");
  },
  createObject(body: {
    kind: string;
    title: string;
    description: string;
    provenance: Record<string, string>;
    entity_kind?: string;
    happened_at?: string;
  }) {
    return request<SharedObject>("/api/v2/objects", write("POST", body));
  },
  updateObject(id: string, body: Record<string, unknown>) {
    return request<SharedObject>(`/api/v2/objects/${id}`, write("PATCH", body));
  },
  connections(id: string) {
    return request<Connection[]>(`/api/v2/objects/${id}/connections`);
  },
  connection(id: string) {
    return request<Connection>(`/api/v2/connections/${id}`);
  },
  createConnection(body: Record<string, unknown>) {
    return request<Connection>("/api/v2/connections", write("POST", body));
  },
  updateConnection(id: string, body: Record<string, unknown>) {
    return request<Connection>(`/api/v2/connections/${id}`, write("PATCH", body));
  },
  events(id: string) {
    return request<ObjectEvent[]>(`/api/v2/objects/${id}/events`);
  },
  chatMessages(id: string) {
    return request<ChatMessage[]>(`/api/v2/chats/${id}/messages`);
  },
  users() {
    return request<User[]>("/api/v2/users");
  },
  user(id: string) {
    return request<User>(`/api/v2/users/${id}`);
  },
  userIdentities(id: string) {
    return request<ExternalIdentity[]>(`/api/v2/users/${id}/identities`);
  },
  runs(filters: Record<string, string> = {}) {
    const params = new URLSearchParams(filters);
    params.set("limit", "100");
    return request<Run[]>(`/api/v2/runs?${params}`);
  },
  run(id: string) {
    return request<RunDetail>(`/api/v2/runs/${id}`);
  },
  reviewRun(id: string, body: { verdict: RunVerdict; notes: string | null; expected_revision: number }) {
    return request<Run>(`/api/v2/runs/${id}/review`, write("PATCH", body));
  },
  undoRun(id: string) {
    return request<Record<string, unknown>>(`/api/v2/runs/${id}/undo`, write("POST", {}));
  },
  tasks() {
    return request<Task[]>("/api/v2/tasks");
  },
  task(id: string) {
    return request<Task>(`/api/v2/tasks/${id}`);
  },
  createTask(body: Record<string, unknown>) {
    return request<Task>("/api/v2/tasks", write("POST", body));
  },
  updateTask(id: string, body: Record<string, unknown>) {
    return request<Task>(`/api/v2/tasks/${id}`, write("PATCH", body));
  },
  sources(query = "") {
    const params = new URLSearchParams({ limit: "100" });
    if (query.trim()) params.set("q", query.trim());
    return request<SourcePage>(`/api/v2/sources?${params}`);
  },
  source(id: string) {
    return request<Source>(`/api/v2/sources/${id}`);
  },
  createSource(body: Record<string, unknown>) {
    return request<Source>("/api/v2/sources", write("POST", body));
  },
  updateSource(id: string, body: Record<string, unknown>) {
    return request<Source>(`/api/v2/sources/${id}`, write("PATCH", body));
  },
  artifacts(id: string) {
    return request<Artifact[]>(`/api/v2/objects/${id}/artifacts`);
  },
  createArtifact(id: string, body: Record<string, unknown>) {
    return request<Artifact>(`/api/v2/objects/${id}/artifacts`, write("POST", body));
  },
  artifactContent(artifactId: string, offset = 0, limit = 8_000) {
    const params = new URLSearchParams({ offset: String(offset), limit: String(limit) });
    return request<ArtifactWindow>(`/api/v2/artifacts/${artifactId}/content?${params}`);
  },
  notes(query = "") {
    const params = new URLSearchParams({ limit: "100" });
    if (query.trim()) params.set("q", query.trim());
    return request<NotePage>(`/api/v2/notes?${params}`);
  },
  note(id: string) {
    return request<Note>(`/api/v2/notes/${id}`);
  },
  updateNote(id: string, body: Record<string, unknown>) {
    return request<Note>(`/api/v2/notes/${id}`, write("PATCH", body));
  },
  createNote(body: Record<string, unknown>) {
    return request<Note>("/api/v2/notes", write("POST", body));
  },
  themes() {
    return request<Theme[]>("/api/v2/themes");
  },
  theme(id: string) {
    return request<Theme>(`/api/v2/themes/${id}`);
  },
  themeObjects(id: string) {
    return request<SharedObject[]>(`/api/v2/themes/${id}/objects?limit=100`);
  },
  createTheme(body: Record<string, unknown>) {
    return request<Theme>("/api/v2/themes", write("POST", body));
  },
};

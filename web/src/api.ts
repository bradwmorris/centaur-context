import type { Artifact, ArtifactWindow, ChatMessage, Connection, ConnectionGraphSnapshot, EmbeddingStatus, ExternalIdentity, Note, NotePage, ObjectEvent, ObjectVisual, Run, RunDetail, RunVerdict, SchemaProfile, SchemaRowPage, SchemaSnapshot, SharedObject, Source, SourcePage, Task, Theme, User } from "./types";

interface Envelope<T> {
  data: T;
}

interface ErrorEnvelope {
  error?: { code?: string; message?: string };
}

export type ListSort = "recent" | "connections";

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
  async objects(query = "", kind?: string, sort: ListSort = "recent") {
    const items: SharedObject[] = [];
    let cursor: string | undefined;
    do {
      const params = new URLSearchParams({ lifecycle: "active", limit: "500" });
      if (query.trim()) params.set("q", query.trim());
      if (kind) params.set("kind", kind);
      params.set("sort", sort);
      if (cursor) params.set("cursor", cursor);
      const page = await request<SharedObject[]>(`/api/v2/objects?${params}`);
      items.push(...page);
      cursor = page.length === 500 ? page.at(-1)?.id : undefined;
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
  connectionGraph() {
    return request<ConnectionGraphSnapshot>("/api/v2/connection-graph");
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
  async evalRuns() {
    const load = async (pinned: boolean) => {
      const items: Run[] = [];
      let before: Run | undefined;
      do {
        const params = new URLSearchParams({ root_only: "true", pinned: String(pinned), limit: "100" });
        if (before) {
          params.set("before", before.created_at);
          params.set("before_id", before.id);
        }
        const page = await request<Run[]>(`/api/v2/runs?${params}`);
        items.push(...page);
        before = page.length === 100 ? page.at(-1) : undefined;
      } while (before);
      return items;
    };
    const [pinned, other] = await Promise.all([load(true), load(false)]);
    return [...pinned, ...other];
  },
  run(id: string) {
    return request<RunDetail>(`/api/v2/runs/${id}`);
  },
  reviewRun(id: string, body: { verdict: RunVerdict; notes: string | null; pinned?: boolean; expected_revision: number }) {
    return request<Run>(`/api/v2/runs/${id}/review`, write("PATCH", body));
  },
  undoRun(id: string) {
    return request<Record<string, unknown>>(`/api/v2/runs/${id}/undo`, write("POST", {}));
  },
  async tasks(sort: ListSort = "recent") {
    const items: Task[] = [];
    let cursor: string | undefined;
    do {
      const params = new URLSearchParams({ sort, limit: "100" });
      if (cursor) params.set("cursor", cursor);
      const page = await request<Task[]>(`/api/v2/tasks?${params}`);
      items.push(...page);
      cursor = page.length === 100 ? page.at(-1)?.object_id : undefined;
    } while (cursor);
    return items;
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
  async sources(query = "", sort: ListSort = "recent") {
    const items: Source[] = [];
    let cursor: string | null = null;
    do {
      const params = new URLSearchParams({ limit: "100", sort });
      if (query.trim()) params.set("q", query.trim());
      if (cursor) params.set("cursor", cursor);
      const page = await request<SourcePage>(`/api/v2/sources?${params}`);
      items.push(...page.items); cursor = page.next_cursor;
    } while (cursor);
    return { items, next_cursor: null };
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
  embeddingStatus() {
    return request<EmbeddingStatus>("/api/v2/embeddings/status");
  },
  async notes(query = "", sort: ListSort = "recent") {
    const items: NotePage["items"] = [];
    let cursor: string | null = null;
    do {
      const params = new URLSearchParams({ limit: "100", sort });
      if (query.trim()) params.set("q", query.trim());
      if (cursor) params.set("cursor", cursor);
      const page = await request<NotePage>(`/api/v2/notes?${params}`);
      items.push(...page.items); cursor = page.next_cursor;
    } while (cursor);
    return { items, next_cursor: null };
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
  async themes(sort: ListSort = "recent") {
    const items: Theme[] = [];
    let cursor: string | undefined;
    do {
      const params = new URLSearchParams({ sort, limit: "500" });
      if (cursor) params.set("cursor", cursor);
      const page = await request<Theme[]>(`/api/v2/themes?${params}`);
      items.push(...page);
      cursor = page.length === 500 ? page.at(-1)?.object_id : undefined;
    } while (cursor);
    return items;
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

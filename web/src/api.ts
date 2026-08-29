import type { ChatMessage, Connection, CuratorRun, CuratorRunDetail, EvalDetail, EvalSummary, EvalVerdict, ExternalIdentity, ObjectEvent, ObjectVisual, SchemaRowPage, SchemaSnapshot, SharedObject, Task, User } from "./types";

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
  objects(query = "") {
    const params = new URLSearchParams({ lifecycle: "active" });
    if (query.trim()) params.set("q", query.trim());
    return request<SharedObject[]>(`/api/v1/objects?${params}`);
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
};

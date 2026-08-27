import type { Connection, ObjectEvent, SharedObject, Task } from "./types";

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
  objects(query = "") {
    const params = new URLSearchParams({ lifecycle: "active" });
    if (query.trim()) params.set("q", query.trim());
    return request<SharedObject[]>(`/api/v1/objects?${params}`);
  },
  object(id: string) {
    return request<SharedObject>(`/api/v1/objects/${id}`);
  },
  createObject(body: {
    kind: string;
    title: string;
    body: string;
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
  createConnection(body: Record<string, unknown>) {
    return request<Connection>("/api/v1/connections", write("POST", body));
  },
  events(id: string) {
    return request<ObjectEvent[]>(`/api/v1/objects/${id}/events`);
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

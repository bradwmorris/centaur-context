import { describe, expect, it, vi } from "vitest";
import { artifactPath, connectionPath, detailPath, navigate, objectPath, parseRoute, schemaPath, schemaRowPath, schemaView, sectionPath, workingDocumentPath } from "./routing";

describe("durable application routes", () => {
  it.each([
    ["/objects", "objects", null],
    ["/connections", "connections", null],
    ["/tasks/task-id", "tasks", "task-id"],
    ["/chats/chat-id", "chats", "chat-id"],
    ["/users/user-id", "users", "user-id"],
    ["/entities/entity-id", "entities", "entity-id"],
    ["/memories/memory-id", "memories", "memory-id"],
    ["/sources/source-id", "sources", "source-id"],
    ["/notes/note-id", "notes", "note-id"],
    ["/runs/run-id", "runs", "run-id"],
    ["/evals/run-id", "evals", "run-id"],
    ["/schema/objects/structure", "schema", "objects"],
  ])("parses %s", (path, section, selectedId) => {
    expect(parseRoute(path)).toEqual({ section, selectedId, connectionId: null, artifact: null });
  });

  it("distinguishes supporting Connection routes from canonical Object routes", () => {
    expect(parseRoute("/connections/connection-id")).toEqual({ section: "connections", selectedId: null, connectionId: "connection-id", artifact: null });
    expect(objectPath("object id")).toBe("/objects/object%20id");
    expect(connectionPath("connection id")).toBe("/connections/connection%20id");
    expect(sectionPath("runs")).toBe("/runs");
    expect(sectionPath("evals")).toBe("/evals");
    expect(detailPath("tasks", "task-id")).toBe("/tasks/task-id");
    expect(schemaPath()).toBe("/schema");
    expect(schemaPath("objects")).toBe("/schema/objects/rows");
    expect(schemaPath("objects", "rows")).toBe("/schema/objects/rows");
    expect(schemaRowPath("users", "object_id", "an id")).toBe("/schema/users/rows?focus_column=object_id&focus_value=an+id");
    expect(schemaView("/schema")).toBe("map");
    expect(schemaView("/schema/objects/rows")).toBe("rows");
    expect(schemaView("/schema/objects/structure")).toBe("rows");
    expect(detailPath("notes", "note-id")).toBe("/notes/note-id");
    expect(workingDocumentPath("sources", "source-id", "research notes")).toBe("/sources/source-id/working/research%20notes");
    expect(artifactPath("notes", "note-id", "artifact-id")).toBe("/notes/note-id/artifacts/artifact-id");
    expect(artifactPath("objects", "source-id", "artifact-id")).toBe("/objects/source-id/artifacts/artifact-id");
    expect(workingDocumentPath("objects", "note-id", "research notes")).toBe("/objects/note-id/working/research%20notes");
    expect(parseRoute("/sources/source-id/working/research%20notes").artifact).toEqual({ kind: "latest", id: "research notes" });
    expect(parseRoute("/notes/note-id/artifacts/artifact-id").artifact).toEqual({ kind: "exact", id: "artifact-id" });
    expect(parseRoute("/objects/source-id/working/research%20notes").artifact).toEqual({ kind: "latest", id: "research notes" });
    expect(parseRoute("/objects/note-id/artifacts/artifact-id").artifact).toEqual({ kind: "exact", id: "artifact-id" });
  });

  it("fails closed for unknown and malformed paths", () => {
    expect(parseRoute("/unknown/record-id")).toEqual({ section: "objects", selectedId: null, connectionId: null, artifact: null });
    expect(parseRoute("/objects/%E0%A4%A")).toEqual({ section: "objects", selectedId: "%E0%A4%A", connectionId: null, artifact: null });
  });

  it("updates browser history and notifies the application", () => {
    window.history.replaceState({}, "", "/objects");
    const listener = vi.fn();
    window.addEventListener("popstate", listener);
    navigate("/objects/object-id");
    expect(window.location.pathname).toBe("/objects/object-id");
    expect(listener).toHaveBeenCalledOnce();
    window.removeEventListener("popstate", listener);
  });
});

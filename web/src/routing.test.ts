import { describe, expect, it, vi } from "vitest";
import { connectionPath, detailPath, navigate, objectPath, parseRoute, sectionPath } from "./routing";

describe("durable application routes", () => {
  it.each([
    ["/objects", "objects", null],
    ["/tasks/task-id", "tasks", "task-id"],
    ["/chats/chat-id", "chats", "chat-id"],
    ["/users/user-id", "users", "user-id"],
    ["/entities/entity-id", "entities", "entity-id"],
    ["/memories/memory-id", "memories", "memory-id"],
    ["/curator-runs/run-id", "curator", "run-id"],
  ])("parses %s", (path, section, selectedId) => {
    expect(parseRoute(path)).toEqual({ section, selectedId, connectionId: null });
  });

  it("distinguishes supporting Connection routes from canonical Object routes", () => {
    expect(parseRoute("/connections/connection-id")).toEqual({ section: "objects", selectedId: null, connectionId: "connection-id" });
    expect(objectPath("object id")).toBe("/objects/object%20id");
    expect(connectionPath("connection id")).toBe("/connections/connection%20id");
    expect(sectionPath("curator")).toBe("/curator-runs");
    expect(detailPath("tasks", "task-id")).toBe("/tasks/task-id");
  });

  it("fails closed for unknown and malformed paths", () => {
    expect(parseRoute("/unknown/record-id")).toEqual({ section: "objects", selectedId: null, connectionId: null });
    expect(parseRoute("/objects/%E0%A4%A")).toEqual({ section: "objects", selectedId: "%E0%A4%A", connectionId: null });
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

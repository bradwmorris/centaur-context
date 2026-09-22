import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ContextModuleView, ModuleViewSwitcher, taskViewProps } from "./moduleRegistry";
import { api } from "../api";
import type { Task } from "../types";
vi.mock("../api", () => ({ api: { updateTask: vi.fn() } }));
const context = () => ({ tasks: [], visuals: new Map(), loading: false, onReload: vi.fn().mockResolvedValue(undefined), onTasksChange: vi.fn() });
describe("view host", () => {
  it("contains render failures and can return to the canonical list", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    window.history.replaceState({}, "", "/tasks?view=example%3Abroken");
    render(<ContextModuleView module={{ id: "example:broken", section: "tasks", label: "Broken", icon: "", render: () => { throw new Error("broken view"); } }} context={context()} />);
    expect(screen.getByRole("alert")).toHaveTextContent("This view could not be displayed");
    fireEvent.click(screen.getByRole("button", { name: "Return to list" }));
    expect(window.location.pathname + window.location.search).toBe("/tasks");
    spy.mockRestore();
  });
  it("explains unknown or removed views", () => {
    window.history.replaceState({}, "", "/tasks?view=example%3Aremoved");
    render(<ModuleViewSwitcher section="tasks" activeId={null} />);
    expect(screen.getByRole("status")).toHaveTextContent("Showing the list");
  });
  it("reports incomplete data after failure and never gives external views a setter", () => {
    const props = taskViewProps({ ...context(), error: "load failed" });
    expect(props.completeness).toBe("unknown");
    expect(props).not.toHaveProperty("onTasksChange");
    expect(props.error).toBe("load failed");
  });
  it("isolates view snapshots from canonical state even if a view bypasses readonly types", () => {
    const original = { object_id: "one", revision: 1, status: "todo", provenance: { source: { label: "original" } } } as unknown as Task;
    const ctx = { ...context(), tasks: [original] };
    const snapshot = taskViewProps(ctx);
    const changed = snapshot.tasks[0] as Task;
    changed.status = "done";
    (changed.provenance.source as unknown as { label: string }).label = "changed";
    expect(original.status).toBe("todo");
    expect(original.provenance.source).toEqual({ label: "original" });
  });
  it("rejects missing blocked reasons before any write and preserves newer local revisions", async () => {
    const ctx = context(); const props = taskViewProps(ctx);
    const task = { object_id: "one", revision: 3, status: "todo" } as Task;
    await expect(props.changeStatus(task, "blocked", " ")).rejects.toThrow("blocked reason");
    expect(api.updateTask).not.toHaveBeenCalled();
    const updated = { ...task, revision: 4, status: "done" as const, completed_at: "2026-09-22T00:00:00Z" };
    vi.mocked(api.updateTask).mockResolvedValue(updated);
    await props.changeStatus(task, "done");
    const reconcile = ctx.onTasksChange.mock.calls[0][0];
    expect(reconcile([task])).toEqual([updated]);
    const newer = { ...task, revision: 5 };
    expect(reconcile([newer])).toEqual([newer]);
  });
});

describe("Sources view host", () => {
  it("keeps Source snapshots isolated and navigates to canonical detail", async () => {
    const { sourceViewProps } = await import("./moduleRegistry");
    const original = { object_id: "source-one", title: "Original", provenance: { nested: { value: 1 } } } as unknown as import("../types").Source;
    const props = sourceViewProps({ ...context(), sources: [original] });
    (props.sources[0] as import("../types").Source).title = "Changed";
    expect(original.title).toBe("Original");
    expect(props.completeness).toBe("complete");
    expect(props).not.toHaveProperty("changeStatus");
    expect(props).not.toHaveProperty("onSourcesChange");
    props.openSource(original.object_id);
    expect(window.location.pathname).toContain("sources/source-one");
    expect(sourceViewProps({ ...context(), error: "failed" }).completeness).toBe("unknown");
  });
  it("recovers a broken Sources view to Sources, not Tasks", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    window.history.replaceState({}, "", "/sources?view=sources%3Abroken");
    render(<ContextModuleView module={{ id: "sources:broken", section: "sources", label: "Broken", icon: "", render: () => { throw new Error("broken"); } }} context={context()} />);
    fireEvent.click(screen.getByRole("button", { name: "Return to list" }));
    expect(window.location.pathname).toBe("/sources"); spy.mockRestore();
  });
  it("explains a removed Sources view even when no Source modules remain", () => {
    window.history.replaceState({}, "", "/sources?view=sources%3Aremoved");
    render(<ModuleViewSwitcher section="sources" activeId={null} />);
    expect(screen.getByRole("status")).toHaveTextContent("Showing the list");
    fireEvent.click(screen.getByRole("button", { name: "List" }));
    expect(window.location.pathname + window.location.search).toBe("/sources");
  });
});

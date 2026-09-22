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

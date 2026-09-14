import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ModuleViewSwitcher, resolveActiveModule } from "./moduleRegistry";

describe("Context UI module registry", () => {
  it("resolves only a registered module for its section", () => {
    expect(resolveActiveModule("tasks", "?view=kanban")?.label).toBe("Board");
    expect(resolveActiveModule("objects", "?view=kanban")).toBeNull();
    expect(resolveActiveModule("tasks", "?view=unknown")).toBeNull();
  });

  it("uses durable query routes for switching between list and board", () => {
    window.history.replaceState({}, "", "/tasks");
    render(<ModuleViewSwitcher section="tasks" activeId={null} />);
    fireEvent.click(screen.getByRole("button", { name: "Board" }));
    expect(`${window.location.pathname}${window.location.search}`).toBe("/tasks?view=kanban");
    fireEvent.click(screen.getByRole("button", { name: "List" }));
    expect(`${window.location.pathname}${window.location.search}`).toBe("/tasks");
  });
});

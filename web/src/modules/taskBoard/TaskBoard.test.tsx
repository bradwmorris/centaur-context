import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../../api";
import type { Task } from "../../types";
import { TaskBoard as Board } from "./TaskBoard";
import { taskViewProps, type ModuleContext } from "../moduleRegistry";
function TaskBoard(props: ModuleContext) { return <Board {...taskViewProps(props)} />; }

vi.mock("../../api", () => ({ api: { updateTask: vi.fn() } }));
const task: Task = { object_id: "task-1", title: "Polish the board", description: "Make it feel calm and fast.", lifecycle: "active", revision: 2, provenance: {}, protected: false, status: "todo", priority: "high", owner_object_id: null, agent_suitable: true, blocked_reason: null, due_at: null, completed_at: null, github_issue_url: "https://github.com/example/project/issues/1", brief_markdown: null, created_at: "2026-09-05T00:00:00Z", updated_at: "2026-09-05T00:00:00Z" };
const props = { visuals: new Map(), loading: false, onTasksChange: vi.fn(), onReload: vi.fn().mockResolvedValue(undefined) };

describe("TaskBoard", () => {
  beforeEach(() => vi.clearAllMocks());

  it("groups and filters tasks while hiding completed work by default", () => {
    render(<TaskBoard {...props} tasks={[task, { ...task, object_id: "task-2", title: "Finished", status: "done" }]} />);
    expect(screen.getByText("Polish the board")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Open GitHub issue example/project/issues/1" })).toHaveAttribute("href", task.github_issue_url);
    expect(screen.queryByText("Finished")).not.toBeInTheDocument();
    fireEvent.change(screen.getByRole("textbox", { name: "Search task board" }), { target: { value: "missing" } });
    expect(screen.queryByText("Polish the board")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Show completed (0)" }));
    expect(screen.getByRole("region", { name: "Done tasks" })).toBeInTheDocument();
  });

  it("renders a distinct loading state", () => {
    render(<TaskBoard {...props} tasks={[]} loading />);
    expect(screen.getByRole("status")).toHaveTextContent("Loading tasks…");
    expect(screen.queryByRole("region", { name: "Backlog tasks" })).not.toBeInTheDocument();
  });

  it("moves a task through the typed API and reconciles the confirmed response", async () => {
    vi.mocked(api.updateTask).mockResolvedValue({ ...task, status: "doing", revision: 3 });
    const onTasksChange = vi.fn();
    render(<TaskBoard {...props} tasks={[task]} onTasksChange={onTasksChange} />);
    fireEvent.change(screen.getByRole("combobox", { name: "Move Polish the board" }), { target: { value: "doing" } });
    await waitFor(() => expect(api.updateTask).toHaveBeenCalledWith("task-1", { expected_revision: 2, status: "doing" }));
    expect(onTasksChange).toHaveBeenCalledOnce();
  });

  it("supports pointer drag-and-drop with the same typed move", async () => {
    vi.mocked(api.updateTask).mockResolvedValue({ ...task, status: "doing", revision: 3 });
    render(<TaskBoard {...props} tasks={[task]} />);
    fireEvent.dragStart(screen.getByRole("article"), { dataTransfer: { effectAllowed: "", setData: vi.fn() } });
    fireEvent.drop(screen.getByRole("region", { name: "In progress tasks" }));
    await waitFor(() => expect(api.updateTask).toHaveBeenCalledWith("task-1", { expected_revision: 2, status: "doing" }));
  });

  it("collects a reason before moving a task into Blocked", async () => {
    vi.mocked(api.updateTask).mockResolvedValue({ ...task, status: "blocked", blocked_reason: "Waiting for review", revision: 3 });
    render(<TaskBoard {...props} tasks={[task]} />);
    fireEvent.change(screen.getByRole("combobox", { name: "Move Polish the board" }), { target: { value: "blocked" } });
    expect(api.updateTask).not.toHaveBeenCalled();
    fireEvent.change(screen.getByRole("textbox", { name: "Blocked reason" }), { target: { value: "  Waiting for review  " } });
    fireEvent.click(screen.getByRole("button", { name: "Block task" }));
    await waitFor(() => expect(api.updateTask).toHaveBeenCalledWith("task-1", { expected_revision: 2, status: "blocked", blocked_reason: "Waiting for review" }));
  });

  it("clears the reason when moving a task out of Blocked", async () => {
    const blocked = { ...task, status: "blocked" as const, blocked_reason: "Waiting" };
    vi.mocked(api.updateTask).mockResolvedValue({ ...blocked, status: "doing", blocked_reason: null, revision: 3 });
    render(<TaskBoard {...props} tasks={[blocked]} />);
    expect(screen.getByText("Blocked:").parentElement).toHaveTextContent("Blocked: Waiting");
    fireEvent.change(screen.getByRole("combobox", { name: "Move Polish the board" }), { target: { value: "doing" } });
    await waitFor(() => expect(api.updateTask).toHaveBeenCalledWith("task-1", { expected_revision: 2, status: "doing", clear_blocked_reason: true }));
  });

  it("keeps confirmed state and offers reload after a rejected move", async () => {
    vi.mocked(api.updateTask).mockRejectedValue(new Error("Task changed; reload and retry"));
    const onTasksChange = vi.fn();
    const onReload = vi.fn().mockResolvedValue(undefined);
    render(<TaskBoard {...props} tasks={[task]} onTasksChange={onTasksChange} onReload={onReload} />);
    fireEvent.change(screen.getByRole("combobox", { name: "Move Polish the board" }), { target: { value: "doing" } });
    expect(await screen.findByRole("alert")).toHaveTextContent("Task changed; reload and retry");
    expect(onTasksChange).not.toHaveBeenCalled();
    expect(screen.getByRole("combobox", { name: "Move Polish the board" })).toHaveValue("todo");
    fireEvent.click(screen.getByRole("button", { name: "Reload tasks" }));
    expect(onReload).toHaveBeenCalledOnce();
  });
});

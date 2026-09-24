import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { NewTask } from "./App";
import { api } from "./api";
import type { SharedObject, Task } from "./types";

vi.mock("./api", async (original) => ({ ...await original<typeof import("./api")>(), api: { objects: vi.fn(), createTask: vi.fn() } }));

it("loads assignees on a fresh Tasks page and saves the chosen task contract", async () => {
  vi.mocked(api.objects).mockResolvedValue([{ id: "user-1", title: "Ada", kind: "user", lifecycle: "active" }] as SharedObject[]);
  vi.mocked(api.createTask).mockResolvedValue({ object_id: "new-task" } as Task);
  const created = vi.fn();
  render(<NewTask onCancel={vi.fn()} onCreated={created} />);
  await screen.findByRole("option", { name: "Ada" });
  expect(api.objects).toHaveBeenCalledWith("", "user");
  fireEvent.change(screen.getByLabelText("Task title"), { target: { value: "Verify new task flow" } });
  fireEvent.change(screen.getByLabelText("Task description"), { target: { value: "Verify the task creation flow from a fresh page." } });
  fireEvent.change(screen.getByLabelText("Assigned to"), { target: { value: "user-1" } });
  fireEvent.change(screen.getByLabelText("Project"), { target: { value: "general" } });
  fireEvent.change(screen.getByLabelText("Due"), { target: { value: "2099-01-02T17:00" } });
  fireEvent.change(screen.getByLabelText("Work type"), { target: { value: "code" } });
  fireEvent.change(screen.getByLabelText("GitHub issue"), { target: { value: "https://github.com/example/project/issues/42" } });
  fireEvent.change(screen.getByLabelText("Execution brief"), { target: { value: "Run the browser check and save its evidence." } });
  fireEvent.click(screen.getByRole("button", { name: "Create task" }));
  await waitFor(() => expect(created).toHaveBeenCalledWith({ object_id: "new-task" }));
  expect(api.createTask).toHaveBeenCalledWith(expect.objectContaining({ brief_markdown: "Project: general\n\nRun the browser check and save its evidence.", owner_object_id: "user-1", work_kind: "code", github_issue_url: "https://github.com/example/project/issues/42", due_at: new Date("2099-01-02T17:00").toISOString() }));
});

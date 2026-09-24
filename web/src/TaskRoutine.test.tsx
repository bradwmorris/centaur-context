import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { TaskRoutine } from "./TaskRoutine";
import { api } from "./api";
import type { Task } from "./types";
vi.mock("./api", () => ({ api: { routine: vi.fn(), configureRoutine: vi.fn(), acceptRoutineRun: vi.fn() } }));
const task = { object_id: "task-1", revision: 7, due_at: "2099-01-01T00:00:00Z" } as Task;
beforeEach(() => { vi.clearAllMocks(); vi.mocked(api.routine).mockResolvedValue({routine:null,runs:[]}); vi.mocked(api.configureRoutine).mockResolvedValue({}); });
it("does not schedule from a due date and saves a new Routine paused", async () => {
  render(<TaskRoutine task={task} onChanged={vi.fn()} />);
  await waitFor(() => expect(api.routine).toHaveBeenCalledWith("task-1"));
  expect(api.configureRoutine).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:"Make a Routine"}));
  fireEvent.change(screen.getByLabelText("Timezone"),{target:{value:"Australia/Sydney"}});
  fireEvent.click(screen.getByRole("button",{name:"Save paused"}));
  await waitFor(() => expect(api.configureRoutine).toHaveBeenCalledWith("task-1",expect.objectContaining({expected_revision:7,enabled:false,confirmed:false,schedule:expect.objectContaining({timezone:"Australia/Sydney"})})));
});
it("enables only when the explicit enable control is used", async () => {
  vi.mocked(api.routine).mockResolvedValue({routine:{enabled:false,next_run_at:null,schedule:{timezone:"UTC",every_minutes:60,local_time:null,weekdays:[]}},runs:[]});
  render(<TaskRoutine task={task} onChanged={vi.fn()} />);
  fireEvent.click(await screen.findByRole("button",{name:"Enable Routine"}));
  await waitFor(() => expect(api.configureRoutine).toHaveBeenCalledWith("task-1",expect.objectContaining({enabled:true,confirmed:true})));
});

it("refreshes occurrence history when the task revision is unchanged", async () => {
  const props = {task,onChanged:vi.fn()};
  const view=render(<TaskRoutine {...props} refreshKey={0} />);
  await waitFor(() => expect(api.routine).toHaveBeenCalledTimes(1));
  vi.mocked(api.routine).mockResolvedValue({routine:null,runs:[{id:"run",scheduled_for:"2026-09-25T00:00:00Z",status:"completed",execution_url:null,result:"Verified occurrence result"}]});
  view.rerender(<TaskRoutine {...props} refreshKey={1} />);
  expect(await screen.findByText("Verified occurrence result")).toBeInTheDocument();
});

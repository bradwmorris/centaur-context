/** Version 1 of the trusted, compile-time Task view interface. */
import type { ObjectVisual, Task, TaskStatus } from "../types";
export type { ObjectVisual, Task, TaskStatus } from "../types";
export { TaskAssignee, TaskIssueLink, TaskReadiness } from "../TaskIdentity";
export { objectPath, detailPath, navigate } from "../routing";

export interface TaskViewProps {
  tasks: readonly Readonly<Task>[];
  visuals: ReadonlyMap<string, ObjectVisual>;
  loading: boolean;
  error: string | null;
  /** All pages from /api/v2/tasks have loaded. This is not a database snapshot. */
  completeness: "loading" | "complete" | "unknown";
  reload(): Promise<void>;
  openTask(id: string): void;
  changeStatus(task: Readonly<Task>, status: TaskStatus, blockedReason?: string): Promise<Task>;
}

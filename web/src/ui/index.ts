/** Version 1 of the trusted, compile-time collection view interfaces. */
import type { ObjectVisual, Source, Task, TaskStatus } from "../types";
export type { ObjectVisual, Source, SourceKind, Task, TaskStatus } from "../types";
export { TaskAssignee, TaskIssueLink, TaskReadiness, TaskProject } from "../TaskIdentity";
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

/** Sources collection slot. Query/sort remain host-owned and shared with List. */
export interface SourceViewProps {
  sources: readonly Readonly<Source>[];
  visuals: ReadonlyMap<string, ObjectVisual>;
  loading: boolean;
  error: string | null;
  completeness: "loading" | "complete" | "unknown";
  reload(): Promise<void>;
  openSource(id: string): void;
}

/** Optional same-origin preview endpoint; disabled hosts return a fallback-worthy 404. */
export function sourceThumbnailUrl(source: Readonly<Source>, refreshKey = 0): string {
  return `/api/v2/sources/${encodeURIComponent(source.object_id)}/thumbnail?revision=${source.revision}&refresh=${refreshKey > 0}&request=${refreshKey}`;
}

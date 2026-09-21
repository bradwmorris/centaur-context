import { FormEvent, useDeferredValue, useMemo, useState } from "react";
import { api } from "../../api";
import { detailPath, navigate } from "../../routing";
import type { Task, TaskStatus, ObjectVisual } from "../../types";
import type { ModuleContext } from "../moduleRegistry";
import "./taskBoard.css";
import { TaskAssignee, TaskIssueLink, TaskReadiness } from "../../TaskIdentity";

const columns: Array<{ status: TaskStatus; label: string; icon: string }> = [
  { status: "backlog", label: "Backlog", icon: "◌" },
  { status: "todo", label: "To do", icon: "○" },
  { status: "doing", label: "In progress", icon: "◐" },
  { status: "review", label: "Review", icon: "◇" },
  { status: "blocked", label: "Blocked", icon: "!" },
  { status: "done", label: "Done", icon: "✓" },
];

const dueDateFormatter = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });

export function TaskBoard({ tasks, visuals, loading, onTasksChange, onReload }: ModuleContext) {
  const [query, setQuery] = useState("");
  const [showDone, setShowDone] = useState(false);
  const [movingId, setMovingId] = useState<string | null>(null);
  const [draggingId, setDraggingId] = useState<string | null>(null);
  const [blockingTask, setBlockingTask] = useState<Task | null>(null);
  const [blockedReason, setBlockedReason] = useState("");
  const [error, setError] = useState<string | null>(null);
  const deferredQuery = useDeferredValue(query.trim().toLocaleLowerCase());
  const grouped = useMemo(() => {
    const result = new Map<TaskStatus, Task[]>(columns.map(({ status }) => [status, []]));
    for (const task of tasks) {
      if (deferredQuery && !`${task.title} ${task.description}`.toLocaleLowerCase().includes(deferredQuery)) continue;
      result.get(task.status)?.push(task);
    }
    return result;
  }, [tasks, deferredQuery]);
  const visibleColumns = showDone ? columns : columns.filter(({ status }) => status !== "done");
  const completedCount = grouped.get("done")?.length ?? 0;

  const moveTask = async (task: Task, status: TaskStatus, reason?: string) => {
    if (task.status === status || movingId) return false;
    setMovingId(task.object_id);
    setError(null);
    try {
      const changes: Record<string, unknown> = { expected_revision: task.revision, status };
      if (status === "blocked") changes.blocked_reason = reason;
      if (task.status === "blocked" && status !== "blocked") changes.clear_blocked_reason = true;
      const updated = await api.updateTask(task.object_id, changes);
      onTasksChange((current) => current.map((item) => item.object_id === updated.object_id ? updated : item));
      return true;
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Task could not be moved.");
      return false;
    } finally {
      setMovingId(null);
      setDraggingId(null);
    }
  };

  const requestMove = (task: Task, status: TaskStatus) => {
    setDraggingId(null);
    if (task.status === status || movingId) return;
    if (status === "blocked") {
      setBlockingTask(task);
      setBlockedReason("");
      setError(null);
      return;
    }
    void moveTask(task, status);
  };

  const blockTask = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const reason = blockedReason.trim();
    if (!blockingTask || !reason) return;
    if (await moveTask(blockingTask, "blocked", reason)) {
      setBlockingTask(null);
      setBlockedReason("");
    }
  };

  return <section className="task-board-module" aria-label="Task board">
    <div className="task-board-toolbar">
      <label className="board-search"><SearchIcon /><input aria-label="Search task board" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Filter tasks…" /></label>
      <span className="board-total">{tasks.length} tasks</span>
      <button className={showDone ? "board-filter active" : "board-filter"} type="button" onClick={() => setShowDone((value) => !value)}><span aria-hidden="true">✓</span>{showDone ? "Hide completed" : `Show completed (${completedCount})`}</button>
    </div>
    {error ? <div className="board-error" role="alert"><span>{error}</span><span><button type="button" onClick={() => { setError(null); void onReload(); }}>Reload tasks</button><button type="button" onClick={() => setError(null)} aria-label="Dismiss error">×</button></span></div> : null}
    {loading ? <div className="board-loading" role="status">Loading tasks…</div> : <div className="task-board-columns" data-columns={visibleColumns.length}>
      {visibleColumns.map((column) => <section className={`task-column ${column.status}`} aria-label={`${column.label} tasks`} key={column.status} onDragOver={(event) => event.preventDefault()} onDrop={() => {
        const task = tasks.find((item) => item.object_id === draggingId);
        if (task) requestMove(task, column.status);
      }}>
        <header><span className="column-status" aria-hidden="true">{column.icon}</span><strong>{column.label}</strong><span>{grouped.get(column.status)?.length ?? 0}</span></header>
        <div className="task-column-cards">
          {(grouped.get(column.status) ?? []).map((task) => <TaskCard key={task.object_id} task={task} visuals={visuals} moving={movingId === task.object_id} onDragStart={() => setDraggingId(task.object_id)} onDragEnd={() => setDraggingId(null)} onMove={(status) => requestMove(task, status)} />)}
          {(grouped.get(column.status)?.length ?? 0) === 0 ? <div className="column-empty">Drop tasks here</div> : null}
        </div>
      </section>)}
    </div>}
    {blockingTask ? <div className="board-dialog-backdrop">
      <form className="board-block-dialog" role="dialog" aria-modal="true" aria-labelledby="block-task-title" onSubmit={(event) => void blockTask(event)}>
        <h2 id="block-task-title">Block “{blockingTask.title}”</h2>
        <label>Blocked reason<textarea autoFocus required maxLength={2000} value={blockedReason} onChange={(event) => setBlockedReason(event.target.value)} placeholder="What is preventing progress?" /></label>
        <div><button className="ghost" type="button" disabled={movingId !== null} onClick={() => { setBlockingTask(null); setBlockedReason(""); }}>Cancel</button><button className="primary small" type="submit" disabled={!blockedReason.trim() || movingId !== null}>{movingId ? "Saving…" : "Block task"}</button></div>
      </form>
    </div> : null}
  </section>;
}

function TaskCard({ task, visuals, moving, onDragStart, onDragEnd, onMove }: { task: Task; visuals: Map<string, ObjectVisual>; moving: boolean; onDragStart: () => void; onDragEnd: () => void; onMove: (status: TaskStatus) => void }) {
  return <article className={moving ? "task-board-card moving" : "task-board-card"} draggable={!moving} onDragStart={(event) => { event.dataTransfer.effectAllowed = "move"; event.dataTransfer.setData("text/plain", task.object_id); onDragStart(); }} onDragEnd={onDragEnd}>
    <button className="task-card-open" type="button" onClick={() => navigate(detailPath("tasks", task.object_id))} aria-label={`Open ${task.title}`} />
    <div className="task-card-title"><span className={`priority-dot ${task.priority}`} title={`${task.priority} priority`} /><strong>{task.title}</strong></div>
    {task.description ? <p>{task.description}</p> : null}
    {task.blocked_reason ? <p className="task-blocked-reason"><strong>Blocked:</strong> {task.blocked_reason}</p> : null}
    <TaskReadiness task={task} />
    <footer><div className="task-card-meta"><TaskIssueLink url={task.github_issue_url} />{task.due_at ? <span className={isOverdue(task) ? "task-due overdue" : "task-due"}><CalendarIcon />{formatDue(task.due_at)}</span> : null}{task.agent_suitable ? <span className="agent-ready" title="Agent suitable">✦</span> : null}</div><div className="task-card-actions"><TaskAssignee task={task} visuals={visuals} /><select aria-label={`Move ${task.title}`} value={task.status} disabled={moving} onClick={(event) => event.stopPropagation()} onChange={(event) => onMove(event.target.value as TaskStatus)}>{columns.map((column) => <option key={column.status} value={column.status}>{column.label}</option>)}</select></div></footer>
  </article>;
}

function SearchIcon() { return <svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="7" cy="7" r="4.25" /><path d="m10.25 10.25 3 3" /></svg>; }
function CalendarIcon() { return <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="2.5" y="3.5" width="11" height="10" rx="2" /><path d="M5 2v3M11 2v3M2.5 6.5h11" /></svg>; }
function formatDue(value: string) { return dueDateFormatter.format(new Date(value)); }
function isOverdue(task: Task) { return Boolean(task.due_at && task.status !== "done" && new Date(task.due_at).getTime() < Date.now()); }

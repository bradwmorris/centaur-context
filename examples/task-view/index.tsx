import { useState } from "react";
import { TaskAssignee, TaskIssueLink, type TaskViewProps } from "@centaur-context/ui";
import styles from "./styles.module.css";

export default function TaskSummary({ tasks, visuals, loading, error, completeness, reload, openTask, changeStatus }: TaskViewProps) {
  const [writeError, setWriteError] = useState<string | null>(null);
  const [pending, setPending] = useState<string | null>(null);
  if (loading) return <p role="status">Loading task summary…</p>;
  return <section className={styles.summary} aria-label="External task summary">
    <h2>Task summary</h2>
    {(error || writeError) && <p role="alert">{error || writeError}</p>}
    <button type="button" onClick={() => void reload()}>Reload tasks</button>
    <p>{tasks.length} tasks · {completeness === "complete" ? "All active task pages loaded" : "Results may be incomplete"}</p>
    {!tasks.length && <p>No tasks yet.</p>}
    <ul>{tasks.map(task => <li key={task.object_id}>
      <button type="button" onClick={() => openTask(task.object_id)}>{task.title}</button>
      <span>{task.status}</span><TaskAssignee task={task} visuals={visuals} /><TaskIssueLink url={task.github_issue_url} />
      {task.status === "todo" && <button type="button" disabled={pending !== null} onClick={async () => {
        setPending(task.object_id); setWriteError(null);
        try { await changeStatus(task, "doing"); }
        catch (cause) { setWriteError(cause instanceof Error ? cause.message : "Update failed"); }
        finally { setPending(null); }
      }}>{pending === task.object_id ? "Saving…" : `Start ${task.title}`}</button>}
    </li>)}</ul>
  </section>;
}

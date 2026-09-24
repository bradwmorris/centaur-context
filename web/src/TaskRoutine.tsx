import { useEffect, useState } from "react";
import { api } from "./api";
import type { Task } from "./types";

type Schedule = { timezone: string; every_minutes: number | null; local_time: string | null; weekdays: number[] };
export type RoutineDetail = { routine: { schedule: Schedule; enabled: boolean; next_run_at: string | null } | null;
  runs: { id: string; scheduled_for: string; status: string; execution_url: string | null; result: string | null }[] };
export function TaskRoutine({ task, onChanged, refreshKey = 0 }: { task: Task; onChanged: () => Promise<void>; refreshKey?: number }) {
  const [detail, setDetail] = useState<RoutineDetail | null>(null);
  const [editing, setEditing] = useState(false);
  const [mode, setMode] = useState("daily");
  const [zone, setZone] = useState(Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC");
  const [clock, setClock] = useState("09:00");
  const [minutes, setMinutes] = useState(60);
  const [days, setDays] = useState([1, 2, 3, 4, 5, 6, 7]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let current = true;
    api.routine(task.object_id).then((value) => {
      if (!current) return;
      setDetail(value);
      const schedule = value.routine?.schedule;
      if (schedule) { setZone(schedule.timezone); setMode(schedule.every_minutes ? "interval" : "daily"); setMinutes(schedule.every_minutes ?? 60); setClock(schedule.local_time ?? "09:00"); setDays(schedule.weekdays); }
    }).catch((cause: Error) => { if (current) setError(cause.message); });
    return () => { current = false; };
  }, [task.object_id, task.revision, refreshKey]);
  const save = async (enabled: boolean, keepSchedule = false) => {
    setBusy(true); setError(null);
    try {
      const schedule = keepSchedule && detail?.routine ? detail.routine.schedule : { timezone: zone, every_minutes: mode === "interval" ? minutes : null, local_time: mode === "daily" ? clock : null, weekdays: mode === "daily" ? days : [] };
      await api.configureRoutine(task.object_id, { expected_revision: task.revision, schedule, enabled, confirmed: enabled });
      setEditing(false); await onChanged();
    } catch (cause) { setError(cause instanceof Error ? cause.message : "Could not save Routine"); }
    finally { setBusy(false); }
  };
  return <section className="properties-block" aria-label="Routine schedule">
    <h2>{detail?.routine ? "Routine" : "Schedule"}</h2>
    <p>A due date is a review date. Only an enabled Routine starts automatically.</p>
    {detail?.routine && <>
      <p>{detail.routine.enabled ? "Enabled" : "Paused"} · {detail.routine.schedule.timezone}</p>
      <p>Next run: {detail.routine.next_run_at ? new Date(detail.routine.next_run_at).toLocaleString() : "None"}</p>
      <button disabled={busy} onClick={() => void save(!detail.routine?.enabled, true)}>{detail.routine.enabled ? "Pause Routine" : "Enable Routine"}</button>
    </>}
    <button disabled={busy} onClick={() => setEditing(!editing)}>{detail?.routine ? "Edit schedule" : "Make a Routine"}</button>
    {editing && <div>
      <label>Schedule <select value={mode} onChange={(event) => setMode(event.target.value)}><option value="daily">Selected days at a time</option><option value="interval">Every N minutes</option></select></label>
      <label>Timezone <input value={zone} onChange={(event) => setZone(event.target.value)} /></label>
      {mode === "interval" ? <label>Minutes <input type="number" min="1" max="525600" value={minutes} onChange={(event) => setMinutes(Number(event.target.value))} /></label> : <>
        <label>Local time <input type="time" value={clock} onChange={(event) => setClock(event.target.value)} /></label>
        <div>{["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].map((day, index) => <label key={day}><input type="checkbox" checked={days.includes(index + 1)} onChange={(event) => setDays(event.target.checked ? [...days, index + 1] : days.filter((value) => value !== index + 1))} />{day}</label>)}</div>
      </>}
      <p>Enabling authorizes repeated execution of this task’s current brief. Changes to the task pause the schedule until you enable it again.</p>
      <button disabled={busy} onClick={() => void save(false)}>Save paused</button>
      <button disabled={busy} onClick={() => void save(true)}>Save and enable Routine</button>
    </div>}
    {error && <p role="alert">{error}</p>}
    {!!detail?.runs.length && <><h3>Execution history</h3><ul>{detail.runs.map((run) => <li key={run.id}>
      {new Date(run.scheduled_for).toLocaleString()} · {run.status} {run.execution_url && /^https:\/\//.test(run.execution_url) && <a href={run.execution_url} target="_blank" rel="noreferrer">Open execution</a>}
      {run.result && <p>{run.result}</p>}
      {run.status === "review" && <button disabled={busy} onClick={async () => { setBusy(true); try { await api.acceptRoutineRun(run.id); setDetail(await api.routine(task.object_id)); } catch (cause) { setError(String(cause)); } finally { setBusy(false); } }}>Accept result</button>}
    </li>)}</ul></>}
  </section>;
}

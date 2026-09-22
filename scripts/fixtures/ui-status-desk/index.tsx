import { useState } from "react";
import { type TaskViewProps, type TaskStatus } from "@centaur-context/ui";
import styles from "./styles.module.css";
export default function StatusDesk({tasks, loading, error, completeness, reload, openTask, changeStatus}: TaskViewProps) {
 const [id,setId]=useState(""); const [status,setStatus]=useState<TaskStatus>("doing");
 const [reason,setReason]=useState(""); const [message,setMessage]=useState(""); const [failure,setFailure]=useState(""); const [busy,setBusy]=useState(false); const [crash,setCrash]=useState(false);
 if(crash) throw new Error("Intentional acceptance-test render failure");
 const task=tasks.find(t=>t.object_id===id)??tasks[0];
 return <section className={styles.desk} aria-label="External status desk">
 <p className={styles.eyebrow}>SYNTHETIC DEMO · EXTERNAL OVERLAY</p><h2>Status desk</h2>
 <p>Manage a canonical Task from a separately owned view.</p>
 <p data-testid="completeness">{tasks.length} tasks · {completeness}</p>
 {loading&&<p role="status">Loading tasks…</p>}{(error||failure)&&<p role="alert">{error||failure}</p>}{message&&<p role="status">{message}</p>}
 {!loading&&!tasks.length&&<p>No tasks yet.</p>}
 <button onClick={()=>void reload().catch(e=>setFailure(String(e)))}>Reload tasks</button>
 {task&&<div className={styles.form}>
 <label>Task<select value={task.object_id} onChange={e=>{setId(e.target.value);setMessage("");setFailure("");}}>{tasks.map(t=><option key={t.object_id} value={t.object_id}>{t.title}</option>)}</select></label>
 <p data-testid="current-status">Current status: <strong>{task.status}</strong> · revision {task.revision}</p>
 <label>New status<select value={status} onChange={e=>setStatus(e.target.value as TaskStatus)}>{["todo","doing","blocked","review","done"].map(s=><option key={s} value={s}>{s}</option>)}</select></label>
 <label>Blocked reason<input value={reason} onChange={e=>setReason(e.target.value)} placeholder="Required when blocking a task"/></label>
 <div className={styles.actions}><button disabled={busy} onClick={async()=>{setBusy(true);setMessage("");setFailure("");try{const t=await changeStatus(task,status,reason);setMessage(`Saved ${t.status} · revision ${t.revision}`);}catch(e){setFailure(e instanceof Error?e.message:String(e));}finally{setBusy(false);}}}>{busy?"Saving…":"Save status"}</button><button onClick={()=>openTask(task.object_id)}>Open canonical detail</button></div>
 </div>}
 <details><summary>Test error recovery</summary><p>This deliberately crashes only this view so the host recovery screen can be checked.</p><button onClick={()=>setCrash(true)}>Trigger test render failure</button></details>
 </section>;
}

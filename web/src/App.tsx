import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { ApiError, api } from "./api";
import type { Connection, ObjectEvent, SharedObject, Task, TaskStatus } from "./types";

type Section = "objects" | "tasks" | "chats" | "entities" | "memories";

const connectionKinds = ["supports", "depends_on", "references", "part_of", "supersedes"];
const taskStatuses: TaskStatus[] = ["todo", "doing", "blocked", "review", "done"];
const sectionLabels: Record<Section, string> = { objects: "Objects", tasks: "Tasks", chats: "Chats", entities: "Entities", memories: "Memories" };
const sectionSingular: Record<Section, string> = { objects: "object", tasks: "task", chats: "chat", entities: "entity", memories: "memory" };
const sectionDescriptions: Record<Section, string> = {
  objects: "The canonical record of everything in Centaur OS.",
  tasks: "Work that can be tracked or handed to an agent.",
  chats: "Canonical conversations shared across the system.",
  entities: "People, organisations, places, and named things.",
  memories: "Durable context worth carrying forward.",
};
const sectionKinds = { chats: "chat", entities: "entity", memories: "memory" } as const;

export default function App() {
  const [section, setSection] = useState<Section>("objects");
  const [collapsed, setCollapsed] = useState(false);
  const [objects, setObjects] = useState<SharedObject[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [nextObjects, nextTasks] = await Promise.all([api.objects(query), api.tasks()]);
      setObjects(nextObjects);
      setTasks(nextTasks);
      const current = itemsForSection(section, nextObjects, nextTasks);
      if (selectedId && !current.some((item) => item.id === selectedId)) {
        setSelectedId(null);
      }
    } catch (cause) {
      setError(message(cause));
    } finally {
      setLoading(false);
    }
  }, [query, section, selectedId]);

  useEffect(() => {
    const timeout = window.setTimeout(() => void load(), 150);
    return () => window.clearTimeout(timeout);
  }, [load]);

  const selectSection = (next: Section) => {
    setSection(next);
    setCreateOpen(false);
    setSelectedId(null);
    setQuery("");
  };

  const currentItems = itemsForSection(section, objects, tasks);
  const selectedItem = currentItems.find((item) => item.id === selectedId);
  const sectionLabel = sectionLabels[section];

  return (
    <main className={collapsed ? "app nav-collapsed" : "app"}>
      <aside className="nav-rail">
        <div className="nav-head">
          <div className="brand">
            <span className="brand-mark">C</span>
            {!collapsed && <span>Centaur OS</span>}
          </div>
          <button className="collapse-button" onClick={() => setCollapsed((value) => !value)} aria-label={collapsed ? "Expand navigation" : "Collapse navigation"}>
            {collapsed ? "›" : "‹"}
          </button>
        </div>
        <nav aria-label="Centaur OS">
          <NavButton active={section === "objects"} compact={collapsed} icon="◇" label="Objects" onClick={() => selectSection("objects")} />
          <NavButton active={section === "tasks"} compact={collapsed} icon="✓" label="Tasks" onClick={() => selectSection("tasks")} />
          <NavButton active={section === "chats"} compact={collapsed} icon="◌" label="Chats" onClick={() => selectSection("chats")} />
          <NavButton active={section === "entities"} compact={collapsed} icon="◎" label="Entities" onClick={() => selectSection("entities")} />
          <NavButton active={section === "memories"} compact={collapsed} icon="✦" label="Memories" onClick={() => selectSection("memories")} />
        </nav>
        <div className="nav-foot" title="Running locally"><span className="status-dot" />{!collapsed && "Local workspace"}</div>
      </aside>

      <section className="main-panel">
        <header className="topbar">
          <div className="page-path">
            <button className="path-root" onClick={() => setSelectedId(null)}>{sectionLabel}</button>
            {selectedItem && <><span>›</span><strong>{selectedItem.title}</strong></>}
          </div>
        </header>
        {error && <div className="error-banner">{error}<button onClick={() => setError(null)}>×</button></div>}

        <div className="workspace">
          {!selectedId ? <section className="list-view" aria-label={`${section} records`}>
            <header className="list-view-head">
              <div><div className="title-with-action"><h1>{sectionLabel}</h1><button className="add-icon" onClick={() => setCreateOpen(true)} aria-label={`New ${sectionSingular[section]}`}>+</button></div><p>{sectionDescriptions[section]}</p></div>
            </header>
            <div className="list-toolbar">
              {section !== "tasks" && <label className="search"><span>⌕</span><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder={`Search ${sectionLabel.toLowerCase()}`} /></label>}
              <span>{currentItems.length} {currentItems.length === 1 ? "record" : "records"}</span>
            </div>
            <div className="list-group-head"><span className="status-ring" /><strong>All {sectionLabel.toLowerCase()}</strong><span>{currentItems.length}</span></div>
            <div className="record-list">
              {currentItems.map((item) => (
                <button key={item.id} className="record" onClick={() => setSelectedId(item.id)}>
                  <span className="row-grip">···</span>
                  <span className={`kind ${"kind" in item ? item.kind : item.status}`}>{"kind" in item ? item.kind : item.status}</span>
                  <strong>{item.title}</strong>
                  <p>{item.body || "No description"}</p>
                  {"agent_eligible" in item && item.agent_eligible ? <span className="agent-pill">Agent</span> : <span />}
                  <time>{relative(item.updated_at)}</time>
                </button>
              ))}
              {!loading && currentItems.length === 0 && <div className="empty-list">Nothing here yet.</div>}
            </div>
          </section> : <section className="detail-page">
            {section === "tasks" ? <TaskDetail id={selectedId} onChanged={load} /> : <ObjectDetail id={selectedId} objects={objects} onChanged={load} />}
          </section>}
        </div>
      </section>

      {createOpen && (section === "tasks"
        ? <NewTask onCancel={() => setCreateOpen(false)} onCreated={(item) => finishCreate(item.id, load, setCreateOpen, setSelectedId)} />
        : <NewObject fixedKind={section in sectionKinds ? sectionKinds[section as keyof typeof sectionKinds] : undefined} label={sectionSingular[section]} onCancel={() => setCreateOpen(false)} onCreated={(item) => finishCreate(item.id, load, setCreateOpen, setSelectedId)} />)}
    </main>
  );
}

function itemsForSection(section: Section, objects: SharedObject[], tasks: Task[]): Array<SharedObject | Task> {
  if (section === "tasks") return tasks;
  if (section === "objects") return objects;
  const kind = sectionKinds[section];
  return objects.filter((item) => item.kind === kind);
}

function NavButton({ active, compact, icon, label, onClick }: { active: boolean; compact: boolean; icon: string; label: string; onClick: () => void }) {
  return <button className={active ? "nav-button active" : "nav-button"} onClick={onClick} aria-label={label} aria-current={active ? "page" : undefined} title={compact ? label : undefined}><span aria-hidden="true">{icon}</span>{!compact && label}</button>;
}

function NewObject({ fixedKind, label, onCancel, onCreated }: { fixedKind?: "chat" | "entity" | "memory"; label: string; onCancel: () => void; onCreated: (item: SharedObject) => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); setBusy(true); setError(null);
    const data = new FormData(event.currentTarget);
    try {
      onCreated(await api.createObject({
        kind: fixedKind ?? String(data.get("kind")), title: String(data.get("title")), body: String(data.get("body")),
        provenance: { source_type: "human", note: "Created in Centaur OS" },
      }));
    } catch (cause) { setError(message(cause)); setBusy(false); }
  };
  const name = label.charAt(0).toUpperCase() + label.slice(1);
  return <CreateModal title={`New ${label}`} onClose={onCancel}><form className="create-form" onSubmit={submit}>
    <input className="create-title" name="title" required maxLength={300} autoFocus placeholder={`${name} title`} aria-label={`${name} title`} />
    <textarea className="create-body" name="body" rows={5} placeholder="Add details…" aria-label={`${name} body`} />
    {error && <p className="form-error">{error}</p>}
    <div className="create-footer">{fixedKind ? <span className="property-chip">{name}</span> : <Field label="Type"><select name="kind" defaultValue="note"><option value="note">Note</option><option value="source">Source</option><option value="decision">Decision</option><option value="chat">Chat</option><option value="entity">Entity</option><option value="memory">Memory</option></select></Field>}<div className="create-actions"><button type="button" className="ghost" onClick={onCancel}>Cancel</button><button className="primary" disabled={busy}>{busy ? "Creating…" : `Create ${label}`}</button></div></div>
  </form></CreateModal>;
}

function NewTask({ onCancel, onCreated }: { onCancel: () => void; onCreated: (item: Task) => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); setBusy(true); setError(null); const data = new FormData(event.currentTarget);
    try { onCreated(await api.createTask({ title: String(data.get("title")), body: String(data.get("body")), status: "todo", agent_eligible: data.get("agent_eligible") === "on", provenance: { source_type: "human", note: "Created in Centaur OS" } })); }
    catch (cause) { setError(message(cause)); setBusy(false); }
  };
  return <CreateModal title="New task" onClose={onCancel}><form className="create-form" onSubmit={submit}>
    <input className="create-title" name="title" required maxLength={300} autoFocus placeholder="Task title" aria-label="Task title" />
    <textarea className="create-body" name="body" rows={5} placeholder="Add a description…" aria-label="Task description" />
    {error && <p className="form-error">{error}</p>}
    <div className="create-footer"><label className="property-chip"><input type="checkbox" name="agent_eligible" /> Agent eligible</label><div className="create-actions"><button type="button" className="ghost" onClick={onCancel}>Cancel</button><button className="primary" disabled={busy}>{busy ? "Creating…" : "Create task"}</button></div></div>
  </form></CreateModal>;
}

function ObjectDetail({ id, objects, onChanged }: { id: string; objects: SharedObject[]; onChanged: () => Promise<void> }) {
  const [item, setItem] = useState<SharedObject | null>(null);
  const [connections, setConnections] = useState<Connection[]>([]);
  const [events, setEvents] = useState<ObjectEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(async () => {
    try { const [nextItem, nextConnections, nextEvents] = await Promise.all([api.object(id), api.connections(id), api.events(id)]); setItem(nextItem); setConnections(nextConnections); setEvents(nextEvents); setError(null); }
    catch (cause) { setError(message(cause)); }
  }, [id]);
  useEffect(() => { void load(); }, [load]);
  if (!item) return <DetailLoading error={error} />;
  const save = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); const data = new FormData(event.currentTarget);
    try { setItem(await api.updateObject(id, { expected_revision: item.revision, title: String(data.get("title")), body: String(data.get("body")) })); await Promise.all([load(), onChanged()]); }
    catch (cause) { setError(conflictMessage(cause)); }
  };
  return <div className="record-page">
    <div className="record-primary">
      <form className="detail-form" onSubmit={save}>
        <input className="title-input" name="title" aria-label="Object title" defaultValue={item.title} key={`${item.id}-${item.revision}-title`} />
        <section className="properties-block" aria-label="Object properties">
          <h2>Properties</h2>
          <div className="properties-grid">
            <Property label="Type"><span className={`kind ${item.kind}`}>{item.kind}</span></Property>
            <Property label="Revision">{item.revision}</Property>
            <Property label="Created by"><span className="property-value-wrap">{item.created_by_type}:{item.created_by_id}</span></Property>
            <Property label="Source">{item.provenance.source_type ?? "Unspecified"}</Property>
            <Property label="Updated">{relative(item.updated_at)}</Property>
          </div>
        </section>
        <textarea className="body-input" name="body" aria-label="Object body" defaultValue={item.body} key={`${item.id}-${item.revision}-body`} rows={9} placeholder="Add details…" />
        <button className="secondary save-button">Save changes</button>
      </form>
      {error && <p className="form-error">{error}</p>}
      <Connections object={item} objects={objects} connections={connections} onCreated={load} />
      <Section title="Activity"><div className="timeline">{events.map((event) => <div className="event" key={event.id}><span className="event-dot" /><div><strong>{event.action.replaceAll("_", " ")}</strong><p>{event.actor_type}:{event.actor_id}{event.centaur_thread_key ? ` · ${event.centaur_thread_key}` : ""}</p></div><time>{relative(event.created_at)}</time></div>)}</div></Section>
    </div>
  </div>;
}

function Connections({ object, objects, connections, onCreated }: { object: SharedObject; objects: SharedObject[]; connections: Connection[]; onCreated: () => Promise<void> }) {
  const [open, setOpen] = useState(false); const [error, setError] = useState<string | null>(null);
  const titles = useMemo(() => new Map(objects.map((item) => [item.id, item.title])), [objects]);
  const submit = async (event: FormEvent<HTMLFormElement>) => { event.preventDefault(); const data = new FormData(event.currentTarget);
    try { await api.createConnection({ source_object_id: object.id, kind: String(data.get("kind")), target_object_id: String(data.get("target")), reason: String(data.get("reason")), provenance: { source_type: "human" } }); setOpen(false); await onCreated(); }
    catch (cause) { setError(message(cause)); }
  };
  return <Section title="Relationships" action={<button className="text-button" onClick={() => setOpen((value) => !value)}>+ Connect</button>}>
    {open && <form className="connection-form" onSubmit={submit}><select name="kind">{connectionKinds.map((kind) => <option key={kind}>{kind}</option>)}</select><select name="target" required defaultValue=""><option value="" disabled>Target record</option>{objects.filter((item) => item.id !== object.id && item.kind !== "task").map((item) => <option value={item.id} key={item.id}>{item.title}</option>)}</select><input name="reason" required placeholder="Why are these connected?" /><button className="secondary">Add</button>{error && <p className="form-error">{error}</p>}</form>}
    <div className="connections">{connections.map((connection) => { const other = connection.source_object_id === object.id ? connection.target_object_id : connection.source_object_id; return <div className="connection" key={connection.id}><span>{connection.kind.replace("_", " ")}</span><strong>{titles.get(other) ?? other}</strong><p>{connection.reason}</p></div>; })}{connections.length === 0 && <p className="muted">No explained relationships yet.</p>}</div>
  </Section>;
}

function TaskDetail({ id, onChanged }: { id: string; onChanged: () => Promise<void> }) {
  const [task, setTask] = useState<Task | null>(null); const [error, setError] = useState<string | null>(null);
  const load = useCallback(async () => { try { setTask(await api.task(id)); setError(null); } catch (cause) { setError(message(cause)); } }, [id]);
  useEffect(() => { void load(); }, [load]);
  if (!task) return <DetailLoading error={error} />;
  const save = async (event: FormEvent<HTMLFormElement>) => { event.preventDefault(); const data = new FormData(event.currentTarget);
    try { setTask(await api.updateTask(id, { expected_revision: task.revision, title: String(data.get("title")), body: String(data.get("body")), status: String(data.get("status")), agent_eligible: data.get("agent_eligible") === "on" })); await onChanged(); }
    catch (cause) { setError(conflictMessage(cause)); }
  };
  return <form className="record-page" onSubmit={save}>
    <div className="record-primary">
      <div className="detail-form">
        <input className="title-input" name="title" aria-label="Task title" defaultValue={task.title} key={`${task.id}-${task.revision}-title`} />
        <section className="properties-block" aria-label="Task properties">
          <h2>Properties</h2>
          <div className="properties-grid">
            <Field label="Status"><select name="status" defaultValue={task.status} key={`${task.id}-${task.revision}-status`}>{taskStatuses.map((status) => <option key={status}>{status}</option>)}</select></Field>
            <Property label="Agent access"><label className="check"><input type="checkbox" name="agent_eligible" defaultChecked={task.agent_eligible} key={`${task.id}-${task.revision}-eligible`} /> Eligible</label></Property>
            <Property label="Owner">{task.owner_id ?? "Unassigned"}</Property>
            <Property label="Due">{task.due_at ? new Date(task.due_at).toLocaleString() : "No due date"}</Property>
            <Property label="Revision">{task.revision}</Property>
            <Property label="Updated">{relative(task.updated_at)}</Property>
          </div>
        </section>
        <textarea className="body-input" name="body" aria-label="Task body" defaultValue={task.body} key={`${task.id}-${task.revision}-body`} rows={9} placeholder="Add a description…" />
        <button className="secondary save-button">Save changes</button>
      </div>
      {error && <p className="form-error">{error}</p>}
      <Section title="Activity"><div className="activity-empty"><span className="event-dot" /><p>Task created · {relative(task.created_at)}</p></div></Section>
    </div>
  </form>;
}

function CreateModal({ title, onClose, children }: { title: string; onClose: () => void; children: React.ReactNode }) {
  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => { if (event.key === "Escape") onClose(); };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose]);
  return <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
    <section className="create-modal" role="dialog" aria-modal="true" aria-labelledby="create-modal-title">
      <header className="create-modal-head"><div><span className="modal-mark">C</span><span className="modal-separator">›</span><h2 id="create-modal-title">{title}</h2></div><button className="modal-close" onClick={onClose} aria-label="Close">×</button></header>
      {children}
    </section>
  </div>;
}
function Field({ label, children }: { label: string; children: React.ReactNode }) { return <label className="field"><span>{label}</span>{children}</label>; }
function Property({ label, children }: { label: string; children: React.ReactNode }) { return <div className="property"><span>{label}</span><div>{children}</div></div>; }
function Section({ title, action, children }: { title: string; action?: React.ReactNode; children: React.ReactNode }) { return <section className="detail-section"><header><h3>{title}</h3>{action}</header>{children}</section>; }
function DetailLoading({ error }: { error: string | null }) { return <div className="blank-state"><span>◇</span><h2>{error ? "Could not load" : "Loading"}</h2><p>{error ?? "Reading the shared record…"}</p></div>; }
function relative(value: string) { const seconds = Math.round((Date.now() - new Date(value).getTime()) / 1000); if (seconds < 60) return "now"; if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`; if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`; return `${Math.floor(seconds / 86400)}d ago`; }
function message(cause: unknown) { return cause instanceof Error ? cause.message : "Something went wrong."; }
function conflictMessage(cause: unknown) { return cause instanceof ApiError && cause.status === 409 ? "This record changed elsewhere. Refresh before saving again." : message(cause); }
function finishCreate(id: string, load: () => Promise<void>, setCreateOpen: (open: boolean) => void, setSelectedId: (id: string) => void) { setCreateOpen(false); setSelectedId(id); void load(); }

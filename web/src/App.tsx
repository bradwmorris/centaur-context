import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { ApiError, api } from "./api";
import type { ChatMessage, Connection, CuratorRun, CuratorRunDetail, ExternalIdentity, ObjectEvent, SharedObject, Task, TaskStatus, User } from "./types";

type Section = "objects" | "tasks" | "chats" | "users" | "entities" | "memories" | "curator";

const connectionKinds = ["involves", "about", "related_to", "depends_on", "derived_from"];
const taskStatuses: TaskStatus[] = ["todo", "doing", "blocked", "review", "done"];
const sectionLabels: Record<Section, string> = { objects: "Objects", tasks: "Tasks", chats: "Chats", users: "Users", entities: "Entities", memories: "Memories", curator: "Curator Runs" };
const sectionSingular = { objects: "object", tasks: "task", chats: "chat", entities: "entity", memories: "memory" } as const;
const sectionDescriptions: Record<Section, string> = {
  objects: "The canonical record of everything in Centaur OS.",
  tasks: "Work that can be tracked or handed to an agent.",
  chats: "Canonical conversations shared across the system.",
  users: "Humans and agents with one canonical identity in the graph.",
  entities: "Organisations, places, products, projects, and named things.",
  memories: "Simple event-shaped records of what happened.",
  curator: "Atomic context updates produced after completed interactions.",
};
const sectionKinds = { chats: "chat", users: "user", entities: "entity", memories: "memory" } as const;
const createSections = new Set<Section>(["objects", "tasks", "chats", "entities", "memories"]);
type CreateSection = keyof typeof sectionSingular;

export default function App() {
  const [section, setSection] = useState<Section>("objects");
  const [collapsed, setCollapsed] = useState(false);
  const [objects, setObjects] = useState<SharedObject[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [curatorRuns, setCuratorRuns] = useState<CuratorRun[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [nextObjects, nextTasks, nextCuratorRuns] = await Promise.all([api.objects(query), api.tasks(), api.curatorRuns()]);
      setObjects(nextObjects);
      setTasks(nextTasks);
      setCuratorRuns(nextCuratorRuns);
      const current = itemsForSection(section, nextObjects, nextTasks, nextCuratorRuns);
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

  const currentItems = itemsForSection(section, objects, tasks, curatorRuns);
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
          <NavButton active={section === "users"} compact={collapsed} icon="♙" label="Users" onClick={() => selectSection("users")} />
          <NavButton active={section === "entities"} compact={collapsed} icon="◎" label="Entities" onClick={() => selectSection("entities")} />
          <NavButton active={section === "memories"} compact={collapsed} icon="✦" label="Memories" onClick={() => selectSection("memories")} />
          <NavButton active={section === "curator"} compact={collapsed} icon="↻" label="Curator Runs" onClick={() => selectSection("curator")} />
        </nav>
        <div className="nav-foot" title="Running locally"><span className="status-dot" />{!collapsed && "Local workspace"}</div>
      </aside>

      <section className="main-panel">
        <header className="topbar">
          <div className="page-path">
            <button className="path-root" onClick={() => setSelectedId(null)}>{sectionLabel}</button>
            {selectedItem && <><span>›</span><strong>{itemTitle(selectedItem, objects)}</strong></>}
          </div>
        </header>
        {error && <div className="error-banner">{error}<button onClick={() => setError(null)}>×</button></div>}

        <div className="workspace">
          {!selectedId ? <section className="list-view" aria-label={`${section} records`}>
            <header className="list-view-head">
              <div><div className="title-with-action"><h1>{sectionLabel}</h1>{createSections.has(section) && <button className="add-icon" onClick={() => setCreateOpen(true)} aria-label={`New ${sectionSingular[section as keyof typeof sectionSingular]}`}>+</button>}</div><p>{sectionDescriptions[section]}</p></div>
            </header>
            <div className="list-toolbar">
              {section !== "tasks" && section !== "curator" && <label className="search"><span>⌕</span><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder={`Search ${sectionLabel.toLowerCase()}`} /></label>}
              <span>{currentItems.length} {currentItems.length === 1 ? "record" : "records"}</span>
            </div>
            <div className="list-group-head"><span className="status-ring" /><strong>All {sectionLabel.toLowerCase()}</strong><span>{currentItems.length}</span></div>
            <div className="record-list">
              {currentItems.map((item) => (
                <button key={item.id} className="record" onClick={() => setSelectedId(item.id)}>
                  <span className="row-grip">···</span>
                  <span className={`kind ${itemBadge(item)}`}>{itemBadge(item)}</span>
                  <strong>{itemTitle(item, objects)}</strong>
                  <p>{itemDescription(item)}</p>
                  {"agent_eligible" in item && item.agent_eligible ? <span className="agent-pill">Agent</span> : <span />}
                  <time>{relative("updated_at" in item ? item.updated_at : item.created_at)}</time>
                </button>
              ))}
              {!loading && currentItems.length === 0 && <div className="empty-list">Nothing here yet.</div>}
            </div>
          </section> : <section className="detail-page">
            {section === "tasks" ? <TaskDetail id={selectedId} objects={objects} onChanged={load} /> : section === "curator" ? <CuratorDetail id={selectedId} objects={objects} onChanged={load} /> : <ObjectDetail id={selectedId} objects={objects} onChanged={load} />}
          </section>}
        </div>
      </section>

      {createOpen && isCreateSection(section) && (section === "tasks"
        ? <NewTask onCancel={() => setCreateOpen(false)} onCreated={(item) => finishCreate(item.id, load, setCreateOpen, setSelectedId)} />
        : <NewObject fixedKind={fixedCreateKind(section)} label={sectionSingular[section]} onCancel={() => setCreateOpen(false)} onCreated={(item) => finishCreate(item.id, load, setCreateOpen, setSelectedId)} />)}
    </main>
  );
}

type ListItem = SharedObject | Task | CuratorRun;

function itemsForSection(section: Section, objects: SharedObject[], tasks: Task[], curatorRuns: CuratorRun[]): ListItem[] {
  if (section === "tasks") return tasks;
  if (section === "curator") return curatorRuns;
  if (section === "objects") return objects;
  const kind = sectionKinds[section];
  return objects.filter((item) => item.kind === kind);
}

function itemBadge(item: ListItem) { return "kind" in item ? item.kind : "trigger" in item ? item.status : item.status; }
function itemTitle(item: ListItem, objects: SharedObject[]) { return "title" in item ? item.title : `Chat · ${objects.find((object) => object.id === item.chat_object_id)?.title ?? shortId(item.chat_object_id)}`; }
function itemDescription(item: ListItem) { return "description" in item ? item.description : `${item.trigger.replace("_", " ")} · ${item.message_count} message${item.message_count === 1 ? "" : "s"}`; }
function isCreateSection(section: Section): section is CreateSection { return createSections.has(section); }
function fixedCreateKind(section: CreateSection): "chat" | "entity" | "memory" | undefined { return section === "chats" ? "chat" : section === "entities" ? "entity" : section === "memories" ? "memory" : undefined; }

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
        kind: fixedKind ?? String(data.get("kind")), title: String(data.get("title")), description: String(data.get("description")),
        provenance: { source_type: "human", note: "Created in Centaur OS" },
      }));
    } catch (cause) { setError(message(cause)); setBusy(false); }
  };
  const name = label.charAt(0).toUpperCase() + label.slice(1);
  return <CreateModal title={`New ${label}`} onClose={onCancel}><form className="create-form" onSubmit={submit}>
    <input className="create-title" name="title" required maxLength={300} autoFocus placeholder={`${name} title`} aria-label={`${name} title`} />
    <textarea className="create-body" name="description" rows={5} required placeholder="Explain what this is in simple language…" aria-label={`${name} description`} />
    {error && <p className="form-error">{error}</p>}
    <div className="create-footer">{fixedKind ? <span className="property-chip">{name}</span> : <Field label="Type"><select name="kind" defaultValue="memory"><option value="memory">Memory</option><option value="entity">Entity</option><option value="chat">Chat</option></select></Field>}<div className="create-actions"><button type="button" className="ghost" onClick={onCancel}>Cancel</button><button className="primary" disabled={busy}>{busy ? "Creating…" : `Create ${label}`}</button></div></div>
  </form></CreateModal>;
}

function NewTask({ onCancel, onCreated }: { onCancel: () => void; onCreated: (item: Task) => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); setBusy(true); setError(null); const data = new FormData(event.currentTarget);
    try { onCreated(await api.createTask({ title: String(data.get("title")), description: String(data.get("description")), status: "todo", priority: "medium", agent_eligible: data.get("agent_eligible") === "on", provenance: { source_type: "human", note: "Created in Centaur OS" } })); }
    catch (cause) { setError(message(cause)); setBusy(false); }
  };
  return <CreateModal title="New task" onClose={onCancel}><form className="create-form" onSubmit={submit}>
    <input className="create-title" name="title" required maxLength={300} autoFocus placeholder="Task title" aria-label="Task title" />
    <textarea className="create-body" name="description" rows={5} required placeholder="Explain what this task is…" aria-label="Task description" />
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
    try { setItem(await api.updateObject(id, { expected_revision: item.revision, title: String(data.get("title")), description: String(data.get("description")), protected: data.get("protected") === "on" })); await Promise.all([load(), onChanged()]); }
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
            <Property label="Protected"><label className="property-check"><input type="checkbox" name="protected" defaultChecked={item.protected} key={`${item.id}-${item.revision}-protected`} /> Keep this record curator-safe</label></Property>
            <Property label="Source">{textValue(item.provenance.source_type, "Unspecified")}</Property>
            <Property label="Updated">{relative(item.updated_at)}</Property>
          </div>
        </section>
        <textarea className="body-input" name="description" required aria-label="Object description" defaultValue={item.description} key={`${item.id}-${item.revision}-description`} rows={9} placeholder="Explain what this is…" />
        <button className="secondary save-button">Save changes</button>
      </form>
      {error && <p className="form-error">{error}</p>}
      {item.kind === "user" && <UserIdentityPanel id={item.id} />}
      {item.kind === "chat" && <ChatTranscript id={item.id} />}
      <Provenance value={item.provenance} />
      <Connections object={item} objects={objects} connections={connections} onCreated={load} />
      <Section title="Activity"><div className="timeline">{events.map((event) => <div className="event" key={event.id}><span className="event-dot" /><div><strong>{event.action.replaceAll("_", " ")}</strong><p>{event.actor_type}:{event.actor_id}{event.centaur_thread_key ? ` · ${event.centaur_thread_key}` : ""}</p></div><time>{relative(event.created_at)}</time></div>)}</div></Section>
    </div>
  </div>;
}

function UserIdentityPanel({ id }: { id: string }) {
  const [user, setUser] = useState<User | null>(null);
  const [identities, setIdentities] = useState<ExternalIdentity[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    void Promise.all([api.user(id), api.userIdentities(id)])
      .then(([nextUser, nextIdentities]) => { if (active) { setUser(nextUser); setIdentities(nextIdentities); } })
      .catch((cause) => { if (active) setError(message(cause)); });
    return () => { active = false; };
  }, [id]);
  return <Section title="Identity">
    {error && <p className="form-error">{error}</p>}
    {user && <div className="properties-block compact-properties"><div className="properties-grid"><Property label="User kind"><span className={`kind ${user.user_kind}`}>{user.user_kind}</span></Property>{identities.map((identity) => <div className="identity" key={identity.id}><span>{identity.provider}</span><strong>{identity.display_name ?? identity.provider_user_id}</strong><small>{identity.workspace_id || "Default workspace"} · {identity.provider_user_id}</small></div>)}{identities.length === 0 && <p className="muted">No external identities.</p>}</div></div>}
  </Section>;
}

function ChatTranscript({ id }: { id: string }) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    void api.chatMessages(id)
      .then((items) => { if (active) setMessages(items); })
      .catch((cause) => { if (active) setError(message(cause)); });
    return () => { active = false; };
  }, [id]);
  return <Section title="Messages">
    {error && <p className="form-error">{error}</p>}
    <div className="chat-transcript">
      {messages.map((item) => <article className="chat-message" key={item.id}>
        <header><strong>{item.sender_title}</strong><span>{item.sender_kind}</span><time>{new Date(item.source_created_at).toLocaleString()}</time></header>
        <p>{item.content}</p>
      </article>)}
      {!error && messages.length === 0 && <p className="muted">No messages have been ingested.</p>}
    </div>
  </Section>;
}

function Connections({ object, objects, connections, onCreated }: { object: SharedObject; objects: SharedObject[]; connections: Connection[]; onCreated: () => Promise<void> }) {
  const [open, setOpen] = useState(false); const [error, setError] = useState<string | null>(null);
  const titles = useMemo(() => new Map(objects.map((item) => [item.id, item.title])), [objects]);
  const submit = async (event: FormEvent<HTMLFormElement>) => { event.preventDefault(); const data = new FormData(event.currentTarget);
    try { await api.createConnection({ source_object_id: object.id, kind: String(data.get("kind")), target_object_id: String(data.get("target")), description: String(data.get("description")), protected: data.get("protected") === "on", provenance: { source_type: "human" } }); setOpen(false); await onCreated(); }
    catch (cause) { setError(message(cause)); }
  };
  const toggleProtected = async (connection: Connection) => {
    try { setError(null); await api.updateConnection(connection.id, { expected_revision: connection.revision, protected: !connection.protected }); await onCreated(); }
    catch (cause) { setError(conflictMessage(cause)); }
  };
  return <Section title="Relationships" action={<button className="text-button" onClick={() => setOpen((value) => !value)}>+ Connect</button>}>
    {open && <form className="connection-form" onSubmit={submit}><select name="kind">{connectionKinds.map((kind) => <option key={kind}>{kind}</option>)}</select><select name="target" required defaultValue=""><option value="" disabled>Target record</option>{objects.filter((item) => item.id !== object.id && item.kind !== "task").map((item) => <option value={item.id} key={item.id}>{item.title}</option>)}</select><input name="description" required placeholder="Explain the exact relationship…" /><label className="property-check"><input type="checkbox" name="protected" /> Protect from curator changes</label><button className="secondary">Add</button>{error && <p className="form-error">{error}</p>}</form>}
    <div className="connections">{connections.map((connection) => { const outgoing = connection.source_object_id === object.id; const other = outgoing ? connection.target_object_id : connection.source_object_id; const messageIds = stringList(connection.provenance.supporting_message_ids); return <div className="connection" key={connection.id}><span>{outgoing ? "Outgoing" : "Incoming"}</span><strong>{connection.kind.replace("_", " ")} · {titles.get(other) ?? shortId(other)}</strong><button className="protect-action" type="button" onClick={() => void toggleProtected(connection)}>{connection.protected ? "Unprotect" : "Protect"}</button><p>{connection.description}</p>{messageIds.length > 0 && <small>Messages · {messageIds.map(shortId).join(", ")}</small>}</div>; })}{connections.length === 0 && <p className="muted">No explained relationships yet.</p>}</div>
  </Section>;
}

function TaskDetail({ id, objects, onChanged }: { id: string; objects: SharedObject[]; onChanged: () => Promise<void> }) {
  const [task, setTask] = useState<Task | null>(null);
  const [object, setObject] = useState<SharedObject | null>(null);
  const [connections, setConnections] = useState<Connection[]>([]);
  const [events, setEvents] = useState<ObjectEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(async () => {
    try {
      const [nextTask, nextObject, nextConnections, nextEvents] = await Promise.all([api.task(id), api.object(id), api.connections(id), api.events(id)]);
      setTask(nextTask); setObject(nextObject); setConnections(nextConnections); setEvents(nextEvents); setError(null);
    } catch (cause) { setError(message(cause)); }
  }, [id]);
  useEffect(() => { void load(); }, [load]);
  if (!task || !object) return <DetailLoading error={error} />;
  const save = async (event: FormEvent<HTMLFormElement>) => { event.preventDefault(); const data = new FormData(event.currentTarget);
    try { await api.updateTask(id, { expected_revision: task.revision, title: String(data.get("title")), description: String(data.get("description")), protected: data.get("protected") === "on", status: String(data.get("status")), priority: String(data.get("priority")), agent_eligible: data.get("agent_eligible") === "on" }); await Promise.all([load(), onChanged()]); }
    catch (cause) { setError(conflictMessage(cause)); }
  };
  return <div className="record-page">
    <div className="record-primary">
      <form className="detail-form" onSubmit={save}>
        <input className="title-input" name="title" aria-label="Task title" defaultValue={task.title} key={`${task.id}-${task.revision}-title`} />
        <section className="properties-block" aria-label="Task properties">
          <h2>Properties</h2>
          <div className="properties-grid">
            <Field label="Status"><select name="status" defaultValue={task.status} key={`${task.id}-${task.revision}-status`}>{taskStatuses.map((status) => <option key={status}>{status}</option>)}</select></Field>
            <Property label="Agent access"><label className="check"><input type="checkbox" name="agent_eligible" defaultChecked={task.agent_eligible} key={`${task.id}-${task.revision}-eligible`} /> Eligible</label></Property>
            <Property label="Protected"><label className="property-check"><input type="checkbox" name="protected" defaultChecked={task.protected} key={`${task.id}-${task.revision}-protected`} /> Keep this record curator-safe</label></Property>
            <Property label="Priority"><select name="priority" defaultValue={task.priority} key={`${task.id}-${task.revision}-priority`}><option value="low">Low</option><option value="medium">Medium</option><option value="high">High</option></select></Property>
            <Property label="Owner">{task.owner_object_id ?? "Unassigned"}</Property>
            <Property label="Due">{task.due_at ? new Date(task.due_at).toLocaleString() : "No due date"}</Property>
            <Property label="Revision">{task.revision}</Property>
            <Property label="Updated">{relative(task.updated_at)}</Property>
          </div>
        </section>
        <textarea className="body-input" name="description" required aria-label="Task description" defaultValue={task.description} key={`${task.id}-${task.revision}-description`} rows={9} placeholder="Explain what this task is…" />
        <button className="secondary save-button">Save changes</button>
      </form>
      {error && <p className="form-error">{error}</p>}
      <Provenance value={task.provenance} />
      <Connections object={object} objects={objects} connections={connections} onCreated={load} />
      <Section title="Activity"><div className="timeline">{events.map((event) => <div className="event" key={event.id}><span className="event-dot" /><div><strong>{event.action.replaceAll("_", " ")}</strong><p>{event.actor_type}:{event.actor_id}</p></div><time>{relative(event.created_at)}</time></div>)}</div></Section>
    </div>
  </div>;
}

function CuratorDetail({ id, objects, onChanged }: { id: string; objects: SharedObject[]; onChanged: () => Promise<void> }) {
  const [detail, setDetail] = useState<CuratorRunDetail | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(async () => {
    try { setDetail(await api.curatorRun(id)); setError(null); }
    catch (cause) { setError(message(cause)); }
  }, [id]);
  useEffect(() => { void load(); }, [load]);
  if (!detail) return <DetailLoading error={error} />;
  const { run, messages, changes } = detail;
  const chatTitle = objects.find((object) => object.id === run.chat_object_id)?.title ?? shortId(run.chat_object_id);
  const undo = async () => {
    if (!window.confirm("Undo every graph change made by this Curator Run? The source messages and audit history will be kept.")) return;
    setBusy(true); setError(null);
    try { await api.undoCuratorRun(id); await Promise.all([load(), onChanged()]); }
    catch (cause) { setError(conflictMessage(cause)); }
    finally { setBusy(false); }
  };
  return <div className="record-page"><div className="record-primary">
    <h1 className="detail-title">{chatTitle}</h1>
    <p className="detail-description">Context reconciliation for {run.message_count} message{run.message_count === 1 ? "" : "s"} in this interaction window.</p>
    <section className="properties-block" aria-label="Curator Run properties"><h2>Properties</h2><div className="properties-grid">
      <Property label="Status"><span className={`kind ${run.status}`}>{run.status}</span></Property>
      <Property label="Trigger">{run.trigger.replace("_", " ")}</Property>
      <Property label="Chat">{chatTitle}</Property>
      <Property label="Messages">{run.message_count}</Property>
      <Property label="Attempts">{run.attempts}</Property>
      <Property label="Model">{run.model ?? "Not assigned"}</Property>
      <Property label="Prompt">{run.prompt_version ?? "Not assigned"}</Property>
      <Property label="Created">{new Date(run.created_at).toLocaleString()}</Property>
    </div></section>
    {run.error_message && <p className="run-error">{run.error_message}</p>}
    {error && <p className="form-error">{error}</p>}
    {run.status === "completed" && <button className="danger-button" type="button" disabled={busy} onClick={() => void undo()}>{busy ? "Undoing…" : "Undo whole run"}</button>}
    {run.status === "reversed" && <p className="undo-note">This run was undone. Messages and audit history were preserved.</p>}
    <Section title="Interaction window"><div className="chat-transcript">{messages.map((item) => <article className="chat-message" key={item.id}><header><strong>{item.sender_title}</strong><span>{item.sender_kind}</span><time>{new Date(item.source_created_at).toLocaleString()}</time></header><p>{item.content}</p></article>)}</div></Section>
    <Section title="Graph changes"><div className="change-list">{changes.map((change) => <article className="change" key={change.id}><span className="event-dot" /><div><strong>{change.action} {change.entity_type}</strong><p>{textValue(change.after_state.title, shortId(change.entity_id))} · revision {change.after_revision}</p>{stringList((change.after_state.provenance as Record<string, unknown> | undefined)?.supporting_message_ids).length > 0 && <small>Messages · {stringList((change.after_state.provenance as Record<string, unknown>).supporting_message_ids).map(shortId).join(", ")}</small>}</div><span className={change.undone_at ? "change-state undone" : "change-state"}>{change.undone_at ? "Undone" : "Applied"}</span></article>)}{changes.length === 0 && <p className="muted">No graph changes have been committed.</p>}</div></Section>
  </div></div>;
}

function Provenance({ value }: { value: Record<string, unknown> }) {
  const messageIds = stringList(value.supporting_message_ids);
  const fields = [
    ["Source", textValue(value.source_type, "Unspecified")],
    ["Source reference", textValue(value.source_ref)],
    ["Curator Run", textValue(value.curator_run_id)],
    ["Source Chat", textValue(value.chat_object_id)],
    ["Model", textValue(value.model)],
    ["Prompt", textValue(value.prompt_version)],
  ].filter(([, fieldValue]) => Boolean(fieldValue));
  return <Section title="Provenance"><div className="properties-block compact-properties"><div className="properties-grid">{fields.map(([label, fieldValue]) => <Property label={label} key={label}><span className="property-value-wrap">{fieldValue}</span></Property>)}{messageIds.length > 0 && <Property label="Supporting messages"><span className="property-value-wrap">{messageIds.join(", ")}</span></Property>}</div></div></Section>;
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
function shortId(value: string) { return value.length > 12 ? `${value.slice(0, 8)}…` : value; }
function textValue(value: unknown, fallback = "") { return typeof value === "string" && value.trim() ? value : fallback; }
function stringList(value: unknown) { return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : []; }

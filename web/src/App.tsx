import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { ApiError, api } from "./api";
import { DescriptionSnippet } from "./DescriptionSnippet";
import { ConnectionGraphWorkspace } from "./ConnectionGraph";
import { ConnectionId, ObjectId } from "./ObjectIdentity";
import { AttributionStack, ObjectContext, ObjectTypeBadge, SourceBadge, StateBadge, TaskStatusBadge } from "./RecordVisuals";
import { SchemaWorkspace } from "./SchemaWorkspace";
import { detailPath, navigate, parseRoute, sectionPath } from "./routing";
import type { Section } from "./routing";
import type { Artifact, ArtifactWindow, ChatMessage, Connection, EmbeddingStatus, ExternalIdentity, Note, NoteSummary, ObjectEvent, ObjectKind, ObjectVisual, Run, RunDetail, RunVerdict, SharedObject, Source, SourceKind, Task, TaskStatus, Theme, User } from "./types";

const connectionKinds = ["involves", "about", "related_to", "depends_on", "derived_from", "themed"];
const taskStatuses: TaskStatus[] = ["backlog", "todo", "doing", "review", "done", "blocked"];
const sectionLabels: Record<Section, string> = { objects: "Objects", connections: "Connections", tasks: "Tasks", chats: "Chats", users: "Users", entities: "Entities", memories: "Memories", sources: "Sources", notes: "Notes", themes: "Themes", runs: "Runs", schema: "Schema" };
const sectionSingular = { objects: "object", tasks: "task", chats: "chat", entities: "entity", memories: "memory", sources: "source", notes: "note", themes: "theme" } as const;
const sectionKinds = { chats: "chat", users: "user", entities: "entity", memories: "memory" } as const;
const createSections = new Set<Section>(["objects", "tasks", "chats", "entities", "memories", "sources", "notes", "themes"]);
type CreateSection = keyof typeof sectionSingular;
const descriptionExamples: Record<ObjectKind, string> = {
  task: "Prepare and publish the approved launch notes for customers.",
  chat: "A Slack conversation where the release team approved the launch checklist.",
  user: "A human product lead responsible for the customer migration program.",
  entity: "A customer organization participating in the August migration pilot.",
  memory: "The product team approved the customer migration during the August review.",
  source: "A concise summary of the evidence and why it matters.",
  note: "A short summary that helps people recognize what this note contains.",
  theme: "A research vertical used to group related work for retrieval and audience interests.",
};
const sourceKinds: SourceKind[] = ["article", "paper", "podcast_episode", "video", "book", "report", "document", "dataset", "web_page", "social_post", "other"];

export default function App() {
  const [route, setRoute] = useState(() => parseRoute(window.location.pathname));
  const { section, selectedId, connectionId } = route;
  const [collapsed, setCollapsed] = useState(false);
  const [objects, setObjects] = useState<SharedObject[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [sources, setSources] = useState<Source[]>([]);
  const [notes, setNotes] = useState<NoteSummary[]>([]);
  const [themes, setThemes] = useState<Theme[]>([]);
  const [runs, setRuns] = useState<Run[]>([]);
  const [visuals, setVisuals] = useState<ObjectVisual[]>([]);
  const [createOpen, setCreateOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (section === "schema" || (section === "connections" && !connectionId)) {
      setLoading(false);
      setError(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const objectKind = section in sectionKinds ? sectionKinds[section as keyof typeof sectionKinds] : undefined;
      const [nextObjects, nextTasks, nextSources, nextNotes, nextThemes, nextRuns, nextVisuals] = await Promise.all([api.objects(query, objectKind), api.tasks(), api.sources(section === "sources" ? query : ""), api.notes(section === "notes" ? query : ""), section === "themes" ? api.themes() : Promise.resolve([]), section === "runs" ? api.runs() : Promise.resolve([]), api.objectVisuals()]);
      setObjects(nextObjects);
      setTasks(nextTasks);
      setSources(nextSources.items);
      setNotes(nextNotes.items);
      setThemes(nextThemes);
      setRuns(nextRuns);
      setVisuals(nextVisuals);
    } catch (cause) {
      setError(message(cause));
    } finally {
      setLoading(false);
    }
  }, [query, section, connectionId]);

  useEffect(() => {
    const syncRoute = () => setRoute(parseRoute(window.location.pathname));
    window.addEventListener("popstate", syncRoute);
    if (window.location.pathname === "/") navigate(sectionPath("objects"), true);
    return () => window.removeEventListener("popstate", syncRoute);
  }, []);

  useEffect(() => {
    const timeout = window.setTimeout(() => void load(), 150);
    return () => window.clearTimeout(timeout);
  }, [load]);

  const selectSection = (next: Section) => {
    setCreateOpen(false);
    setQuery("");
    navigate(sectionPath(next));
  };

  const currentItems = itemsForSection(section, objects, tasks, sources, notes, themes, runs, query);
  const visualsById = useMemo(() => new Map(visuals.map((visual) => [visual.object_id, visual])), [visuals]);
  const selectedItem = currentItems.find((item) => itemRouteId(item) === selectedId);
  const sectionLabel = sectionLabels[section];

  return (
    <main className={collapsed ? "app nav-collapsed" : "app"}>
      <aside className="nav-rail">
        <div className="nav-head">
          <div className="brand">
            <span className="brand-mark">C</span>
            {!collapsed && <span>Centaur Context</span>}
          </div>
          <button className="collapse-button" onClick={() => setCollapsed((value) => !value)} aria-label={collapsed ? "Expand navigation" : "Collapse navigation"}>
            {collapsed ? "›" : "‹"}
          </button>
        </div>
        <nav aria-label="Centaur Context">
          <NavButton active={section === "objects"} compact={collapsed} icon="◇" label="Objects" onClick={() => selectSection("objects")} />
          <NavButton active={section === "connections"} compact={collapsed} icon="⌁" label="Connections" onClick={() => selectSection("connections")} />
          <NavButton active={section === "tasks"} compact={collapsed} icon="✓" label="Tasks" onClick={() => selectSection("tasks")} />
          <NavButton active={section === "chats"} compact={collapsed} icon="◌" label="Chats" onClick={() => selectSection("chats")} />
          <NavButton active={section === "users"} compact={collapsed} icon="♙" label="Users" onClick={() => selectSection("users")} />
          <NavButton active={section === "entities"} compact={collapsed} icon="◎" label="Entities" onClick={() => selectSection("entities")} />
          <NavButton active={section === "memories"} compact={collapsed} icon="✦" label="Memories" onClick={() => selectSection("memories")} />
          <NavButton active={section === "sources"} compact={collapsed} icon="▤" label="Sources" onClick={() => selectSection("sources")} />
          <NavButton active={section === "notes"} compact={collapsed} icon="▱" label="Notes" onClick={() => selectSection("notes")} />
          <NavButton active={section === "themes"} compact={collapsed} icon="#" label="Themes" onClick={() => selectSection("themes")} />
          <NavButton active={section === "runs"} compact={collapsed} icon="↻" label="Runs" onClick={() => selectSection("runs")} />
          <NavButton active={section === "schema"} compact={collapsed} icon="⌘" label="Schema" onClick={() => selectSection("schema")} />
        </nav>
        <div className="nav-foot" title="Running locally"><span className="status-dot" />{!collapsed && "Local workspace"}</div>
      </aside>

      <section className="main-panel">
        <header className="topbar">
          <div className="page-path">
            <button className="path-root" onClick={() => navigate(sectionPath(section))}>{sectionLabel}</button>
            {(selectedId || connectionId) && <><span>›</span><strong>{connectionId ? `Connection ${shortId(connectionId)}` : section === "schema" ? selectedId : selectedItem ? itemTitle(selectedItem, objects) : shortId(selectedId ?? "")}</strong></>}
          </div>
        </header>
        {error && <div className="error-banner">{error}<button onClick={() => setError(null)}>×</button></div>}

        <div className="workspace">
          {section === "schema" ? <SchemaWorkspace selectedTable={selectedId} /> : section === "connections" && !connectionId ? <ConnectionGraphWorkspace /> : !selectedId && !connectionId ? <section className="list-view" aria-label={`${section} records`}>
            <header className="list-view-head">
              <div className="title-with-action"><h1>{sectionLabel}</h1>{createSections.has(section) && <button className="add-icon" type="button" onClick={() => setCreateOpen(true)} aria-label={`New ${sectionSingular[section as keyof typeof sectionSingular]}`}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 3.25v9.5M3.25 8h9.5" /></svg></button>}</div>
            </header>
            <div className="list-toolbar">
              {section !== "tasks" && <label className="search"><svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="7" cy="7" r="4.25" /><path d="m10.25 10.25 3 3" /></svg><input aria-label={`Search ${sectionLabel.toLowerCase()}`} value={query} onChange={(event) => setQuery(event.target.value)} placeholder={`Search ${sectionLabel.toLowerCase()}`} /></label>}
              <span>{currentItems.length} {currentItems.length === 1 ? "record" : "records"}</span>
            </div>
            <div className="list-group-head"><span className="status-ring" /><strong>All {sectionLabel.toLowerCase()}</strong><span>{currentItems.length}</span></div>
            <div className="record-list">
              {currentItems.map((item) => (
                <div key={itemRouteId(item)} className="record">
                  <button className="record-open" onClick={() => navigate(detailPath(section, itemRouteId(item)))} aria-label={`Open ${itemTitle(item, objects)}`} />
                  <span className="record-source"><SourceBadge provider={visualsById.get(canonicalObjectId(item))?.source_provider} /></span>
                  {"actor_type" in item ? <span className="object-id-pill">ID: {shortId(item.id)}</span> : <ObjectId id={canonicalObjectId(item)} rowPill />}
                  <span className="record-title"><strong>{itemTitle(item, objects)}</strong><span className="record-badges">{"actor_type" in item ? <StateBadge state={item.status} /> : <><ObjectTypeBadge kind={itemObjectKind(item)} />{"status" in item && <TaskStatusBadge status={item.status} />}</>}</span>{!("actor_type" in item) && <AttributionStack users={visualsById.get(canonicalObjectId(item))?.users ?? []} />}<DescriptionSnippet description={itemDescription(item)} /></span>
                  <time>{relative(item.updated_at)}</time>
                </div>
              ))}
              {!loading && currentItems.length === 0 && <div className="empty-list">Nothing here yet.</div>}
            </div>
          </section> : <section className="detail-page">
            {connectionId ? <ConnectionDetail id={connectionId} objects={objects} visuals={visualsById} /> : section === "tasks" ? <TaskDetail id={selectedId!} objects={objects} visuals={visualsById} onChanged={load} /> : section === "sources" ? <SourceDetail id={selectedId!} objects={objects} visuals={visualsById} onChanged={load} /> : section === "notes" ? <NoteDetail id={selectedId!} objects={objects} visuals={visualsById} onChanged={load} /> : section === "themes" ? <ThemeDetail id={selectedId!} objects={objects} visuals={visualsById} /> : section === "runs" ? <RunDetailView id={selectedId!} onChanged={load} /> : <ObjectDetail id={selectedId!} objects={objects} visuals={visualsById} onChanged={load} />}
          </section>}
        </div>
      </section>

      {createOpen && isCreateSection(section) && (section === "tasks"
        ? <NewTask onCancel={() => setCreateOpen(false)} onCreated={(item) => finishCreate(section, item.object_id, load, setCreateOpen)} />
        : section === "sources" ? <NewSource onCancel={() => setCreateOpen(false)} onCreated={(item) => finishCreate(section, item.object_id, load, setCreateOpen)} />
        : section === "notes" ? <NewNote onCancel={() => setCreateOpen(false)} onCreated={(item) => finishCreate(section, item.object_id, load, setCreateOpen)} />
        : section === "themes" ? <NewTheme onCancel={() => setCreateOpen(false)} onCreated={(item) => finishCreate(section, item.object_id, load, setCreateOpen)} />
        : <NewObject fixedKind={fixedCreateKind(section)} label={sectionSingular[section]} onCancel={() => setCreateOpen(false)} onCreated={(item) => finishCreate(section, item.id, load, setCreateOpen)} />)}
    </main>
  );
}

type ListItem = SharedObject | Task | Source | NoteSummary | Theme | Run;

function itemsForSection(section: Section, objects: SharedObject[], tasks: Task[], sources: Source[], notes: NoteSummary[], themes: Theme[], runs: Run[], query: string): ListItem[] {
  if (section === "schema" || section === "connections") return [];
  if (section === "tasks") return tasks;
  if (section === "sources") return sources;
  if (section === "notes") return notes;
  if (section === "themes") {
    const normalized = query.trim().toLocaleLowerCase();
    return normalized ? themes.filter((item) => `${item.title} ${item.slug} ${item.description}`.toLocaleLowerCase().includes(normalized)) : themes;
  }
  if (section === "runs") {
    const normalized = query.trim().toLocaleLowerCase();
    return normalized ? runs.filter((item) => `${item.id} ${item.kind} ${item.status} ${item.actor_type} ${item.actor_id}`.toLocaleLowerCase().includes(normalized)) : runs;
  }
  if (section === "objects") return objects;
  const kind = sectionKinds[section];
  return objects.filter((item) => item.kind === kind);
}

function itemRouteId(item: ListItem) { return "actor_type" in item ? item.id : canonicalObjectId(item); }
function canonicalObjectId(item: ListItem) { return "actor_type" in item ? item.chat_object_id ?? item.id : "object_id" in item ? item.object_id : item.id; }

function itemObjectKind(item: Exclude<ListItem, Run>): ObjectKind { return "kind" in item ? item.kind : "slug" in item ? "theme" : "source_kind" in item ? "source" : "content_format" in item ? "note" : "task"; }
function itemTitle(item: ListItem, objects: SharedObject[]) { return "actor_type" in item ? `${item.kind.replaceAll("_", " ")} run${item.chat_object_id ? ` · ${objects.find((object) => object.id === item.chat_object_id)?.title ?? shortId(item.chat_object_id)}` : ""}` : "title" in item ? item.title : shortId(canonicalObjectId(item)); }
function itemDescription(item: ListItem) { return "actor_type" in item ? `${item.actor_type}:${item.actor_id} · ${item.verdict}` : "description" in item ? item.description : ""; }
function isCreateSection(section: Section): section is CreateSection { return createSections.has(section); }
function fixedCreateKind(section: CreateSection): "chat" | "entity" | "memory" | undefined { return section === "chats" ? "chat" : section === "entities" ? "entity" : section === "memories" ? "memory" : undefined; }

function NavButton({ active, compact, icon, label, onClick }: { active: boolean; compact: boolean; icon: string; label: string; onClick: () => void }) {
  return <button className={active ? "nav-button active" : "nav-button"} onClick={onClick} aria-label={label} aria-current={active ? "page" : undefined} title={compact ? label : undefined}><span aria-hidden="true">{icon}</span>{!compact && label}</button>;
}

function ThemeDetail({ id, objects, visuals }: { id: string; objects: SharedObject[]; visuals: Map<string, ObjectVisual> }) {
  const [theme, setTheme] = useState<Theme | null>(null);
  const [assigned, setAssigned] = useState<SharedObject[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    void Promise.all([api.theme(id), api.themeObjects(id)])
      .then(([nextTheme, nextAssigned]) => { if (active) { setTheme(nextTheme); setAssigned(nextAssigned); } })
      .catch((cause) => { if (active) setError(message(cause)); });
    return () => { active = false; };
  }, [id]);
  if (!theme) return <DetailLoading error={error} />;
  const themeObject = objects.find((item) => item.id === id);
  return <div className="record-page"><div className="record-primary">
    <h1 className="detail-title">{theme.title}</h1><p className="detail-description">{theme.description}</p>
    <section className="properties-block" aria-label="Theme properties"><h2>Properties</h2><div className="properties-grid"><Property label="Slug"><code>{theme.slug}</code></Property><Property label="Assigned Objects">{assigned.length}</Property><Property label="Protected">{theme.protected ? "Yes" : "No"}</Property><Property label="Updated">{relative(theme.updated_at)}</Property></div></section>
    <Section title="Themed Objects"><div className="connections themed-object-list">{assigned.map((item) => <article className="connection themed-object-row" key={item.id}><ObjectId id={item.id} linkPill /><ObjectTypeBadge kind={item.kind} /><strong className="themed-object-title" title={item.title}>{item.title}</strong><ObjectContext visual={visuals.get(item.id)} /></article>)}{assigned.length === 0 && <p className="muted">No Objects use this Theme yet.</p>}</div></Section>
    {themeObject && <Provenance value={themeObject.provenance} />}
  </div></div>;
}

function NewTheme({ onCancel, onCreated }: { onCancel: () => void; onCreated: (item: Theme) => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); setBusy(true); setError(null);
    const data = new FormData(event.currentTarget);
    try {
      onCreated(await api.createTheme({ title: String(data.get("title")), slug: String(data.get("slug")), description: String(data.get("description")), protected: true, provenance: { source_type: "human", note: "Approved and created in Centaur Context" } }));
    } catch (cause) { setError(message(cause)); setBusy(false); }
  };
  return <CreateModal title="New theme" onClose={onCancel}><form className="create-form" onSubmit={submit}><input className="create-title" name="title" required maxLength={300} autoFocus placeholder="Theme title" aria-label="Theme title" /><Field label="Slug"><input name="slug" required maxLength={100} pattern="[a-z0-9]+(-[a-z0-9]+)*" placeholder="research-vertical" /></Field><textarea className="create-body" name="description" rows={5} required maxLength={2000} placeholder={descriptionExamples.theme} aria-label="Theme description" /><DescriptionHelp id="new-theme-description-help" kind="theme" />{error && <p className="form-error">{error}</p>}<div className="modal-actions"><button type="button" className="text-button" onClick={onCancel}>Cancel</button><button disabled={busy}>{busy ? "Creating…" : "Create approved theme"}</button></div></form></CreateModal>;
}

function NewObject({ fixedKind, label, onCancel, onCreated }: { fixedKind?: "chat" | "entity" | "memory"; label: string; onCancel: () => void; onCreated: (item: SharedObject) => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [kind, setKind] = useState<"chat" | "entity" | "memory">(fixedKind ?? "memory");
  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); setBusy(true); setError(null);
    const data = new FormData(event.currentTarget);
    try {
      onCreated(await api.createObject({
        kind, title: String(data.get("title")), description: String(data.get("description")),
        provenance: { source_type: "human", note: "Created in Centaur Context" },
        entity_kind: kind === "entity" ? String(data.get("entity_kind")) : undefined,
        happened_at:
          kind === "memory" ? (optionalDate(data, "happened_at") ?? undefined) : undefined,
      }));
    } catch (cause) { setError(message(cause)); setBusy(false); }
  };
  const name = label.charAt(0).toUpperCase() + label.slice(1);
  return <CreateModal title={`New ${label}`} onClose={onCancel}><form className="create-form" onSubmit={submit}>
    <input className="create-title" name="title" required maxLength={300} autoFocus placeholder={`${name} title`} aria-label={`${name} title`} />
    <textarea className="create-body" name="description" rows={5} required maxLength={2000} placeholder={descriptionExamples[kind]} aria-label={`${name} description`} aria-describedby="new-object-description-help" />
    <DescriptionHelp id="new-object-description-help" kind={kind} />
    {kind === "entity" && <Field label="Entity kind"><select name="entity_kind" defaultValue="person"><option value="person">Person</option><option value="organization">Organization</option><option value="product">Product</option><option value="project">Project</option><option value="publication">Publication</option><option value="place">Place</option><option value="concept">Concept</option><option value="other">Other</option></select></Field>}
    {kind === "memory" && <Field label="Happened at"><input name="happened_at" type="datetime-local" required /></Field>}
    {error && <p className="form-error">{error}</p>}
    <div className="create-footer">{fixedKind ? <span className="property-chip">{name}</span> : <Field label="Type"><select name="kind" value={kind} onChange={(event) => setKind(event.target.value as typeof kind)}><option value="memory">Memory</option><option value="entity">Entity</option><option value="chat">Chat</option></select></Field>}<div className="create-actions"><button type="button" className="ghost" onClick={onCancel}>Cancel</button><button className="primary" disabled={busy}>{busy ? "Creating…" : `Create ${label}`}</button></div></div>
  </form></CreateModal>;
}

function NewTask({ onCancel, onCreated }: { onCancel: () => void; onCreated: (item: Task) => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); setBusy(true); setError(null); const data = new FormData(event.currentTarget);
    try { onCreated(await api.createTask({ title: String(data.get("title")), description: String(data.get("description")), status: "todo", priority: "medium", agent_suitable: data.get("agent_suitable") === "on", provenance: { source_type: "human", note: "Created in Centaur Context" } })); }
    catch (cause) { setError(message(cause)); setBusy(false); }
  };
  return <CreateModal title="New task" onClose={onCancel}><form className="create-form" onSubmit={submit}>
    <input className="create-title" name="title" required maxLength={300} autoFocus placeholder="Task title" aria-label="Task title" />
    <textarea className="create-body" name="description" rows={5} required maxLength={2000} placeholder={descriptionExamples.task} aria-label="Task description" aria-describedby="new-task-description-help" />
    <DescriptionHelp id="new-task-description-help" kind="task" />
    {error && <p className="form-error">{error}</p>}
    <div className="create-footer"><label className="property-chip"><input type="checkbox" name="agent_suitable" /> Agent suitable</label><div className="create-actions"><button type="button" className="ghost" onClick={onCancel}>Cancel</button><button className="primary" disabled={busy}>{busy ? "Creating…" : "Create task"}</button></div></div>
  </form></CreateModal>;
}

function NewNote({ onCancel, onCreated }: { onCancel: () => void; onCreated: (item: Note) => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); setBusy(true); setError(null); const data = new FormData(event.currentTarget);
    try {
      onCreated(await api.createNote({
        title: String(data.get("title")),
        description: String(data.get("description")),
        content: String(data.get("content")),
        content_format: String(data.get("content_format")),
        provenance: { source_type: "human", note: "Created in Centaur Context" },
      }));
    } catch (cause) { setError(message(cause)); setBusy(false); }
  };
  return <CreateModal title="New note" onClose={onCancel}><form className="create-form note-create-form" onSubmit={submit}>
    <input className="create-title" name="title" required maxLength={300} autoFocus placeholder="Note title" aria-label="Note title" />
    <textarea className="create-description" name="description" rows={3} required maxLength={2000} placeholder={descriptionExamples.note} aria-label="Note description" aria-describedby="new-note-description-help" />
    <DescriptionHelp id="new-note-description-help" kind="note" />
    <Field label="Content"><textarea className="create-body" name="content" rows={14} required placeholder="Write plain text or Markdown…" aria-label="Note content" /></Field>
    {error && <p className="form-error">{error}</p>}
    <div className="create-footer"><Field label="Format"><select name="content_format" aria-label="Note content format" defaultValue="markdown"><option value="markdown">Markdown</option><option value="plain_text">Plain text</option></select></Field><div className="create-actions"><button type="button" className="ghost" onClick={onCancel}>Cancel</button><button className="primary" disabled={busy}>{busy ? "Creating…" : "Create note"}</button></div></div>
  </form></CreateModal>;
}

function NewSource({ onCancel, onCreated }: { onCancel: () => void; onCreated: (item: Source) => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); setBusy(true); setError(null); const data = new FormData(event.currentTarget);
    try {
      const publishedAt = optionalDate(data, "published_at");
      onCreated(await api.createSource({
        title: String(data.get("title")), description: String(data.get("description")), source_kind: String(data.get("source_kind")),
        canonical_uri: optional(data, "canonical_uri"), byline: optional(data, "byline"), publisher: optional(data, "publisher"),
        published_at: publishedAt, published_at_precision: publishedAt ? String(data.get("published_at_precision")) : undefined,
        last_accessed_at: optionalDate(data, "last_accessed_at"), original_language: optional(data, "original_language"),
        original_media_type: optional(data, "original_media_type"), original_artifact_reference: optional(data, "original_artifact_reference"),
        provenance: { source_type: "human", note: "Created in Centaur Context" },
      }));
    } catch (cause) { setError(message(cause)); setBusy(false); }
  };
  return <CreateModal title="New source" onClose={onCancel}><form className="create-form source-create-form" onSubmit={submit}>
    <input className="create-title" name="title" required maxLength={300} autoFocus placeholder="Source title" aria-label="Source title" />
    <textarea className="create-body" name="description" rows={3} required maxLength={2000} placeholder={descriptionExamples.source} aria-label="Source description" aria-describedby="new-source-description-help" />
    <DescriptionHelp id="new-source-description-help" kind="source" />
    <div className="source-fields">
      <Field label="Kind"><select name="source_kind" aria-label="Source kind">{sourceKinds.map((kind) => <option value={kind} key={kind}>{kind.replaceAll("_", " ")}</option>)}</select></Field>
      <Field label="Canonical URL"><input name="canonical_uri" type="url" maxLength={2048} placeholder="https://…" /></Field>
      <Field label="Byline"><input name="byline" maxLength={500} /></Field>
      <Field label="Publisher"><input name="publisher" maxLength={300} /></Field>
      <Field label="Published"><input name="published_at" type="datetime-local" /></Field>
      <Field label="Published precision"><select name="published_at_precision" defaultValue="instant"><option value="instant">Exact instant</option><option value="day">Day</option><option value="month">Month</option><option value="year">Year</option></select></Field>
      <Field label="Accessed"><input name="last_accessed_at" type="datetime-local" /></Field>
      <Field label="Original language"><input name="original_language" maxLength={35} placeholder="en" /></Field>
      <Field label="Original media type"><input name="original_media_type" maxLength={100} placeholder="text/html" /></Field>
      <Field label="Original artifact reference"><input name="original_artifact_reference" maxLength={1000} /></Field>
    </div>
    {error && <p className="form-error">{error}</p>}
    <div className="create-footer"><span className="property-chip">Source</span><div className="create-actions"><button type="button" className="ghost" onClick={onCancel}>Cancel</button><button className="primary" disabled={busy}>{busy ? "Creating…" : "Create source"}</button></div></div>
  </form></CreateModal>;
}

function SourceDetail({ id, objects, visuals, onChanged }: { id: string; objects: SharedObject[]; visuals: Map<string, ObjectVisual>; onChanged: () => Promise<void> }) {
  const [source, setSource] = useState<Source | null>(null);
  const [object, setObject] = useState<SharedObject | null>(null);
  const [connections, setConnections] = useState<Connection[]>([]);
  const [events, setEvents] = useState<ObjectEvent[]>([]);
  const [artifacts, setArtifacts] = useState<Artifact[]>([]);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(async () => {
    try {
      const [nextSource, nextObject, nextConnections, nextEvents, nextArtifacts] = await Promise.all([api.source(id), api.object(id), api.connections(id), api.events(id), api.artifacts(id)]);
      setSource(nextSource); setObject(nextObject); setConnections(nextConnections); setEvents(nextEvents); setArtifacts(nextArtifacts); setError(null);
    } catch (cause) { setError(message(cause)); }
  }, [id]);
  useEffect(() => { void load(); }, [load]);
  if (!source || !object) return <DetailLoading error={error} />;
  const save = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); const data = new FormData(event.currentTarget); setError(null);
    try {
      await api.updateSource(id, {
        expected_revision: source.revision, title: String(data.get("title")), description: String(data.get("description")), protected: source.protected,
      });
      await Promise.all([load(), onChanged()]);
    } catch (cause) { setError(conflictMessage(cause)); }
  };
  return <div className="record-page"><div className="record-primary">
    <form className="detail-form source-detail-form" onSubmit={save}>
      <div className="detail-heading"><ObjectId id={source.object_id} rowPill navigate /><input className="title-input" name="title" aria-label="Source title" defaultValue={source.title} key={`${source.object_id}-${source.revision}-title`} /></div>
      <section className="properties-block" aria-label="Source properties"><h2>Properties</h2><div className="properties-grid">
        <Property label="Kind"><ObjectTypeBadge kind="source" /> <span className="property-value-wrap">{source.source_kind.replaceAll("_", " ")}</span></Property>
        <Property label="Canonical URL">{source.canonical_uri ? <a href={source.canonical_uri} target="_blank" rel="noreferrer">{source.canonical_uri}</a> : "Not set"}</Property>
        <Property label="Byline"><span className="property-value-wrap">{source.byline ?? "Not set"}</span></Property>
        <Property label="Publisher"><span className="property-value-wrap">{source.publisher ?? "Not set"}</span></Property>
        <Property label="Published">{source.published_at ? `${new Date(source.published_at).toLocaleString()} (${source.published_at_precision})` : "Not set"}</Property>
        <Property label="Accessed">{source.last_accessed_at ? new Date(source.last_accessed_at).toLocaleString() : "Not set"}</Property>
        <Property label="Original language">{source.original_language ?? "Not set"}</Property>
        <Property label="Original media type">{source.original_media_type ?? "Not set"}</Property>
        <Property label="Original artifact reference"><span className="property-value-wrap">{source.original_artifact_reference ?? "Not set"}</span></Property>
      </div></section>
      <textarea className="body-input" name="description" required maxLength={2000} aria-label="Source description" aria-describedby="source-description-help" defaultValue={source.description} key={`${source.object_id}-${source.revision}-description`} rows={4} placeholder={descriptionExamples.source} />
      <DescriptionHelp id="source-description-help" kind="source" />
      <button className="secondary save-button">Save changes</button>
    </form>
    {error && <p className="form-error">{error}</p>}
    <Artifacts objectId={id} artifacts={artifacts} currentArtifactId={source.current_artifact_id} onCreated={load} />
    <Connections object={object} objects={objects} visuals={visuals} connections={connections} onCreated={load} />
    <ActivityTimeline events={events} visuals={visuals} />
    <Provenance value={source.provenance} />
  </div></div>;
}

function Artifacts({ objectId, artifacts, currentArtifactId, onCreated }: { objectId: string; artifacts: Artifact[]; currentArtifactId: string | null; onCreated: () => Promise<void> }) {
  const selectedDefault = currentArtifactId ?? artifacts[0]?.id ?? null;
  const [selectedId, setSelectedId] = useState<string | null>(selectedDefault);
  const [preview, setPreview] = useState<ArtifactWindow[]>([]);
  const [pasteOpen, setPasteOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [embeddingStatus, setEmbeddingStatus] = useState<EmbeddingStatus | null>(null);
  useEffect(() => { void api.embeddingStatus().then(setEmbeddingStatus).catch(() => setEmbeddingStatus(null)); }, [artifacts]);
  useEffect(() => { setSelectedId(selectedDefault); setPreview([]); }, [selectedDefault]);
  const read = async (offset: number) => {
    if (selectedId === null) return; setBusy(true); setError(null);
    try { const window = await api.artifactContent(selectedId, offset); setPreview((items) => offset === 0 ? [window] : [...items, window]); }
    catch (cause) { setError(message(cause)); }
    finally { setBusy(false); }
  };
  const append = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); setBusy(true); setError(null); const data = new FormData(event.currentTarget);
    try {
      const captureOutcome = String(data.get("capture_outcome"));
      await api.createArtifact(objectId, { kind: String(data.get("kind")), title: optional(data, "title"), content: String(data.get("text")), media_type: "text/plain", language: optional(data, "language"), capture_outcome: captureOutcome, capture_reason: captureOutcome === "complete" ? null : optional(data, "capture_reason"), expected_size_bytes: null, metadata: { source_type: "human_paste" }, supersedes_artifact_id: selectedId, captured_at: optionalDate(data, "captured_at") });
      setPasteOpen(false); setPreview([]); await onCreated();
    } catch (cause) { setError(message(cause)); }
    finally { setBusy(false); }
  };
  const nextOffset = preview.at(-1)?.next_offset ?? null;
  return <Section title="Artifacts" action={<button className="text-button" type="button" onClick={() => setPasteOpen((value) => !value)}>+ Add artifact</button>}>
    <p className="muted">{embeddingStatus?.configured ? `Semantic indexing: ${embeddingStatus.configuration?.model} (${embeddingStatus.configuration?.dimensions} dimensions)` : "Semantic indexing disabled; full-text search remains available."}</p>
    {pasteOpen && <form className="form source-content-form" onSubmit={append}>
      <div className="source-fields"><Field label="Kind"><input name="kind" required maxLength={100} defaultValue="transcript" /></Field><Field label="Title"><input name="title" maxLength={300} /></Field><Field label="Captured"><input name="captured_at" type="datetime-local" /></Field><Field label="Language"><input name="language" maxLength={35} placeholder="en" /></Field><Field label="Capture outcome"><select name="capture_outcome" defaultValue="complete"><option value="complete">Complete</option><option value="incomplete">Incomplete</option><option value="unavailable">Unavailable</option><option value="paywalled">Paywalled</option><option value="disallowed">Disallowed</option><option value="too_large">Too large</option><option value="unsupported">Unsupported</option></select></Field><Field label="Reason when not complete"><input name="capture_reason" maxLength={1000} /></Field></div>
      <Field label="Text"><textarea name="text" aria-label="Artifact text" rows={12} required placeholder="Paste a transcript or other supporting text…" /></Field>
      <div className="create-actions"><button type="button" className="ghost" onClick={() => setPasteOpen(false)}>Cancel</button><button className="secondary" disabled={busy}>{busy ? "Saving…" : "Save artifact"}</button></div>
    </form>}
    {artifacts.length > 0 ? <div className="content-preview">
      <div className="content-toolbar"><label>Artifact <select aria-label="Artifact" value={selectedId ?? ""} onChange={(event) => { setSelectedId(event.target.value); setPreview([]); }}>{artifacts.map((artifact) => <option value={artifact.id} key={artifact.id}>{artifact.title ?? artifact.kind}{artifact.id === currentArtifactId ? " · current" : ""}</option>)}</select></label>{preview.length === 0 && <button className="secondary" type="button" disabled={busy} onClick={() => void read(0)}>{busy ? "Loading…" : "Load preview"}</button>}</div>
      {selectedId !== null && <ArtifactSummary artifact={artifacts.find((item) => item.id === selectedId)} />}
      {preview.length > 0 && <pre className="source-text-preview" aria-label="Artifact content preview">{preview.map((item) => item.text).join("")}</pre>}
      {nextOffset !== null && <button className="secondary" type="button" disabled={busy} onClick={() => void read(nextOffset)}>{busy ? "Loading…" : "Load next 8,000 characters"}</button>}
    </div> : <p className="muted">No artifacts yet.</p>}
    {error && <p className="form-error">{error}</p>}
  </Section>;
}

function ArtifactSummary({ artifact }: { artifact: Artifact | undefined }) {
  if (!artifact) return null;
  return <p className="content-version-summary">{artifact.kind.replaceAll("_", " ")} · {artifact.capture_outcome} · {artifact.semantic_indexing_enabled ? "semantic indexing enabled" : "lexical only"} · {artifact.size_bytes.toLocaleString()} bytes · {artifact.language ?? "language unspecified"} · {relative(artifact.created_at)}{artifact.capture_reason ? ` · ${artifact.capture_reason}` : ""}</p>;
}

function NoteDetail({ id, objects, visuals, onChanged }: { id: string; objects: SharedObject[]; visuals: Map<string, ObjectVisual>; onChanged: () => Promise<void> }) {
  const [note, setNote] = useState<Note | null>(null);
  const [object, setObject] = useState<SharedObject | null>(null);
  const [connections, setConnections] = useState<Connection[]>([]);
  const [events, setEvents] = useState<ObjectEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(async () => {
    try {
      const [nextNote, nextObject, nextConnections, nextEvents] = await Promise.all([api.note(id), api.object(id), api.connections(id), api.events(id)]);
      setNote(nextNote); setObject(nextObject); setConnections(nextConnections); setEvents(nextEvents); setError(null);
    } catch (cause) { setError(message(cause)); }
  }, [id]);
  useEffect(() => { void load(); }, [load]);
  if (!note || !object) return <DetailLoading error={error} />;
  const save = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); const data = new FormData(event.currentTarget); setError(null);
    try {
      await api.updateNote(id, {
        expected_revision: note.revision,
        title: String(data.get("title")),
        description: String(data.get("description")),
        content: String(data.get("content")),
        content_format: String(data.get("content_format")),
        protected: note.protected,
      });
      await Promise.all([load(), onChanged()]);
    } catch (cause) { setError(conflictMessage(cause)); }
  };
  return <div className="record-page"><div className="record-primary">
    <form className="detail-form note-detail-form" onSubmit={save}>
      <div className="detail-heading"><ObjectId id={note.object_id} rowPill navigate /><input className="title-input" name="title" aria-label="Note title" defaultValue={note.title} key={`${note.object_id}-${note.revision}-title`} /><AttributionStack users={visuals.get(note.object_id)?.users ?? []} /></div>
      <section className="properties-block" aria-label="Note properties"><h2>Properties</h2><div className="properties-grid">
        <Property label="Type"><ObjectTypeBadge kind="note" /></Property>
        <Property label="Users">{(visuals.get(note.object_id)?.users.length ?? 0) > 0 ? <AttributionStack users={visuals.get(note.object_id)?.users ?? []} /> : "None"}</Property>
        <Property label="Format"><select name="content_format" aria-label="Note content format" defaultValue={note.content_format} key={`${note.object_id}-${note.revision}-format`}><option value="markdown">Markdown</option><option value="plain_text">Plain text</option></select></Property>
        <Property label="Updated">{relative(note.updated_at)}</Property>
      </div></section>
      <textarea className="body-input" name="description" required maxLength={2000} aria-label="Note description" defaultValue={note.description} key={`${note.object_id}-${note.revision}-description`} rows={3} />
      <Section title="Content"><textarea className="note-content note-content-editor" name="content" aria-label="Note content" required maxLength={100000} defaultValue={note.content} key={`${note.object_id}-${note.revision}-content`} rows={16} /></Section>
      <button className="secondary save-button">Save note</button>
    </form>
    {error && <p className="form-error">{error}</p>}
    <Connections object={object} objects={objects} visuals={visuals} connections={connections} onCreated={load} />
    <ActivityTimeline events={events} visuals={visuals} />
    <Provenance value={note.provenance} />
  </div></div>;
}

function ObjectDetail({ id, objects, visuals, onChanged }: { id: string; objects: SharedObject[]; visuals: Map<string, ObjectVisual>; onChanged: () => Promise<void> }) {
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
    try { setItem(await api.updateObject(id, { expected_revision: item.revision, title: String(data.get("title")), description: String(data.get("description")), protected: item.protected })); await Promise.all([load(), onChanged()]); }
    catch (cause) { setError(conflictMessage(cause)); }
  };
  return <div className="record-page">
    <div className="record-primary">
      <form className="detail-form" onSubmit={save}>
        <div className="detail-heading"><ObjectId id={item.id} rowPill navigate /><input className="title-input" name="title" aria-label="Object title" defaultValue={item.title} key={`${item.id}-${item.revision}-title`} /></div>
        <section className="properties-block" aria-label="Object properties">
          <h2>Properties</h2>
          <div className="properties-grid">
            <Property label="Type"><ObjectTypeBadge kind={item.kind} /></Property>
            <Property label="Source">{visuals.get(item.id)?.source_provider ? <SourceBadge provider={visuals.get(item.id)?.source_provider} /> : textValue(item.provenance.source_type, "Unspecified")}</Property>
            <Property label="Users">{(visuals.get(item.id)?.users.length ?? 0) > 0 ? <AttributionStack users={visuals.get(item.id)?.users ?? []} /> : "None"}</Property>
            <Property label="Created by"><span className="property-value-wrap">{item.created_by_type}:{item.created_by_id}</span></Property>
            <Property label="Updated">{relative(item.updated_at)}</Property>
          </div>
        </section>
        <textarea className="body-input" name="description" required maxLength={2000} aria-label="Object description" aria-describedby="object-description-help" defaultValue={item.description} key={`${item.id}-${item.revision}-description`} rows={4} placeholder={descriptionExamples[item.kind]} />
        <DescriptionHelp id="object-description-help" kind={item.kind} />
        <button className="secondary save-button">Save changes</button>
      </form>
      {error && <p className="form-error">{error}</p>}
      {item.kind === "user" && <UserIdentityPanel id={item.id} visual={visuals.get(item.id)} />}
      {item.kind === "chat" && <ChatTranscript id={item.id} visuals={visuals} />}
      <Connections object={item} objects={objects} visuals={visuals} connections={connections} onCreated={load} />
      <ActivityTimeline events={events} visuals={visuals} includeThread />
      <Provenance value={item.provenance} />
    </div>
  </div>;
}

function UserIdentityPanel({ id, visual }: { id: string; visual: ObjectVisual | undefined }) {
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
    {user && <div className="properties-block compact-properties"><div className="properties-grid"><Property label="Object ID"><ObjectId id={user.object_id} label={false} /><ObjectContext visual={visual} /></Property><Property label="User kind"><span className="user-kind-label">{user.user_kind === "agent" ? "Agent" : "Human"}</span></Property>{identities.map((identity) => <div className="identity" key={identity.id}><SourceBadge provider={identity.provider} /><strong>{identity.display_name ?? identity.provider_user_id}</strong><small>{identity.workspace_id || "Default workspace"} · {identity.provider_user_id}</small><ObjectId id={user.object_id} compact /></div>)}{identities.length === 0 && <p className="muted">No external identities.</p>}</div></div>}
  </Section>;
}

function ChatTranscript({ id, visuals }: { id: string; visuals: Map<string, ObjectVisual> }) {
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
      {messages.map((item) => <MessageRow item={item} visual={visuals.get(item.sender_user_object_id)} key={item.id} />)}
      {!error && messages.length === 0 && <p className="muted">No messages have been ingested.</p>}
    </div>
  </Section>;
}

function MessageRow({ item, visual }: { item: ChatMessage; visual: ObjectVisual | undefined }) {
  return <article className="chat-message"><ObjectContext visual={visual} /><strong>{item.sender_title}</strong><span className="message-kind">{item.sender_kind}</span><p title={item.content}>{item.content}</p><time>{relative(item.source_created_at)}</time></article>;
}

function ActivityTimeline({ events, visuals }: { events: ObjectEvent[]; visuals: Map<string, ObjectVisual>; includeThread?: boolean }) {
  return <Section title="Activity"><div className="timeline">{events.map((event) => <div className="event" key={event.id}><span className="event-dot" /><strong>{event.action.replaceAll("_", " ")}</strong><span className="event-actor">{event.actor_type}:{event.actor_id}</span>{event.target_type === "object" ? <><ObjectId id={event.target_id} linkPill /><ObjectContext visual={visuals.get(event.target_id)} /></> : <ConnectionId id={event.target_id} label={false} compact />}<time>{relative(event.created_at)}</time></div>)}</div></Section>;
}

function Connections({ object, objects, visuals, connections, onCreated }: { object: SharedObject; objects: SharedObject[]; visuals: Map<string, ObjectVisual>; connections: Connection[]; onCreated: () => Promise<void> }) {
  const [open, setOpen] = useState(false); const [error, setError] = useState<string | null>(null);
  const submit = async (event: FormEvent<HTMLFormElement>) => { event.preventDefault(); const data = new FormData(event.currentTarget);
    try { await api.createConnection({ source_object_id: object.id, kind: String(data.get("kind")), target_object_id: String(data.get("target")), description: String(data.get("description")), protected: data.get("protected") === "on", provenance: { source_type: "human" } }); setOpen(false); await onCreated(); }
    catch (cause) { setError(message(cause)); }
  };
  return <Section title="Relationships" action={<button className="text-button" onClick={() => setOpen((value) => !value)}>+ Connect</button>}>
    {open && <form className="connection-form" onSubmit={submit}><select name="kind">{connectionKinds.map((kind) => <option key={kind}>{kind}</option>)}</select><select name="target" required defaultValue=""><option value="" disabled>Target record</option>{objects.filter((item) => item.id !== object.id && item.kind !== "task").map((item) => <option value={item.id} key={item.id}>{item.title}</option>)}</select><input name="description" required placeholder="Explain the exact relationship…" /><label className="property-check"><input type="checkbox" name="protected" /> Protect from curator changes</label><button className="secondary">Add</button>{error && <p className="form-error">{error}</p>}</form>}
    <div className="connections">{connections.map((connection) => <article className="connection" key={connection.id}><ConnectionFlow connection={connection} objects={objects} visuals={visuals} /></article>)}{connections.length === 0 && <p className="muted">No explained relationships yet.</p>}</div>
  </Section>;
}

function ConnectionFlow({ connection, objects, visuals }: { connection: Connection; objects: SharedObject[]; visuals: Map<string, ObjectVisual> }) {
  const source = objects.find((item) => item.id === connection.source_object_id);
  const target = objects.find((item) => item.id === connection.target_object_id);
  const endpoint = (id: string, item: SharedObject | undefined, label: string) => <div className="connection-endpoint" aria-label={`${label} Object`}>
    <strong>{item?.title ?? shortId(id)}</strong>
    {item && <ObjectTypeBadge kind={item.kind} />}
    <ObjectId id={id} linkPill />
    <ObjectContext visual={visuals.get(id)} />
  </div>;
  return <div className="connection-flow">{endpoint(connection.source_object_id, source, "Source")}<span className="connection-arrow" aria-label="connects to">→</span>{endpoint(connection.target_object_id, target, "Target")}</div>;
}

function TaskDetail({ id, objects, visuals, onChanged }: { id: string; objects: SharedObject[]; visuals: Map<string, ObjectVisual>; onChanged: () => Promise<void> }) {
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
    const status = String(data.get("status"));
    const blockedReason = optional(data, "blocked_reason");
    const issueUrl = optional(data, "github_issue_url");
    const brief = optional(data, "brief_markdown");
    try { await api.updateTask(id, { expected_revision: task.revision, title: String(data.get("title")), description: String(data.get("description")), protected: task.protected, status, priority: String(data.get("priority")), agent_suitable: data.get("agent_suitable") === "on", blocked_reason: status === "blocked" ? blockedReason : undefined, clear_blocked_reason: status !== "blocked", github_issue_url: issueUrl, clear_github_issue_url: !issueUrl, brief_markdown: brief, clear_brief_markdown: !brief }); await Promise.all([load(), onChanged()]); }
    catch (cause) { setError(conflictMessage(cause)); }
  };
  return <div className="record-page">
    <div className="record-primary">
      <form className="detail-form" onSubmit={save}>
        <div className="detail-heading"><ObjectId id={task.object_id} rowPill navigate /><input className="title-input" name="title" aria-label="Task title" defaultValue={task.title} key={`${task.object_id}-${task.revision}-title`} /></div>
        <section className="properties-block" aria-label="Task properties">
          <h2>Properties</h2>
          <div className="properties-grid">
            <Property label="Type"><ObjectTypeBadge kind="task" /></Property>
            <Property label="Source">{visuals.get(task.object_id)?.source_provider ? <SourceBadge provider={visuals.get(task.object_id)?.source_provider} /> : textValue(task.provenance.source_type, "Unspecified")}</Property>
            <Property label="Users">{(visuals.get(task.object_id)?.users.length ?? 0) > 0 ? <AttributionStack users={visuals.get(task.object_id)?.users ?? []} /> : "None"}</Property>
            <Field label="Status"><select name="status" defaultValue={task.status} key={`${task.object_id}-${task.revision}-status`}>{taskStatuses.map((status) => <option key={status}>{status}</option>)}</select></Field>
            <Property label="Agent suitability"><label className="check"><input type="checkbox" name="agent_suitable" defaultChecked={task.agent_suitable} key={`${task.object_id}-${task.revision}-suitable`} /> Suitable</label></Property>
            <Property label="Priority"><select name="priority" defaultValue={task.priority} key={`${task.object_id}-${task.revision}-priority`}><option value="low">Low</option><option value="medium">Medium</option><option value="high">High</option></select></Property>
            <Property label="Owner">{task.owner_object_id ? <><ObjectId id={task.owner_object_id} /><ObjectContext visual={visuals.get(task.owner_object_id)} /></> : "Unassigned"}</Property>
            <Property label="Due">{task.due_at ? new Date(task.due_at).toLocaleString() : "No due date"}</Property>
            <Property label="Completed">{task.completed_at ? new Date(task.completed_at).toLocaleString() : "Not complete"}</Property>
            <Field label="Blocked reason"><input name="blocked_reason" maxLength={2000} defaultValue={task.blocked_reason ?? ""} key={`${task.object_id}-${task.revision}-blocked`} /></Field>
            <Field label="GitHub issue"><input name="github_issue_url" type="url" maxLength={2000} defaultValue={task.github_issue_url ?? ""} key={`${task.object_id}-${task.revision}-issue`} placeholder="https://github.com/owner/repo/issues/123" /></Field>
            <Property label="Updated">{relative(task.updated_at)}</Property>
          </div>
        </section>
        <textarea className="body-input" name="description" required maxLength={2000} aria-label="Task description" aria-describedby="task-description-help" defaultValue={task.description} key={`${task.object_id}-${task.revision}-description`} rows={4} placeholder={descriptionExamples.task} />
        <Field label="Requirement brief"><textarea name="brief_markdown" maxLength={100000} defaultValue={task.brief_markdown ?? ""} key={`${task.object_id}-${task.revision}-brief`} rows={10} placeholder="Scope, constraints, acceptance criteria, and verification…" /></Field>
        <DescriptionHelp id="task-description-help" kind="task" />
        <button className="secondary save-button">Save changes</button>
      </form>
      {error && <p className="form-error">{error}</p>}
      <Connections object={object} objects={objects} visuals={visuals} connections={connections} onCreated={load} />
      <ActivityTimeline events={events} visuals={visuals} />
      <Provenance value={task.provenance} />
    </div>
  </div>;
}

function RunDetailView({ id, onChanged }: { id: string; onChanged: () => Promise<void> }) {
  const [detail, setDetail] = useState<RunDetail | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(async () => {
    try { setDetail(await api.run(id)); setError(null); }
    catch (cause) { setError(message(cause)); }
  }, [id]);
  useEffect(() => { void load(); }, [load]);
  if (!detail) return <DetailLoading error={error} />;
  const { run, objects, events } = detail;
  const save = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); setBusy(true); setError(null);
    const data = new FormData(event.currentTarget);
    try {
      const revision = Number((run.result.review_revision as number | undefined) ?? 0);
      await api.reviewRun(id, { verdict: String(data.get("verdict")) as RunVerdict, notes: optional(data, "notes"), expected_revision: revision });
      await load();
    } catch (cause) { setError(conflictMessage(cause)); }
    finally { setBusy(false); }
  };
  const undo = async () => {
    if (!window.confirm("Create a compensating run that reverses this run’s durable mutations?")) return;
    setBusy(true); setError(null);
    try { await api.undoRun(id); await Promise.all([load(), onChanged()]); }
    catch (cause) { setError(conflictMessage(cause)); }
    finally { setBusy(false); }
  };
  return <div className="record-page"><div className="record-primary eval-detail">
    <h1 className="detail-title">{run.kind.replaceAll("_", " ")} run</h1>
    <p className="detail-description">{run.actor_type}:{run.actor_id}</p>
    <section className="properties-block" aria-label="Run properties"><h2>Properties</h2><div className="properties-grid">
      <Property label="Status"><StateBadge state={run.status} /></Property><Property label="Verdict"><span className={`eval-verdict ${run.verdict}`}>{run.verdict}</span></Property>
      <Property label="Created">{new Date(run.created_at).toLocaleString()}</Property><Property label="Parent">{run.parent_run_id ? <a href={detailPath("runs", run.parent_run_id)}>{shortId(run.parent_run_id)}</a> : "None"}</Property>
      {run.chat_object_id && <Property label="Chat"><ObjectId id={run.chat_object_id} /></Property>}<Property label="Consulted Objects">{run.consulted_object_ids.length}</Property><Property label="Mutations">{events.length}</Property>
    </div></section>
    {run.error && <p className="run-error">{run.error}</p>}{error && <p className="form-error">{error}</p>}
    <form className="eval-annotation" onSubmit={save}><label className="eval-review-field"><span>Verdict</span><select name="verdict" aria-label="Verdict" defaultValue={run.verdict}>{["unreviewed", "pass", "mixed", "fail"].map((value) => <option key={value}>{value}</option>)}</select></label><label className="eval-review-field eval-review-notes"><span>Review notes</span><input name="notes" aria-label="Review notes" maxLength={4000} defaultValue={run.review_notes ?? ""} /></label><button className="secondary" disabled={busy}>{busy ? "Saving…" : "Save"}</button></form>
    {events.some((item) => item.reversible) && <button className="danger-button" type="button" disabled={busy} onClick={() => void undo()}>{busy ? "Creating reversal…" : "Undo with compensating run"}</button>}
    <Section title="Trace"><div className="trace-list">{run.trace.map((entry, index) => <article className="trace-entry" key={index}><span>{index + 1}</span><strong>{textValue(entry.type, "step").replaceAll("_", " ")}</strong><code title={JSON.stringify(entry)}>{JSON.stringify(entry)}</code></article>)}</div></Section>
    <Section title="Result"><pre className="source-text-preview">{JSON.stringify(run.result, null, 2)}</pre></Section>
    <Section title="Related Objects"><div className="eval-objects">{objects.map((object) => <ObjectId id={object.object_id} linkPill key={`${object.object_id}-${object.role}`} />)}</div></Section>
    <Section title="Durable mutations"><div className="change-list">{events.map((item) => <article className="change" key={item.id}><span className="event-dot" /><div><strong>{item.action} {item.target_type}</strong><p>revision {item.from_revision ?? "new"} → {item.to_revision}</p>{item.target_type === "object" ? <ObjectId id={item.target_id} compact /> : <ConnectionId id={item.target_id} />}</div></article>)}</div></Section>
  </div></div>;
}

function ConnectionDetail({ id, objects, visuals }: { id: string; objects: SharedObject[]; visuals: Map<string, ObjectVisual> }) {
  const [connection, setConnection] = useState<Connection | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    void api.connection(id)
      .then((item) => { if (active) setConnection(item); })
      .catch((cause) => { if (active) setError(message(cause)); });
    return () => { active = false; };
  }, [id]);
  if (!connection) return <DetailLoading error={error} />;
  return <div className="record-page"><div className="record-primary">
    <h1 className="detail-title">{connection.kind.replaceAll("_", " ")}</h1>
    <p className="detail-description">{connection.description}</p>
    <ConnectionFlow connection={connection} objects={objects} visuals={visuals} />
    <section className="properties-block connection-properties" aria-label="Connection properties"><h2>Properties</h2><div className="properties-grid">
      <Property label="Connection ID"><ConnectionId id={connection.id} label={false} /></Property>
      <Property label="Connection type">{connection.kind.replaceAll("_", " ")}</Property>
      <Property label="Revision">{connection.revision}</Property>
      <Property label="Protected">{connection.protected ? "Yes" : "No"}</Property>
      <Property label="Updated">{relative(connection.updated_at)}</Property>
    </div></section>
    <Provenance value={connection.provenance} />
  </div></div>;
}

function Provenance({ value }: { value: Record<string, unknown> }) {
  const messageIds = stringList(value.supporting_message_ids);
  const sourceChatId = textValue(value.chat_object_id);
  const fields = [
    ["Source", textValue(value.source_type, "Unspecified")],
    ["Source reference", textValue(value.source_ref)],
    ["Curator Run", textValue(value.curator_run_id)],
    ["Model", textValue(value.model)],
    ["Prompt", textValue(value.prompt_version)],
  ].filter(([, fieldValue]) => Boolean(fieldValue));
  return <details className="provenance"><summary><span>Provenance</span><span className="provenance-chevron" aria-hidden="true">›</span></summary><div className="properties-block compact-properties"><div className="properties-grid">{fields.map(([label, fieldValue]) => <Property label={label} key={label}><span className="property-value-wrap">{fieldValue}</span></Property>)}{sourceChatId && <Property label="Source Chat"><ObjectId id={sourceChatId} /></Property>}{messageIds.length > 0 && <Property label="Supporting messages"><span className="property-value-wrap">{messageIds.join(", ")}</span></Property>}</div></div></details>;
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
function DescriptionHelp({ id, kind }: { id: string; kind: ObjectKind }) { return <p className="description-help" id={id}>Describe this specific {kind} directly. Example: “{descriptionExamples[kind]}”</p>; }
function Property({ label, children }: { label: string; children: React.ReactNode }) { return <div className="property"><span>{label}</span><div>{children}</div></div>; }
function Section({ title, action, children }: { title: string; action?: React.ReactNode; children: React.ReactNode }) { return <section className="detail-section"><header><h3>{title}</h3>{action}</header>{children}</section>; }
function DetailLoading({ error }: { error: string | null }) { return <div className="blank-state"><span>◇</span><h2>{error ? "Could not load" : "Loading"}</h2><p>{error ?? "Reading the shared record…"}</p></div>; }
function relative(value: string) { const seconds = Math.round((Date.now() - new Date(value).getTime()) / 1000); if (seconds < 60) return "now"; if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`; if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`; return `${Math.floor(seconds / 86400)}d ago`; }
function message(cause: unknown) { return cause instanceof Error ? cause.message : "Something went wrong."; }
function conflictMessage(cause: unknown) { return cause instanceof ApiError && cause.status === 409 ? "This record changed elsewhere. Refresh before saving again." : message(cause); }
function finishCreate(section: Section, id: string, load: () => Promise<void>, setCreateOpen: (open: boolean) => void) { setCreateOpen(false); navigate(detailPath(section, id)); void load(); }
function shortId(value: string) { return value.length > 12 ? `${value.slice(0, 8)}…` : value; }
function textValue(value: unknown, fallback = "") { return typeof value === "string" && value.trim() ? value : fallback; }
function stringList(value: unknown) { return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : []; }
function optional(data: FormData, name: string) { const value = String(data.get(name) ?? "").trim(); return value || null; }
function optionalDate(data: FormData, name: string) { const value = optional(data, name); return value ? new Date(value).toISOString() : null; }

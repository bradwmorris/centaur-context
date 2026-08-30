import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { ApiError, api } from "./api";
import { DescriptionSnippet } from "./DescriptionSnippet";
import { ConnectionId, ObjectId } from "./ObjectIdentity";
import { AttributionStack, ObjectContext, ObjectTypeBadge, SourceBadge, StateBadge, TaskStatusBadge } from "./RecordVisuals";
import { SchemaWorkspace } from "./SchemaWorkspace";
import { detailPath, navigate, parseRoute, sectionPath } from "./routing";
import type { Section } from "./routing";
import type { ChatMessage, Connection, CuratorRun, CuratorRunDetail, EvalDetail, EvalSummary, EvalTraceEntry, EvalUsageSource, EvalVerdict, ExternalIdentity, Note, NoteSummary, ObjectEvent, ObjectKind, ObjectVisual, SharedObject, Source, SourceContentVersion, SourceContentWindow, SourceKind, Task, TaskStatus, Theme, ThemeProposal, User } from "./types";

const connectionKinds = ["involves", "about", "related_to", "depends_on", "derived_from", "themed"];
const taskStatuses: TaskStatus[] = ["todo", "doing", "blocked", "review", "done"];
const sectionLabels: Record<Section, string> = { objects: "Objects", tasks: "Tasks", chats: "Chats", users: "Users", entities: "Entities", memories: "Memories", sources: "Sources", notes: "Notes", themes: "Themes", curator: "Curator Runs", evals: "Evals", schema: "Schema" };
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
const sourceKinds: SourceKind[] = ["article", "paper", "podcast", "video", "book", "report", "document", "dataset", "web_page", "other"];

export default function App() {
  const [route, setRoute] = useState(() => parseRoute(window.location.pathname));
  const { section, selectedId, connectionId } = route;
  const [collapsed, setCollapsed] = useState(false);
  const [objects, setObjects] = useState<SharedObject[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [sources, setSources] = useState<Source[]>([]);
  const [notes, setNotes] = useState<NoteSummary[]>([]);
  const [themes, setThemes] = useState<Theme[]>([]);
  const [themeProposals, setThemeProposals] = useState<ThemeProposal[]>([]);
  const [curatorRuns, setCuratorRuns] = useState<CuratorRun[]>([]);
  const [visuals, setVisuals] = useState<ObjectVisual[]>([]);
  const [createOpen, setCreateOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (section === "schema") {
      setLoading(false);
      setError(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const [nextObjects, nextTasks, nextSources, nextNotes, nextThemes, nextThemeProposals, nextCuratorRuns, nextVisuals] = await Promise.all([api.objects(query), api.tasks(), api.sources(section === "sources" ? query : ""), api.notes(section === "notes" ? query : ""), section === "themes" ? api.themes() : Promise.resolve([]), section === "themes" ? api.themeProposals() : Promise.resolve([]), api.curatorRuns(), api.objectVisuals()]);
      setObjects(nextObjects);
      setTasks(nextTasks);
      setSources(nextSources.items);
      setNotes(nextNotes.items);
      setThemes(nextThemes);
      setThemeProposals(nextThemeProposals);
      setCuratorRuns(nextCuratorRuns);
      setVisuals(nextVisuals);
    } catch (cause) {
      setError(message(cause));
    } finally {
      setLoading(false);
    }
  }, [query, section]);

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

  const currentItems = itemsForSection(section, objects, tasks, sources, notes, themes, curatorRuns, query);
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
          <NavButton active={section === "tasks"} compact={collapsed} icon="✓" label="Tasks" onClick={() => selectSection("tasks")} />
          <NavButton active={section === "chats"} compact={collapsed} icon="◌" label="Chats" onClick={() => selectSection("chats")} />
          <NavButton active={section === "users"} compact={collapsed} icon="♙" label="Users" onClick={() => selectSection("users")} />
          <NavButton active={section === "entities"} compact={collapsed} icon="◎" label="Entities" onClick={() => selectSection("entities")} />
          <NavButton active={section === "memories"} compact={collapsed} icon="✦" label="Memories" onClick={() => selectSection("memories")} />
          <NavButton active={section === "sources"} compact={collapsed} icon="▤" label="Sources" onClick={() => selectSection("sources")} />
          <NavButton active={section === "notes"} compact={collapsed} icon="▱" label="Notes" onClick={() => selectSection("notes")} />
          <NavButton active={section === "themes"} compact={collapsed} icon="#" label="Themes" onClick={() => selectSection("themes")} />
          <NavButton active={section === "curator"} compact={collapsed} icon="↻" label="Curator Runs" onClick={() => selectSection("curator")} />
          <NavButton active={section === "evals"} compact={collapsed} icon="≋" label="Evals" onClick={() => selectSection("evals")} />
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
          {section === "schema" ? <SchemaWorkspace selectedTable={selectedId} /> : section === "evals" ? (selectedId ? <section className="detail-page"><EvalDetailView id={selectedId} /></section> : <EvalsList />) : !selectedId && !connectionId ? <section className="list-view" aria-label={`${section} records`}>
            <header className="list-view-head">
              <div className="title-with-action"><h1>{sectionLabel}</h1>{createSections.has(section) && <button className="add-icon" type="button" onClick={() => setCreateOpen(true)} aria-label={`New ${sectionSingular[section as keyof typeof sectionSingular]}`}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 3.25v9.5M3.25 8h9.5" /></svg></button>}</div>
            </header>
            <div className="list-toolbar">
              {section !== "tasks" && section !== "curator" && <label className="search"><svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="7" cy="7" r="4.25" /><path d="m10.25 10.25 3 3" /></svg><input aria-label={`Search ${sectionLabel.toLowerCase()}`} value={query} onChange={(event) => setQuery(event.target.value)} placeholder={`Search ${sectionLabel.toLowerCase()}`} /></label>}
              <span>{currentItems.length} {currentItems.length === 1 ? "record" : "records"}</span>
            </div>
            <div className="list-group-head"><span className="status-ring" /><strong>All {sectionLabel.toLowerCase()}</strong><span>{currentItems.length}</span></div>
            {section === "themes" && <ThemeProposalQueue proposals={themeProposals} onChanged={load} />}
            <div className="record-list">
              {currentItems.map((item) => (
                <div key={itemRouteId(item)} className="record">
                  <button className="record-open" onClick={() => navigate(detailPath(section, itemRouteId(item)))} aria-label={`Open ${itemTitle(item, objects)}`} />
                  <span className="record-source"><SourceBadge provider={visualsById.get(canonicalObjectId(item))?.source_provider} /></span>
                  <ObjectId id={canonicalObjectId(item)} rowPill />
                  <span className="record-title"><strong>{itemTitle(item, objects)}</strong><span className="record-badges"><ObjectTypeBadge kind={itemObjectKind(item)} />{"status" in item && ("trigger" in item ? <StateBadge state={item.status} /> : <TaskStatusBadge status={item.status} />)}</span><AttributionStack users={visualsById.get(canonicalObjectId(item))?.users ?? []} /><DescriptionSnippet description={itemDescription(item)} /></span>
                  <time>{relative("updated_at" in item ? item.updated_at : item.created_at)}</time>
                </div>
              ))}
              {!loading && currentItems.length === 0 && <div className="empty-list">Nothing here yet.</div>}
            </div>
          </section> : <section className="detail-page">
            {connectionId ? <ConnectionDetail id={connectionId} objects={objects} visuals={visualsById} /> : section === "tasks" ? <TaskDetail id={selectedId!} objects={objects} visuals={visualsById} onChanged={load} /> : section === "sources" ? <SourceDetail id={selectedId!} objects={objects} visuals={visualsById} onChanged={load} /> : section === "notes" ? <NoteDetail id={selectedId!} objects={objects} visuals={visualsById} onChanged={load} /> : section === "themes" ? <ThemeDetail id={selectedId!} objects={objects} visuals={visualsById} /> : section === "curator" ? <CuratorDetail id={selectedId!} objects={objects} visuals={visualsById} onChanged={load} /> : <ObjectDetail id={selectedId!} objects={objects} visuals={visualsById} onChanged={load} />}
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

type ListItem = SharedObject | Task | Source | NoteSummary | Theme | CuratorRun;

function itemsForSection(section: Section, objects: SharedObject[], tasks: Task[], sources: Source[], notes: NoteSummary[], themes: Theme[], curatorRuns: CuratorRun[], query: string): ListItem[] {
  if (section === "schema") return [];
  if (section === "tasks") return tasks;
  if (section === "sources") return sources;
  if (section === "notes") return notes;
  if (section === "themes") {
    const normalized = query.trim().toLocaleLowerCase();
    return normalized ? themes.filter((item) => `${item.title} ${item.slug} ${item.description}`.toLocaleLowerCase().includes(normalized)) : themes;
  }
  if (section === "curator") return curatorRuns;
  if (section === "objects") return objects;
  if (section === "evals") return [];
  const kind = sectionKinds[section];
  return objects.filter((item) => item.kind === kind);
}

function itemRouteId(item: ListItem) { return "trigger" in item ? item.id : canonicalObjectId(item); }
function canonicalObjectId(item: ListItem) { return "trigger" in item ? item.chat_object_id : "object_id" in item ? item.object_id : item.id; }

function itemObjectKind(item: ListItem): ObjectKind { return "kind" in item ? item.kind : "trigger" in item ? "chat" : "slug" in item ? "theme" : "source_kind" in item ? "source" : "content_format" in item ? "note" : "task"; }
function itemTitle(item: ListItem, objects: SharedObject[]) { return "title" in item ? item.title : `Chat · ${objects.find((object) => object.id === item.chat_object_id)?.title ?? shortId(item.chat_object_id)}`; }
function itemDescription(item: ListItem) { return "description" in item ? item.description : `${item.trigger.replace("_", " ")} · ${item.message_count} message${item.message_count === 1 ? "" : "s"}`; }
function isCreateSection(section: Section): section is CreateSection { return createSections.has(section); }
function fixedCreateKind(section: CreateSection): "chat" | "entity" | "memory" | undefined { return section === "chats" ? "chat" : section === "entities" ? "entity" : section === "memories" ? "memory" : undefined; }

function NavButton({ active, compact, icon, label, onClick }: { active: boolean; compact: boolean; icon: string; label: string; onClick: () => void }) {
  return <button className={active ? "nav-button active" : "nav-button"} onClick={onClick} aria-label={label} aria-current={active ? "page" : undefined} title={compact ? label : undefined}><span aria-hidden="true">{icon}</span>{!compact && label}</button>;
}

function ThemeProposalQueue({ proposals, onChanged }: { proposals: ThemeProposal[]; onChanged: () => Promise<void> }) {
  if (proposals.length === 0) return null;
  return <Section title={`Pending proposals (${proposals.length})`}>
    <div className="change-list">{proposals.map((proposal) => <ThemeProposalCard proposal={proposal} onChanged={onChanged} key={proposal.id} />)}</div>
  </Section>;
}

function ThemeProposalCard({ proposal, onChanged }: { proposal: ThemeProposal; onChanged: () => Promise<void> }) {
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const decide = async (decision: "approve" | "reject") => {
    if (!reason.trim()) { setError("A decision reason is required."); return; }
    setBusy(true); setError(null);
    try {
      if (decision === "approve") await api.approveThemeProposal(proposal.id, reason.trim());
      else await api.rejectThemeProposal(proposal.id, reason.trim());
      await onChanged();
    } catch (cause) { setError(message(cause)); }
    finally { setBusy(false); }
  };
  return <article className="change"><span className="event-dot" /><div><strong>{proposal.title}</strong><p><code>{proposal.slug}</code> · {proposal.description}</p><small>{proposal.rationale} · proposed by {proposal.proposed_by_id}</small><input value={reason} onChange={(event) => setReason(event.target.value)} maxLength={1000} placeholder="Decision reason" aria-label={`Decision reason for ${proposal.title}`} />{error && <p className="form-error">{error}</p>}</div><span><button className="secondary" type="button" disabled={busy} onClick={() => void decide("approve")}>Approve</button><button className="text-button" type="button" disabled={busy} onClick={() => void decide("reject")}>Reject</button></span></article>;
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
  return <CreateModal title="New theme" onClose={onCancel}><form className="create-form" onSubmit={submit}><input className="create-title" name="title" required maxLength={300} autoFocus placeholder="Theme title" aria-label="Theme title" /><Field label="Slug"><input name="slug" required maxLength={100} pattern="[a-z0-9]+(-[a-z0-9]+)*" placeholder="research-vertical" /></Field><textarea className="create-body" name="description" rows={5} required maxLength={1000} placeholder={descriptionExamples.theme} aria-label="Theme description" /><DescriptionHelp id="new-theme-description-help" kind="theme" />{error && <p className="form-error">{error}</p>}<div className="modal-actions"><button type="button" className="text-button" onClick={onCancel}>Cancel</button><button disabled={busy}>{busy ? "Creating…" : "Create approved theme"}</button></div></form></CreateModal>;
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
      }));
    } catch (cause) { setError(message(cause)); setBusy(false); }
  };
  const name = label.charAt(0).toUpperCase() + label.slice(1);
  return <CreateModal title={`New ${label}`} onClose={onCancel}><form className="create-form" onSubmit={submit}>
    <input className="create-title" name="title" required maxLength={300} autoFocus placeholder={`${name} title`} aria-label={`${name} title`} />
    <textarea className="create-body" name="description" rows={5} required maxLength={1000} placeholder={descriptionExamples[kind]} aria-label={`${name} description`} aria-describedby="new-object-description-help" />
    <DescriptionHelp id="new-object-description-help" kind={kind} />
    {error && <p className="form-error">{error}</p>}
    <div className="create-footer">{fixedKind ? <span className="property-chip">{name}</span> : <Field label="Type"><select name="kind" value={kind} onChange={(event) => setKind(event.target.value as typeof kind)}><option value="memory">Memory</option><option value="entity">Entity</option><option value="chat">Chat</option></select></Field>}<div className="create-actions"><button type="button" className="ghost" onClick={onCancel}>Cancel</button><button className="primary" disabled={busy}>{busy ? "Creating…" : `Create ${label}`}</button></div></div>
  </form></CreateModal>;
}

function NewTask({ onCancel, onCreated }: { onCancel: () => void; onCreated: (item: Task) => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); setBusy(true); setError(null); const data = new FormData(event.currentTarget);
    try { onCreated(await api.createTask({ title: String(data.get("title")), description: String(data.get("description")), status: "todo", priority: "medium", agent_eligible: data.get("agent_eligible") === "on", provenance: { source_type: "human", note: "Created in Centaur Context" } })); }
    catch (cause) { setError(message(cause)); setBusy(false); }
  };
  return <CreateModal title="New task" onClose={onCancel}><form className="create-form" onSubmit={submit}>
    <input className="create-title" name="title" required maxLength={300} autoFocus placeholder="Task title" aria-label="Task title" />
    <textarea className="create-body" name="description" rows={5} required maxLength={1000} placeholder={descriptionExamples.task} aria-label="Task description" aria-describedby="new-task-description-help" />
    <DescriptionHelp id="new-task-description-help" kind="task" />
    {error && <p className="form-error">{error}</p>}
    <div className="create-footer"><label className="property-chip"><input type="checkbox" name="agent_eligible" /> Agent eligible</label><div className="create-actions"><button type="button" className="ghost" onClick={onCancel}>Cancel</button><button className="primary" disabled={busy}>{busy ? "Creating…" : "Create task"}</button></div></div>
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
    <textarea className="create-description" name="description" rows={3} required maxLength={1000} placeholder={descriptionExamples.note} aria-label="Note description" aria-describedby="new-note-description-help" />
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
      onCreated(await api.createSource({
        title: String(data.get("title")), description: String(data.get("description")), source_kind: String(data.get("source_kind")),
        canonical_uri: optional(data, "canonical_uri"), byline: optional(data, "byline"), publisher: optional(data, "publisher"),
        published_at: optionalDate(data, "published_at"), accessed_at: optionalDate(data, "accessed_at"), language: optional(data, "language"),
        media_type: optional(data, "media_type"), artifact_reference: optional(data, "artifact_reference"), content_hash: optional(data, "content_hash"),
        provenance: { source_type: "human", note: "Created in Centaur Context" },
      }));
    } catch (cause) { setError(message(cause)); setBusy(false); }
  };
  return <CreateModal title="New source" onClose={onCancel}><form className="create-form source-create-form" onSubmit={submit}>
    <input className="create-title" name="title" required maxLength={300} autoFocus placeholder="Source title" aria-label="Source title" />
    <textarea className="create-body" name="description" rows={3} required maxLength={1000} placeholder={descriptionExamples.source} aria-label="Source description" aria-describedby="new-source-description-help" />
    <DescriptionHelp id="new-source-description-help" kind="source" />
    <div className="source-fields">
      <Field label="Kind"><select name="source_kind" aria-label="Source kind">{sourceKinds.map((kind) => <option value={kind} key={kind}>{kind.replaceAll("_", " ")}</option>)}</select></Field>
      <Field label="Canonical URL"><input name="canonical_uri" type="url" maxLength={2048} placeholder="https://…" /></Field>
      <Field label="Byline"><input name="byline" maxLength={500} /></Field>
      <Field label="Publisher"><input name="publisher" maxLength={300} /></Field>
      <Field label="Published"><input name="published_at" type="datetime-local" /></Field>
      <Field label="Accessed"><input name="accessed_at" type="datetime-local" /></Field>
      <Field label="Language"><input name="language" maxLength={35} placeholder="en" /></Field>
      <Field label="Media type"><input name="media_type" maxLength={100} placeholder="text/html" /></Field>
      <Field label="Artifact reference"><input name="artifact_reference" maxLength={1000} /></Field>
      <Field label="Content hash"><input name="content_hash" maxLength={64} pattern="[0-9a-f]{64}" placeholder="64-character lowercase SHA-256" /></Field>
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
  const [versions, setVersions] = useState<SourceContentVersion[]>([]);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(async () => {
    try {
      const [nextSource, nextObject, nextConnections, nextEvents, nextVersions] = await Promise.all([api.source(id), api.object(id), api.connections(id), api.events(id), api.sourceContents(id)]);
      setSource(nextSource); setObject(nextObject); setConnections(nextConnections); setEvents(nextEvents); setVersions(nextVersions); setError(null);
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
        <Property label="Published">{source.published_at ? new Date(source.published_at).toLocaleString() : "Not set"}</Property>
        <Property label="Accessed">{source.accessed_at ? new Date(source.accessed_at).toLocaleString() : "Not set"}</Property>
        <Property label="Language">{source.language ?? "Not set"}</Property>
        <Property label="Media type">{source.media_type ?? "Not set"}</Property>
        <Property label="Artifact reference"><span className="property-value-wrap">{source.artifact_reference ?? "Not set"}</span></Property>
        <Property label="Content hash"><span className="property-value-wrap">{source.content_hash ?? "Not set"}</span></Property>
      </div></section>
      <textarea className="body-input" name="description" required maxLength={1000} aria-label="Source description" aria-describedby="source-description-help" defaultValue={source.description} key={`${source.object_id}-${source.revision}-description`} rows={4} placeholder={descriptionExamples.source} />
      <DescriptionHelp id="source-description-help" kind="source" />
      <button className="secondary save-button">Save changes</button>
    </form>
    {error && <p className="form-error">{error}</p>}
    <SourceContents sourceId={id} sourceRevision={source.revision} versions={versions} currentContentId={source.current_content_id} onCreated={load} />
    <Connections object={object} objects={objects} visuals={visuals} connections={connections} onCreated={load} />
    <ActivityTimeline events={events} visuals={visuals} />
    <Provenance value={source.provenance} />
  </div></div>;
}

function SourceContents({ sourceId, sourceRevision, versions, currentContentId, onCreated }: { sourceId: string; sourceRevision: number; versions: SourceContentVersion[]; currentContentId: string | null; onCreated: () => Promise<void> }) {
  const currentVersion = versions.find((item) => item.id === currentContentId)?.version ?? versions[0]?.version ?? null;
  const [selectedVersion, setSelectedVersion] = useState<number | null>(currentVersion);
  const [preview, setPreview] = useState<SourceContentWindow[]>([]);
  const [pasteOpen, setPasteOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { setSelectedVersion(currentVersion); setPreview([]); }, [currentContentId, currentVersion]);
  const read = async (offset: number) => {
    if (selectedVersion === null) return; setBusy(true); setError(null);
    try { const window = await api.sourceContent(sourceId, selectedVersion, offset); setPreview((items) => offset === 0 ? [window] : [...items, window]); }
    catch (cause) { setError(message(cause)); }
    finally { setBusy(false); }
  };
  const append = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); setBusy(true); setError(null); const data = new FormData(event.currentTarget);
    try {
      await api.createSourceContent(sourceId, { expected_revision: sourceRevision, content_kind: String(data.get("content_kind")), normalized_text: String(data.get("text")), language: optional(data, "language"), extraction_method: optional(data, "extraction_method"), extraction_version: optional(data, "extraction_version"), artifact_reference: optional(data, "artifact_reference") });
      setPasteOpen(false); setPreview([]); await onCreated();
    } catch (cause) { setError(message(cause)); }
    finally { setBusy(false); }
  };
  const nextOffset = preview.at(-1)?.next_offset ?? null;
  return <Section title="Content" action={<button className="text-button" type="button" onClick={() => setPasteOpen((value) => !value)}>+ Paste version</button>}>
    {pasteOpen && <form className="form source-content-form" onSubmit={append}>
      <div className="source-fields"><Field label="Content kind"><select name="content_kind" aria-label="Content kind"><option value="article_text">Article text</option><option value="transcript">Transcript</option><option value="paper_text">Paper text</option><option value="document_text">Document text</option><option value="dataset_description">Dataset description</option><option value="other">Other</option></select></Field><Field label="Language"><input name="language" maxLength={35} placeholder="en" /></Field><Field label="Extraction method"><input name="extraction_method" maxLength={200} defaultValue="human_paste" /></Field><Field label="Extraction version"><input name="extraction_version" maxLength={100} /></Field><Field label="Artifact reference"><input name="artifact_reference" maxLength={1000} /></Field></div>
      <Field label="Normalized text"><textarea name="text" aria-label="Normalized source text" rows={12} required placeholder="Paste the normalized article or transcript…" /></Field>
      <div className="create-actions"><button type="button" className="ghost" onClick={() => setPasteOpen(false)}>Cancel</button><button className="secondary" disabled={busy}>{busy ? "Saving…" : "Save new version"}</button></div>
    </form>}
    {versions.length > 0 ? <div className="content-preview">
      <div className="content-toolbar"><label>Version <select aria-label="Content version" value={selectedVersion ?? ""} onChange={(event) => { setSelectedVersion(Number(event.target.value)); setPreview([]); }}>{versions.map((version) => <option value={version.version} key={version.id}>v{version.version}{version.id === currentContentId ? " · current" : ""}</option>)}</select></label>{preview.length === 0 && <button className="secondary" type="button" disabled={busy} onClick={() => void read(0)}>{busy ? "Loading…" : "Load preview"}</button>}</div>
      {selectedVersion !== null && <SourceVersionSummary version={versions.find((item) => item.version === selectedVersion)} />}
      {preview.length > 0 && <pre className="source-text-preview" aria-label="Source content preview">{preview.map((item) => item.text).join("")}</pre>}
      {nextOffset !== null && <button className="secondary" type="button" disabled={busy} onClick={() => void read(nextOffset)}>{busy ? "Loading…" : "Load next 8,000 characters"}</button>}
    </div> : <p className="muted">No content versions yet. Metadata remains available without loading long-form text.</p>}
    {error && <p className="form-error">{error}</p>}
  </Section>;
}

function SourceVersionSummary({ version }: { version: SourceContentVersion | undefined }) {
  if (!version) return null;
  return <p className="content-version-summary">{version.content_kind.replaceAll("_", " ")} · {version.size_bytes.toLocaleString()} bytes · {version.language ?? "language unspecified"} · {relative(version.created_at)}</p>;
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
      <textarea className="body-input" name="description" required maxLength={1000} aria-label="Note description" defaultValue={note.description} key={`${note.object_id}-${note.revision}-description`} rows={3} />
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
        <textarea className="body-input" name="description" required maxLength={1000} aria-label="Object description" aria-describedby="object-description-help" defaultValue={item.description} key={`${item.id}-${item.revision}-description`} rows={4} placeholder={descriptionExamples[item.kind]} />
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
    {user && <div className="properties-block compact-properties"><div className="properties-grid"><Property label="Object ID"><ObjectId id={user.object_id} label={false} /><ObjectContext visual={visual} /></Property><Property label="User kind"><span className="user-kind-label">{user.user_kind === "agent" ? "Agent" : "Human"}</span></Property>{identities.map((identity) => <div className="identity" key={identity.id}><SourceBadge provider={identity.provider} /><strong>{identity.display_name ?? identity.provider_user_id}</strong><small>{identity.workspace_id || "Default workspace"} · {identity.provider_user_id}</small><ObjectId id={identity.user_object_id} compact /></div>)}{identities.length === 0 && <p className="muted">No external identities.</p>}</div></div>}
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

function ActivityTimeline({ events, visuals, includeThread = false }: { events: ObjectEvent[]; visuals: Map<string, ObjectVisual>; includeThread?: boolean }) {
  return <Section title="Activity"><div className="timeline">{events.map((event) => <div className="event" key={event.id}><span className="event-dot" /><strong>{event.action.replaceAll("_", " ")}</strong><span className="event-actor" title={`${event.actor_type}:${event.actor_id}${includeThread && event.centaur_thread_key ? ` · ${event.centaur_thread_key}` : ""}`}>{event.actor_type}:{event.actor_id}{includeThread && event.centaur_thread_key ? ` · ${event.centaur_thread_key}` : ""}</span><ObjectId id={event.object_id} linkPill /><ObjectContext visual={visuals.get(event.object_id)} />{event.entity_type === "connection" && <ConnectionId id={event.entity_id} label={false} compact />}<time>{relative(event.created_at)}</time></div>)}</div></Section>;
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
    try { await api.updateTask(id, { expected_revision: task.revision, title: String(data.get("title")), description: String(data.get("description")), protected: task.protected, status: String(data.get("status")), priority: String(data.get("priority")), agent_eligible: data.get("agent_eligible") === "on" }); await Promise.all([load(), onChanged()]); }
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
            <Property label="Agent access"><label className="check"><input type="checkbox" name="agent_eligible" defaultChecked={task.agent_eligible} key={`${task.object_id}-${task.revision}-eligible`} /> Eligible</label></Property>
            <Property label="Priority"><select name="priority" defaultValue={task.priority} key={`${task.object_id}-${task.revision}-priority`}><option value="low">Low</option><option value="medium">Medium</option><option value="high">High</option></select></Property>
            <Property label="Owner">{task.owner_object_id ? <><ObjectId id={task.owner_object_id} /><ObjectContext visual={visuals.get(task.owner_object_id)} /></> : "Unassigned"}</Property>
            <Property label="Due">{task.due_at ? new Date(task.due_at).toLocaleString() : "No due date"}</Property>
            <Property label="Updated">{relative(task.updated_at)}</Property>
          </div>
        </section>
        <textarea className="body-input" name="description" required maxLength={1000} aria-label="Task description" aria-describedby="task-description-help" defaultValue={task.description} key={`${task.object_id}-${task.revision}-description`} rows={4} placeholder={descriptionExamples.task} />
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

function CuratorDetail({ id, objects, visuals, onChanged }: { id: string; objects: SharedObject[]; visuals: Map<string, ObjectVisual>; onChanged: () => Promise<void> }) {
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
      <Property label="Status"><StateBadge state={run.status} /></Property>
      <Property label="Trigger">{run.trigger.replace("_", " ")}</Property>
      <Property label="Chat"><span>{chatTitle}</span><ObjectId id={run.chat_object_id} /><ObjectContext visual={visuals.get(run.chat_object_id)} /></Property>
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
    <Section title="Interaction window"><div className="chat-transcript">{messages.map((item) => <MessageRow item={item} visual={visuals.get(item.sender_user_object_id)} key={item.id} />)}</div></Section>
    <Section title="Graph changes"><div className="change-list">{changes.map((change) => <article className="change" key={change.id}><span className="event-dot" /><div><strong>{change.action} {change.entity_type}</strong><p>{textValue(change.after_state.title, shortId(change.entity_id))} · revision {change.after_revision}</p>{change.entity_type === "object" ? <><ObjectId id={change.entity_id} compact /><ObjectContext visual={visuals.get(change.entity_id)} /></> : <ConnectionId id={change.entity_id} />}{stringList((change.after_state.provenance as Record<string, unknown> | undefined)?.supporting_message_ids).length > 0 && <small>Messages · {stringList((change.after_state.provenance as Record<string, unknown>).supporting_message_ids).map(shortId).join(", ")}</small>}</div><span className={change.undone_at ? "change-state undone" : "change-state"}>{change.undone_at ? "Undone" : "Applied"}</span></article>)}{changes.length === 0 && <p className="muted">No graph changes have been committed.</p>}</div></Section>
  </div></div>;
}

function EvalsList() {
  const [items, setItems] = useState<EvalSummary[]>([]);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    setLoading(true);
    void api.evals().then((data) => { if (active) { setItems(data); setError(null); } })
      .catch((cause) => { if (active) setError(message(cause)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, []);
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const visibleItems = useMemo(() => normalizedQuery ? items.filter((item) => evalSearchText(item).includes(normalizedQuery)) : items, [items, normalizedQuery]);
  return <section className="list-view eval-list" aria-label="eval records">
    <header className="list-view-head"><h1>Evals</h1></header>
    <div className="list-toolbar">
      <label className="search"><svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="7" cy="7" r="4.25" /><path d="m10.25 10.25 3 3" /></svg><input aria-label="Search evals" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search evals" /></label>
      <span>{visibleItems.length} {visibleItems.length === 1 ? "record" : "records"}</span>
    </div>
    {error && <div className="error-banner">{error}</div>}
    <div className="list-group-head"><span className="status-ring" /><strong>All evals</strong><span>{visibleItems.length}</span></div>
    <div className="record-list eval-records">
      {visibleItems.map((item) => <div className="record eval-record" key={item.id}>
        <button className="record-open" onClick={() => navigate(detailPath("evals", item.id))} aria-label={`Open eval ${item.summary}`} />
        <span className="eval-row-mark" aria-hidden="true">≋</span>
        <EvalId id={item.id} />
        <span className="record-title"><strong>{item.summary}</strong><span className="record-badges"><span className={`eval-verdict ${item.verdict}`}>{item.verdict}</span><StateBadge state={item.status} /></span><DescriptionSnippet description={evalRowDescription(item)} /></span>
        <time>{relative(item.created_at)}</time>
      </div>)}
      {!loading && visibleItems.length === 0 && <div className="empty-list">No evals match this search.</div>}
      {loading && <div className="empty-list">Loading evals…</div>}
    </div>
  </section>;
}

function EvalId({ id }: { id: string }) {
  const path = detailPath("evals", id);
  return <span className="object-identity row-pill eval-id"><a className="object-id-pill" href={path} onClick={(event) => { event.preventDefault(); navigate(path); }} title={id} aria-label={`Open Eval ID ${id}`}>ID: {id.slice(0, 5)}</a></span>;
}

function evalSearchText(item: EvalSummary) {
  return [item.id, item.summary, item.kind, item.status, item.verdict, item.actor_type, item.actor_id, item.chat_object_id, item.curator_run_id, ...item.usage_sources.flatMap((source) => [source.component, source.provider, source.model_id, source.display_tier, source.execution_type, source.auth_mode, source.billing_mode])].filter(Boolean).join(" ").toLocaleLowerCase();
}

function evalRowDescription(item: EvalSummary) {
  return `${item.kind.replaceAll("_", " ")} · ${item.actor_type}:${item.actor_id} · ${item.total_tokens.toLocaleString()} tokens · ${item.affected_object_count} Objects · ${chargeLabel(item)}`;
}

function EvalDetailView({ id }: { id: string }) {
  const [detail, setDetail] = useState<EvalDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const load = useCallback(async () => {
    try { setDetail(await api.eval(id)); setError(null); }
    catch (cause) { setError(message(cause)); }
  }, [id]);
  useEffect(() => { void load(); }, [load]);
  if (!detail) return <DetailLoading error={error} />;
  const item = detail.eval;
  const save = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault(); setBusy(true); setError(null);
    const data = new FormData(event.currentTarget);
    try { await api.annotateEval(id, { verdict: String(data.get("verdict")) as EvalVerdict, notes: String(data.get("notes")).trim() || null, expected_revision: item.annotation_revision }); await load(); }
    catch (cause) { setError(conflictMessage(cause)); }
    finally { setBusy(false); }
  };
  return <div className="record-page"><div className="record-primary eval-detail">
    <h1 className="detail-title">{item.summary}</h1>
    <p className="detail-description">{item.kind.replaceAll("_", " ")} · {item.actor_type}:{item.actor_id}</p>
    <section className="properties-block" aria-label="Eval properties"><h2>Properties</h2><div className="properties-grid">
      <Property label="Status"><StateBadge state={item.status} /></Property><Property label="Verdict"><span className={`eval-verdict ${item.verdict}`}>{item.verdict}</span></Property>
      <Property label="Created">{new Date(item.created_at).toLocaleString()}</Property><Property label="Tokens">{item.total_tokens.toLocaleString()}</Property><Property label="Charge">{chargeLabel(item)}</Property><Property label="Affected Objects">{item.affected_object_count}</Property>
      {item.chat_object_id && <Property label="Chat"><ObjectId id={item.chat_object_id} /></Property>}{item.curator_run_id && <Property label="Curator Run"><a href={detailPath("curator", item.curator_run_id)}>{item.curator_run_id}</a></Property>}
    </div></section>
    {item.error_summary && <p className="run-error">{item.error_summary}</p>}{error && <p className="form-error">{error}</p>}
    <form className="eval-annotation" onSubmit={save}><label className="eval-review-field"><span>Verdict</span><select name="verdict" aria-label="Verdict" defaultValue={item.verdict} key={`${item.id}-${item.annotation_revision}-verdict`}>{["unreviewed", "pass", "mixed", "fail"].map((value) => <option key={value}>{value}</option>)}</select></label><label className="eval-review-field eval-review-notes"><span>Review notes</span><input name="notes" aria-label="Review notes" maxLength={4000} defaultValue={item.notes ?? ""} key={`${item.id}-${item.annotation_revision}-notes`} placeholder="Optional note" /></label><button className="secondary" disabled={busy}>{busy ? "Saving…" : "Save"}</button>{item.annotated_by && <small>Reviewed by {item.annotated_by}</small>}</form>
    <Section title="Usage and charge provenance"><div className="usage-detail">{item.usage_sources.map((source, index) => <span className="usage-badge" key={index}>{usageLabel(source)}</span>)}<p>{chargeLabel(item)}</p></div></Section>
    <Section title="Ordered trace"><div className="trace-list">{detail.trace.map((entry) => <article className={entry.entry_type === "failure" ? "trace-entry failure" : "trace-entry"} key={entry.id}><span>{entry.sequence}</span><strong>{entry.entry_type.replaceAll("_", " ")}</strong><small title={traceDescription(entry)}>{traceDescription(entry)}</small><code title={JSON.stringify(entry.facts)}>{JSON.stringify(entry.facts)}</code></article>)}</div></Section>
    <Section title="Related Objects"><div className="eval-objects">{detail.objects.map((object) => <ObjectId id={object.object_id} linkPill key={`${object.object_id}-${object.role}`} />)}</div></Section>
  </div></div>;
}

function usageLabel(source: EvalUsageSource) {
  const provider = source.provider === "openai" ? "OpenAI" : source.provider ?? "Unknown provider";
  const model = source.display_tier ?? source.model_id ?? "Unknown model";
  const execution = source.execution_type?.replaceAll("_", " ") ?? "Unknown execution";
  const auth = source.auth_mode?.replaceAll("_", " ") ?? "Unknown auth";
  return `${provider} · ${model} · ${execution} · ${auth}`;
}

function chargeLabel(item: EvalSummary) {
  const labels: string[] = [];
  if (item.chatgpt_credit_microunits !== null) labels.push(`${(item.chatgpt_credit_microunits / 1_000_000).toFixed(4)} ChatGPT credits; subscription per-trace USD unavailable`);
  else if (item.usage_sources.some((source) => source.billing_mode === "subscription_allowance")) labels.push("Included subscription usage; per-trace USD unavailable");
  if (item.estimated_micro_usd !== null) labels.push(`Metered API estimate $${(item.estimated_micro_usd / 1_000_000).toFixed(6)} USD`);
  return labels.join(" · ") || (item.usage_sources.length === 0 ? "Not applicable" : "Charge unavailable");
}

function traceChargeLabel(entry: EvalTraceEntry) {
  if (entry.chatgpt_credit_microunits !== null) return `${(entry.chatgpt_credit_microunits / 1_000_000).toFixed(4)} ChatGPT credits; per-trace USD unavailable`;
  if (entry.billing_mode === "subscription_allowance") {
    const equivalent = entry.api_equivalent_micro_usd === null ? "" : `; API-equivalent estimate $${(entry.api_equivalent_micro_usd / 1_000_000).toFixed(6)} USD`;
    return `included subscription usage; per-trace USD unavailable${equivalent}`;
  }
  if (entry.estimated_micro_usd !== null) return `estimated $${(entry.estimated_micro_usd / 1_000_000).toFixed(6)} USD${entry.rate_card_version ? ` (${entry.rate_card_version})` : ""}`;
  return "charge unavailable";
}

function traceDescription(entry: EvalTraceEntry) {
  const model = entry.model_id ? `${entry.provider ?? "Unknown provider"} · ${entry.model_id} · ${entry.execution_type ?? "unknown execution"}` : null;
  const usage = entry.usage_status === "not_applicable" ? null : `${entry.usage_status} · ${(entry.total_tokens ?? 0).toLocaleString()} tokens · ${traceChargeLabel(entry)}${entry.usage_missing_reason ? ` · ${entry.usage_missing_reason}` : ""}`;
  return [model, usage].filter(Boolean).join(" · ") || "No model usage";
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

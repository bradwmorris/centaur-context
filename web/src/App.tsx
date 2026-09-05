import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ApiError, api } from "./api";
import type { ListSort } from "./api";
import { DescriptionSnippet } from "./DescriptionSnippet";
import { ConnectionGraphWorkspace, FocusedObjectGraph } from "./ConnectionGraph";
import { ConnectionId, ObjectId } from "./ObjectIdentity";
import { AttributionStack, CompactKindBadge, ObjectContext, ObjectTypeBadge, SourceBadge, SourceSiteIcon, StateBadge, TaskStatusBadge } from "./RecordVisuals";
import { InlineEditor } from "./InlineEditor";
import { SchemaWorkspace } from "./SchemaWorkspace";
import { detailPath, navigate, parseRoute, sectionPath } from "./routing";
import type { Section } from "./routing";
import type { Artifact, ArtifactWindow, ChatMessage, Connection, ConnectionGraphSnapshot, EmbeddingStatus, ExternalIdentity, Note, NoteSummary, ObjectEvent, ObjectKind, ObjectVisual, Run, RunDetail, RunObject, RunVerdict, SharedObject, Source, SourceKind, Task, TaskStatus, Theme, User } from "./types";

const connectionKinds = ["involves", "about", "related_to", "depends_on", "derived_from", "themed"];
const taskStatuses: TaskStatus[] = ["backlog", "todo", "doing", "review", "done", "blocked"];
const sectionLabels: Record<Section, string> = { objects: "Objects", connections: "Connections", tasks: "Tasks", chats: "Chats", users: "Users", entities: "Entities", memories: "Memories", sources: "Sources", notes: "Notes", themes: "Themes", runs: "Runs", evals: "Evals", schema: "Schema" };
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
  const [sort, setSort] = useState<ListSort>("recent");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const [refreshState, setRefreshState] = useState<"idle" | "refreshing" | "done" | "error">("idle");
  const requestGeneration = useRef(0);

  const load = useCallback(async () => {
    const generation = ++requestGeneration.current;
    if (section === "schema" || (section === "connections" && !connectionId)) {
      setLoading(false);
      setError(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const objectKind = section in sectionKinds ? sectionKinds[section as keyof typeof sectionKinds] : undefined;
      const needsObjects = Boolean(selectedId || connectionId) || section === "objects" || section in sectionKinds || section === "runs" || section === "evals";
      const [nextObjects, nextTasks, nextSources, nextNotes, nextThemes, nextRuns, nextVisuals, densityGraph] = await Promise.all([
        needsObjects ? api.objects(selectedId || section === "runs" || section === "evals" ? "" : query, objectKind, sort) : Promise.resolve(null),
        section === "tasks" ? api.tasks(sort) : Promise.resolve(null),
        section === "sources" ? api.sources(selectedId ? "" : query, sort) : Promise.resolve(null),
        section === "notes" ? api.notes(selectedId ? "" : query, sort) : Promise.resolve(null),
        section === "themes" ? api.themes(sort) : Promise.resolve(null),
        (section === "runs" || section === "evals") && !selectedId ? section === "evals" ? api.evalRuns() : api.runs({ root_only: "true" }) : Promise.resolve(null),
        api.objectVisuals(),
        sort === "connections" && isObjectBackedSection(section) ? api.connectionGraph() : Promise.resolve(null),
      ]);
      if (generation !== requestGeneration.current) return;
      if (nextObjects) setObjects(sortWithGraphFallback(nextObjects, densityGraph));
      if (nextTasks) setTasks(sortWithGraphFallback(nextTasks, densityGraph));
      if (nextSources) setSources(sortWithGraphFallback(nextSources.items, densityGraph));
      if (nextNotes) setNotes(sortWithGraphFallback(nextNotes.items, densityGraph));
      if (nextThemes) setThemes(sortWithGraphFallback(nextThemes, densityGraph));
      if (nextRuns) setRuns(nextRuns);
      setVisuals(nextVisuals);
    } catch (cause) {
      if (generation !== requestGeneration.current) return;
      setError(message(cause));
    } finally {
      if (generation === requestGeneration.current) setLoading(false);
    }
  }, [query, section, selectedId, connectionId, sort, refreshKey]);

  useEffect(() => {
    const syncRoute = () => setRoute(parseRoute(window.location.pathname));
    window.addEventListener("popstate", syncRoute);
    if (window.location.pathname === "/") navigate(sectionPath("objects"), true);
    return () => window.removeEventListener("popstate", syncRoute);
  }, []);

  useEffect(() => {
    requestGeneration.current += 1;
  }, [section, selectedId, connectionId, query, sort, refreshKey]);

  useEffect(() => {
    const timeout = window.setTimeout(() => void load(), 150);
    return () => window.clearTimeout(timeout);
  }, [load]);

  useEffect(() => {
    if (refreshState !== "refreshing" || loading) return;
    setRefreshState(error ? "error" : "done");
    const timeout = window.setTimeout(() => setRefreshState("idle"), 1400);
    return () => window.clearTimeout(timeout);
  }, [error, loading, refreshState]);

  const selectSection = (next: Section) => {
    setCreateOpen(false);
    setQuery("");
    setSort("recent");
    navigate(sectionPath(next));
  };

  const refresh = async () => {
    if (refreshState === "refreshing") return;
    setRefreshState("refreshing");
    setLoading(true);
    setRefreshKey((value) => value + 1);
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
        <div className="nav-evals"><NavButton active={section === "evals"} compact={collapsed} icon="★" label="Evals" onClick={() => selectSection("evals")} /></div>
        <div className="nav-foot" title="Running locally"><span className="status-dot" />{!collapsed && "Local workspace"}</div>
      </aside>

      <section className="main-panel">
        <header className="topbar">
          <div className="page-path">
            <button className="path-root" onClick={() => navigate(sectionPath(section))}>{sectionLabel}</button>
            {(selectedId || connectionId) && <><span>›</span><strong>{connectionId ? `Connection ${shortId(connectionId)}` : section === "schema" ? selectedId : selectedItem ? itemTitle(selectedItem, objects) : shortId(selectedId ?? "")}</strong></>}
          </div>
          <button className="refresh-button" type="button" onClick={() => void refresh()} disabled={refreshState === "refreshing"} aria-label="Refresh current view">
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M13.2 5.5A5.5 5.5 0 1 0 13 11"/><path d="M13.2 2.5v3.2H10"/></svg><span>{refreshState === "refreshing" ? "Refreshing…" : refreshState === "done" ? "Updated" : refreshState === "error" ? "Retry refresh" : "Refresh"}</span>
          </button>
        </header>
        {error && <div className="error-banner">{error}<button onClick={() => setError(null)}>×</button></div>}

        <div className="workspace">
          {section === "schema" ? <SchemaWorkspace selectedTable={selectedId} refreshKey={refreshKey} /> : section === "connections" && !connectionId ? <ConnectionGraphWorkspace refreshKey={refreshKey} /> : !selectedId && !connectionId && section === "evals" ? <EvalsView runs={currentItems as Run[]} objects={objects} visuals={visualsById} query={query} onQuery={setQuery} loading={loading} onUpdated={(updated) => setRuns((current) => current.map((run) => run.id === updated.id ? updated : run))} /> : !selectedId && !connectionId ? <section className="list-view" aria-label={`${section} records`}>
            <header className="list-view-head">
              <div className="title-with-action"><h1>{sectionLabel}</h1>{createSections.has(section) && <button className="add-icon" type="button" onClick={() => setCreateOpen(true)} aria-label={`New ${sectionSingular[section as keyof typeof sectionSingular]}`}><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 3.25v9.5M3.25 8h9.5" /></svg></button>}</div>
            </header>
            <div className="list-toolbar">
              {section !== "tasks" && <label className="search"><svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="7" cy="7" r="4.25" /><path d="m10.25 10.25 3 3" /></svg><input aria-label={`Search ${sectionLabel.toLowerCase()}`} value={query} onChange={(event) => setQuery(event.target.value)} placeholder={`Search ${sectionLabel.toLowerCase()}`} /></label>}
              {isObjectBackedSection(section) && <label className="sort-control"><span className="sr-only">Sort {sectionLabel}</span><select aria-label={`Sort ${sectionLabel}`} value={sort} onChange={(event) => setSort(event.target.value as ListSort)}><option value="recent">Recently added</option><option value="connections">Most connected</option></select></label>}
              <span>{currentItems.length} {currentItems.length === 1 ? "record" : "records"}</span>
            </div>
            <div className="list-group-head"><span className="status-ring" /><strong>All {sectionLabel.toLowerCase()}</strong><span>{currentItems.length}</span></div>
            <div className="record-list">
              {currentItems.map((item) => (
                <div key={itemRouteId(item)} className="record">
                  <button className="record-open" onClick={() => navigate(detailPath(section, itemRouteId(item)))} aria-label={`Open ${itemTitle(item, objects)}`} />
                  <span className="record-kind">{"actor_type" in item ? <CompactKindBadge kind="run" label={runType(item, objects)} /> : <ObjectTypeBadge kind={itemObjectKind(item)} compact />}</span>
                  <span className="record-id">{"actor_type" in item ? <span className="object-id-pill">{shortId(item.id)}</span> : <ObjectId id={canonicalObjectId(item)} rowPill />}</span>
                  <span className="record-main">
                    <span className="record-title"><strong>{itemTitle(item, objects)}</strong>{"source_kind" in item && <SourceSiteIcon sourceKind={item.source_kind} canonicalUri={item.canonical_uri} />}{"actor_type" in item && <StateBadge state={item.status} />}{"status" in item && !('actor_type' in item) && <TaskStatusBadge status={item.status} />}</span>
                    <span className="record-source"><SourceBadge provider={visualsById.get(itemVisualObjectId(item))?.source_provider} /></span>
                    <span className="record-users"><AttributionStack users={visualsById.get(itemVisualObjectId(item))?.users ?? []} /></span>
                  </span>
                  <DescriptionSnippet description={itemDescription(item, objects)} />
                  <time>{relative(item.created_at)}</time>
                </div>
              ))}
              {!loading && currentItems.length === 0 && <div className="empty-list">Nothing here yet.</div>}
            </div>
          </section> : <section className="detail-page">
            {connectionId ? <ConnectionDetail id={connectionId} objects={objects} visuals={visualsById} refreshKey={refreshKey} /> : section === "tasks" ? <TaskDetail id={selectedId!} objects={objects} visuals={visualsById} onChanged={load} refreshKey={refreshKey} /> : section === "sources" ? <SourceDetail id={selectedId!} objects={objects} visuals={visualsById} onChanged={load} refreshKey={refreshKey} /> : section === "notes" ? <NoteDetail id={selectedId!} objects={objects} visuals={visualsById} onChanged={load} refreshKey={refreshKey} /> : section === "themes" ? <ThemeDetail id={selectedId!} objects={objects} visuals={visualsById} refreshKey={refreshKey} onChanged={load} /> : section === "runs" || section === "evals" ? <RunDetailView id={selectedId!} visuals={visualsById} onChanged={load} refreshKey={refreshKey} /> : <ObjectDetail id={selectedId!} objects={objects} visuals={visualsById} onChanged={load} refreshKey={refreshKey} />}
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

function sortWithGraphFallback<T extends Exclude<ListItem, Run>>(items: T[], graph: ConnectionGraphSnapshot | null): T[] {
  if (!graph) return items;
  const degree = new Map<string, number>();
  for (const edge of graph.edges) {
    degree.set(edge.source_object_id, (degree.get(edge.source_object_id) ?? 0) + 1);
    degree.set(edge.target_object_id, (degree.get(edge.target_object_id) ?? 0) + 1);
  }
  return [...items].sort((left, right) =>
    (degree.get(canonicalObjectId(right)) ?? 0) - (degree.get(canonicalObjectId(left)) ?? 0)
    || right.created_at.localeCompare(left.created_at)
    || canonicalObjectId(right).localeCompare(canonicalObjectId(left))
  );
}

function itemsForSection(section: Section, objects: SharedObject[], tasks: Task[], sources: Source[], notes: NoteSummary[], themes: Theme[], runs: Run[], query: string): ListItem[] {
  if (section === "schema" || section === "connections") return [];
  if (section === "tasks") return tasks;
  if (section === "sources") return sources;
  if (section === "notes") return notes;
  if (section === "themes") {
    const normalized = query.trim().toLocaleLowerCase();
    return normalized ? themes.filter((item) => `${item.title} ${item.slug} ${item.description}`.toLocaleLowerCase().includes(normalized)) : themes;
  }
  if (section === "runs" || section === "evals") {
    const normalized = query.trim().toLocaleLowerCase();
    const rootRuns = runs.filter((item) => item.parent_run_id === null);
    const filtered = normalized ? rootRuns.filter((item) => `${item.id} ${item.kind} ${item.status} ${item.actor_type} ${item.actor_id} ${itemTitle(item, objects)} ${runActualResult(item, objects)} ${runInput(item)} ${item.review_notes ?? ""}`.toLocaleLowerCase().includes(normalized)) : rootRuns;
    return section === "evals" ? [...filtered].sort((left, right) => Number(right.pinned) - Number(left.pinned) || right.created_at.localeCompare(left.created_at) || right.id.localeCompare(left.id)) : filtered;
  }
  if (section === "objects") return objects;
  const kind = sectionKinds[section];
  return objects.filter((item) => item.kind === kind);
}

function itemRouteId(item: ListItem) { return "actor_type" in item ? item.id : canonicalObjectId(item); }
function canonicalObjectId(item: ListItem) { return "actor_type" in item ? item.chat_object_id ?? item.id : "object_id" in item ? item.object_id : item.id; }
function itemVisualObjectId(item: ListItem) { return "actor_type" in item ? item.chat_object_id ?? item.primary_object_id ?? item.id : canonicalObjectId(item); }

function itemObjectKind(item: Exclude<ListItem, Run>): ObjectKind { return "kind" in item ? item.kind : "slug" in item ? "theme" : "source_kind" in item ? "source" : "content_format" in item ? "note" : "task"; }
function itemTitle(item: ListItem, objects: SharedObject[]) {
  if (!("actor_type" in item)) return "title" in item ? item.title : shortId(canonicalObjectId(item));
  const primary = item.primary_object_id ? objects.find((object) => object.id === item.primary_object_id) : undefined;
  if (primary) return `${runType(item, objects)} · ${primary.title}`;
  const interactionTitle = item.kind === "slack_interaction" ? textValue(item.input.title) : "";
  return interactionTitle ? `${runType(item, objects)} · ${interactionTitle}` : runType(item, objects);
}
function itemDescription(item: ListItem, objects: SharedObject[]) {
  if (!("actor_type" in item)) return "description" in item ? item.description : "";
  return runOutcome(item, objects);
}
function runType(run: Run, objects: SharedObject[] | RunObject[]) {
  const primary = run.primary_object_id ? objects.find((object) => ("id" in object ? object.id : object.object_id) === run.primary_object_id) : undefined;
  if (run.kind === "workflow" && run.input.workflow_name === "enyu_source_ingestion") return "Source ingestion";
  if (run.kind === "intake" && run.parent_run_id) return "Context commit";
  if (run.kind === "intake" && primary?.kind === "source") return "Source ingestion";
  if (run.kind === "intake") return "Data ingestion";
  if (run.kind === "slack_interaction" && primary) {
    const created = run.trace.some((entry) => entry.entry_type === "tool_call" && /create|add|write/i.test(textValue(entry.name)));
    const objectType = primary.kind.replace(/^./, (value) => value.toUpperCase());
    return created ? `${objectType} creation` : `${objectType} interaction`;
  }
  if (run.kind === "slack_interaction") return "Bot interaction";
  if (run.kind === "curator") return "Conversation curation";
  if (run.kind === "curator_undo") return "Curation reversal";
  if (run.kind === "external_action") return "External action";
  if (["human_mutation", "system_mutation", "mutation"].includes(run.kind)) return "Record change";
  if (run.kind === "legacy_import") return "Legacy import";
  return `${run.kind.replaceAll("_", " ").replace(/^./, (value) => value.toUpperCase())} run`;
}
function runOutcome(run: Run, objects: SharedObject[] | RunObject[]) {
  const counts = run.result.counts as Record<string, unknown> | undefined;
  const primary = run.primary_object_id ? objects.find((object) => ("id" in object ? object.id : object.object_id) === run.primary_object_id) : undefined;
  const objectCount = Number(counts?.objects ?? 0);
  const connectionCount = Number(counts?.connections ?? 0);
  if (run.kind === "intake" && primary) {
    const subject = primary.kind === "source" ? "Source" : "Object";
    const action = objectCount > 0 ? `Created ${objectCount} ${subject}${objectCount === 1 ? "" : "s"}` : `Reused ${subject}`;
    return connectionCount > 0 ? `${action} · Added ${connectionCount} connection${connectionCount === 1 ? "" : "s"}` : action;
  }
  if (run.kind === "workflow") return textValue(run.result.summary, run.status === "running" ? "Source ingestion is running." : "Workflow completed.");
  if (run.kind === "slack_interaction") return textValue(run.result.summary, run.status === "running" ? "The bot is working." : "The bot interaction finished.");
  if (run.error) return run.error;
  return `${run.actor_type}:${run.actor_id} · ${run.verdict}`;
}
function runInput(run: Run) {
  for (const value of [run.input.title, run.input.prompt, run.input.query, run.input.requested_title, run.input.source_locator, run.input.summary]) {
    const text = textValue(value);
    if (text) return text;
  }
  return Object.keys(run.input).length > 0 ? JSON.stringify(run.input).slice(0, 500) : "No input recorded";
}
function runActualResult(run: Run, objects: SharedObject[] | RunObject[]) {
  if (run.error) return run.error;
  if (run.kind === "intake") return runOutcome(run, objects);
  return textValue(run.result.summary, runOutcome(run, objects));
}
function isCreateSection(section: Section): section is CreateSection { return createSections.has(section); }
function isObjectBackedSection(section: Section) { return !["connections", "runs", "evals", "schema"].includes(section); }
function fixedCreateKind(section: CreateSection): "chat" | "entity" | "memory" | undefined { return section === "chats" ? "chat" : section === "entities" ? "entity" : section === "memories" ? "memory" : undefined; }

function useSerializedSave() {
  const chain = useRef<Promise<void>>(Promise.resolve());
  return useCallback(<T,>(operation: () => Promise<T>) => {
    const result = chain.current.then(operation, operation);
    chain.current = result.then(() => undefined, () => undefined);
    return result;
  }, []);
}

function NavButton({ active, compact, icon, label, onClick }: { active: boolean; compact: boolean; icon: string; label: string; onClick: () => void }) {
  return <button className={active ? "nav-button active" : "nav-button"} onClick={onClick} aria-label={label} aria-current={active ? "page" : undefined} title={compact ? label : undefined}><span aria-hidden="true">{icon}</span>{!compact && label}</button>;
}

function EvalsView({ runs, objects, visuals, query, onQuery, loading, onUpdated }: { runs: Run[]; objects: SharedObject[]; visuals: Map<string, ObjectVisual>; query: string; onQuery: (value: string) => void; loading: boolean; onUpdated: (run: Run) => void }) {
  return <section className="list-view evals-view" aria-label="evals records">
    <header className="list-view-head"><div className="title-with-action"><h1>Evals</h1></div></header>
    <div className="list-toolbar">
      <label className="search"><svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="7" cy="7" r="4.25" /><path d="m10.25 10.25 3 3" /></svg><input aria-label="Search evals" value={query} onChange={(event) => onQuery(event.target.value)} placeholder="Search evals" /></label>
      <span>{runs.length} {runs.length === 1 ? "run" : "runs"}</span>
    </div>
    <div className="eval-table-wrap">
      <table className="eval-table" aria-label="Eval runs">
        <thead><tr><th>Golden</th><th>Run</th><th>Users</th><th>Actual result</th><th>Verdict</th><th>Annotation</th><th>Date</th></tr></thead>
        <tbody>{runs.map((run) => <EvalRunRow run={run} objects={objects} visual={visuals.get(itemVisualObjectId(run))} onUpdated={onUpdated} key={run.id} />)}</tbody>
      </table>
      {!loading && runs.length === 0 && <div className="empty-list">Nothing here yet.</div>}
    </div>
  </section>;
}

function EvalRunRow({ run, objects, visual, onUpdated }: { run: Run; objects: SharedObject[]; visual: ObjectVisual | undefined; onUpdated: (run: Run) => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const latest = useRef(run);
  const serialize = useSerializedSave();
  useEffect(() => { latest.current = run; }, [run]);
  const save = (changes: { verdict?: RunVerdict; notes?: string | null; pinned?: boolean }) => serialize(async () => {
    const current = latest.current;
    const updated = await api.reviewRun(current.id, {
      verdict: changes.verdict ?? current.verdict,
      notes: changes.notes === undefined ? current.review_notes : changes.notes,
      pinned: changes.pinned ?? current.pinned,
      expected_revision: Number((current.result.review_revision as number | undefined) ?? 0),
    });
    latest.current = updated;
    onUpdated(updated);
    return updated;
  });
  const saveControl = async (changes: { verdict?: RunVerdict; pinned?: boolean }) => {
    setBusy(true); setError(null);
    try { await save(changes); }
    catch (cause) { setError(conflictMessage(cause)); }
    finally { setBusy(false); }
  };
  const reload = async () => {
    const next = (await api.run(run.id)).run;
    latest.current = next;
    onUpdated(next);
  };
  return <tr className={run.pinned ? "pinned" : undefined}>
    <td className="eval-pin-cell"><button type="button" className={run.pinned ? "eval-pin active" : "eval-pin"} disabled={busy} aria-label={run.pinned ? `Unpin ${shortId(run.id)} from golden evals` : `Pin ${shortId(run.id)} as a golden eval`} aria-pressed={run.pinned} onClick={() => void saveControl({ pinned: !run.pinned })}>{run.pinned ? "★" : "☆"}</button></td>
    <td><button type="button" className="eval-run-link" title={`Open ${itemTitle(run, objects)}`} onClick={() => navigate(detailPath("evals", run.id))}><strong>{itemTitle(run, objects)}</strong><span>{shortId(run.id)}</span><span className="eval-run-arrow" aria-hidden="true">›</span></button></td>
    <td className="eval-users-cell"><AttributionStack users={visual?.users ?? []} /></td>
    <td><span className="eval-cell-text" title={runActualResult(run, objects)}>{runActualResult(run, objects)}</span></td>
    <td><select aria-label={`Verdict for ${shortId(run.id)}`} value={run.verdict} disabled={busy} onChange={(event) => void saveControl({ verdict: event.target.value as RunVerdict })}><option value="unreviewed">Unreviewed</option><option value="pass">Pass</option><option value="mixed">Mixed</option><option value="fail">Fail</option></select>{error && <span className="eval-row-error">{error}</span>}</td>
    <td><InlineEditor label={`annotation for ${shortId(run.id)}`} value={run.review_notes ?? ""} multiline maxLength={4000} placeholder="Add what happened" className="eval-table-annotation" onSave={async (value) => (await save({ notes: value || null })).review_notes ?? ""} onReload={reload} /></td>
    <td><time dateTime={run.created_at}>{relative(run.created_at)}</time></td>
  </tr>;
}

function ThemeDetail({ id, objects, visuals, refreshKey, onChanged }: { id: string; objects: SharedObject[]; visuals: Map<string, ObjectVisual>; refreshKey: number; onChanged: () => Promise<void> }) {
  const [theme, setTheme] = useState<Theme | null>(null);
  const [assigned, setAssigned] = useState<SharedObject[]>([]);
  const [error, setError] = useState<string | null>(null);
  const revision = useRef(0);
  const loadGeneration = useRef(0);
  const serialize = useSerializedSave();
  const load = useCallback(async () => {
    const generation = ++loadGeneration.current;
    try { const [nextTheme, nextAssigned] = await Promise.all([api.theme(id), api.themeObjects(id)]); if (generation !== loadGeneration.current) return; revision.current = nextTheme.revision; setTheme(nextTheme); setAssigned(nextAssigned); setError(null); }
    catch (cause) { if (generation === loadGeneration.current) setError(message(cause)); }
  }, [id]);
  useEffect(() => { void load(); }, [load, refreshKey]);
  if (!theme) return <DetailLoading error={error} />;
  const themeObject = objects.find((item) => item.id === id);
  const saveObjectField = async (field: "title" | "description", value: string) => {
    return serialize(async () => {
      const updated = await api.updateObject(id, { expected_revision: revision.current, [field]: value });
      revision.current = updated.revision;
      setTheme((existing) => existing ? { ...existing, [field]: updated[field], revision: updated.revision, updated_at: updated.updated_at } : existing);
      await onChanged();
      return updated[field];
    });
  };
  return <div className="record-page"><div className="record-primary">
    <InlineEditor label="Theme title" value={theme.title} required maxLength={300} className="detail-title-editor" heading onSave={(value) => saveObjectField("title", value)} onReload={load} />
    <section className="properties-block" aria-label="Theme properties"><h2>Properties</h2><div className="properties-grid"><Property label="Object ID"><ObjectId id={theme.object_id} label={false} navigate /></Property><Property label="Slug"><code>{theme.slug}</code></Property><Property label="Assigned Objects">{assigned.length}</Property><Property label="Protected">{theme.protected ? "Yes" : "No"}</Property><Property label="Updated">{relative(theme.updated_at)}</Property></div></section>
    <InlineEditor label="Theme description" value={theme.description} multiline required maxLength={2000} className="detail-body-editor" onSave={(value) => saveObjectField("description", value)} onReload={load} />
    <Section title="Themed Objects"><div className="connections themed-object-list">{assigned.map((item) => <article className="connection themed-object-row" key={item.id}><ObjectId id={item.id} linkPill /><ObjectTypeBadge kind={item.kind} /><strong className="themed-object-title" title={item.title}>{item.title}</strong><ObjectContext visual={visuals.get(item.id)} /></article>)}{assigned.length === 0 && <p className="muted">No Objects use this Theme yet.</p>}</div></Section>
    <FocusedObjectGraph objectId={theme.object_id} objectTitle={theme.title} refreshKey={refreshKey} />
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
  return <CreateModal title="New theme" onClose={onCancel}><form className="create-form" onSubmit={submit}><input className="create-title" name="title" required maxLength={300} autoFocus placeholder="Theme title" aria-label="Theme title" /><Field label="Slug"><input name="slug" required maxLength={100} pattern="[a-z0-9]+(-[a-z0-9]+)*" placeholder="research-vertical" /></Field><textarea className="create-body" name="description" rows={5} required maxLength={2000} placeholder={descriptionExamples.theme} aria-label="Theme description" />{error && <p className="form-error">{error}</p>}<div className="modal-actions"><button type="button" className="text-button" onClick={onCancel}>Cancel</button><button disabled={busy}>{busy ? "Creating…" : "Create approved theme"}</button></div></form></CreateModal>;
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
    <textarea className="create-body" name="description" rows={5} required maxLength={2000} placeholder={descriptionExamples[kind]} aria-label={`${name} description`} />
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
    <textarea className="create-body" name="description" rows={5} required maxLength={2000} placeholder={descriptionExamples.task} aria-label="Task description" />
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
    <textarea className="create-description" name="description" rows={3} required maxLength={2000} placeholder={descriptionExamples.note} aria-label="Note description" />
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
    <textarea className="create-body" name="description" rows={3} required maxLength={2000} placeholder={descriptionExamples.source} aria-label="Source description" />
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

function SourceDetail({ id, objects, visuals, onChanged, refreshKey }: { id: string; objects: SharedObject[]; visuals: Map<string, ObjectVisual>; onChanged: () => Promise<void>; refreshKey: number }) {
  const [source, setSource] = useState<Source | null>(null);
  const [object, setObject] = useState<SharedObject | null>(null);
  const [connections, setConnections] = useState<Connection[]>([]);
  const [events, setEvents] = useState<ObjectEvent[]>([]);
  const [artifacts, setArtifacts] = useState<Artifact[]>([]);
  const [error, setError] = useState<string | null>(null);
  const revision = useRef(0);
  const loadGeneration = useRef(0);
  const serialize = useSerializedSave();
  const load = useCallback(async () => {
    const generation = ++loadGeneration.current;
    try {
      const [nextSource, nextObject, nextConnections, nextEvents, nextArtifacts] = await Promise.all([api.source(id), api.object(id), api.connections(id), api.events(id), api.artifacts(id)]);
      if (generation !== loadGeneration.current) return;
      revision.current = nextSource.revision; setSource(nextSource); setObject(nextObject); setConnections(nextConnections); setEvents(nextEvents); setArtifacts(nextArtifacts); setError(null);
    } catch (cause) { if (generation === loadGeneration.current) setError(message(cause)); }
  }, [id, refreshKey]);
  useEffect(() => { void load(); }, [load]);
  if (!source || !object) return <DetailLoading error={error} />;
  const saveField = async (field: "title" | "description", value: string) => {
    return serialize(async () => {
      const updated = await api.updateSource(id, { expected_revision: revision.current, [field]: value });
      revision.current = updated.revision; setSource(updated);
      setObject((current) => current ? { ...current, title: updated.title, description: updated.description, revision: updated.revision, updated_at: updated.updated_at } : current);
      await onChanged(); return updated[field];
    });
  };
  return <div className="record-page"><div className="record-primary">
    <div className="detail-form source-detail-form">
      <InlineEditor label="Source title" value={source.title} required maxLength={300} className="detail-title-editor" heading onSave={(value) => saveField("title", value)} onReload={load} />
      <section className="properties-block" aria-label="Source properties"><h2>Properties</h2><div className="properties-grid">
        <Property label="Object ID"><ObjectId id={source.object_id} label={false} navigate /></Property>
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
      <InlineEditor label="Source description" value={source.description} multiline required maxLength={2000} placeholder={descriptionExamples.source} className="detail-body-editor" onSave={(value) => saveField("description", value)} onReload={load} />
    </div>
    {error && <p className="form-error">{error}</p>}
    <Artifacts objectId={id} artifacts={artifacts} currentArtifactId={source.current_artifact_id} onCreated={load} />
    <Connections object={object} objects={objects} visuals={visuals} connections={connections} onCreated={load} refreshKey={refreshKey} />
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

function NoteDetail({ id, objects, visuals, onChanged, refreshKey }: { id: string; objects: SharedObject[]; visuals: Map<string, ObjectVisual>; onChanged: () => Promise<void>; refreshKey: number }) {
  const [note, setNote] = useState<Note | null>(null);
  const [object, setObject] = useState<SharedObject | null>(null);
  const [connections, setConnections] = useState<Connection[]>([]);
  const [events, setEvents] = useState<ObjectEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const revision = useRef(0);
  const loadGeneration = useRef(0);
  const serialize = useSerializedSave();
  const load = useCallback(async () => {
    const generation = ++loadGeneration.current;
    try {
      const [nextNote, nextObject, nextConnections, nextEvents] = await Promise.all([api.note(id), api.object(id), api.connections(id), api.events(id)]);
      if (generation !== loadGeneration.current) return;
      revision.current = nextNote.revision; setNote(nextNote); setObject(nextObject); setConnections(nextConnections); setEvents(nextEvents); setError(null);
    } catch (cause) { if (generation === loadGeneration.current) setError(message(cause)); }
  }, [id, refreshKey]);
  useEffect(() => { void load(); }, [load]);
  if (!note || !object) return <DetailLoading error={error} />;
  const saveField = async (field: "title" | "content", value: string) => {
    return serialize(async () => {
      const updated = await api.updateNote(id, { expected_revision: revision.current, [field]: value });
      revision.current = updated.revision; setNote(updated);
      setObject((current) => current ? { ...current, title: updated.title, revision: updated.revision, updated_at: updated.updated_at } : current);
      await onChanged(); return updated[field];
    });
  };
  const saveFormat = async (value: Note["content_format"]) => {
    try {
      const updated = await serialize(() => api.updateNote(id, { expected_revision: revision.current, content_format: value }));
      revision.current = updated.revision;
      setNote(updated);
      await onChanged();
    } catch (cause) { setError(conflictMessage(cause)); }
  };
  return <div className="record-page"><div className="record-primary">
    <div className="detail-form note-detail-form">
      <InlineEditor label="Note title" value={note.title} required maxLength={300} className="detail-title-editor" heading onSave={(value) => saveField("title", value)} onReload={load} />
      <section className="properties-block" aria-label="Note properties"><h2>Properties</h2><div className="properties-grid">
        <Property label="Object ID"><ObjectId id={note.object_id} label={false} navigate /></Property>
        <Property label="Type"><ObjectTypeBadge kind="note" /></Property>
        <Property label="Users">{(visuals.get(note.object_id)?.users.length ?? 0) > 0 ? <AttributionStack users={visuals.get(note.object_id)?.users ?? []} /> : "None"}</Property>
        <Property label="Format"><select aria-label="Note content format" value={note.content_format} onChange={(event) => void saveFormat(event.target.value as Note["content_format"])}><option value="markdown">Markdown</option><option value="plain_text">Plain text</option></select></Property>
        <Property label="Updated">{relative(note.updated_at)}</Property>
      </div></section>
      <Section title="Content"><InlineEditor label="Note content" value={note.content} multiline required maxLength={100000} placeholder="Write plain text or Markdown…" className="note-content-editor" onSave={(value) => saveField("content", value)} onReload={load} /></Section>
    </div>
    {error && <p className="form-error">{error}</p>}
    <Connections object={object} objects={objects} visuals={visuals} connections={connections} onCreated={load} refreshKey={refreshKey} />
    <ActivityTimeline events={events} visuals={visuals} />
    <Provenance value={note.provenance} />
  </div></div>;
}

function ObjectDetail({ id, objects, visuals, onChanged, refreshKey }: { id: string; objects: SharedObject[]; visuals: Map<string, ObjectVisual>; onChanged: () => Promise<void>; refreshKey: number }) {
  const [item, setItem] = useState<SharedObject | null>(null);
  const [connections, setConnections] = useState<Connection[]>([]);
  const [events, setEvents] = useState<ObjectEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const revision = useRef(0);
  const loadGeneration = useRef(0);
  const serialize = useSerializedSave();
  const load = useCallback(async () => {
    const generation = ++loadGeneration.current;
    try { const [nextItem, nextConnections, nextEvents] = await Promise.all([api.object(id), api.connections(id), api.events(id)]); if (generation !== loadGeneration.current) return; revision.current = nextItem.revision; setItem(nextItem); setConnections(nextConnections); setEvents(nextEvents); setError(null); }
    catch (cause) { if (generation === loadGeneration.current) setError(message(cause)); }
  }, [id, refreshKey]);
  useEffect(() => { void load(); }, [load]);
  if (!item) return <DetailLoading error={error} />;
  const saveField = async (field: "title" | "description", value: string) => {
    return serialize(async () => {
      const updated = await api.updateObject(id, { expected_revision: revision.current, [field]: value });
      revision.current = updated.revision; setItem(updated); await onChanged(); return updated[field];
    });
  };
  return <div className="record-page">
    <div className="record-primary">
      <div className="detail-form">
        <InlineEditor label="Object title" value={item.title} required maxLength={300} className="detail-title-editor" heading onSave={(value) => saveField("title", value)} onReload={load} />
        <section className="properties-block" aria-label="Object properties">
          <h2>Properties</h2>
          <div className="properties-grid">
            <Property label="Object ID"><ObjectId id={item.id} label={false} navigate /></Property>
            <Property label="Type"><ObjectTypeBadge kind={item.kind} /></Property>
            <Property label="Source">{visuals.get(item.id)?.source_provider ? <SourceBadge provider={visuals.get(item.id)?.source_provider} /> : textValue(item.provenance.source_type, "Unspecified")}</Property>
            <Property label="Users">{(visuals.get(item.id)?.users.length ?? 0) > 0 ? <AttributionStack users={visuals.get(item.id)?.users ?? []} /> : "None"}</Property>
            <Property label="Created by"><span className="property-value-wrap">{item.created_by_type}:{item.created_by_id}</span></Property>
            <Property label="Updated">{relative(item.updated_at)}</Property>
          </div>
        </section>
        <InlineEditor label="Object description" value={item.description} multiline required maxLength={2000} placeholder={descriptionExamples[item.kind]} className="detail-body-editor" onSave={(value) => saveField("description", value)} onReload={load} />
      </div>
      {error && <p className="form-error">{error}</p>}
      {item.kind === "user" && <UserIdentityPanel id={item.id} visual={visuals.get(item.id)} refreshKey={refreshKey} />}
      {item.kind === "chat" && <ChatTranscript id={item.id} visuals={visuals} refreshKey={refreshKey} />}
      <Connections object={item} objects={objects} visuals={visuals} connections={connections} onCreated={load} refreshKey={refreshKey} />
      <ActivityTimeline events={events} visuals={visuals} includeThread />
      <Provenance value={item.provenance} />
    </div>
  </div>;
}

function UserIdentityPanel({ id, visual, refreshKey }: { id: string; visual: ObjectVisual | undefined; refreshKey: number }) {
  const [user, setUser] = useState<User | null>(null);
  const [identities, setIdentities] = useState<ExternalIdentity[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    void Promise.all([api.user(id), api.userIdentities(id)])
      .then(([nextUser, nextIdentities]) => { if (active) { setUser(nextUser); setIdentities(nextIdentities); } })
      .catch((cause) => { if (active) setError(message(cause)); });
    return () => { active = false; };
  }, [id, refreshKey]);
  return <Section title="Identity">
    {error && <p className="form-error">{error}</p>}
    {user && <div className="properties-block compact-properties"><div className="properties-grid"><Property label="Object ID"><ObjectId id={user.object_id} label={false} /><ObjectContext visual={visual} /></Property><Property label="User kind"><span className="user-kind-label">{user.user_kind === "agent" ? "Agent" : "Human"}</span></Property>{identities.map((identity) => <div className="identity" key={identity.id}><SourceBadge provider={identity.provider} /><strong>{identity.display_name ?? identity.provider_user_id}</strong><small>{identity.workspace_id || "Default workspace"} · {identity.provider_user_id}</small><ObjectId id={user.object_id} compact /></div>)}{identities.length === 0 && <p className="muted">No external identities.</p>}</div></div>}
  </Section>;
}

function ChatTranscript({ id, visuals, refreshKey }: { id: string; visuals: Map<string, ObjectVisual>; refreshKey: number }) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    void api.chatMessages(id)
      .then((items) => { if (active) setMessages(items); })
      .catch((cause) => { if (active) setError(message(cause)); });
    return () => { active = false; };
  }, [id, refreshKey]);
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

function Connections({ object, objects, visuals, connections, onCreated, refreshKey }: { object: SharedObject; objects: SharedObject[]; visuals: Map<string, ObjectVisual>; connections: Connection[]; onCreated: () => Promise<void>; refreshKey: number }) {
  const [open, setOpen] = useState(false); const [error, setError] = useState<string | null>(null);
  const submit = async (event: FormEvent<HTMLFormElement>) => { event.preventDefault(); const data = new FormData(event.currentTarget);
    try { await api.createConnection({ source_object_id: object.id, kind: String(data.get("kind")), target_object_id: String(data.get("target")), description: String(data.get("description")), protected: data.get("protected") === "on", provenance: { source_type: "human" } }); setOpen(false); await onCreated(); }
    catch (cause) { setError(message(cause)); }
  };
  return <>
    <Section title="Related Objects" action={<button className="text-button" onClick={() => setOpen((value) => !value)}>+ Connect</button>}>
      {open && <form className="connection-form" onSubmit={submit}><select name="kind">{connectionKinds.map((kind) => <option key={kind}>{kind}</option>)}</select><select name="target" required defaultValue=""><option value="" disabled>Target record</option>{objects.filter((item) => item.id !== object.id && item.kind !== "task").map((item) => <option value={item.id} key={item.id}>{item.title}</option>)}</select><input name="description" required placeholder="Explain the exact relationship…" /><label className="property-check"><input type="checkbox" name="protected" /> Protect from curator changes</label><button className="secondary">Add</button>{error && <p className="form-error">{error}</p>}</form>}
      <div className="connections related-object-list">{connections.map((connection) => <RelatedObjectRow currentObjectId={object.id} connection={connection} objects={objects} visuals={visuals} key={connection.id} />)}{connections.length === 0 && <p className="muted">No related Objects.</p>}</div>
    </Section>
    <FocusedObjectGraph objectId={object.id} objectTitle={object.title} refreshKey={refreshKey} />
  </>;
}

function RelatedObjectRow({ currentObjectId, connection, objects, visuals }: { currentObjectId: string; connection: Connection; objects: SharedObject[]; visuals: Map<string, ObjectVisual> }) {
  const outbound = connection.source_object_id === currentObjectId;
  const relatedId = outbound ? connection.target_object_id : connection.source_object_id;
  const related = objects.find((item) => item.id === relatedId);
  return <article className="related-object-row">
    {related ? <ObjectTypeBadge kind={related.kind} compact /> : <span />}
    <ObjectId id={relatedId} rowPill />
    <span className="related-object-copy">
      <a className="related-object-title" href={detailPath("objects", relatedId)}>{related?.title ?? shortId(relatedId)}</a>
      <span className="related-object-description" title={connection.description}>{connection.description}</span>
    </span>
    <ObjectContext visual={visuals.get(relatedId)} />
    <span className="related-object-meaning"><span title={outbound ? "Outbound from this Object" : "Inbound to this Object"}>{outbound ? "→" : "←"}</span> {connection.kind.replaceAll("_", " ")}</span>
    <a className="related-connection-link" href={detailPath("connections", connection.id)} aria-label={`Open connection ${connection.id}`}>Connection</a>
  </article>;
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

function TaskDetail({ id, objects, visuals, onChanged, refreshKey }: { id: string; objects: SharedObject[]; visuals: Map<string, ObjectVisual>; onChanged: () => Promise<void>; refreshKey: number }) {
  const [task, setTask] = useState<Task | null>(null);
  const [object, setObject] = useState<SharedObject | null>(null);
  const [connections, setConnections] = useState<Connection[]>([]);
  const [events, setEvents] = useState<ObjectEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const revision = useRef(0);
  const loadGeneration = useRef(0);
  const serialize = useSerializedSave();
  const load = useCallback(async () => {
    const generation = ++loadGeneration.current;
    try {
      const [nextTask, nextObject, nextConnections, nextEvents] = await Promise.all([api.task(id), api.object(id), api.connections(id), api.events(id)]);
      if (generation !== loadGeneration.current) return;
      revision.current = nextTask.revision; setTask(nextTask); setObject(nextObject); setConnections(nextConnections); setEvents(nextEvents); setError(null);
    } catch (cause) { if (generation === loadGeneration.current) setError(message(cause)); }
  }, [id, refreshKey]);
  useEffect(() => { void load(); }, [load]);
  if (!task || !object) return <DetailLoading error={error} />;
  const saveFields = async (changes: Record<string, unknown>) => {
    return serialize(async () => {
      const updated = await api.updateTask(id, { expected_revision: revision.current, ...changes });
      revision.current = updated.revision; setTask(updated);
      setObject((current) => current ? { ...current, title: updated.title, revision: updated.revision, updated_at: updated.updated_at } : current);
      await onChanged(); return updated;
    });
  };
  const saveText = async (field: "title" | "brief_markdown", value: string) => {
    const updated = await saveFields(field === "brief_markdown" ? { brief_markdown: value || undefined, clear_brief_markdown: !value } : { title: value });
    return field === "brief_markdown" ? updated.brief_markdown ?? "" : updated.title;
  };
  const saveProperty = async (changes: Record<string, unknown>) => {
    try { await saveFields(changes); }
    catch (cause) { setError(conflictMessage(cause)); }
  };
  return <div className="record-page">
    <div className="record-primary">
      <div className="detail-form">
        <InlineEditor label="Task title" value={task.title} required maxLength={300} className="detail-title-editor" heading onSave={(value) => saveText("title", value)} onReload={load} />
        <section className="properties-block" aria-label="Task properties">
          <h2>Properties</h2>
          <div className="properties-grid">
            <Property label="Object ID"><ObjectId id={task.object_id} label={false} navigate /></Property>
            <Property label="Type"><ObjectTypeBadge kind="task" /></Property>
            <Property label="Source">{visuals.get(task.object_id)?.source_provider ? <SourceBadge provider={visuals.get(task.object_id)?.source_provider} /> : textValue(task.provenance.source_type, "Unspecified")}</Property>
            <Property label="Users">{(visuals.get(task.object_id)?.users.length ?? 0) > 0 ? <AttributionStack users={visuals.get(task.object_id)?.users ?? []} /> : "None"}</Property>
            <Field label="Status"><select aria-label="Task status" value={task.status} onChange={(event) => void saveProperty({ status: event.target.value })}>{taskStatuses.map((status) => <option key={status}>{status}</option>)}</select></Field>
            <Property label="Agent suitability"><label className="check"><input type="checkbox" checked={task.agent_suitable} onChange={(event) => void saveProperty({ agent_suitable: event.target.checked })} /> Suitable</label></Property>
            <Property label="Priority"><span className="property-value-wrap">{task.priority}</span></Property>
            <Property label="Owner">{task.owner_object_id ? <><ObjectId id={task.owner_object_id} /><ObjectContext visual={visuals.get(task.owner_object_id)} /></> : "Unassigned"}</Property>
            <Property label="Due">{task.due_at ? new Date(task.due_at).toLocaleString() : "No due date"}</Property>
            <Property label="Completed">{task.completed_at ? new Date(task.completed_at).toLocaleString() : "Not complete"}</Property>
            <Property label="Blocked reason"><span className="property-value-wrap">{task.blocked_reason ?? "None"}</span></Property>
            <Property label="GitHub issue">{safeGithubUrl(task.github_issue_url) ? <a href={safeGithubUrl(task.github_issue_url)!} target="_blank" rel="noopener noreferrer">Open issue</a> : "None"}</Property>
            <Property label="Updated">{relative(task.updated_at)}</Property>
          </div>
        </section>
        <Section title="Brief"><InlineEditor label="Task brief" value={task.brief_markdown ?? ""} multiline maxLength={100000} placeholder="Scope, constraints, acceptance criteria, and verification…" className="detail-body-editor task-brief-editor" onSave={(value) => saveText("brief_markdown", value)} onReload={load} /></Section>
      </div>
      {error && <p className="form-error">{error}</p>}
      <Connections object={object} objects={objects} visuals={visuals} connections={connections} onCreated={load} refreshKey={refreshKey} />
      <ActivityTimeline events={events} visuals={visuals} />
      <Provenance value={task.provenance} />
    </div>
  </div>;
}

function RunDetailView({ id, visuals, onChanged, refreshKey }: { id: string; visuals: Map<string, ObjectVisual>; onChanged: () => Promise<void>; refreshKey: number }) {
  const [detail, setDetail] = useState<RunDetail | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reviewRevision = useRef(0);
  const loadGeneration = useRef(0);
  const serialize = useSerializedSave();
  const load = useCallback(async () => {
    const generation = ++loadGeneration.current;
    try { const next = await api.run(id); if (generation !== loadGeneration.current) return; reviewRevision.current = Number((next.run.result.review_revision as number | undefined) ?? 0); setDetail(next); setError(null); }
    catch (cause) { if (generation === loadGeneration.current) setError(message(cause)); }
  }, [id, refreshKey]);
  useEffect(() => { void load(); }, [load]);
  if (!detail) return <DetailLoading error={error} />;
  const { run, children, objects, events } = detail;
  const saveReview = async (verdict: RunVerdict, notes: string | null) => {
    return serialize(async () => {
      const updated = await api.reviewRun(id, { verdict, notes, expected_revision: reviewRevision.current });
      reviewRevision.current = Number((updated.result.review_revision as number | undefined) ?? reviewRevision.current + 1);
      setDetail((current) => current ? { ...current, run: updated } : current);
      await onChanged(); return updated;
    });
  };
  const chooseVerdict = async (verdict: "pass" | "fail") => {
    setBusy(true); setError(null);
    try { await saveReview(verdict, run.review_notes); }
    catch (cause) { setError(conflictMessage(cause)); }
    finally { setBusy(false); }
  };
  const undo = async () => {
    if (!window.confirm("Create a compensating run that reverses this run’s durable mutations?")) return;
    setBusy(true); setError(null);
    try { await api.undoRun(id); await Promise.all([load(), onChanged()]); }
    catch (cause) { setError(conflictMessage(cause)); }
    finally { setBusy(false); }
  };
  const primary = run.primary_object_id ? objects.find((object) => object.object_id === run.primary_object_id) : undefined;
  const chatVisual = run.chat_object_id ? visuals.get(run.chat_object_id) : undefined;
  const interactionTitle = run.kind === "slack_interaction" ? textValue(run.input.title) : "";
  const title = primary ? `${runType(run, objects)} · ${primary.title}` : interactionTitle ? `${runType(run, objects)} · ${interactionTitle}` : runType(run, objects);
  const outcome = runOutcome(run, objects);
  const metrics = runMetrics(run);
  return <div className="record-page"><div className="record-primary eval-detail">
    <div className="detail-heading run-heading"><h1 className="detail-title">{title}</h1></div>
    <p className="detail-description">{outcome}</p>
    <section className="properties-block" aria-label="Run properties"><h2>Properties</h2><div className="properties-grid">
      <Property label="Run ID"><span className="object-id-pill">{shortId(run.id)}</span></Property>
      <Property label="Type"><span className="visual-badge run-type-badge">↻ {runType(run, objects)}</span></Property><Property label="Status"><StateBadge state={run.status} /></Property><Property label="Verdict"><span className={`eval-verdict ${run.verdict}`}>{run.verdict}</span></Property><Property label="Golden eval">{run.pinned ? "Pinned" : "Not pinned"}</Property>
      <Property label="Source">{chatVisual?.source_provider ? <SourceBadge provider={chatVisual.source_provider} /> : "Internal"}</Property><Property label="Users">{(chatVisual?.users.length ?? 0) > 0 ? <AttributionStack users={chatVisual?.users ?? []} /> : "None"}</Property>
      {primary && <Property label="Primary Object"><a className="run-object-title" href={detailPath("objects", primary.object_id)}>{primary.title}</a><ObjectTypeBadge kind={primary.kind} /><ObjectId id={primary.object_id} compact /></Property>}
      <Property label="Created">{new Date(run.created_at).toLocaleString()}</Property><Property label="Parent">{run.parent_run_id ? <a href={detailPath("runs", run.parent_run_id)}>{shortId(run.parent_run_id)}</a> : "None"}</Property>
      {run.chat_object_id && <Property label="Originating Chat"><a href={detailPath("objects", run.chat_object_id)}>Open Slack conversation</a><ObjectId id={run.chat_object_id} compact /></Property>}<Property label="Technical actor">{run.actor_type}:{run.actor_id}</Property><Property label="Consulted Objects">{run.consulted_object_ids.length}</Property><Property label="Mutations">{events.length}</Property>
    </div></section>
    <section className="run-summary" aria-label="Run summary"><h2>Metrics</h2><div className="run-metrics">
      <RunMetric label="Duration" value={metrics.duration} />
      <RunMetric label="Model" value={metrics.model} />
      <RunMetric label="Total tokens" value={metrics.tokens.total} />
      <RunMetric label="Input" value={metrics.tokens.input} />
      <RunMetric label="Fresh input (derived)" value={metrics.tokens.freshInput} />
      <RunMetric label="Cache creation" value={metrics.tokens.cacheCreation} />
      <RunMetric label="Cache read" value={metrics.tokens.cacheRead} />
      <RunMetric label="Output" value={metrics.tokens.output} />
      <RunMetric label="Non-reasoning output (derived)" value={metrics.tokens.nonReasoningOutput} />
      <RunMetric label="Reasoning" value={metrics.tokens.reasoning} />
      <RunMetric label="Tool calls" value={String(metrics.toolCalls)} />
      <RunMetric label="Readiness polls" value={String(metrics.polls)} />
      <RunMetric label="Failures" value={String(metrics.failures)} warning={metrics.failures > 0} />
    </div></section>
    <p className="token-explanation">Provider total = input + output. Cache figures are part of input; reasoning is part of output. Fresh input is derived, while captured component sizes below are estimates.</p>
    {run.error && <p className="run-error">{run.error}</p>}{error && <p className="form-error">{error}</p>}
    <section className="eval-annotation" aria-label="Run review"><div className="verdict-segments" role="group" aria-label="Review verdict"><button type="button" className={run.verdict === "pass" ? "active pass" : "pass"} disabled={busy} aria-pressed={run.verdict === "pass"} onClick={() => void chooseVerdict("pass")}>Pass</button><button type="button" className={run.verdict === "fail" ? "active fail" : "fail"} disabled={busy} aria-pressed={run.verdict === "fail"} onClick={() => void chooseVerdict("fail")}>Fail</button>{!(["pass", "fail"] as string[]).includes(run.verdict) && <span className={`eval-verdict ${run.verdict}`}>Legacy: {run.verdict}</span>}</div><InlineEditor label="Review notes" value={run.review_notes ?? ""} multiline maxLength={4000} placeholder="Add review notes" className="review-notes-editor" onSave={async (value) => { const updated = await saveReview(run.verdict, value || null); return updated.review_notes ?? ""; }} onReload={load} /></section>
    {events.some((item) => item.reversible) && <button className="danger-button" type="button" disabled={busy} onClick={() => void undo()}>{busy ? "Creating reversal…" : "Undo with compensating run"}</button>}
    <ConversationEvidence run={run} />
    <AgentInputEvidence trace={run.trace} />
    <RetrievedContextEvidence trace={run.trace} />
    <ToolEvidence trace={run.trace} />
    <Section title="Execution trace"><div className="run-trace-list">{run.trace.map((entry, index) => <RunTraceEntry entry={entry} index={index} key={textValue(entry.id, String(index))} />)}{run.trace.length === 0 && <p className="run-empty-trace">No detailed trace was recorded for this run.</p>}</div></Section>
    <Section title="Outcome"><p className="run-outcome">{outcome}</p><details className="run-technical run-result"><summary>Technical result</summary><pre className="source-text-preview">{JSON.stringify(run.result, null, 2)}</pre></details></Section>
    {children.length > 0 && <Section title="Child runs"><div className="run-child-list">{children.map((child) => <a className="run-child" href={detailPath("runs", child.id)} key={child.id}><span className="status-ring" /><span><strong>{runType(child, objects)}</strong><small>{runOutcome(child, objects)}</small></span><StateBadge state={child.status} /><span className="object-id-pill">{shortId(child.id)}</span></a>)}</div></Section>}
    <Section title="Related Objects"><div className="run-related-objects">{objects.map((object) => <article className="run-related-object" key={object.object_id}><ObjectTypeBadge kind={object.kind} compact /><ObjectId id={object.object_id} compact /><a href={detailPath("objects", object.object_id)}>{object.title}</a><span>{object.role.replaceAll("_", " ")}</span><ObjectContext visual={visuals.get(object.object_id)} /></article>)}{objects.length === 0 && <p className="muted">No related Objects.</p>}</div></Section>
    <Section title="Durable mutations"><div className="change-list">{events.map((item) => <article className="change" key={item.id}><span className="event-dot" /><strong>{item.action} {item.target_type}</strong><span>revision {item.from_revision ?? "new"} → {item.to_revision}</span>{item.target_type === "object" ? <ObjectId id={item.target_id} compact /> : <ConnectionId id={item.target_id} compact />}{item.reversible && <span className="change-state">reversible</span>}</article>)}</div></Section>
  </div></div>;
}

function ConversationEvidence({ run }: { run: Run }) {
  const request = recordValue(run.input.request_message);
  const response = recordValue(run.result.response_message);
  return <Section title="Conversation"><div className="evidence-grid">
    <EvidenceMessage label="User request" value={request} />
    <EvidenceMessage label="Agent response" value={response} />
  </div></Section>;
}

function EvidenceMessage({ label, value }: { label: string; value?: Record<string, unknown> }) {
  if (!value) return null;
  const sender = recordValue(value.sender);
  return <details className="evidence-row evidence-text"><summary><strong>{label}</strong></summary><div className="evidence-meta"><span>{textValue(sender?.display_name, "Unknown sender")}</span><span>{formatEvidenceTime(value.source_created_at)}</span><span>Slack message {textValue(value.provider_message_id, "unknown")}</span></div><pre>{textValue(value.content, "")}</pre></details>;
}

function AgentInputEvidence({ trace }: { trace: Record<string, unknown>[] }) {
  const turn = trace.find((entry) => entry.entry_type === "input_snapshot");
  const instructions = trace.find((entry) => entry.entry_type === "instruction_snapshot");
  const turnFacts = recordValue(turn?.facts);
  const instructionFacts = recordValue(instructions?.facts);
  const application = recordValue(instructionFacts?.application_instructions);
  const toolCatalogue = recordValue(instructionFacts?.tool_catalogue);
  const applicationTools = recordValue(toolCatalogue?.application);
  const components = arrayRecords(turnFacts?.components).filter((component) => textValue(component.status, "captured") === "captured");
  const capturedApplication = application && textValue(application.status, "captured") === "captured" ? application : undefined;
  const capturedTools = applicationTools && textValue(applicationTools.status, "captured") === "captured" ? applicationTools : undefined;
  const hasCapturedInput = Boolean(capturedApplication || capturedTools || components.length > 0);
  return <Section title="What the agent received">
    {!hasCapturedInput && <p className="evidence-unavailable">Application-controlled input was not captured for this Run.</p>}
    {capturedApplication && <EvidenceText title="Application instructions" value={capturedApplication} estimated />}
    {capturedTools && <EvidenceText title="Application tool catalogue" value={capturedTools} estimated />}
    {components.length > 0 && <div className="evidence-list">{components.map((component, index) => <EvidenceText key={`${textValue(component.kind)}-${index}`} title={textValue(component.kind, "Input component").replaceAll("_", " ")} value={component} estimated />)}</div>}
  </Section>;
}

function EvidenceText({ title, value, estimated = false }: { title: string; value: Record<string, unknown>; estimated?: boolean }) {
  const chars = numberValue(value.chars);
  const capturedTokens = numberValue(value.estimated_tokens);
  const tokens = capturedTokens ?? (estimated && chars !== null ? Math.ceil(chars / 4) : null);
  return <details className="evidence-row evidence-text"><summary><strong>{title}</strong></summary><div className="evidence-meta"><span>{chars === null ? "Size unavailable" : `${formatCount(chars)} chars`}{tokens === null ? "" : ` · ~${formatCount(tokens)} tokens${estimated ? " estimated" : ""}`}</span>{textValue(value.source) && <span>{textValue(value.source)}</span>}{textValue(value.sha256) && <code>sha256 {textValue(value.sha256)}</code>}</div><pre>{textValue(value.text, "No text captured.")}</pre></details>;
}

function RetrievedContextEvidence({ trace }: { trace: Record<string, unknown>[] }) {
  const retrieval = trace.find((entry) => entry.entry_type === "context_retrieval");
  const facts = recordValue(retrieval?.facts);
  const packet = recordValue(facts?.packet);
  const objects = arrayRecords(packet?.objects);
  return <Section title="Retrieved Context">
    {!packet ? <p className="evidence-unavailable">Not captured for this Run. Historical Runs are never reconstructed.</p> : <>
      <details className="evidence-row evidence-text"><summary><strong>Retrieval details</strong></summary><div className="evidence-meta"><span>Query: {textValue(packet.query, "Unavailable")}</span><span>Method: {textValue(packet.retrieval, "Unavailable")}</span><span>Captured: {formatEvidenceTime(packet.captured_at ?? retrieval?.created_at)}</span>{numberValue(packet.duration_ms) !== null && <span>Retrieval: {formatDuration(numberValue(packet.duration_ms) ?? 0)}</span>}<span>Omitted: {formatCount(numberValue(recordValue(packet.budget)?.omitted_objects) ?? numberValue(packet.omitted_object_count) ?? 0)} objects · {formatCount(numberValue(recordValue(packet.budget)?.omitted_connections) ?? 0)} connections</span></div></details>
      <div className="evidence-list">{objects.map((object, index) => { const relevance = recordValue(object.relevance); return <details className="evidence-row evidence-text" key={textValue(object.id, String(index))}><summary><strong>{index + 1}. {textValue(object.title, "Untitled Object")}</strong></summary><div className="evidence-meta"><span>{textValue(object.kind)}</span><span>Revision {scalarValue(object.revision, "?")}</span><span>Score {scalarValue(relevance?.score, "?")}</span><span>{textValue(object.id)}</span></div><p>{textValue(relevance?.rationale, "No retrieval rationale captured.")}</p><pre>{textValue(object.description, "")}</pre><details className="run-technical"><summary>Evidence and connections</summary><pre>{JSON.stringify(object, null, 2)}</pre></details></details>; })}</div>
      <details className="evidence-row evidence-text"><summary><strong>Exact injected Context text</strong></summary><div className="evidence-meta"><span>{packet.transport_truncated === true ? "Truncated" : "Complete"}</span></div><pre>{textValue(packet.injected_text, "No Context text was injected.")}</pre></details>
      {recordValue(packet.budget) && <details className="evidence-row evidence-text"><summary><strong>Retrieval budget</strong></summary><pre>{JSON.stringify(packet.budget, null, 2)}</pre></details>}
    </>}
  </Section>;
}

function ToolEvidence({ trace }: { trace: Record<string, unknown>[] }) {
  const tools = trace.filter((entry) => entry.entry_type === "tool_call");
  return <Section title="Tool activity"><div className="evidence-list">{tools.map((entry, index) => { const facts = recordValue(entry.facts); const detail = facts && ["command", "arguments", "output", "result", "error"].some((key) => facts[key] !== undefined); return <details className="evidence-row evidence-text" key={textValue(entry.id, String(index))}><summary><strong>{textValue(entry.name, "Tool call")}</strong></summary><div className="evidence-meta"><span>{textValue(entry.status, "completed")}</span></div>{detail ? <pre>{JSON.stringify(facts, null, 2)}</pre> : <p>Arguments and results were not captured for this Run.</p>}</details>; })}{tools.length === 0 && <p className="evidence-unavailable">No tool calls were recorded.</p>}</div></Section>;
}

function RunMetric({ label, value, warning = false }: { label: string; value: string; warning?: boolean }) {
  return <div className={warning ? "run-metric warning" : "run-metric"}><span>{label}</span><strong>{value}</strong></div>;
}

function RunTraceEntry({ entry, index }: { entry: Record<string, unknown>; index: number }) {
  const entryType = textValue(entry.entry_type ?? entry.type, "step").replaceAll("_", " ");
  const name = textValue(entry.name, entryType).replaceAll("_", " ");
  const status = textValue(entry.status, "completed");
  const duration = numberValue(entry.duration_ms);
  const tokens = numberValue(entry.total_tokens);
  return <details className={`run-trace-card ${status}`}>
    <summary>
      <span className="run-trace-index">{index + 1}</span>
      <span className="run-trace-summary"><strong>{name}</strong><small>{entryType} · {runTraceDescription(entry)}</small></span>
      {duration !== null && <span className="run-trace-stat">{formatDuration(duration)}</span>}
      {tokens !== null && <span className="run-trace-stat">{formatCount(tokens)} tokens</span>}
      <StateBadge state={status} />
      <span className="run-trace-chevron" aria-hidden="true">›</span>
    </summary>
    <div className="run-trace-detail">
      <div><span>Component</span><strong>{textValue(entry.component, "workflow")}</strong></div>
      {textValue(entry.model_id) && <div><span>Model</span><strong>{textValue(entry.model_id)}</strong></div>}
      {numberValue(entry.input_tokens) !== null && <div><span>Input</span><strong>{formatCount(numberValue(entry.input_tokens) ?? 0)}</strong></div>}
      {numberValue(entry.cache_creation_tokens) !== null && <div><span>Cache creation</span><strong>{formatCount(numberValue(entry.cache_creation_tokens) ?? 0)}</strong></div>}
      {numberValue(entry.output_tokens) !== null && <div><span>Output</span><strong>{formatCount(numberValue(entry.output_tokens) ?? 0)}</strong></div>}
      {numberValue(entry.cache_read_tokens) !== null && <div><span>Cache read</span><strong>{formatCount(numberValue(entry.cache_read_tokens) ?? 0)}</strong></div>}
      {numberValue(entry.reasoning_tokens) !== null && <div><span>Reasoning</span><strong>{formatCount(numberValue(entry.reasoning_tokens) ?? 0)}</strong></div>}
      {textValue((entry.facts as Record<string, unknown> | undefined)?.error_class) && <div><span>Error class</span><strong>{textValue((entry.facts as Record<string, unknown>).error_class)}</strong></div>}
      <details className="run-technical"><summary>Raw trace entry</summary><code>{JSON.stringify(entry)}</code></details>
    </div>
  </details>;
}

function runMetrics(run: Run) {
  const tokenTotals = { total: 0, input: 0, cacheCreation: 0, cacheRead: 0, output: 0, reasoning: 0 };
  const hasTokens = { total: false, input: false, cacheCreation: false, cacheRead: false, output: false, reasoning: false };
  let toolCalls = 0;
  let polls = 0;
  let failures = 0;
  let model = "—";
  for (const entry of run.trace) {
    const method = textValue((entry.facts as Record<string, unknown> | undefined)?.method);
    if (entry.entry_type === "tool_call") {
      if (method === "source_intake_status") polls += 1;
      else toolCalls += 1;
    }
    if (entry.status === "failed") failures += 1;
    for (const [field, key] of [["total_tokens", "total"], ["input_tokens", "input"], ["cache_creation_tokens", "cacheCreation"], ["cache_read_tokens", "cacheRead"], ["output_tokens", "output"], ["reasoning_tokens", "reasoning"]] as const) {
      const value = numberValue(entry[field]);
      if (value !== null) { tokenTotals[key] += value; hasTokens[key] = true; }
    }
    if (model === "—") model = textValue(entry.model_id, "—");
  }
  const started = run.started_at ? new Date(run.started_at).getTime() : null;
  const completed = run.completed_at ? new Date(run.completed_at).getTime() : null;
  const duration = started !== null && completed !== null ? formatDuration(Math.max(0, completed - started)) : run.status === "running" ? "Running" : "—";
  const freshInput = hasTokens.input ? formatCount(Math.max(0, tokenTotals.input - tokenTotals.cacheCreation - tokenTotals.cacheRead)) : "—";
  const nonReasoningOutput = hasTokens.output ? formatCount(Math.max(0, tokenTotals.output - tokenTotals.reasoning)) : "—";
  return {
    duration,
    model,
    tokens: { ...Object.fromEntries(Object.entries(tokenTotals).map(([key, value]) => [key, hasTokens[key as keyof typeof hasTokens] ? formatCount(value) : "—"])), freshInput, nonReasoningOutput } as Record<keyof typeof tokenTotals | "freshInput" | "nonReasoningOutput", string>,
    toolCalls,
    polls,
    failures,
  };
}

function recordValue(value: unknown): Record<string, unknown> | undefined { return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : undefined; }
function arrayRecords(value: unknown): Record<string, unknown>[] { return Array.isArray(value) ? value.map(recordValue).filter((item): item is Record<string, unknown> => Boolean(item)) : []; }
function scalarValue(value: unknown, fallback = "") { return typeof value === "string" || typeof value === "number" || typeof value === "boolean" ? String(value) : fallback; }
function formatEvidenceTime(value: unknown) {
  const text = textValue(value);
  if (text) return new Date(text).toLocaleString();
  if (Array.isArray(value) && value.length >= 6 && value.slice(0, 6).every((part) => typeof part === "number")) {
    const [year, ordinal, hour, minute, second, nanosecond, offsetHour = 0, offsetMinute = 0, offsetSecond = 0] = value as number[];
    const offset = Math.sign(offsetHour || offsetMinute || offsetSecond) * (Math.abs(offsetHour) * 3600 + Math.abs(offsetMinute) * 60 + Math.abs(offsetSecond));
    const instant = new Date(Date.UTC(year, 0, ordinal, hour, minute, second, Math.floor(nanosecond / 1_000_000)) - offset * 1000);
    if (!Number.isNaN(instant.getTime())) return instant.toLocaleString();
  }
  return "Time unavailable";
}

function runTraceDescription(entry: Record<string, unknown>) {
  const facts = entry.facts as Record<string, unknown> | undefined;
  return textValue(facts?.description, textValue(facts?.purpose, textValue(facts?.method, "Recorded workflow activity."))).replaceAll("_", " ");
}

function numberValue(value: unknown) { return typeof value === "number" && Number.isFinite(value) ? value : null; }
function formatCount(value: number) { return new Intl.NumberFormat().format(value); }
function formatDuration(milliseconds: number) {
  if (milliseconds < 1000) return `${Math.round(milliseconds)} ms`;
  if (milliseconds < 60_000) return `${(milliseconds / 1000).toFixed(milliseconds < 10_000 ? 1 : 0)} s`;
  const minutes = Math.floor(milliseconds / 60_000);
  const seconds = Math.round((milliseconds % 60_000) / 1000);
  return `${minutes}m ${seconds}s`;
}

function ConnectionDetail({ id, objects, visuals, refreshKey }: { id: string; objects: SharedObject[]; visuals: Map<string, ObjectVisual>; refreshKey: number }) {
  const [connection, setConnection] = useState<Connection | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    void api.connection(id)
      .then((item) => { if (active) setConnection(item); })
      .catch((cause) => { if (active) setError(message(cause)); });
    return () => { active = false; };
  }, [id, refreshKey]);
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
function Property({ label, children }: { label: string; children: React.ReactNode }) { return <div className="property"><span>{label}</span><div>{children}</div></div>; }
function Section({ title, action, children }: { title: string; action?: React.ReactNode; children: React.ReactNode }) { return <section className="detail-section"><header><h3>{title}</h3>{action}</header>{children}</section>; }
function DetailLoading({ error }: { error: string | null }) { return <div className="blank-state"><span>◇</span><h2>{error ? "Could not load" : "Loading"}</h2><p>{error ?? "Reading the shared record…"}</p></div>; }
function relative(value: string) { const seconds = Math.round((Date.now() - new Date(value).getTime()) / 1000); if (seconds < 60) return "now"; if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`; if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`; return `${Math.floor(seconds / 86400)}d ago`; }
function message(cause: unknown) { return cause instanceof Error ? cause.message : "Something went wrong."; }
function conflictMessage(cause: unknown) { return cause instanceof ApiError && cause.status === 409 ? "This record changed elsewhere. Refresh before saving again." : message(cause); }
function safeGithubUrl(value: string | null) {
  if (!value) return null;
  try { const url = new URL(value); return url.protocol === "https:" && url.hostname.toLowerCase() === "github.com" ? url.href : null; }
  catch { return null; }
}
function finishCreate(section: Section, id: string, load: () => Promise<void>, setCreateOpen: (open: boolean) => void) { setCreateOpen(false); navigate(detailPath(section, id)); void load(); }
function shortId(value: string) { return value.length > 12 ? `${value.slice(0, 8)}…` : value; }
function textValue(value: unknown, fallback = "") { return typeof value === "string" && value.trim() ? value : fallback; }
function stringList(value: unknown) { return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : []; }
function optional(data: FormData, name: string) { const value = String(data.get(name) ?? "").trim(); return value || null; }
function optionalDate(data: FormData, name: string) { const value = optional(data, name); return value ? new Date(value).toISOString() : null; }

import { Component, useMemo, type Dispatch, type ReactNode, type SetStateAction } from "react";
import { api } from "../api";
import { detailPath } from "../routing";
import type { SourceViewProps, TaskViewProps } from "../ui";
import { externalViews } from "./externalViews";
import type { Section } from "../routing";
import { navigate, sectionPath } from "../routing";
import type { ObjectVisual, Source, Task } from "../types";
import { TaskBoard } from "./taskBoard/TaskBoard";

export interface ModuleContext {
  tasks: Task[];
  taskControls?: TaskViewProps["controls"];
  sources?: Source[];
  visuals: Map<string, ObjectVisual>;
  loading: boolean;
  error?: string | null;
  onTasksChange: Dispatch<SetStateAction<Task[]>>;
  onReload: () => Promise<void>;
}

interface ModuleIdentity { id: string; label: string; icon: string; }
export type ContextUiModule = ModuleIdentity & (
  | { section: "tasks"; render(context: TaskViewProps): ReactNode }
  | { section: "sources"; render(context: SourceViewProps): ReactNode }
);

const modules: ContextUiModule[] = [
  { id: "kanban", section: "tasks", label: "Board", icon: "▦", render: (context) => <TaskBoard {...context} /> },
  ...externalViews,
];

export function resolveActiveModule(section: Section, search: string): ContextUiModule | null {
  const id = new URLSearchParams(search).get("view");
  return modules.find((module) => module.section === section && module.id === id) ?? null;
}

export function ModuleViewSwitcher({ section, activeId }: { section: Section; activeId: string | null }) {
  const sectionModules = modules.filter((module) => module.section === section);
  if (sectionModules.length === 0 && !new URLSearchParams(window.location.search).has("view")) return null;
  const requested = new URLSearchParams(window.location.search).get("view");
  return <div className="module-switcher" role="group" aria-label={`${section} views`}>
    {requested && !activeId ? <span role="status">This view is unavailable. Showing the list.</span> : null}
    <button className={activeId === null ? "active" : ""} type="button" onClick={() => navigate(viewPath(section, null))}><span aria-hidden="true">☷</span>List</button>
    {sectionModules.map((module) => <button key={module.id} className={activeId === module.id ? "active" : ""} type="button" onClick={() => navigate(viewPath(section, module.id))}><span aria-hidden="true">{module.icon}</span>{module.label}</button>)}
  </div>;
}

export function taskViewProps(context: ModuleContext): TaskViewProps {
  return {
    tasks: structuredClone(context.tasks), controls: context.taskControls, visuals: structuredClone(context.visuals), loading: context.loading,
    error: context.error ?? null,
    completeness: context.loading ? "loading" : context.error ? "unknown" : "complete",
    reload: context.onReload,
    openTask: (id) => navigate(detailPath("tasks", id)),
    async changeStatus(task, status, blockedReason) {
      const changes: Record<string, unknown> = { expected_revision: task.revision, status };
      if (status === "blocked") {
        if (!blockedReason?.trim()) throw new Error("A blocked reason is required.");
        changes.blocked_reason = blockedReason.trim();
      }
      if (task.status === "blocked" && status !== "blocked") changes.clear_blocked_reason = true;
      const updated = await api.updateTask(task.object_id, changes);
      context.onTasksChange((current) => current.map((item) => item.object_id === updated.object_id && item.revision <= updated.revision ? updated : item));
      // Invalidate any earlier collection read and refresh sorting/visuals from the server.
      await context.onReload().catch(() => undefined);
      return updated;
    },
  };
}

class ViewBoundary extends Component<{ children: ReactNode; section: Section }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  render() {
    return this.state.failed ? <div role="alert">This view could not be displayed. <button type="button" onClick={() => navigate(sectionPath(this.props.section))}>Return to list</button></div> : this.props.children;
  }
}

export function sourceViewProps(context: ModuleContext): SourceViewProps {
  return {
    sources: structuredClone(context.sources ?? []), visuals: structuredClone(context.visuals),
    loading: context.loading, error: context.error ?? null,
    completeness: context.loading ? "loading" : context.error ? "unknown" : "complete",
    reload: context.onReload,
    openSource: id => navigate(detailPath("sources", id)),
  };
}

function RenderView({ module, context }: { module: ContextUiModule; context: ModuleContext }) {
  const taskProps = useMemo(() => module.section === "tasks" ? taskViewProps(context) : null, [module.section, context]);
  const sourceProps = useMemo(() => module.section === "sources" ? sourceViewProps(context) : null, [module.section, context]);
  return <>{module.section === "tasks" ? module.render(taskProps!) : module.render(sourceProps!)}</>;
}

export function ContextModuleView({ module, context }: { module: ContextUiModule; context: ModuleContext }) {
  return <ViewBoundary key={module.id} section={module.section}><RenderView module={module} context={context} /></ViewBoundary>;
}


function viewPath(section: Section, view: string | null) {
  const params = new URLSearchParams(window.location.search);
  if (view) params.set("view", view); else params.delete("view");
  return `${sectionPath(section)}${params.size ? `?${params}` : ""}`;
}

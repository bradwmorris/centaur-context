import type { Dispatch, ReactNode, SetStateAction } from "react";
import type { Section } from "../routing";
import { navigate, sectionPath } from "../routing";
import type { ObjectVisual, Task } from "../types";
import { TaskBoard } from "./taskBoard/TaskBoard";

export interface ModuleContext {
  tasks: Task[];
  visuals: Map<string, ObjectVisual>;
  loading: boolean;
  onTasksChange: Dispatch<SetStateAction<Task[]>>;
  onReload: () => Promise<void>;
}

export interface ContextUiModule {
  id: string;
  section: Section;
  label: string;
  icon: string;
  render(context: ModuleContext): ReactNode;
}

const modules: ContextUiModule[] = [
  { id: "kanban", section: "tasks", label: "Board", icon: "▦", render: (context) => <TaskBoard {...context} /> },
];

export function resolveActiveModule(section: Section, search: string): ContextUiModule | null {
  const id = new URLSearchParams(search).get("view");
  return modules.find((module) => module.section === section && module.id === id) ?? null;
}

export function ModuleViewSwitcher({ section, activeId }: { section: Section; activeId: string | null }) {
  const sectionModules = modules.filter((module) => module.section === section);
  if (sectionModules.length === 0) return null;
  return <div className="module-switcher" role="group" aria-label={`${section} views`}>
    <button className={activeId === null ? "active" : ""} type="button" onClick={() => navigate(sectionPath(section))}><span aria-hidden="true">☷</span>List</button>
    {sectionModules.map((module) => <button key={module.id} className={activeId === module.id ? "active" : ""} type="button" onClick={() => navigate(`${sectionPath(section)}?view=${encodeURIComponent(module.id)}`)}><span aria-hidden="true">{module.icon}</span>{module.label}</button>)}
  </div>;
}

export function ContextModuleView({ module, context }: { module: ContextUiModule; context: ModuleContext }) {
  return <>{module.render(context)}</>;
}

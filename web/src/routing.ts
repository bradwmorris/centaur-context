import type { SchemaViewMode } from "./types";

export type Section = "objects" | "connections" | "tasks" | "chats" | "users" | "entities" | "memories" | "sources" | "notes" | "themes" | "runs" | "schema";

export interface AppRoute {
  section: Section;
  selectedId: string | null;
  connectionId: string | null;
}

const sections: Record<string, Section> = {
  objects: "objects",
  connections: "connections",
  tasks: "tasks",
  chats: "chats",
  users: "users",
  entities: "entities",
  memories: "memories",
  sources: "sources",
  notes: "notes",
  themes: "themes",
  runs: "runs",
  schema: "schema",
};

const paths: Record<Section, string> = {
  objects: "objects",
  connections: "connections",
  tasks: "tasks",
  chats: "chats",
  users: "users",
  entities: "entities",
  memories: "memories",
  sources: "sources",
  notes: "notes",
  themes: "themes",
  runs: "runs",
  schema: "schema",
};

export function parseRoute(pathname: string): AppRoute {
  const parts = pathname.split("/").filter(Boolean).map(safeDecode);
  if (parts[0] === "connections" && parts[1]) {
    return { section: "connections", selectedId: null, connectionId: parts[1] };
  }
  const section = sections[parts[0] ?? ""];
  if (!section) return { section: "objects", selectedId: null, connectionId: null };
  return { section, selectedId: parts[1] ?? null, connectionId: null };
}

function safeDecode(value: string): string {
  try { return decodeURIComponent(value); }
  catch { return value; }
}

export function sectionPath(section: Section): string {
  return `/${paths[section]}`;
}

export function detailPath(section: Section, id: string): string {
  return `${sectionPath(section)}/${encodeURIComponent(id)}`;
}

export function objectPath(id: string): string {
  return detailPath("objects", id);
}

export function schemaPath(table?: string, view: SchemaViewMode = table ? "rows" : "map"): string {
  if (!table || view === "map") return "/schema";
  return `/schema/${encodeURIComponent(table)}/${view}`;
}

export function schemaRowPath(table: string, column: string, value: string): string {
  const params = new URLSearchParams({ focus_column: column, focus_value: value });
  return `${schemaPath(table, "rows")}?${params}`;
}

export function schemaView(pathname: string): SchemaViewMode {
  const parts = pathname.split("/").filter(Boolean).map(safeDecode);
  if (parts[0] !== "schema" || !parts[1]) return "map";
  return "rows";
}

export function connectionPath(id: string): string {
  return `/connections/${encodeURIComponent(id)}`;
}

export function navigate(path: string, replace = false): void {
  if (`${window.location.pathname}${window.location.search}` === path) return;
  window.history[replace ? "replaceState" : "pushState"]({}, "", path);
  window.dispatchEvent(new PopStateEvent("popstate"));
}

export function interceptNavigation(event: React.MouseEvent<HTMLAnchorElement>, path: string): void {
  if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
  event.preventDefault();
  navigate(path);
}

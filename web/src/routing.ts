export type Section = "objects" | "tasks" | "chats" | "users" | "entities" | "memories" | "curator";

export interface AppRoute {
  section: Section;
  selectedId: string | null;
  connectionId: string | null;
}

const sections: Record<string, Section> = {
  objects: "objects",
  tasks: "tasks",
  chats: "chats",
  users: "users",
  entities: "entities",
  memories: "memories",
  "curator-runs": "curator",
};

const paths: Record<Section, string> = {
  objects: "objects",
  tasks: "tasks",
  chats: "chats",
  users: "users",
  entities: "entities",
  memories: "memories",
  curator: "curator-runs",
};

export function parseRoute(pathname: string): AppRoute {
  const parts = pathname.split("/").filter(Boolean).map(safeDecode);
  if (parts[0] === "connections" && parts[1]) {
    return { section: "objects", selectedId: null, connectionId: parts[1] };
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

export function connectionPath(id: string): string {
  return `/connections/${encodeURIComponent(id)}`;
}

export function navigate(path: string, replace = false): void {
  if (window.location.pathname === path) return;
  window.history[replace ? "replaceState" : "pushState"]({}, "", path);
  window.dispatchEvent(new PopStateEvent("popstate"));
}

export function interceptNavigation(event: React.MouseEvent<HTMLAnchorElement>, path: string): void {
  if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
  event.preventDefault();
  navigate(path);
}

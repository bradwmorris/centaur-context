// Project metadata lives in the existing brief; keep all UI consumers consistent.
export function taskProject(brief: string | null | undefined): string | null {
  const lines = (brief ?? "").split(/\r?\n/).filter((line) => /^Project:/i.test(line));
  if (lines.length !== 1) return null;
  const match = /^Project:\s*([a-z][a-z0-9-]{0,63})\s*$/i.exec(lines[0]);
  return match ? match[1].toLowerCase() : null;
}

export function withTaskProject(brief: string | null | undefined, project: string): string {
  const slug = project.trim().toLowerCase();
  if (!/^[a-z][a-z0-9-]{0,63}$/.test(slug)) throw new Error("Choose a project using letters, numbers and hyphens, starting with a letter.");
  const body = (brief ?? "").split(/\r?\n/).filter((line) => !/^Project:/i.test(line)).join("\n").replace(/^\n+/, "");
  return `Project: ${slug}\n\n${body}`;
}


/** Stable presentation for arbitrary deployment project slugs, not a catalog. */
export function projectPresentation(project: string | null) {
  const label = project ? project.charAt(0).toUpperCase() + project.slice(1).replaceAll("-", " ") : "No project";
  let hash = 0;
  for (const character of project ?? "") hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
  const icons = ["◇", "◎", "▤", "✦", "⌘", "◈", "◉", "▧", "⌁", "△", "⊞", "⬡"];
  const commonIcons: Record<string, string> = { general: "◇", research: "⌕", networking: "⌁", build: "⚒", dev: "⌘", work: "▣", finance: "$", health: "+", family: "⌂" };
  return { label, short: project ? project.slice(0, 3).toUpperCase() : "—",
    icon: project ? commonIcons[project] ?? icons[hash % icons.length] : "?", hue: hash % 360 };
}

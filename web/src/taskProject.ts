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

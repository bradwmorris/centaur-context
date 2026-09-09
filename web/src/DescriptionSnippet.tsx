const MAX_SNIPPET_CHARACTERS = 220;

export function DescriptionSnippet({ description }: { description: string | null | undefined }) {
  const normalized = normalizeDescription(description);
  const preview = truncateDescription(normalized);
  const missing = preview.length === 0;
  const visible = missing ? "Description unavailable" : preview;
  const accessible = missing
    ? "Description preview unavailable. Open the record for its full details."
    : `Description preview: ${preview} Open the record for the full description.`;
  return <p className="description-snippet" data-empty={missing || undefined} aria-label={accessible}>{visible}</p>;
}

export function normalizeDescription(value: string | null | undefined) {
  return (value ?? "").replace(/\s+/gu, " ").trim();
}

export function truncateDescription(value: string) {
  const characters = Array.from(value);
  if (characters.length <= MAX_SNIPPET_CHARACTERS) return value;
  const head = characters.slice(0, MAX_SNIPPET_CHARACTERS).join("");
  const boundary = head.lastIndexOf(" ");
  const cut = boundary >= Math.floor(MAX_SNIPPET_CHARACTERS * 0.6) ? boundary : head.length;
  return `${head.slice(0, cut).trimEnd()}…`;
}

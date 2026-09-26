import type { Artifact } from "./types";

export interface ArtifactDocument {
  key: string | null;
  latest: Artifact;
  history: Artifact[];
}

export function documentKey(artifact: Artifact): string | null {
  if (artifact.kind !== "research_notes") return null;
  const key = artifact.metadata.document_key;
  return typeof key === "string" && key.trim() ? key : "default";
}

function newest(first: Artifact, second: Artifact): number {
  return second.created_at.localeCompare(first.created_at) || second.id.localeCompare(first.id);
}

export function artifactDocuments(artifacts: Artifact[]): ArtifactDocument[] {
  const sorted = [...artifacts].sort(newest);
  const grouped = new Map<string, ArtifactDocument>();
  const documents: ArtifactDocument[] = [];
  for (const artifact of sorted) {
    const key = documentKey(artifact);
    if (key === null) {
      documents.push({ key: null, latest: artifact, history: [artifact] });
      continue;
    }
    const group = grouped.get(key);
    if (group) group.history.push(artifact);
    else {
      const document = { key, latest: artifact, history: [artifact] };
      grouped.set(key, document);
      documents.push(document);
    }
  }
  for (const document of documents) {
    if (document.key === null) continue;
    const superseded = new Set(document.history.map((item) => item.supersedes_artifact_id).filter(Boolean));
    // Follow explicit predecessor links. A legacy branch with no successor is
    // still listed in History; the newest head is the default reading version.
    document.latest = document.history.find((item) => !superseded.has(item.id)) ?? document.history[0];
  }
  return documents.sort((a, b) => newest(a.latest, b.latest));
}

export function artifactLabel(artifact: Artifact): string {
  if (artifact.title?.trim()) return artifact.title;
  if (artifact.kind === "research_notes") return "Research documents";
  return artifact.kind.replaceAll("_", " ");
}

export function mediaType(artifact: Artifact): string {
  return artifact.media_type?.split(";", 1)[0].trim().toLowerCase() ?? "";
}

export function readableText(artifact: Artifact): boolean {
  const type = mediaType(artifact);
  return type === "text/plain" || type === "text/markdown" || type === "application/json";
}

export function editableText(artifact: Artifact): boolean {
  return artifact.kind === "research_notes" && (mediaType(artifact) === "text/plain" || mediaType(artifact) === "text/markdown") && artifact.capture_outcome === "complete";
}

export function safeArtifactUri(uri: string | null): string | null {
  if (!uri) return null;
  try {
    const parsed = new URL(uri);
    return parsed.protocol === "https:" || parsed.protocol === "http:" ? parsed.href : null;
  } catch { return null; }
}

export async function verifyBody(artifact: Artifact, text: string): Promise<void> {
  const bytes = new TextEncoder().encode(text);
  if (bytes.byteLength !== artifact.size_bytes) throw new Error("Artifact content is incomplete. Editing is unavailable.");
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  const hash = Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
  if (hash !== artifact.sha256) throw new Error("Artifact content failed verification. Editing is unavailable.");
}

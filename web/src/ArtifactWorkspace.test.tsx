import { createHash, webcrypto } from "node:crypto";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ArtifactReader, Artifacts } from "./ArtifactWorkspace";
import { artifactDocuments, safeArtifactUri } from "./artifactDocuments";
import type { Artifact } from "./types";

const objectId = "00000000-0000-4000-8000-000000000001";
const timestamp = "2026-09-01T00:00:00Z";
type TestArtifact = Artifact & { content: string | null };
function fixture(id: string, kind: string, content: string | null, key?: string, predecessor: string | null = null): TestArtifact {
  const text = content ?? "https://example.test/original";
  return {
    id, object_id: objectId, kind, title: kind === "research_notes" ? "Lab notes" : "Transcript", content,
    uri: content === null ? text : null, media_type: kind === "research_notes" ? "text/markdown; charset=utf-8" : "text/plain; charset=utf-8",
    language: "en", sha256: createHash("sha256").update(text).digest("hex"), size_bytes: Buffer.byteLength(text),
    capture_outcome: content === null ? "unavailable" : "complete", capture_reason: content === null ? "Not captured" : null,
    expected_size_bytes: null, semantic_indexing_enabled: false,
    metadata: key ? { document_key: key, predecessor_artifact_id: predecessor } : {},
    supersedes_artifact_id: predecessor, captured_at: null, created_at: timestamp,
  };
}
const envelope = (data: unknown) => Promise.resolve(new Response(JSON.stringify({ data }), { status: 200, headers: { "Content-Type": "application/json" } }));

beforeEach(() => {
  vi.stubGlobal("crypto", webcrypto);
  window.history.replaceState({}, "", `/sources/${objectId}`);
});

describe("artifact documents", () => {
  it("groups working revisions and leaves independent documents and receipts distinct", () => {
    const first = fixture("first", "research_notes", "one", "a");
    const newer = fixture("newer", "research_notes", "two", "a", "first");
    const other = fixture("other", "research_notes", "two", "b");
    const transcript = fixture("transcript", "transcript", "evidence");
    const documents = artifactDocuments([first, newer, other, transcript]);
    expect(documents).toHaveLength(3);
    expect(documents.find((item) => item.key === "a")?.latest.id).toBe("newer");
    expect(documents.find((item) => item.key === "a")?.history).toHaveLength(2);
    expect(documents.find((item) => item.key === "b")?.history).toHaveLength(1);
    expect(documents.find((item) => item.key === null)?.latest.id).toBe("transcript");
  });

  it("rejects active or unsafe artifact URIs", () => {
    expect(safeArtifactUri("javascript:alert(1)")).toBeNull();
    expect(safeArtifactUri("data:text/html,<script>alert(1)</script>")).toBeNull();
    expect(safeArtifactUri("https://example.test/item")).toBe("https://example.test/item");
  });
});

describe("artifact reader", () => {
  function server(initial: TestArtifact[], initialRevision = 4, failLaterWindow = false, loseFirstResponse = false) {
    let artifacts = initial;
    let revision = initialRevision;
    const posts: Array<Record<string, unknown>> = [];
    vi.stubGlobal("fetch", vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = new URL(String(input), "http://localhost");
      if (url.pathname === `/api/v2/objects/${objectId}/artifacts` && init?.method === "POST") {
        const request = JSON.parse(String(init.body)) as Record<string, unknown>;
        posts.push(request);
        if (request.expected_revision !== revision) return Promise.resolve(new Response(JSON.stringify({ error: { message: "revision conflict", code: "conflict" } }), { status: 409 }));
        const next = fixture(`saved-${posts.length}`, String(request.kind), String(request.content), String((request.metadata as Record<string, unknown>).document_key), request.supersedes_artifact_id ? String(request.supersedes_artifact_id) : null);
        next.title = request.title as string | null;
        next.media_type = String(request.media_type);
        next.metadata = request.metadata as Record<string, unknown>;
        artifacts = [next, ...artifacts]; revision += 1;
        if (loseFirstResponse && posts.length === 1) return Promise.reject(new Error("network timeout"));
        return envelope(next);
      }
      if (url.pathname === `/api/v2/objects/${objectId}/artifacts`) return envelope(artifacts);
      if (url.pathname === `/api/v2/objects/${objectId}`) return envelope({ id: objectId, kind: "source", revision, title: "Synthetic source", description: "Synthetic", protected: false });
      const match = url.pathname.match(/^\/api\/v2\/artifacts\/(.+)\/content$/);
      if (match) {
        const artifact = artifacts.find((item) => item.id === match[1])!;
        if (artifact.content === null) return Promise.resolve(new Response(JSON.stringify({ error: { message: "not found" } }), { status: 404 }));
        const offset = Number(url.searchParams.get("offset"));
        if (failLaterWindow && offset > 0) return Promise.resolve(new Response(JSON.stringify({ error: { message: "later window failed" } }), { status: 500 }));
        const chars = Array.from(artifact.content ?? "");
        const limit = Number(url.searchParams.get("limit"));
        const text = chars.slice(offset, offset + limit).join("");
        return envelope({ ...artifact, text, offset, next_offset: offset + Array.from(text).length < chars.length ? offset + Array.from(text).length : null });
      }
      throw new Error(`Unexpected ${url.pathname}`);
    }));
    return { posts, advance: () => { revision += 1; } };
  }

  it("shows three document rows and exact history links", async () => {
    const transcript = fixture("transcript", "transcript", "Evidence");
    const first = fixture("first", "research_notes", "First", "a");
    const latest = fixture("latest", "research_notes", "Second", "a", "first");
    const other = fixture("other", "research_notes", "Independent", "b");
    render(<Artifacts section="sources" objectId={objectId} artifacts={[latest, first, other, transcript]} onCreated={async () => {}} />);
    expect(screen.getAllByRole("link")).toHaveLength(3);
    expect(screen.getByRole("link", { name: /Lab notes.*2 versions/ })).toHaveAttribute("href", `/sources/${objectId}/working/a`);
  });

  it("reconciles an uncertain working-notes creation without a duplicate", async () => {
    const { posts } = server([], 4, false, true);
    render(<Artifacts section="sources" objectId={objectId} artifacts={[]} onCreated={async () => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "+ Add working notes" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Title" }), { target: { value: "New notes" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Artifact body" }), { target: { value: "Synthetic draft" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("network timeout");
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(window.location.pathname).toMatch(/\/working\//));
    expect(posts).toHaveLength(1);
  });

  it("keeps the secondary generic creation fields", async () => {
    const { posts } = server([]);
    render(<Artifacts section="sources" objectId={objectId} artifacts={[]} onCreated={async () => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "Add artifact" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Kind" }), { target: { value: "transcript" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Language" }), { target: { value: "en" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Capture outcome" }), { target: { value: "incomplete" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Reason when not complete" }), { target: { value: "Excerpt only" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Artifact body" }), { target: { value: "Synthetic excerpt" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(posts).toHaveLength(1));
    expect(posts[0]).toMatchObject({ kind: "transcript", language: "en", capture_outcome: "incomplete", capture_reason: "Excerpt only" });
  });

  it("loads a long Unicode document before editing and saves its complete tail", async () => {
    const original = "# Start\n" + "🙂 café line\n".repeat(1900) + "TAIL END";
    const notes = fixture("base", "research_notes", original, "doc");
    const { posts } = server([notes]);
    render(<ArtifactReader section="sources" objectId={objectId} target={{ kind: "latest", id: "doc" }} refreshKey={0} />);
    expect(await screen.findByText(/TAIL END/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    const editor = screen.getByRole("textbox", { name: "Working notes" });
    expect(editor).toHaveValue(original);
    fireEvent.change(editor, { target: { value: original.replace("# Start", "# Revised") } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(posts).toHaveLength(1));
    expect(posts[0]).toMatchObject({ expected_revision: 4, supersedes_artifact_id: "base", media_type: notes.media_type });
    expect(posts[0].content).toBe(original.replace("# Start", "# Revised"));
    expect(await screen.findByText(/TAIL END/)).toBeInTheDocument();
  });

  it("keeps a draft when a later content window fails", async () => {
    const notes = fixture("base", "research_notes", "x".repeat(20_001), "doc");
    server([notes], 4, true);
    render(<ArtifactReader section="sources" objectId={objectId} target={{ kind: "latest", id: "doc" }} refreshKey={0} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("later window failed");
    expect(screen.queryByRole("button", { name: "Edit" })).not.toBeInTheDocument();
  });

  it("keeps an edit draft after a revision conflict", async () => {
    const notes = fixture("base", "research_notes", "Original", "doc");
    const { advance, posts } = server([notes]);
    render(<ArtifactReader section="sources" objectId={objectId} target={{ kind: "latest", id: "doc" }} refreshKey={0} />);
    fireEvent.click(await screen.findByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Working notes" }), { target: { value: "My draft" } });
    advance();
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("changed while you were editing");
    expect(screen.getByRole("textbox", { name: "Working notes" })).toHaveValue("My draft");
    expect(posts).toHaveLength(0);
  });

  it("keeps unsaved edits across blocked navigation and refresh", async () => {
    const notes = fixture("base", "research_notes", "Original", "doc");
    server([notes]);
    window.history.replaceState({}, "", `/sources/${objectId}/working/doc`);
    const confirm = vi.fn(() => false);
    vi.stubGlobal("confirm", confirm);
    const { rerender } = render(<ArtifactReader section="sources" objectId={objectId} target={{ kind: "latest", id: "doc" }} refreshKey={0} />);
    fireEvent.click(await screen.findByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Working notes" }), { target: { value: "Unsaved" } });
    fireEvent.click(screen.getByRole("link", { name: /Back to Object/ }));
    expect(confirm).toHaveBeenCalledOnce();
    expect(window.location.pathname).toBe(`/sources/${objectId}/working/doc`);
    rerender(<ArtifactReader section="sources" objectId={objectId} target={{ kind: "latest", id: "doc" }} refreshKey={1} />);
    expect(screen.getByRole("textbox", { name: "Working notes" })).toHaveValue("Unsaved");
  });

  it("shows unsupported and URI-only fallbacks, and suppresses raw HTML", async () => {
    const uri = fixture("uri", "image", null);
    uri.media_type = "image/png";
    const uriText = fixture("uri-text", "text", null);
    server([uri, uriText]);
    render(<ArtifactReader section="sources" objectId={objectId} target={{ kind: "exact", id: "uri" }} refreshKey={0} />);
    expect(await screen.findByText(/No inline reader/)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /Open original URI/ })).toHaveAttribute("href", "https://example.test/original");
    expect(vi.mocked(fetch).mock.calls.filter(([url]) => String(url).includes("/content"))).toHaveLength(0);
    render(<ArtifactReader section="sources" objectId={objectId} target={{ kind: "exact", id: "uri-text" }} refreshKey={0} />);
    await waitFor(() => expect(screen.getAllByText(/No inline reader/)).toHaveLength(2));
    expect(vi.mocked(fetch).mock.calls.filter(([url]) => String(url).includes("/content"))).toHaveLength(1);
    const malicious = fixture("html", "research_notes", "<script>alert(1)</script>", "doc");
    server([malicious]);
    render(<ArtifactReader section="sources" objectId={objectId} target={{ kind: "latest", id: "doc" }} refreshKey={0} />);
    await screen.findByRole("heading", { name: "Lab notes" });
    await waitFor(() => expect(screen.queryByText("<script>alert(1)</script>")).not.toBeInTheDocument());
    expect(document.querySelector("script")).toBeNull();
  });
});

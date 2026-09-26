// Synthetic browser fixture for Issue #110. Use with agent-browser --init-script.
(() => {
  const objectId = "00000000-0000-4000-8000-000000000110";
  const object = {
    id: objectId, kind: "source", title: "Synthetic field interview",
    description: "A synthetic Source used to inspect artifact reading and editing.",
    protected: false, lifecycle: "active", revision: 4,
    created_by_type: "system", created_by_id: "fixture", updated_by_type: "system", updated_by_id: "fixture",
    provenance: {}, created_at: "2026-09-01T09:00:00Z", updated_at: "2026-09-01T09:00:00Z",
    archived_at: null,
  };
  const source = {
    ...object, object_id: objectId, source_kind: "document", canonical_uri: null,
    byline: "Synthetic author", publisher: null, published_at: null, published_at_precision: null,
    last_accessed_at: null, original_language: "en", original_media_type: "text/plain",
    original_artifact_reference: null, current_artifact_id: "00000000-0000-4000-8000-000000000111",
  };
  async function hash(text) {
    const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
    return Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, "0")).join("");
  }
  async function artifact(id, kind, title, content, mediaType, documentKey, predecessor, createdAt) {
    return {
      id, object_id: objectId, kind, title, uri: null, media_type: mediaType, language: "en",
      sha256: await hash(content), size_bytes: new TextEncoder().encode(content).length,
      capture_outcome: "complete", capture_reason: null, expected_size_bytes: null,
      semantic_indexing_enabled: false,
      metadata: documentKey ? { document_key: documentKey, predecessor_artifact_id: predecessor } : {},
      supersedes_artifact_id: predecessor, captured_at: null, created_at: createdAt,
      fixtureContent: content,
    };
  }
  const ready = (async () => {
    const transcript = await artifact("00000000-0000-4000-8000-000000000111", "transcript", "Original transcript", "Exact synthetic evidence.\nA second paragraph.", "text/plain; charset=utf-8", null, null, "2026-09-01T09:00:00Z");
    const first = await artifact("00000000-0000-4000-8000-000000000112", "research_notes", "Interview working notes", "# Interview notes\n\nThe first observation is synthetic.\n\n- Keep the source intact.\n", "text/markdown; charset=utf-8", "interview-notes", null, "2026-09-01T10:00:00Z");
    const second = await artifact("00000000-0000-4000-8000-000000000113", "research_notes", "Interview working notes", "# Interview notes\n\nThe revised observation is synthetic.\n\n- Keep the source intact.\n- Record each version.\n", "text/markdown; charset=utf-8", "interview-notes", first.id, "2026-09-01T11:00:00Z");
    const other = await artifact("00000000-0000-4000-8000-000000000114", "research_notes", "Analysis notes", "# Analysis\n\nAn independent working document.", "text/markdown", "analysis-notes", null, "2026-09-01T12:00:00Z");
    return [other, second, first, transcript];
  })();
  const response = (data, status = 200) => new Response(JSON.stringify(status < 400 ? { data } : { error: { message: data, code: "conflict" } }), { status, headers: { "Content-Type": "application/json" } });
  const realFetch = window.fetch.bind(window);
  window.__artifactFixture = { object, source, ready, advance: () => { object.revision += 1; source.revision = object.revision; } };
  window.fetch = async (input, init) => {
    const url = new URL(typeof input === "string" ? input : input.url, location.origin);
    if (!url.pathname.startsWith("/api/v2/")) return realFetch(input, init);
    const artifacts = await ready;
    const method = init?.method ?? "GET";
    if (url.pathname === "/api/v2/object-visuals") return response([]);
    if (url.pathname === "/api/v2/connection-graph") return response({ fingerprint: "synthetic", node_count: 1, connection_count: 0, nodes: [{ id: objectId, kind: "source", title: object.title }], edges: [] });
    if (url.pathname === "/api/v2/objects") return response([object]);
    if (url.pathname === "/api/v2/sources") return response({ items: [source], next_cursor: null });
    if (url.pathname === "/api/v2/notes") return response({ items: [], next_cursor: null });
    if (url.pathname === `/api/v2/objects/${objectId}`) return response(object);
    if (url.pathname === `/api/v2/sources/${objectId}`) return response(source);
    if (url.pathname === `/api/v2/objects/${objectId}/connections` || url.pathname === `/api/v2/objects/${objectId}/events`) return response([]);
    if (url.pathname === `/api/v2/objects/${objectId}/artifacts`) {
      if (method === "POST") {
        const body = JSON.parse(init.body);
        if (body.expected_revision !== object.revision) return response("Object revision conflict", 409);
        const next = await artifact(crypto.randomUUID(), body.kind, body.title, body.content, body.media_type, body.metadata.document_key, body.supersedes_artifact_id, new Date().toISOString());
        next.metadata = body.metadata;
        artifacts.unshift(next);
        object.revision += 1; source.revision = object.revision;
        const { fixtureContent, ...publicArtifact } = next;
        return response(publicArtifact, 201);
      }
      return response(artifacts.map(({ fixtureContent, ...publicArtifact }) => publicArtifact));
    }
    const match = url.pathname.match(/^\/api\/v2\/artifacts\/([^/]+)\/content$/);
    if (match) {
      const item = artifacts.find(value => value.id === match[1]);
      if (!item) return response("Artifact missing", 404);
      const offset = Number(url.searchParams.get("offset") ?? 0);
      const limit = Number(url.searchParams.get("limit") ?? 8000);
      const chars = Array.from(item.fixtureContent);
      const text = chars.slice(offset, offset + limit).join("");
      const { fixtureContent, ...publicArtifact } = item;
      return response({ ...publicArtifact, text, offset, next_offset: offset + Array.from(text).length < chars.length ? offset + Array.from(text).length : null });
    }
    return response(`Unhandled synthetic fixture route: ${url.pathname}`, 404);
  };
})();

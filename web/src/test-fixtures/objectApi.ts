import fixture from "./object-routes.json" with { type: "json" };

/** Synthetic HTTP fixture only: never connects to a Context service or database. */
export function createObjectFixtureApi({ emptyList = false }: { emptyList?: boolean } = {}) {
  const data = structuredClone(fixture);
  const subtypes = data.subtypes as Record<string, Record<string, unknown>>;
  const respond = (value: unknown, status = 200) => new Response(JSON.stringify(status === 200 ? { data: value } : { error: { message: value } }), { status, headers: { "Content-Type": "application/json" } });
  return async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const url = new URL(String(input), "http://synthetic.test");
    const path = url.pathname;
    const body = typeof init?.body === "string" ? JSON.parse(init.body) : {};
    if (path === "/api/v2/object-visuals") return respond([]);
    if (path === "/api/v2/read") {
      if (body.include?.includes("entity_categories")) return respond({ entity_categories: { categories: [{ id: "synthetic-category", label: "Organization", legacy_kind: "organization" }], revision: "1" } });
      const object = data.objects.find(o => o.id === body.object_ids?.[0]);
      return respond({ objects: object ? [{ object, subtype: subtypes[object.kind], corrections: [] }] : [] });
    }
    if (path === "/api/v2/connection-graph") return respond({ fingerprint: "synthetic", node_count: data.objects.length, connection_count: data.connections.length, nodes: data.objects, edges: data.connections });
    if (path === "/api/v2/objects") return respond(emptyList ? [] : data.objects.filter(o => (!url.searchParams.get("kind") || o.kind === url.searchParams.get("kind")) && o.title.toLowerCase().includes((url.searchParams.get("q") ?? "").toLowerCase())));
    const parts = path.split("/").filter(Boolean);
    const [, , section, id, suffix] = parts;
    if (section === "connections") return respond(data.connections.find(c => c.id === id));
    if (section === "artifacts") {
      const artifact = data.artifacts.find(a => a.id === id);
      if (!artifact) return respond("Artifact not found", 404);
      return respond({ ...artifact, text: data.content, offset: 0, next_offset: null });
    }
    const object = data.objects.find(o => o.id === id);
    if (!id) {
      const kind = ({ tasks: "task", notes: "note", sources: "source", themes: "theme" } as Record<string, string>)[section];
      const items = kind && !emptyList ? [subtypes[kind]] : [];
      return respond(section === "notes" || section === "sources" ? { items, next_cursor: null } : items);
    }
    if (!object) return respond("Object not found", 404);
    if (suffix === "connections") return respond(data.connections.filter(c => c.source_object_id === id || c.target_object_id === id));
    if (suffix === "events") return respond([]);
    if (suffix === "artifacts") return respond(data.artifacts.filter(a => a.object_id === id));
    if (suffix === "objects") return respond([data.objects.find(o => o.kind === "note")]);
    if (suffix === "routine") return respond({ routine: null, runs: [] });
    if (suffix === "identities") return respond([]);
    if (suffix === "messages") return respond([{ id: "synthetic-message", content: "Synthetic chat transcript message.", sender_title: "Synthetic user", sender_kind: "human", sender_user_object_id: "synthetic-user", source_created_at: object.created_at }]);
    if (init?.method === "PATCH") {
      if (body.expected_revision !== object.revision) return respond("Revision conflict", 409);
      Object.assign(object, body, { revision: object.revision + 1 });
      Object.assign(subtypes[object.kind], body, { revision: object.revision });
    }
    return respond(section === "objects" ? object : subtypes[object.kind]);
  };
}

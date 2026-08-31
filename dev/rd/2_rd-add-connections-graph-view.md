# 2 — RD: Add a Connections Graph View

**Status:** `scoped`
**Created:** 2026-08-31
**Centaur Context Task:** `08752bd6-d14d-4b01-8216-3ca062c74b70`
**Dependencies:** Active priority 1, because the graph must target the simplified
canonical Object and Connection contract that survives that RD.

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** `AGENTS.md`; `dev/AGENTS.md`; active priorities 1, 3, 4, and
5; the live `/objects` workspace; current navigation and durable routing;
Object, Connection, and visual API types; per-Object relationship and Connection
detail UI; UI/routing tests; and the live local profile of 398 active Objects and
1,367 active Connections.

**Missing:** none after active priority 1 completes.

1. Add one compact, read-only graph API that returns every active canonical
   Object and active Connection without per-node requests.
2. Add `Connections` to the application navigation and render an Obsidian-like,
   cluster-first graph at `/connections`, preserving `/connections/:id` as the
   durable Connection detail route.
3. Verify topology, prominence, navigation, interaction, accessibility, and
   performance on representative sparse and dense graphs; run all repository
   checks and record completion evidence.

## What We Are Doing

- [ ] Show all active Objects as nodes and all active Connections as lines in one
  dedicated Connections workspace.
- [ ] Make highly connected Objects and natural connected groups visually
  prominent while preserving Connection direction and meaning.
- [ ] Let a user find, focus, and open any Object or Connection from the graph.

## Contract

- **Goal:** Give users a simple visual map of the canonical Object graph, with
  the most connected Objects and clusters easiest to recognize.
- **Done:** `/connections` loads the complete active graph from one bounded API
  flow; stable clustering promotes high-degree nodes; every visible node and
  edge resolves to its canonical detail route; and the graph remains usable and
  understandable at desktop and narrow widths without relying on colour,
  animation, or hover.
- **Files:** Rust graph query/API types in `src/`; routing, API types, graph view,
  styles, and tests in `web/src/`; no migration unless active priority 1 proves
  an index is required; this RD.
- **Agent owns:** Read-only graph contract, layout and rendering, interaction,
  accessibility, targeted tests, performance proof, and local verification.
- **Requester owns:** Any later approval for editing Connections from the graph,
  persisted personal layouts, hosted access, or deployment.
- **Out of scope:** A graph editor, Connection creation/deletion, saved node
  positions, ontology changes, graph analytics, ranking search results by
  popularity, public ingress, and external visualization services.

## Graph Data Contract

- Add a human-listener read endpoint that emits compact nodes (`id`, `kind`,
  `title`) and directed edges (`id`, `source_object_id`, `target_object_id`,
  `kind`, `description`) for active records. Do not expose provenance, event
  history, subtype payloads, or a database-shaped row API.
- Return a consistent snapshot with stable ordering, a fingerprint/ETag, and
  counts. Loading must not make one Connection request per Object. If transport
  pagination is needed, bind its cursor to the snapshot and never silently omit
  nodes or edges.
- Exclude archived Objects and Connections in the initial view, matching the
  primary workspaces. Reject or omit any edge whose endpoint is not present in
  the same snapshot, and prove this behavior in tests.
- Query through the canonical Object and Connection API boundary after priority
  1; never couple the frontend to raw table names or access another database.

## Visual And Interaction Contract

- Add `Connections` beside the other first-class navigation items with correct
  expanded, collapsed, active, keyboard, direct-load, and browser-history
  behavior. `/connections` is the graph; `/connections/:id` remains Connection
  detail and now belongs to the Connections section rather than Objects.
- Use a restrained dark full-workspace graph inspired by Obsidian: small typed
  nodes, thin connecting lines, labels revealed when useful, generous empty
  space, and no decorative panels. Reuse the existing Object type visual
  language.
- Treat topology as undirected for clustering and degree prominence, while
  retaining source-to-target direction in edge styling, accessible text, and
  detail. Connected components form spatial clusters; higher-degree nodes pull
  nearer cluster centres and receive modestly stronger size/label prominence.
  Isolated Objects remain visible in a quiet outer group.
- Seed layout from stable IDs so the same snapshot settles consistently rather
  than reshuffling on every visit. Prefer a small maintained dependency or a
  focused local implementation based on measured bundle, frame-time, and
  interaction results; do not prescribe WebGL unless the fixture requires it.
- Support pan, zoom, fit/reset, pointer and keyboard focus, node search, and a
  concise legend. Selecting a node highlights its direct neighbourhood and
  reveals title, type, degree, and Object link. Selecting an edge reveals kind,
  description, both endpoints, and its Connection link. Escape clears focus.
- On narrow or non-pointer surfaces, retain the graph when usable and provide an
  ordered accessible cluster/neighbour list rather than shrinking labels into an
  unreadable miniature. Honour reduced motion, visible focus, contrast, text
  zoom, touch targets, and screen-reader summaries.

## Checks

- [ ] API tests prove complete stable snapshots, active-only filtering,
  endpoint integrity, pagination/fingerprint behavior if used, and no N+1
  requests.
- [ ] Layout tests prove deterministic connected components, degree prominence,
  isolated-node placement, direction preservation, and safe empty/single-node
  behavior.
- [ ] UI/routing tests cover navigation, `/connections` and
  `/connections/:id`, search/focus/reset, Object and Connection links, keyboard
  operation, loading/error/empty states, and accessible text equivalents.
- [ ] Browser checks cover the live-size graph plus dense and disconnected
  fixtures at desktop and narrow widths, reduced motion, text zoom, responsive
  interaction, acceptable frame time, and absence of console errors.
- [ ] The repository verification suite and `git diff --check` pass.

## Approval Boundary

This RD authorizes only a read-only local/private visualization and its supporting
human API. It does not authorize graph mutation, public ingress, deployment,
external services, credentials, hosted writes, or querying Centaur-owned
databases.

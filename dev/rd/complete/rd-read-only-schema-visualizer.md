# RD: Add a Read-Only Schema Visualizer

**Status:** `complete`
**Created:** 2026-08-28
**Completed:** 2026-08-29
**GitHub Issue:** #18

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** `AGENTS.md`; migrations through
`0009_interaction_evals.sql`; `src/api.rs`, `src/db.rs`, and the human/agent
router boundaries; `web/src/App.tsx`, `web/src/api.ts`, `web/src/routing.ts`,
`web/src/types.ts`, and `web/src/styles.css`; database contract tests; and the
completed description and Context Builder RDs.

**Missing:** none. The prerequisite RDs are complete and merged.

1. Re-audit every migration present at execution time; add a migration-owned
   table registry and human-only operations that introspect live PostgreSQL
   metadata and read paginated rows from registered application tables.
2. Add a restrained Schema workspace with a searchable catalog, automatically
   laid-out relationship map, focused table structure, and separate full-width
   row view.
3. Test safe dynamic discovery, schema changes, isolation, pagination, value
   rendering, responsive interaction, and read-only behavior; run all repository
   checks and record completion evidence.

## What We Are Doing

- [x] Add a `Schema` view that explains the complete live Centaur Context
  relational model, including first-class subtype and supporting tables.
- [x] Add `Schema` as a first-class option in the existing left navigation at
  `/schema`, alongside Objects, Tasks, Chats, Users, Entities, Memories, Curator
  Runs, and Evals.
- [x] Let a human on the trusted local/private UI inspect every column and
  paginated row in every Centaur Context application table without providing
  arbitrary SQL or mutation.
- [x] Make primary keys, foreign keys, nullability, defaults, constraints, row
  counts, and relationships understandable from the UI.
- [x] Update the catalog, columns, keys, constraints, and relationship map from
  the actual applied schema without a frontend rebuild or hand-authored diagram.

## Contract

- **Goal:** Give repository operators a trustworthy, exceptionally clean visual
  explorer for the complete Centaur Context schema and its stored data.
- **Done:** Applying a migration that registers, changes, renames, or removes an
  application table is reflected by the running Schema view after revalidation;
  every registered table is inspectable, all relationships come from live
  constraints, and foreign-key values navigate to the referenced row or Object.
- **Files:** One narrow registry migration; schema inspection and row-read
  code in `src/`; human UI routes; `web/src/`; targeted Rust, API, and UI tests;
  this RD.
- **Agent owns:** The read-only API contract, safe table discovery, stable
  pagination, schema/relationship visualization, responsive data grid, tests,
  and local verification.
- **Requester owns:** Approval for any future editing, raw-query console, export,
  hosted access, new human-authentication system, or exposure beyond the trusted
  local/private human UI.
- **Out of scope:** Schema or row editing, arbitrary SQL, migration execution,
  database administration, access to Centaur's `ai_v2` or Console databases,
  cross-database discovery, dashboards/charts, and agent/sandbox access.

## Dynamic Schema Contract

- PostgreSQL catalogs are the runtime source of truth for tables, columns,
  ordinal order, formatted types, nullability, defaults, identity/generated
  state, primary/unique/check constraints, foreign keys, and estimated row
  counts. Do not duplicate that metadata in TypeScript or a static diagram.
- Keep discovery safe with a migration-owned registry of inspectable table
  names. Register every existing application table in the visualizer migration;
  future table migrations register, rename, or remove their entry atomically.
  Intersect the registry with ordinary tables currently present in `public`.
  The registry itself and `_sqlx_migrations` are never inspectable.
- Validate the connected logical database as `centaur_context`, legacy
  `centaur_os`, or the disposable test patterns allowed by `AGENTS.md`. Never
  discover another schema or database, accept a client-supplied schema, or
  interpolate an identifier before exact server-side registry validation.
- Return a normalized schema fingerprint/ETag. Revalidate on entering Schema,
  window focus, a low-frequency background check, and explicit refresh. A
  changed fingerprint replaces metadata and redraws the map without a server
  restart; preserve selection when possible and clearly handle a removed table.
- Infer canonical subtype tables from the discriminator-preserving primary-key
  foreign key `(object_id, object_kind) → objects(id, kind)`; a supporting table
  such as `object_embedding_jobs` remains supporting even when its Object foreign
  key is also its primary key. New registered tables and foreign keys need no UI
  code. A database contract test keeps the registry synchronized and requires
  each browsable table to have a deterministic primary key.

## Visual Design Contract

- Add a `Schema` item to the existing collapsible navigation rail, with a quiet
  diagram/table icon, correct expanded and collapsed labels, active-page state,
  keyboard behavior, and direct routing to `/schema`. Opening or refreshing that
  URL must render the Schema workspace inside the existing application shell;
  selected tables and `Map`/`Structure`/`Rows` state should be deep-linkable
  without creating a separate application or browser surface.
- Use one calm workspace, not a dense administration dashboard: a narrow,
  searchable table catalog and one main surface with `Map`, `Structure`, and
  `Rows` views. Keep counts and refresh state quiet and secondary; use the
  existing dark palette, typography, spacing, and controls.
- Generate the default Map from foreign-key topology. Anchor `objects`, place
  inferred one-to-one subtypes beside it, arrange supporting tables in stable
  dependency layers, and put disconnected tables in a final utilities group.
  Layout is deterministic for the same fingerprint and needs no saved positions.
- Render compact table nodes with only name, classification, column count, and
  approximate row count. Use thin orthogonal connectors; distinguish subtype
  links from ordinary foreign keys without relying on colour alone. Reveal
  column names and cardinality on focus or selection, not on every edge.
- Selecting a table highlights its direct neighbourhood and mutes unrelated
  nodes instead of hiding them. A node, edge, catalog item, or foreign key is
  keyboard-operable and opens the same focused Structure view. Avoid decorative
  animation; use only brief transitions that preserve orientation.
- Structure presents the table name, classification, concise relationships, and
  one ordered column list. Small badges mark PK, FK, unique, nullable,
  generated/identity, and default properties; full constraint text is disclosed
  progressively instead of remaining visible.
- Rows is a separate full-width, horizontally scrollable grid so data never
  competes with the diagram. On narrow screens replace the map with an ordered
  relationship list and retain Structure/Rows; never shrink labels into an
  unreadable miniature. Preserve selection and scroll context across views.
- Meet reduced-motion, contrast, visible-focus, screen-reader relationship
  summaries, text zoom, and touch-target expectations. The schema remains fully
  understandable without colour, hover, or the SVG connector layer.

## Read-Only Data Contract

- Mount metadata and row endpoints only on the existing human listener. Expose
  no POST/PATCH/PUT/DELETE counterpart and do not add them to agent, ingestion,
  curator, sandbox, or standard-client surfaces.
- Use bounded keyset pages ordered by the complete primary key, with opaque
  cursors bound to the table and schema fingerprint. Never load a large table in
  one request; stale cursors fail clearly after a schema change.
- Return every column through a type-aware, lossless JSON/text representation.
  Distinguish null, empty string, booleans, numbers, UUIDs, timestamps, arrays,
  JSON, binary, vector/search, and unknown PostgreSQL types. Truncate only the
  preview; the unchanged value remains expandable and copyable.
- Foreign-key cells navigate to the referenced registered row. Subtype keys and
  references to `objects.id` also offer the normal canonical Object route.

## Checks

- [x] API/database tests prove registry synchronization, live add/alter/rename/
  drop reflection, fingerprint changes, subtype/FK inference, and rejection of
  unregistered schemas, tables, identifiers, and databases.
- [x] Row tests cover empty/composite-key tables, stable keyset pagination,
  concurrent inserts, stale cursors, supported value shapes, long values, nulls,
  FK navigation, and read-only methods/permissions.
- [x] UI tests cover deterministic layout, catalog search, selection,
  progressive constraints, refresh with preserved/removed selection,
  expansion/copying, Object links, the navigation item and `/schema` deep links,
  active/collapsed navigation states, and loading/error/empty/large-table states.
- [x] Browser checks cover a dense relationship fixture at desktop and narrow
  widths, keyboard-only use, screen-reader summaries, reduced motion, text zoom,
  overflow, and absence of console errors.
- [x] The repository verification suite and `git diff --check` pass.

## Verification Results

- Rust formatting, Clippy with warnings denied, 24 library tests, 13 API/auth
  tests, the curator evaluation, the disposable-database contract, and doc tests
  passed.
- Migration `0010` and dynamic add/alter/rename/drop discovery passed against
  `centaur_context_test_issue_18`; registry isolation, ETags, subtype inference,
  focused foreign rows, composite keys, concurrent inserts, stale cursors, and
  PostgreSQL value representations passed.
- All 40 web tests, TypeScript type-checking, and the production build passed.
- All 11 Python client tests and Python bytecode compilation passed. The local
  test environment supplied `pytest` because system Python did not include it.
- Browser verification passed at 1280px and 390px with live database data: Map,
  Structure, Rows, exact FK navigation, mobile table selection, text zoom,
  dynamic add/remove refresh, and page overflow checks passed; no error overlay,
  console errors, or page errors appeared.
- `git diff --check` passed.

## Approval Boundary

This RD authorizes only trusted local/private, human-listener, read-only
inspection of the `centaur_context` logical database. It does not authorize
querying Centaur-owned databases, exposing a database DSN, adding application login,
public ingress, deploying, exporting data, executing arbitrary SQL, or
changing/deleting any schema or row.

# RD: Add a Read-Only Schema Visualizer

**Status:** `backlog`
**Created:** 2026-08-28

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** `AGENTS.md`, `migrations/0001_initial.sql` through
`migrations/0006_context_curator.sql`, `src/api.rs`, `src/db.rs`, `src/search.rs`,
`src/curator.rs`, `web/src/App.tsx`, `web/src/api.ts`, `web/src/types.ts`, and
database contract tests.

**Missing:** none

1. Add narrowly constrained, human-only API operations that describe the
   application-owned schema and return paginated rows from an allowlisted table.
2. Add a Schema navigation view with a table catalog, relationship overview,
   column definitions, and a full-width data grid for the selected table.
3. Test schema isolation, identifier allowlisting, pagination, rendering of all
   supported PostgreSQL values, and read-only behavior; run the repository
   checks and record completion evidence.

## What We Are Doing

- [ ] Add a `Schema` view that explains the complete Centaur Context relational
  model, including first-class subtype and supporting tables.
- [ ] Let an authorized human inspect every column and paginated row in every
  Centaur Context application table without providing arbitrary SQL or mutation.
- [ ] Make primary keys, foreign keys, nullability, defaults, constraints, row
  counts, and relationships understandable from the UI.

## Contract

- **Goal:** Give repository operators a trustworthy visual database explorer for the complete
  Centaur Context schema and its stored data.
- **Done:** The UI discovers every application-owned table, visualizes its
  columns and relationships, and displays all of its rows through pagination;
  foreign-key values can navigate to the referenced row or canonical Object.
- **Files:** Narrow migrations only if required; schema inspection and row-read
  code in `src/`; human UI routes; `web/src/`; targeted Rust, API, and UI tests;
  this RD.
- **Agent owns:** The read-only API contract, safe table discovery, stable
  pagination, schema/relationship visualization, responsive data grid, tests,
  and local verification.
- **Requester owns:** Approval for any future editing, raw-query console, export,
  hosted access, or exposure beyond the authenticated local human UI.
- **Out of scope:** Schema or row editing, arbitrary SQL, migration execution,
  database administration, access to Centaur's `ai_v2` or Console databases,
  cross-database discovery, dashboards/charts, and agent/sandbox access.

## Detailed Requirements

- Discover tables from the `public` application schema but enforce a server-side
  allowlist derived from Centaur Context migrations. Never accept a client-supplied
  schema or interpolate an unvalidated table or column identifier.
- Exclude PostgreSQL catalogs, `information_schema`, migration bookkeeping,
  credentials, environment state, and every logical database except
  `centaur_context` or a disposable `centaur_context_test` database.
- Show tables such as Objects, subtype tables, Connections, external identities,
  Chat Messages, Curator Runs, Curator Run Changes, Object Embeddings, Object
  Embedding Jobs, and Object Events even when they do not have a dedicated
  product navigation view.
- Distinguish canonical first-class Objects from supporting records. The schema
  diagram must show one-to-one subtype keys and ordinary foreign-key
  relationships accurately rather than implying every table row is an Object.
- The row grid shows every column. Long text and JSON use a truncated preview
  with an expandable full value; null, timestamps, booleans, UUIDs, and arrays
  remain unambiguous and copyable.
- Use bounded page sizes and a deterministic primary-key ordering. Large tables
  must not be loaded into browser memory in one request; the user can continue
  through every page.
- Keep this surface read-only at both the UI and HTTP layers. It must not be
  exposed through the standard agent client or an interactive sandbox token.

## Checks

- [ ] API tests prove only allowlisted Centaur Context tables can be inspected and
  arbitrary schema/table identifiers are rejected.
- [ ] Database-backed tests cover table/column metadata, foreign keys, empty
  tables, stable pagination, JSON, long text, and null values.
- [ ] UI tests cover table selection, schema relationships, expansion/copying,
  canonical Object links, loading, error, empty, narrow, and large-table states.
- [ ] The repository verification suite and `git diff --check` pass.

## Approval Boundary

This RD authorizes only local, authenticated, read-only inspection of the
`centaur_context` logical database. It does not authorize querying Centaur-owned
databases, exposing a database DSN, adding public ingress, deploying, exporting
data, executing arbitrary SQL, or changing/deleting any schema or row.

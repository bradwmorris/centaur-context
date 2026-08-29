# RD: Add Canonical Sources, Long-Form Content, and Notes

**Status:** `complete`
**Created:** 2026-08-30
**GitHub Issue:** `#20`

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Requester confirmation of zero users and installed databases;
current Source-to-Memory migrations; Object/subtype enforcement; search; human
and agent APIs; React UI; Curator; schema visualizer; compatibility metadata;
and the standard read-only agent client.

**Missing:** none. The requester has explicitly chosen a clean pre-installation
schema history over compatibility with disposable development databases.

1. Rewrite the pre-release ontology migration history so `source` is a canonical
   Object kind from the clean baseline, with no Source-to-Memory conversion or
   compatibility backfill.
2. Add Source metadata and versioned long-form content contracts, then implement
   their database, domain, human UI, Curator, search, and read-only agent paths.
3. Keep Note canonical in the clean baseline, add a bounded Markdown/plain-text
   subtype and Notes UI, and expose search/read plus a separately credentialed,
   attributed, idempotent agent Note-write path.
4. Recreate disposable databases, update contracts and documentation, and prove
   Source-to-insight and agent-Note flows with synthetic fixtures.

## What We Are Doing

- [x] Make Source a reusable canonical Object subtype for articles, papers,
  podcasts, videos, books, reports, documents, datasets, and web pages.
- [x] Store normalized article text and transcripts durably without placing
  long content in `objects.description` or the Source metadata row.
- [x] Let humans and authorized agents find and read Sources through bounded
  APIs, and connect derived Memories or Tasks to them.
- [x] Remove the misleading pre-release history that creates Source and then
  converts it to Memory.
- [x] Keep Note as a canonical subtype whose useful content is not overloaded
  into the short Object description.
- [x] Let explicitly authorized agents create bounded, audited Notes without
  granting database access or widening the general read-only Context API.

## Contract

- **Goal:** Add clean, broadly useful canonical Source and Note models that
  support long-form evidence and the Enyu research POC without an Enyu-specific
  schema.
- **Done:** On a fresh database, a human or Curator can create a Source with
  metadata and versioned text; the UI and standard agent client provide bounded
  search/read access; a derived Memory can cite it; humans and separately
  authorized agents can create bounded audited Notes; synthetic tests pass; and
  no migration converts Source or Note into Memory.
- **Files:** Pre-release migrations and registry; Rust domain, database, search,
  API and Curator; React Source and Note UI; standard agent client; tests and
  docs.
- **Agent owns:** Clean baseline schema history, reusable Source and Note
  implementation, synthetic fixtures, documentation, and local verification
  after execution is explicitly requested.
- **Requester owns:** Real source selection, copyrighted-content policy,
  credentials, external extraction, production data, deployment, publication,
  and any later Enyu workflow or site integration.
- **Out of scope:** Existing-database compatibility; real data; external fetch or
  transcription; blob storage; publishing; agent writes other than the explicit
  Note-create grant; and data modules.

## Canonical Note Contract

- `note` remains a canonical `objects.kind`; every Note has exactly one `notes`
  subtype row from the clean baseline, and no migration converts Notes to
  Memories.
- `objects.title` and `objects.description` provide concise identity and summary.
  `notes.content` stores bounded useful plain text or Markdown with an explicit
  format; it is indexed for search and rendered without unsafe HTML.
- Humans receive Notes list, detail, search, and create surfaces. General agent
  Context access receives bounded search/read only.
- Agent Note creation uses a separate credential and internal listener/grant,
  requires principal and thread attribution plus an idempotency key, validates
  content size and format, and emits immutable Object events. The normal agent
  read credential cannot write Notes and agents never receive a database DSN.

## Canonical Source Contract

### Identity and metadata

- `source` remains a canonical `objects.kind`; every Source has exactly one
  `sources` subtype row keyed by `object_id` and enforced like the other
  one-to-one subtypes.
- `objects.title` names the work and `objects.description` gives a short useful
  summary. Neither stores the complete article or transcript.
- `sources` stores bounded metadata: kind, optional canonical HTTP(S) URI,
  byline, publisher, publication/access time, language, media type, artifact
  reference, and content hash.
- Canonical Object revisions, provenance, lifecycle, events, protection, search,
  visuals, and Connections apply to Sources normally.

### Long-form content

- `source_contents` stores immutable versioned normalized text separately, with
  Source ID, content kind, language, extraction method/version, hash, size,
  creation time, and optional artifact reference.
- A PostgreSQL `text` value may hold one complete normalized article or
  transcript. APIs must never include it accidentally in Source list responses.
- Original PDF, audio, video, image, or other binary bytes are not stored in the
  row. Store an opaque managed-artifact reference and integrity hash; resolving
  that reference is outside this RD.
- Re-extraction appends a version and selects it as current without overwriting
  prior evidence.
- Bounded reads use stable cursors. Full-text search returns small attributed
  excerpts. Optional locators hold pages or timestamps; automatic segmentation
  and passage embeddings are deferred.

### Human, Curator, and agent behavior

- Add a Sources pane with list, detail, metadata editing, content-version paste,
  bounded preview, connections, validation, and error states.
- Human writes and Curator writes use existing separate trusted paths and create
  auditable Object events. Interactive agents remain read-only.
- Curator may create or reconcile a Source only from explicit evidence. Every
  run still creates its primary Memory; Source and Memory are distinct.
- Add bounded agent operations for Source metadata/content search and selected
  content windows; never return an unbounded work by default.
- A Memory or Task derived from research uses `derived_from` to reference the
  canonical Source, while Curator-created Objects also retain required evidence
  back to their source Chat.

## Pre-Release Migration Cleanup

- Rewrite early migrations directly: exclude Source and Note from the legacy
  conversion, retain both Object kinds, and create their subtypes. Do not
  “reintroduce” either kind.
- Add content tables within the rewritten history; update subtype enforcement,
  schema registration, fixtures, and assertions.
- Recreate verification databases; resolve old SQLx checksums by recreation,
  not compatibility SQL.
- Update schema version metadata and package checks to match the final migration
  history. Do not claim upgrade compatibility from the abandoned baseline.

## Checks

- [x] A brand-new disposable database has Source as a canonical subtype and no
  Source-to-Memory conversion or orphan subtype path.
- [x] Database tests cover constraints, versions, long text, hashes, bounded
  search/reads, locators, Connections, events, and deletion restrictions.
- [x] API and agent-client tests cover authentication, pagination, content-size
  bounds, invalid offsets, missing versions, and read-only enforcement.
- [x] Curator tests distinguish Sources from Memories and preserve Chat evidence.
- [x] UI tests cover Source navigation, metadata, content preview/versioning,
  connections, and large-content loading behavior.
- [x] Synthetic article and transcript fixtures prove retrieval and a
  `derived_from` Memory without copyrighted material.
- [x] Note tests cover subtype enforcement, content bounds and formats, search,
  human creation, separate agent-write authorization, attribution, denial, audit
  events, and idempotent retry.
- [x] All repository-root verification commands and `git diff --check` pass.

## Verification Results

- Fresh PostgreSQL 16/pgvector database `centaur_context_test_20`: full Rust
  suite passed, including migrations through schema version 10 and database/API
  integration coverage.
- `cargo fmt --check`, Clippy with warnings denied, and all 41 Rust tests passed.
- 48 React tests, TypeScript type-check, and production web build passed.
- 36 standard agent-client tests and Python compileall passed.
- Deployment manifests passed client-side Kubernetes validation; `git diff
  --check` passed.

## Approval Boundary

Planning does not authorize implementation. Later execution may rewrite
pre-release migrations and recreate explicitly disposable Context databases
because there are no users or installs. It does not authorize touching Centaur
databases, real content, external fetch/transcription, credentials, deployment,
spend, publishing, ingress, or deleting an unconfirmed database.

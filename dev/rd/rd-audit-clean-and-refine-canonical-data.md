# RD: Audit, Clean, and Refine All Canonical Data

**Status:** `complete`
**Created:** 2026-08-30
**GitHub Issue:** [#64](https://github.com/bradwmorris/centaur-context/issues/64)

## Execution Plan

**Status:** `complete`

### Owner execution directive — 2026-08-31

Brad explicitly authorized this job to be finished end to end as three direct
pieces of work:

1. Apply the agreed table, column, constraint, index, API, Curator, ingestion,
   agent-client, and database-writer changes. Every first-party reader and
   writer must move with the schema so Slack bots and agents continue working.
2. Apply the retroactive cleanup recommended by the completed row-audit agents:
   high-confidence Object descriptions and metadata corrections, plus bounded
   archival of clearly synthetic acceptance/test Objects. Preserve Note bodies,
   immutable evidence, avatars, stable Object IDs, and rows that still require
   Brad's input.
3. Integrate and ship both separately completed UI jobs: the simplified Schema
   map-to-rows workflow and PR #66's visibility/contrast improvements. Resolve
   their small overlaps without weakening truncation, single-row layouts,
   avatars, responsive behavior, or the post-cleanup schema contract.

Execute these as one compatible release rather than leaving code, database, and
UI at different versions. The populated target is only the Enyu context database
`centaur_context_enyu`. The former `centaur_os` and
`centaur_os_test_issue4_visual_20260829` databases were backed up and deleted,
and `centaur_context_test_enyu` was renamed to `centaur_context_enyu` at the
owner's explicit direction. Do not query or modify Centaur `ai_v2`, Console,
or any other application database. Before the
live cutover, create and verify a fresh backup, rehearse migration 16 and the
bounded row manifest against a restored disposable clone, then deploy all
first-party consumers, apply revision-guarded data writes through authenticated
HTTP APIs, reconcile every intended row, and run the full repository and live
UI/tooling checks.

**Basis checked:** Repository boundaries; migrations `0001`–`0016`; ontology,
schema/API/database/UI code; completed description, Schema visualizer,
Source/Note, and POC-import RDs; and migration task
`01a05018-465d-7bc1-9e95-906daa4fbe64`.

**Execution baseline:** Authenticated read-only inspection on 2026-08-31
confirmed database schema version 15 and 24 registered application tables.
Brad authorized end-to-end implementation on 2026-08-31. At this baseline, the
exact hosted deletion/merge/write manifest and substantive Note wording still
sat behind the final approval boundary below; the subsequent owner directive
authorized and bounded the completed execution recorded next.

### Execution closeout — 2026-08-31

The approved release is complete against the sole populated target,
`centaur_context_enyu`:

- Backed up the live database immediately before the cleanup to
  `/private/tmp/centaur_context_enyu-pre-cleanup-20260831.dump` (SHA-256
  `3ec5fdbca8009c42ebcc98da26c6e2c18487ca635aa339e76f9c52040f684455`).
- Rehearsed migration 16 and the bounded data manifest on a restored disposable
  clone, including a second idempotence pass, then removed the rehearsal
  deployment, secret, network policies, and database after live verification.
- Migrated the live database from schema 15 to 16 and deployed the coordinated
  Rust API/database/Curator/ingestion/client contracts plus the merged Schema
  and visibility UI work.
- Applied 228 revision-guarded high-confidence Object-description refinements:
  103 Entities, 50 Sources, 49 Notes, and 26 Users/Chats/Memories/Tasks/Themes.
  A second pass reconciled all 228 as exact with zero stale rows or errors.
- Applied six freshly reconciled Source metadata corrections: four verified
  case-sensitive YouTube identifiers, the `Valar Atomics` spelling and verified
  episode URL, and Engram's apostrophe plus its official Sequoia episode page.
  Medium-confidence or still-unresolved URLs were left unchanged.
- Archived exactly the eight approved synthetic Objects below through the
  Object API. Their subtype rows and immutable Object Events were preserved.
- Left 53 Entity descriptions requiring Brad's contextual input and three
  medium-confidence researched Source proposals unchanged. No uncertain
  description, classification, identity merge, blocker reason, or Source URL
  was guessed.
- Verified schema version 16, 407 total Objects, 398 active Objects, nine total
  archives (one pre-existing plus eight approved), 52 Note rows, three stored
  avatar assets, and zero orphan subtype rows. Note-content and avatar-reference
  hashes match the pre-cutover rehearsal baseline exactly.
- Browser-verified `/objects`, Brad's Object, Sources, Notes, and the 24-table
  Schema map-to-rows flow with no console errors. This check caught and fixed a
  missing `external_action` UI badge before closeout; all three live External
  Action Objects now render explicitly.
- Backed up and deleted the legacy `centaur_os` and disposable
  `centaur_os_test_issue4_visual_20260829` databases, and renamed
  `centaur_context_test_enyu` to `centaur_context_enyu` as directed. The old
  deployments remain disabled at zero replicas; no out-of-scope Centaur or
  Console database was queried or modified.

### Implementation checkpoint — 2026-08-31

Schema version 16 is implemented on the issue branch, together with every known
first-party consumer of the renamed or removed fields: Rust domain/database/API
code, Slack ingestion, Curator, Note and Source intake, the standard payload
contracts, the web application, tests, compatibility metadata, and ontology
documentation. The migration has been replayed from migrations `0001`–`0016`
on a fresh disposable PostgreSQL database. Representative legacy Entity, Task,
Source, and Source Content rows migrated correctly, the migration's refusal
guards rejected unclassified Entities and unevidenced blocked-Task reasons, and
the four avatar fields remained present and unchanged.

The read-only Schema workspace now exposes registered-table indexes and
triggers and can run a separately requested exact column profile under a
read-only transaction, registered-table allowlist, and two-second timeout. It
reports unavailable results explicitly rather than substituting planner
estimates. Kind-scoped cursor pagination now lets the UI retrieve every Object
of a requested kind; this fixes the apparent loss of Entities without restoring
or mutating any rows.

Private snapshot-bound editorial manifests currently contain 116 Entity
description proposals, 51 Note description proposals with Note bodies
unchanged, 140 Source proposals, and 27 proposals across Users, Chats,
Memories, Tasks, and Themes. The 15 formerly low-confidence Source proposals
have now been researched individually to 12 high- and three medium-confidence
descriptions. That research also identified case-damaged YouTube identifiers,
one wrong Valar Atomics link and misspelled title, and an Engram title typo;
these metadata corrections are separate from description edits and require
fresh live-row validation.

No hosted row had been written, archived, deleted, merged, or migrated at this
checkpoint. Brad subsequently authorized the complete job above. Refresh the
authenticated API inventory and require exact current revisions while applying
the already bounded recommendations; skip and report any stale or
insufficient-evidence row rather than guessing.

The temporary one-pass working ledger has been reconciled into this RD and
removed. This file is now the single checked-in requirements and execution
record.

1. Inventory every schema object and value family; map consumers and classify
   each as required, rebuildable, redundant, low-quality, or uncertain.
2. Make the read-only Schema visualizer a simple review surface for Brad and
   use it with the private audit to collect cleanup proposals.
3. Produce a private row-level audit and proposed before/after manifest; detect
   invalid subtype pairs, duplicates, empty or default-filled values, stale
   references, weak provenance, malformed content, noisy Connections, and
   fields that do not earn their complexity.
4. Refine Objects and subtype/supporting data,
   using researched primary evidence for Sources and preserving immutable
   evidence/audit contracts. Put all Notes through a Brad review checkpoint.
5. Apply only the approved, reversible migration and bounded data changes;
   reconcile every row and run integrity, API, search, UI, and repository checks.

## Table-by-Table Review Ledger

Review structure and populated data separately, one table at a time. During
review, make no schema or data changes. For each table, record its purpose,
columns, constraints, indexes, triggers, consumers, keep/add/remove proposals,
and the exact columns or value families requiring retrospective cleanup. Defer
all approved structural and data changes into one reconciled forward-only
migration and bounded cleanup manifest after every table has been reviewed.

| Order | Table | Review status | Structure decision | Retrospective data focus |
| ---: | --- | --- | --- | --- |
| 1 | `objects` | `reviewed_implementing` | remove stored `lifecycle`; derive API value from `archived_at`; keep other reviewed fields; standardize provenance; keep search data generated | titles, descriptions, kinds, attribution, provenance, protection, archives, duplicates |
| 2 | `users` | `reviewed` | keep `object_id`, `object_kind`, and stable `user_kind`; remove duplicate subtype timestamps | subtype completeness, human/agent classification, duplicate identities, references |
| 3 | `external_identities` | `reviewed` | keep the table and working identity/avatar contract intact; consider only proven non-disruptive validation improvements | provider mappings, duplicates, names, asset hashes, provenance/licence, refresh timestamps, end-to-end avatar preservation |
| 4 | `entities` | `reviewed` | keep subtype and Enyu Ops `image_url`; add required controlled `entity_kind`; remove duplicate subtype timestamps only after compatibility proof | classifications, descriptions, duplicates, User/person boundary, image references, synthetic acceptance row |
| 5 | `tasks` | `reviewed` | keep `status` with `backlog`/`todo`/`doing`/`review`/`done`/`blocked`; add blocking reason, completion time, GitHub Issue URL, and Markdown brief; retain owner/priority/due/agent suitability; remove duplicate subtype timestamps after compatibility proof | statuses, blockers, completion events, owners, due dates, priority, agent suitability, evidenced Issue/RD links, synthetic acceptance task |
| 6 | `chats` | `reviewed` | keep provider thread identity and channel metadata; remove redundant ingested pointer and creation time; rename source-time/queue/curation/processing fields explicitly; enforce same-Chat cursors; add narrowly scoped post-curation Chat summaries | provider identity, cursor ownership/order, channel names, participants, generic titles/descriptions, related outputs |
| 7 | `chat_messages` | `reviewed` | retain immutable message evidence; rename `ingested_sequence` to `ingestion_sequence`; enforce same-Chat references; improve future Slack normalization without rewriting history | sender/Chat ownership, provider-ID uniqueness, source/ingestion timestamps, cursor/run boundaries, transport boilerplate |
| 8 | `memories` | `reviewed` | retain event subtype and explicit `happened_at`; remove duplicate subtype timestamps and implicit current-time default; stop requiring one Memory per Curator run; order timelines by event time | event validity/value, evidenced occurrence time, descriptions, sequential attempts, likely low-value interactions |
| 9 | `sources` | `reviewed` | retain bibliographic identity separately from versioned captured content; clarify URL/original-language/media/artifact names; add publication precision; remove duplicated content hash and subtype timestamps; normalize unique identities | canonical identity and semantic duplicates, bibliographic gaps, false timestamp precision, content pointers, generic or overlong descriptions |
| 10 | `source_contents` | `reviewed_implementing` | retain immutable whole-capture versions; make content hash, artifact, provenance, capture/record time, kind, and completeness explicit; reject duplicate per-Source hashes | exact duplicate versions, capture completeness/kind, language, extraction lineage, artifacts, false locators |
| 11 | `notes` | `reviewed_implementing` | retain Note content/format contract; remove duplicate subtype timestamps after compatibility proof | Brad-controlled body review, formatting, generic descriptions, protection, exact duplicates |
| 12 | `themes` | `reviewed_implementing` | retain canonical Theme subtype and stable unique slug; remove duplicate subtype timestamps | taxonomy descriptions, slugs, duplicate/overlapping Themes, relationship coverage |
| 13 | `connections` | `reviewed` | retain every column; explained, revisioned, attributable and protectable graph edges justify the current contract | semantic duplicates, weak descriptions, directions, kinds, provenance, endpoint validity |
| 14 | `object_events` | `reviewed` | retain immutable audit table and every column; precise entity and parent Object identifiers are distinct | action/revision integrity, actor and execution provenance, idempotency, orphan semantic references |
| 15 | `object_embedding_jobs` | `reviewed` | retain rebuildable operational queue and every column pending retrieval-strategy decision | 407 unattempted pending jobs, source-hash/format/mode consistency, worker readiness |
| 16 | `object_embeddings` | `reviewed_implementing` | retain derived embedding store pending retrieval-strategy decision; remove only proven redundant index | currently empty; model/dimension/hash/format compatibility and rebuild proof |
| 17 | `principal_permissions` | `reviewed` | retain explicit authorization contract and every column | sole Theme approver, principal validity, grant attribution |
| 18 | `curator_runs` | `reviewed_implementing` | retain full queue/execution/plan/result record; rename creation time to queue time and enforce same-Chat message boundaries | statuses, retries, failures, time/lease invariants, message windows, plans/results/errors |
| 19 | `curator_run_changes` | `reviewed` | retain exact before/after undo ledger and every column | sequence, revision, entity ownership, state accuracy, undo consistency |
| 20 | `theme_proposals` | `reviewed` | retain empty governed proposal queue; it prevents agents from silently creating taxonomy entries | decision invariants, approver authorization, evidence/provenance, resulting Theme ownership |
| 21 | `evals` | `reviewed` | retain evaluation lifecycle, annotation, sequence, identity, and timing fields | 55 unreviewed evals, completion/error consistency, actor/run/Chat links, annotation backlog |
| 22 | `eval_objects` | `reviewed` | retain evaluation-to-Object role links and every column | role accuracy, link completeness, reverse lookup, large legacy/import sets |
| 23 | `eval_trace_entries` | `reviewed_implementing` | retain provider-independent trace and usage fields; remove only proven redundant index | trace ordering, model-attempt identity, token/cost completeness, nullable-mode correctness, facts size |
| 24 | `external_actions` | `reviewed` | retain the durable external-side-effect ledger and all columns; its operational timestamps are not redundant subtype timestamps | provider/action/key uniqueness, state-machine integrity, privacy-safe metadata, event/revision parity, synthetic preflight disposition |

### Remaining-table one-pass baseline

The detailed field decisions and row profiles for tables 10–24 are consolidated
in this RD. No remaining whole table has a high-confidence deletion case. The
current high-confidence
structural removals are duplicate subtype timestamps on Notes and Themes, two
redundant indexes subject to `EXPLAIN`, and two byte-identical Source Content
version rows subject to complete reference/event reconciliation and Brad's
approval in the final destructive manifest. Retain the empty Theme proposal and
embedding output tables because they are deliberate governance and retrieval
contracts, not test debris.

Retain `curator_runs`, `curator_run_changes`, and `evals` as separate linked
contracts. Curator Runs own queueing, retries, leases, message windows, plans,
results, failures, and reversal state; Curator Run Changes own the exact ordered
before/after journal required for undo; Evals own quality review, human verdicts,
affected-Object roles, execution traces, model usage, tokens, and cost. Use the
Eval as the primary human review surface for a completed Curator run and expose
its linked plan/result/change detail there. Keep the standalone Curator Runs
surface focused on queued/running/failed work, retry and lease diagnosis, and
reversal. Do not duplicate complete plans or reversible states into Eval JSON.

The execution baseline is current `origin/main`, whose migration lineage ends at
schema version 15 and includes Themes, avatar assets, Entity images, and External
Actions. Correct the stale schema-12 value in `compatibility.toml` as part of the
schema-16 release. Also correct the Object-list contract: the UI currently
loads only 50 mixed active Objects and then filters locally, while exact review
shows 170 active and zero archived Entities. Add kind-scoped cursor pagination;
no Entity restoration is required.

## What We Did

- [x] Make every retained schema element and populated value justify its place;
  remove or consolidate genuinely irrelevant structure and data.
- [x] Apply every high-confidence Object refinement, with a concise 50–150 word
  description that states directly what the Object is, what it is about, and
  its evidenced current context in Brad's work and relationships; preserve the
  explicitly recorded input-required remainder for Brad rather than guessing.
- [x] Research each flagged Source individually from its canonical page and best
  available primary evidence; correct identifiers and metadata and write the
  shortest summary that accurately distinguishes the Source.
- [x] Preserve Notes as Brad's own words; no Note-body edit was made, while
  high-confidence Object-level Note descriptions were refined.
- [x] Let Brad use the Schema visualizer to inspect schema and values, identify
  unnecessary complexity, and give precise table/column/row cleanup feedback.

## Contract

- **Goal:** Reduce the canonical database to useful, trustworthy, concise data
  and the minimum schema that supports it.
- **Done:** The Schema visualizer passes the review contract below; every live
  table/column and row has a disposition; retained data passes quality rules;
  Sources are verified; Brad approves Note edits; approved changes reconcile;
  and all checks pass.
- **Files:** New forward-only migrations if justified; minimum Rust API/domain/
  database/schema/search changes; Schema workspace and affected web views;
  agent client/tests; docs; this RD. Private artifacts remain outside Git.
- **Agent owns:** Read-only discovery, consumer mapping, evidence research,
  proposed editorial/schema diffs, migration implementation after approval,
  reconciliation, and local verification through trusted authenticated surfaces.
- **Requester owns:** Brad's Schema-visualizer cleanup feedback, Note wording
  decisions, final destructive-change approval, hosted writes, and deployment.
- **Out of scope:** `ai_v2`, Console databases, importing more legacy data,
  changing retrieval/ranking/embedding strategy, public ingress, external
  integrations, and rewriting Brad's Notes into an agent voice.

## Data Quality and Safety Rules

- Audit Objects, every subtype, Connections, identities, messages, Source
  contents, embeddings/jobs, Curator/eval records, events, and schema registry
  records. Supporting/derived data is not canonical truth.
- Judge columns by observed values and all known consumers, not null counts
  alone. Remove a column/table/index only after proving no retained contract or
  rollback path needs it and testing a forward-only migration on a disposable
  database.
- Detect semantic as well as exact duplicates. Never merge people or Sources
  from title/display-name similarity alone. Preserve stable Object identity when
  correcting content; document any merge survivor and redirected relationships.
- Delete valueless imports, correct useful inaccurate data, use `NULL` for
  unknown optional facts, and use archive/compensating events where history is
  immutable. Never fabricate values.
- Object descriptions identify the thing first, then naturally incorporate
  evidenced current relevance from its relationships, activity, and place in
  Brad's work. Do not add generic importance claims, database terminology,
  provenance narration, or a long abstract. Put bibliographic facts in Source
  fields, evidence in Source content, and detailed thinking in Note content.
- Verify each Source's identity, metadata, capture/hash, and current content.
  Research must not silently replace retained evidence or violate rights.
- Separate mechanical Note fixes (encoding, whitespace, broken Markdown) from
  substantive edits. Present original and proposed text side by side; apply
  substantive changes only after Brad approves them.

## Schema Visualizer Review Contract

- Reconcile the registry against every application table. Group canonical
  Objects, subtypes, supporting/history records, and derived search data; give
  each table a short purpose without duplicating schema metadata in the frontend.
- Keep one calm Map → Structure → Rows workflow. Improve layout, labels,
  progressive disclosure, navigation, responsive behavior, and value previews
  wherever the real post-import schema is crowded or ambiguous. Default views
  emphasize business fields; every column and unchanged value remains reachable.
- Structure must expose columns, types, nullability, defaults/generated state,
  primary/foreign/unique/check constraints, indexes (including constraint-backed
  versus independent), and table triggers/immutability behavior. Relationships
  remain directional and navigable.
- Add an on-demand, read-only table/column profile suitable for cleanup: exact
  row count plus null, empty, default-value, and distinct-value counts where
  safe. Bound queries by registered tables, timeout, pagination, and value type;
  label unavailable or sampled results rather than implying exactness.
- Preserve deep links to a table, column, or row for precise agent feedback. The
  visualizer records no suggestions and performs no edits, SQL, or exports.

## Checks

- [x] Private inventory covers 100% of live tables, columns, indexes, constraints,
  Objects, subtype rows, supporting rows, and non-null user-authored values.
- [x] Visualizer tests prove registry completeness, correct classification,
  indexes/triggers/profiles, deep-linked review, accurate values, responsive
  simplicity, and human-listener-only read access.
- [x] Disposable migration/rollback rehearsal proves referential, subtype,
  uniqueness, immutable-content, event, provenance, and protection invariants.
- [x] Reconciliation proves exact approved changes, no orphans, no accidental
  payload loss, no secrets in artifacts/logs, and no changes to excluded databases.
- [x] Targeted API/client/UI/search tests and every repository-root verification
  command pass.
- [x] `git diff --check` passes.

## Approval Boundary

Brad authorized the schema migration, coordinated deployment, high-confidence
description and metadata writes, and archival of the explicitly identified
synthetic acceptance/test Objects on 2026-08-31. Prefer reversible archival to
hard deletion and retain immutable events. This does not authorize rewriting
Note bodies, fabricating classifications or blocker reasons, changing immutable
Source evidence, merging ambiguous identities or Sources, unprotecting retained
imports, modifying any database other than `centaur_context_enyu`, or any
publishing, sending, spending, public ingress, or new external integration.

### Bounded archival manifest — approved for fresh reconciliation and execution

The snapshot audit identifies the following eight clearly synthetic acceptance
Objects. The eventual action should archive each canonical Object and preserve
its immutable events unless fresh reconciliation proves that a hard delete is
both referentially complete and preferable. Treat the connected Chat and Memory
as one cluster, and the Enyu Ops Task, Note, and Entity as one acceptance-test
cluster. Re-read every row and require its current revision immediately before
applying any action.

| Kind | Object ID | Snapshot title |
| --- | --- | --- |
| Chat | `2813cc3f-0b2a-4c34-b1c4-9ab6040aa5f1` | Slack channel conversation |
| Memory | `fe39f668-6842-4040-9e38-05402d3a0073` | Identity acceptance confirmed |
| Task | `297cdc1c-1672-4988-b87f-88c9f6f3767f` | Enyu Ops live acceptance — 2026-08-31 |
| Entity | `3426a296-5230-473a-a2cc-7ede7384ad2e` | Enyu Ops Acceptance Entity |
| Note | `cbad201f-132b-4815-b5b5-31a2824ee5c2` | Enyu Ops live acceptance note — 2026-08-31 |
| Source | `1475fc7a-1d84-523a-afcc-e27b10f2f225` | Enyu Source-ingestion deployment verification |
| Source | `974dd73e-6858-577d-947c-79549d11ffc4` | Enyu Source-ingestion final acceptance |
| Source | `a17dcd4f-69ff-572e-9f23-d25161064b55` | Enyu Editor publishing workflow acceptance |

Two byte-identical same-Source content versions remain proposed for physical
deletion only after current reference and event reconciliation:
`a1fc03e1-b76e-5085-9465-f35f318336a0` and
`a8cd4706-2323-508a-86c1-3c2c525225ff`. Only after that approved cleanup may
the ordinary `(source_object_id, content_sha256)` index become unique.

All eight rows were freshly reconciled against their current revisions and
archived through the authenticated Object API on 2026-08-31. A second dry run
reported all eight already archived and no stale rows or errors.

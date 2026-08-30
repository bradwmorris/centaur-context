# RD: Audit, Clean, and Refine All Canonical Data

**Status:** `backlog`
**Created:** 2026-08-30

## Execution Plan

**Status:** `still needs work`

**Basis checked:** Repository boundaries; migrations `0001`–`0011`; ontology,
schema/API/database/UI code; completed description, Schema visualizer,
Source/Note, and POC-import RDs; and migration task
`01a05018-465d-7bc1-9e95-906daa4fbe64`.

**Missing:** A fresh authenticated read-only snapshot, Brad's review of the
schema-cleanup proposals and Note wording, and approval for the resulting
destructive schema/data change set.

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

## What We Are Doing

- [ ] Make every retained schema element and populated value justify its place;
  remove or consolidate genuinely irrelevant structure and data.
- [ ] Make every Object accurate and high fidelity, with a one-sentence
  description that states directly and simply what the Object is.
- [ ] Research each Source individually from its canonical page and best
  available primary evidence; correct identifiers and metadata and write the
  shortest summary that accurately distinguishes the Source.
- [ ] Preserve Notes as Brad's own words as closely as possible and obtain his
  explicit approval for every substantive Note-body edit.
- [ ] Let Brad use the Schema visualizer to inspect schema and values, identify
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
- Object descriptions identify the thing, not its database type, provenance,
  importance, or a long abstract. Put bibliographic facts in Source fields,
  evidence in Source content, and detailed thinking in Note content.
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

- [ ] Private inventory covers 100% of live tables, columns, indexes, constraints,
  Objects, subtype rows, supporting rows, and non-null user-authored values.
- [ ] Visualizer tests prove registry completeness, correct classification,
  indexes/triggers/profiles, deep-linked review, accurate values, responsive
  simplicity, and human-listener-only read access.
- [ ] Disposable migration/rollback rehearsal proves referential, subtype,
  uniqueness, immutable-content, event, provenance, and protection invariants.
- [ ] Reconciliation proves exact approved changes, no orphans, no accidental
  payload loss, no secrets in artifacts/logs, and no changes to excluded databases.
- [ ] Targeted API/client/UI/search tests and every repository-root verification
  command pass.
- [ ] `git diff --check` passes.

## Approval Boundary

Read-only audits and private proposals are authorized. Dropping schema, deleting
or merging data, unprotecting imported records, changing immutable evidence,
writing to a hosted database, or deploying requires Brad's explicit approval of
the exact manifest. External research is read-only; no publishing, sending,
spending, public ingress, or new integration is authorized.

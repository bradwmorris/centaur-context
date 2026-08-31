# 1 — RD: Consolidate the Canonical Schema

**Status:** `scoped`
**Created:** 2026-08-31

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Schema 16 migrations and live row profiles; every Rust reader,
writer, trigger, worker, API route, listener, web view, standard Python client
method, test suite, deployment/configuration reference, active dependent RD, and
the downstream Enyu checkout.

**Missing:** none for planning. Brad must approve this final target and separately
authorize execution, destructive migration, deployment, and merge.

1. Implement the exact target schema and migration mapping below.
2. Move every first-party reader and writer to that schema in the same release.
3. Rehearse migration and rollback on a restored disposable database.
4. Stop old writers, back up live data, migrate, deploy the coordinated release,
   reconcile every migrated row, and run all checks.

## Target

Reduce the 24 application tables in schema 16 to these 15:

```text
objects             connections       tasks
chats               chat_messages     users
entities            memories          sources
notes               themes            artifacts
runs                embeddings        object_events
```

Delete these 12 tables after their data is migrated:

```text
principal_permissions   theme_proposals       external_identities
external_actions        source_contents       evals
eval_trace_entries      eval_objects          curator_runs
curator_run_changes     object_embedding_jobs object_embeddings
```

Create `artifacts`, `runs`, and `embeddings`. Modify `users`, `sources`, and
`object_events`. Keep `chat_messages` separate because it is a genuine
one-Chat-to-many-Messages relationship. Keep framework-owned
`schema_visualizer_tables` and `_sqlx_migrations` outside the application count.

### Target shapes

**`users`** keeps `object_id`, `object_kind`, and `user_kind`, and gains
`identities jsonb NOT NULL DEFAULT '[]'`. Each identity preserves its stable ID,
provider, workspace, provider user ID, display name, avatar URL/asset fields,
avatar provenance, and refresh time. A database trigger takes a transaction-level
advisory lock and rejects a provider/workspace/user-ID claimed by another User;
application validation alone is not sufficient under concurrent ingestion.

**`artifacts`** replaces and generalizes `source_contents`:

```text
id, object_id, kind, title?, content?, uri?, media_type?, language?,
sha256, size_bytes, metadata jsonb, supersedes_artifact_id?, captured_at?, created_at
```

An Artifact belongs to any Object and may represent a transcript, article text,
document, file, image, audio, video, dataset, or other supporting capture. Require
`content` or `uri`; keep large binaries outside PostgreSQL; make rows immutable;
and use `supersedes_artifact_id` only when a replacement exists. Rename
`sources.current_content_id` to `current_artifact_id` and retain the same-Source
foreign-key guarantee. Source and general search may index bounded textual
Artifact content, but list APIs must never return full bodies accidentally.

**`runs`** replaces External Actions, Evals, Eval traces/Object links, and Curator
Runs/change journals:

```text
id, parent_run_id?, kind, status, actor_type, actor_id, chat_object_id?,
idempotency_key, input jsonb, trace jsonb, result jsonb,
consulted_object_ids uuid[], error?, verdict, review_notes?, reviewed_by?,
reviewed_at?, available_at?, started_at?, completed_at?, created_at, updated_at
```

`input` is immutable configuration and source input. `trace` contains ordered
execution/retrieval steps, model attempts, usage, and errors. `result` contains
terminal output, case scores, affected IDs, and a summary. `consulted_object_ids`
contains Objects read but not changed. `parent_run_id` is a self-reference for
orchestration. Enforce immutable identity/input fields, valid state/timestamp
combinations, unique `(kind,idempotency_key)`, bounded JSON, append-only trace,
and an acyclic parent relationship. Run kinds retain only the workflows in this
RD; allowed statuses are kind-scoped so the External Action state machine remains
exact. A trigger validates that every consulted Object exists. Curator input keeps
its first/last Message IDs and must validate that both belong to `chat_object_id`.

**`embeddings`** combines queue and result state:

```text
object_id, model, dimensions, source_hash, format_version, input_mode,
status, attempts, available_at, started_at?, completed_at?, last_error?,
embedding vector?, created_at, updated_at
```

Use `(object_id,model)` as the key. A pending/running/failed row has no vector; a
completed row has a dimension-checked vector. Object text changes atomically mark
existing rows pending, clear stale vectors, and reset retry state. The configured
worker creates missing model rows, claims them with `FOR UPDATE SKIP LOCKED`, and
fills the same row on success. Retrieval reads only completed, current-hash rows.
The first release keeps the current embedding input contract—Object kind, title,
and description—while active RD 3 decides any later Artifact-aware expansion.

**`object_events`** is the only mutation and reversal history:

```text
id, run_id, sequence, target_type, target_id, action, actor_type, actor_id,
idempotency_key, from_revision?, to_revision, before_state?, after_state,
reversible, created_at
```

Every durable Object or Connection mutation must write one immutable event in the
same transaction. `target_type` is `object` or `connection`; `(run_id,sequence)`
orders reversal. Events store sufficient before/after state to reverse safely.
Undo reads one Run's events in reverse order, verifies current revisions, applies
compensating writes, and creates new events under a child Run; it never edits the
original events. `runs.result` may summarize affected IDs but must never duplicate
complete reversible changes. New events are always reversible. Historical events
that predate full snapshots remain immutable and preserve their original payload,
but are explicitly marked non-reversible rather than receiving fabricated state.

Themes remain Objects and themed Connections remain supported. Delete the empty
proposal workflow and single-row permission table. Theme creation stays
human-only; assigning an existing Theme uses the normal authenticated agent
contract. External Actions stop being Objects, but retain their dedicated
credential/listener: those routes create/update `runs`, and only resulting Object
or Connection mutations create `object_events`.

## Data Migration

Implement `migrations/0017_consolidated_schema.sql` as one forward-only migration
after a fresh backup and disposable rehearsal. It must abort rather than guess
when any guard fails.

1. Add new tables/columns, constraints, indexes, immutability functions, and Run
   transaction context before moving data.
2. Aggregate all `external_identities` into deterministic `users.identities`
   arrays, preserving identity IDs and avatar metadata. Prove every old identity
   appears exactly once and every provider key remains globally unique.
3. Copy all 44 `source_contents` rows to `artifacts` without changing IDs, text,
   hashes, byte sizes, timestamps, capture metadata, or current Source pointers.
   Map extraction, coverage, and locators into bounded `metadata`. Verify hashes
   from stored bytes and preserve the two existing replacement histories.
4. Create one Run for every Eval. For the 16 Evals linked to Curator Runs, use the
   Curator Run ID as the new Run ID and retain the former Eval ID in migration
   metadata; other Runs keep their Eval IDs. Fold ordered trace entries into
   `trace`; map `consulted`/`participant` links to `consulted_object_ids`; map
   affected roles only to `result` summaries; preserve annotations, usage, costs,
   failures, timestamps, plans, message windows, attempts, and leases.
5. Convert each `curator_run_changes` row into the matching existing Object Event
   by exact run/entity/revision/idempotency evidence, adding sequence and full
   before/after state. Abort if a change lacks exactly one matching event. Move
   Curator lifecycle and duplicate mutation trace entries into Run trace/result,
   leaving only Object/Connection mutations in `object_events`.
6. Map every other retained Object/Connection Event to its Run by exact trace
   target/action/revision or idempotency evidence. Move message, Source-content,
   Curator-lifecycle, and other non-mutation events to the appropriate Run trace or
   Artifact metadata. Put genuinely pre-Run mutation events under one legacy Run,
   preserve their original payload, and mark them non-reversible. Abort ambiguous
   mappings.
7. Convert each External Action and its former Object/Event history into one Run,
   preserving its stable ID, provider/action/external key, privacy-safe metadata,
   state, idempotency, actor, and timestamps. Reuse its exact Eval-derived Run when
   one exists; abort ambiguous matches rather than creating duplicate Runs. Then
   remove the three obsolete External Action Objects only after proving they have
   no retained Connection or subtype dependency.
8. Merge every embedding job/result pair into one `embeddings` row. Current input
   is 407 pending jobs and zero vectors. The three removed External Action Objects
   lose their jobs, so the current expected final state is 404 pending rows and
   zero completed vectors. Refresh and guard all counts if live state changes.
9. Remove Eval mutation triggers/functions, old foreign keys, indexes, registry
   entries, Object kind `external_action`, and the 12 old tables only after all
   count, key, hash, and payload comparisons pass inside the rehearsal.
10. Register exactly the 15 target tables, bump database schema, ontology, API,
   standard-client, intake-manifest, and external-action contract versions, and
   record the final table/column counts.

## Required Code and Contract Changes

- **Database/domain:** Update `src/domain.rs`, `src/db.rs`, and `src/schema.rs`;
  add typed Identity, Artifact, Run, Embedding, and ObjectEvent contracts; update
  source, visual/attribution, context, and search joins; remove proposal code and
  obsolete SQL functions/triggers; keep optimistic revisions and idempotency.
- **Curator:** Keep reconciliation logic in `src/curator.rs`, but queue/claim one
  `runs` row, store immutable input and append trace, collect consulted IDs, and
  commit each mutation plus its full Object Event atomically. Undo exclusively
  from Object Events and record reversal as a child Run. Validate first/last
  Message ownership against the Run's Chat before queue, claim, and reconcile.
- **Evals/usage:** Replace `src/evals.rs` with a focused `src/runs.rs`. Ingestion,
  human/system writes, model attempts, usage reports, and failures append to the
  active Run. Every standalone mutation creates a Run rather than relying on an
  implicit Eval trigger.
- **Ingestion/intake:** Update `src/ingest.rs`, `src/intake.rs`, and
  `src/source_intake.rs` to write embedded User identities, generic Artifacts,
  Runs, and authoritative Object Events. Change intake manifests from
  `external_identities`/`source_contents` collections to User identities and
  `artifacts`; update Curator output schema from Source content to Artifact.
- **External Actions:** Keep the isolated listener/token and privacy validation in
  `src/external_actions.rs`, but persist Runs rather than Objects/subtypes. Remove
  `external_action` from Object creation, visuals, and ontology.
- **Embeddings/search:** Update `src/embeddings.rs`, `src/search.rs`, and database
  claim/complete/fail methods for the single-table lifecycle; rebuild vectors after
  cutover; search only current completed rows; index Artifact text where intended.
- **HTTP API:** Update `src/api.rs`. Replace separate human Eval/Curator routes with `/runs`,
  `/runs/{id}`, `/runs/{id}/review`, and `/runs/{id}/undo`; add generic
  `/objects/{id}/artifacts` and bounded Artifact-content routes. Preserve User
  identity and Source-content convenience responses where their meaning remains,
  backed by the consolidated data. Remove Theme-proposal endpoints; move existing
  Theme assignment to normal agent auth. Keep External Action routes but return
  Run-backed state. Publish the coordinated contract under `/api/v2`, update all
  first-party callers, and remove `/api/v1` at cutover rather than maintaining
  indefinite aliases. Unknown and removed versions/routes must fail closed.
- **Configuration/runtime:** Update `src/config.rs`, `src/main.rs`, `src/lib.rs`,
  listener startup, readiness, and token-collision checks. Remove Theme-proposal
  configuration/listener/secret. Do not remove or merge External Action credentials
  merely because its table disappears.
- **Web:** Update `web/src/App.tsx`, `types.ts`, `api.ts`, `routing.ts`,
  `SchemaWorkspace.tsx`, styles, and their tests. Replace separate Curator and Eval
  sections with one Runs list/detail/review/undo surface; show parent/child Runs,
  trace, result, consulted Objects, and authoritative linked events. Replace Source
  Versions with Artifacts, render embedded identities, and remove Theme proposal UI.
- **Standard agent client:** Update `tools/centaur_context/client.py`, `cli.py`,
  `pyproject.toml`, and `test_client.py`: payload validation, permissions, URLs,
  and methods. Remove propose/read Theme
  proposal commands and token; retain assignment of existing Themes. Add generic
  Artifact methods and new intake shapes. Keep External Action methods on their
  dedicated token but adapt responses to Runs.
- **Deployment/docs:** Update `deploy/deployment.yaml`, `service.yaml`,
  `secret.example.yaml`, network policies, `README.md`, `docs/installation.md`,
  `operations.md`, `slack-integration.md`, `context.md`, `ontology.md`, the agent
  handoff prompt, `compatibility.toml`, `src/version.rs`, schema registry, and
  package checks. Remove obsolete Theme listener material; retain required model,
  embedding, Curator, intake, and External Action configuration.
- **Downstream and planning:** Update the pinned standard client, manifests,
  prompts, URLs, and workflow payloads in `/Users/bradleymorris/Desktop/dev/enyu-os`
  in the coordinated release. Reconcile active embedding RD 3 and Eval/golden-
  scenario RD 4 so they target `embeddings` and `runs`, not deleted tables.

### Route and payload cutover

| Old contract | Target contract |
| --- | --- |
| `/api/v1/evals*` and `/api/v1/curator-runs*` | `/api/v2/runs*` |
| `/api/v1/ingest/evals/usage` | `/api/v2/ingest/runs/usage` |
| `/api/v1/sources/{id}/contents` | `/api/v2/objects/{id}/artifacts` |
| `/api/v1/sources/{id}/content` | Keep as a v2 convenience read of `current_artifact_id` |
| `/api/v1/users/{id}/identities` | Keep the array response, now read from `users.identities` |
| `/api/v1/theme-proposals*` | Remove; unknown route fails closed |
| Theme assignment using Theme-proposal token | `/api/v2/theme-assignments*` using normal agent auth |
| `/api/v1/external-actions*` | Keep under v2 with the dedicated token and Run-backed response |
| Intake `external_identities` and `source_contents` | User `identities` and top-level `artifacts` |

Update `tests/api_auth.rs`, `database_contract.rs`, `intake_contract.rs`,
`source_intake_contract.rs`, and `curator_evals.rs` (renamed for Runs) to prove
these replacements and that every removed v1 route fails closed.

## Rollout and Rollback

This is not compatible with old writers. Build and verify the new service/client
first, then use a maintenance cutover: stop all writers and workers; ensure no Run
or Embedding lease is active; take and checksum a fresh backup; rehearse the exact
live snapshot; migrate; deploy the new service, web bundle, client, and Enyu
callers together; then run identity, Slack ingestion, Curator reconcile/undo,
Artifact read/write/search, External Action idempotency, Run review, embedding
claim/complete/search, and Schema UI canaries. Rollback means stopping new writers,
restoring the verified pre-cutover backup, and redeploying the old image/client;
do not attempt a lossy reverse migration.

## What We Are Doing

- [ ] Land the 15-table target with no competing histories or obsolete writers.
- [ ] Preserve every approved identity, Artifact byte/hash, Run/evaluation fact,
      usage/cost record, mutation, reversal, idempotency key, and stable ID defined
      above; report intentionally removed speculative/duplicate structure.
- [ ] Prove every retained workflow through the coordinated API rather than direct
      database access.

## Contract

- **Goal:** Make the database and every first-party consumer materially simpler
  while keeping the accepted operational behavior intact.
- **Done:** Schema 16 migrates to the exact 15-table target; all old-table readers,
  writers, routes, triggers, configs, and UI surfaces are gone or deliberately
  remapped; live reconciliation and all checks pass.
- **Files:** `migrations/`; `src/`; `tests/`; `web/`; `tools/centaur_context/`;
  deployment/configuration; documentation; compatibility metadata; active dependent
  RDs; authorized Enyu overlay contract updates.
- **Agent owns:** Implementation, migration/reconciliation manifests, coordinated
  first-party updates, local/disposable verification, and a ready PR after execution
  is authorized.
- **Requester owns:** Final target approval, permission to discard the explicitly
  obsolete structures/history representations, live cutover, deployment, and merge.
- **Out of scope:** `ai_v2`, Console databases, new public ingress, unrelated
  product features, and new external integrations.

## Checks

- [ ] Fresh schema and populated schema-16 migration tests pass, including a second
      idempotence/reconciliation pass and exact row/key/hash/payload comparisons.
- [ ] Multi-provider identity uniqueness and concurrent Slack replay pass.
- [ ] Artifact ownership, immutability, replacement, bounded reads, and search pass.
- [ ] Run queue, hierarchy, trace ordering, review, usage/cost, External Action
      idempotency, Curator commit, and event-only undo pass.
- [ ] Embedding queue/lease/retry/completion/staleness and hybrid retrieval pass.
- [ ] Auth tests prove removed Theme permissions/routes are gone and retained write
      surfaces still require the correct distinct credential.
- [ ] Web and standard-client contract tests pass; Enyu golden workflows pass.
- [ ] Schema registry contains exactly the 15 application tables and no old name.
- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test`
- [ ] `npm --prefix web run type-check`
- [ ] `npm --prefix web run build`
- [ ] `python3 -m pytest tools/centaur_context/test_client.py`
- [ ] `python3 -m compileall -q tools/centaur_context`
- [ ] `git diff --check` passes.

## Approval Boundary

Planning only. Do not implement, migrate, deploy, delete, write hosted data, or
change the Enyu overlay until Brad approves this target and separately starts RD
execution. Never access or modify `ai_v2` or Console databases.

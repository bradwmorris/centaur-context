# 1 — RD: Drastically Minimize the Canonical Schema

**Status:** `scoped`
**Created:** 2026-08-31

## Current Schema Baseline

Schema version 16 has 24 application tables in PostgreSQL's `public` schema,
plus two framework/inspection tables. This inventory is the post-cleanup schema
materialized from migrations `0001`–`0016`; it is the starting point, not a
presumption that any non-mandated structure should survive. In the compact
schema below, `?` marks a nullable column; all other columns are `NOT NULL`.

### Canonical graph and one-to-one Object subtypes

| Table | Current schema | Purpose today |
| --- | --- | --- |
| `objects` | `id uuid`, `kind text`, `title text`, `description text`, `revision bigint`, `created_by_type text`, `created_by_id text`, `updated_by_type text`, `updated_by_id text`, `provenance jsonb`, `created_at timestamptz`, `updated_at timestamptz`, `archived_at timestamptz?`, `protected boolean`, `search_document tsvector?` | Canonical identity and shared state for every first-class Object; also stores generated full-text search data. |
| `tasks` | `object_id uuid`, `object_kind text`, `status text`, `agent_suitable boolean`, `due_at timestamptz?`, `priority text`, `owner_object_id uuid?`, `blocked_reason text?`, `completed_at timestamptz?`, `github_issue_url text?`, `brief_markdown text?` | Task-specific workflow, ownership, scheduling, suitability, completion, blocker, and work-brief fields. |
| `chats` | `object_id uuid`, `object_kind text`, `processing_updated_at timestamptz`, `provider text?`, `workspace_id text?`, `channel_id text?`, `thread_id text?`, `surface_kind text?`, `channel_name text?`, `latest_source_message_at timestamptz?`, `curation_queued_through_message_id uuid?`, `curated_through_message_id uuid?` | Conversation identity plus ingestion and curation checkpoints. |
| `users` | `object_id uuid`, `object_kind text`, `user_kind text` | Classifies an Object as a human or agent User. |
| `entities` | `object_id uuid`, `object_kind text`, `image_url text?`, `entity_kind text` | Classifies a named subject and optionally supplies its image. |
| `memories` | `object_id uuid`, `object_kind text`, `happened_at timestamptz` | Represents an event-shaped Memory and records when it happened. |
| `sources` | `object_id uuid`, `object_kind text`, `source_kind text`, `canonical_uri text?`, `byline text?`, `publisher text?`, `published_at timestamptz?`, `last_accessed_at timestamptz?`, `original_language text?`, `original_media_type text?`, `original_artifact_reference text?`, `current_content_id uuid?`, `published_at_precision text?` | Bibliographic identity and pointer to the current captured version of evidentiary material. |
| `notes` | `object_id uuid`, `object_kind text`, `content text`, `content_format text` | Stores the full body and format of a human- or agent-authored Note. |
| `themes` | `object_id uuid`, `object_kind text`, `slug text` | Gives a human-approved taxonomy Object its stable slug. |
| `external_actions` | `object_id uuid`, `object_kind text`, `provider text`, `action_kind text`, `external_key text`, `state text`, `metadata jsonb`, `created_at timestamptz`, `updated_at timestamptz` | Durable state and idempotency record for an external side effect. |
| `connections` | `id uuid`, `source_object_id uuid`, `kind text`, `target_object_id uuid`, `description text`, `revision bigint`, `created_by_type text`, `created_by_id text`, `updated_by_type text`, `updated_by_id text`, `provenance jsonb`, `created_at timestamptz`, `updated_at timestamptz`, `archived_at timestamptz?`, `protected boolean` | Explained, revisioned, attributable graph edge between two Objects. |

### Evidence, identity, processing, evaluation, and derived data

| Table | Current schema | Purpose today |
| --- | --- | --- |
| `external_identities` | `id uuid`, `user_object_id uuid`, `provider text`, `workspace_id text`, `provider_user_id text`, `display_name text?`, `created_at timestamptz`, `updated_at timestamptz`, `avatar_url text?`, `avatar_asset_sha256 text?`, `avatar_asset_filename text?`, `avatar_provenance jsonb`, `profile_refreshed_at timestamptz?` | Maps a User to provider identities and cached profile/avatar metadata. |
| `chat_messages` | `id uuid`, `chat_object_id uuid`, `provider_message_id text`, `sender_user_object_id uuid`, `content text`, `source_created_at timestamptz`, `ingestion_sequence bigint`, `ingested_at timestamptz` | Immutable source-message evidence ordered within a Chat. |
| `source_contents` | `id uuid`, `source_object_id uuid`, `version bigint`, `content_kind text`, `normalized_text text`, `language text?`, `extraction_method text?`, `extraction_version text?`, `content_sha256 text`, `size_bytes bigint`, `capture_artifact_reference text?`, `locators jsonb`, `recorded_at timestamptz`, `coverage text`, `captured_at timestamptz?` | Immutable versions of normalized Source text with capture, integrity, and extraction metadata. |
| `object_events` | `id uuid`, `entity_type text`, `entity_id uuid`, `object_id uuid`, `action text`, `actor_type text`, `actor_id text`, `centaur_thread_key text?`, `centaur_execution_id text?`, `idempotency_key text?`, `from_revision bigint?`, `to_revision bigint`, `changes jsonb`, `created_at timestamptz` | Immutable mutation and audit history tied back to a canonical Object. |
| `object_embeddings` | `object_id uuid`, `model text`, `dimensions integer`, `source_hash text`, `embedding vector`, `embedded_at timestamptz`, `format_version text`, `input_mode text` | Rebuildable vector representation used by hybrid retrieval. |
| `object_embedding_jobs` | `object_id uuid`, `source_hash text`, `status text`, `attempts integer`, `available_at timestamptz`, `started_at timestamptz?`, `last_error text?`, `created_at timestamptz`, `updated_at timestamptz`, `format_version text`, `input_mode text` | Retryable queue for generating or refreshing Object embeddings. |
| `curator_runs` | `id uuid`, `chat_object_id uuid`, `first_message_id uuid`, `last_message_id uuid`, `trigger text`, `status text`, `message_count integer`, `queued_at timestamptz`, `started_at timestamptz?`, `completed_at timestamptz?`, `reversed_at timestamptz?`, `error_message text?`, `idempotency_key text`, `attempts integer`, `available_at timestamptz`, `lease_started_at timestamptz?`, `worker_id text?`, `model text?`, `prompt_version text?`, `proposed_plan jsonb?`, `committed_plan jsonb?`, `result jsonb?` | Queue, lease, execution, plan, result, failure, and reversal state for one curation attempt. |
| `curator_run_changes` | `id uuid`, `curator_run_id uuid`, `sequence integer`, `entity_type text`, `entity_id uuid`, `action text`, `before_state jsonb?`, `after_state jsonb`, `after_revision bigint`, `created_at timestamptz`, `undone_at timestamptz?` | Ordered before/after journal used to inspect and undo a Curator Run. |
| `evals` | `id uuid`, `kind text`, `status text`, `actor_type text`, `actor_id text`, `chat_object_id uuid?`, `curator_run_id uuid?`, `summary text`, `error_summary text?`, `idempotency_key text`, `verdict text`, `notes text?`, `annotated_by text?`, `annotated_at timestamptz?`, `annotation_revision bigint`, `next_sequence bigint`, `started_at timestamptz`, `completed_at timestamptz?`, `created_at timestamptz`, `updated_at timestamptz` | Evaluation lifecycle, human verdict, annotation, and links to the activity being assessed. |
| `eval_objects` | `eval_id uuid`, `object_id uuid`, `role text`, `created_at timestamptz` | Many-to-many link recording which Objects an Eval consulted, created, updated, or otherwise affected. |
| `eval_trace_entries` | `id uuid`, `eval_id uuid`, `sequence bigint`, `entry_type text`, `component text?`, `provider text?`, `model_id text?`, `display_tier text?`, `execution_type text?`, `auth_mode text?`, `upstream_service text?`, `billing_mode text?`, `reasoning_effort text?`, `service_tier text?`, `source_thread_id text?`, `source_execution_id text?`, `source_turn_id text?`, `usage_status text`, `usage_missing_reason text?`, `input_tokens bigint?`, `output_tokens bigint?`, `cache_creation_tokens bigint?`, `cache_read_tokens bigint?`, `reasoning_tokens bigint?`, `total_tokens bigint?`, `estimated_micro_usd bigint?`, `chatgpt_credit_microunits bigint?`, `api_equivalent_micro_usd bigint?`, `rate_card_version text?`, `pricing_snapshot jsonb?`, `facts jsonb`, `created_at timestamptz` | Ordered provider-independent execution trace and model usage/cost detail for an Eval. |
| `principal_permissions` | `principal_type text`, `principal_id text`, `permission text`, `granted_by text`, `granted_at timestamptz` | Explicit grant identifying who may approve Theme proposals. |
| `theme_proposals` | `id uuid`, `title text`, `slug text`, `description text`, `rationale text`, `evidence jsonb`, `provenance jsonb`, `status text`, `proposed_by_type text`, `proposed_by_id text`, `centaur_thread_key text`, `centaur_execution_id text?`, `idempotency_key text`, `decided_by_type text?`, `decided_by_id text?`, `decision_reason text?`, `decided_at timestamptz?`, `resulting_theme_object_id uuid?`, `created_at timestamptz`, `updated_at timestamptz` | Immutable agent-proposal and human-decision workflow for creating Themes. |

### Framework and inspection tables

| Table | Current schema | Purpose today |
| --- | --- | --- |
| `schema_visualizer_tables` | `table_name text`, `registered_at timestamptz` | Migration-owned allowlist of the 24 application tables exposed by the read-only Schema workspace. It does not register itself or `_sqlx_migrations`. |
| `_sqlx_migrations` | `version bigint`, `description text`, `installed_on timestamptz`, `success boolean`, `checksum bytea`, `execution_time bigint` | SQLx-owned ledger proving which forward migrations ran and whether their checksums still match. |

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Repository boundaries and RD rules; completed canonical-data
cleanup RD and its 24-table ledger; migrations `0001`–`0016`; materialized
schema-16 column inventory; ontology, compatibility contract, Rust readers and
writers, tests, agent client, and Schema workspace.

**Missing:** none for investigation and design. Execution still requires a
separate instruction and approval of the exact destructive migration manifest.

1. Measure every table and column against live shape, row counts, null/default
   rates, constraints, indexes, triggers, and every first-party reader/writer.
   Mark each item `mandatory`, `keep`, `merge`, `derive`, `rebuild`, or `remove`.
2. Produce the smallest coherent target schema and a complete current-to-target
   map. Start from deletion or consolidation, and require evidence for every
   retained non-mandatory table and column. Do not count moving typed fields into
   opaque JSON as simplification.
3. Present the proposed table/column removals, merges, data-loss consequences,
   migration path, and consumer changes for Brad's approval before implementation.
4. After approval, implement one forward-only migration with all Rust API,
   Curator, ingestion, client, web, test, documentation, and compatibility
   changes in lockstep. Rehearse it on a disposable restored database first.
5. Reconcile the migrated database, run the full repository verification suite,
   and confirm every retained workflow with materially less persisted structure.

## What We Are Doing

- [ ] Reduce table and column count as a primary design objective, accepting
      breaking internal changes where all first-party consumers can move together.
- [ ] Challenge support, queue, governance, subtype, evaluation, trace, identity,
      and derived-data boundaries; merge or remove them unless separation protects
      an evidenced invariant that cannot be kept more simply.
- [ ] Remove stored values that can be cheaply and reliably derived; remove empty,
      speculative, duplicated, and rebuildable persistence unless current operation
      demonstrably requires it.
- [ ] Deliver an exact before/after schema and prove the retained application still
      supports its explicitly accepted workflows and data.

## Contract

- **Goal:** Replace schema 16 with the smallest understandable schema that still
  satisfies Centaur Context's accepted product and safety contracts.
- **Done:** Brad approves an exact current-to-target manifest; every retained
  table and column has a concrete justification and consumer; the approved
  forward migration and coordinated consumers pass all checks; and final table
  and column counts are recorded against this baseline.
- **Files:** `migrations/`; affected Rust domain/database/API/ingestion/Curator/
  eval/search code and tests; `tools/centaur_context`; affected web schema and
  product surfaces; `docs/ontology.md`; `compatibility.toml`; this RD.
- **Agent owns:** Read-only inventory, consumer mapping, minimal target proposal,
  explicit trade-offs, approved migration implementation, reconciliation, and
  local verification.
- **Requester owns:** Choosing which useful workflows may be dropped, approving
  the exact destructive manifest, live database writes/deployment, and merge.
- **Out of scope:** `ai_v2`, Console databases, new product features, public
  ingress, cloud deployment, external integrations, and hiding relational
  complexity inside unstructured JSON.

## Non-Negotiable Repository Boundaries

- Keep canonical Objects; one-to-one subtype records for Tasks, Chats, Users,
  Entities, and event-shaped Memories; explained Connections; and immutable
  Object Events. Their columns remain open to reduction.
- Use the authenticated HTTP API for agents and never expose a database DSN to a
  sandbox. Never query or migrate Centaur's `ai_v2` or Console databases.
- Preserve stable Object IDs and explicitly approved immutable evidence. Any
  proposal to discard retained history, source content, messages, eval evidence,
  or reversibility must quantify the loss and receive Brad's explicit approval.
- Judge total system complexity, not only PostgreSQL table count: a merge that
  increases application branching, weakens constraints, or duplicates records
  must show a net simplification.

## Checks

- [ ] Inventory accounts for all 24 application tables, both framework tables,
      every column, constraint, index, trigger, row family, and first-party consumer.
- [ ] Approved target manifest states exact tables and columns kept, merged,
      derived, rebuilt, and removed, with baseline and final counts.
- [ ] Disposable migration rehearsal proves subtype, referential, revision,
      immutable evidence, idempotency, search, queue, and authorization invariants
      that remain in scope; reconciliation proves no unintended payload loss.
- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test`
- [ ] `npm --prefix web run type-check`
- [ ] `npm --prefix web run build`
- [ ] `python3 -m pytest tools/centaur_context/test_client.py`
- [ ] `python3 -m compileall -q tools/centaur_context`
- [ ] `git diff --check` passes.

## Approval Boundary

This RD authorizes planning only. Do not implement, migrate, deploy, delete,
merge, or write hosted data until Brad separately starts execution and approves
the exact destructive manifest. Prefer a reversible rehearsal and verified
backup before any approved live cutover. No work may touch `ai_v2`, Console, or
another organization's private overlay; assess and explicitly coordinate any
required Enyu overlay update during execution.

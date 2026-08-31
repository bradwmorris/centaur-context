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

**Status:** `still needs work`

**Basis checked:** Schema 16, current code paths, and exact live table profiles on
2026-08-31.

**Missing:** Final types and constraints for consolidated `users`, `artifacts`,
`runs`, `embeddings`, and simplified `object_events`, plus the remaining-table
review.

1. Agree which tables to delete, merge, or keep.
2. Write the exact smaller target schema for approval.
3. Only after separate execution approval, migrate the data and update every
   first-party reader and writer together.

## Agreed Direction in Brad's Order

The default is consolidation. Keep a separate table only when it represents a
real one-to-many relationship or immutable history that becomes materially
worse in one row.

| Question | What the data and code show | Agreed direction |
| --- | --- | --- |
| `principal_permissions` | One row grants one permission to the local human. It duplicates an authorization rule that can live in service configuration. | **Delete.** |
| `theme_proposals` | Zero rows. It exists for a future agent-proposal/human-approval workflow rather than a workflow currently producing data. | **Delete.** Use a normal Task or Note until a dedicated workflow is genuinely needed. |
| `external_actions` | Three rows track reservations and state transitions so an external side effect is not sent twice. The durable protection is useful; making each action a special Object and subtype is not. | **Merge into `runs`.** The Run owns the external-action lifecycle; any resulting Object or Connection mutation belongs in `object_events`. |
| `external_identities` | A User must support multiple provider identities, but these are small provider-specific records that are normally read and maintained with the User. | **Merge into `users` as an `identities` JSON array.** Each entry keeps provider, workspace, provider user ID, display name, avatar data, and refresh time. Enforce provider/workspace/user-ID uniqueness in application validation. |
| `chat_messages` | Eight Chats contain 31 Messages. Ingestion appends individual Messages; each has its own sender, provider ID, source time, and sequence. Runs refer to exact first/last Messages. | **Keep separate.** This is a real one-Chat-to-many-Messages relationship. |
| `source_contents` | It currently stores article text, transcripts, paper text, and similar Source captures. There are 44 rows belonging to 42 Sources. Tasks, Chats, and any other Object can also need supporting files or captured content. | **Rename and generalize to `artifacts`.** An Artifact may belong to any Object, not only a Source. Transcript is one Artifact kind. Keep separate because one Object may have many Artifacts and an Artifact may be large or versioned. |
| `evals`, `eval_trace_entries`, `eval_objects`, `curator_runs`, `curator_run_changes` | 297 Evals, 2,197 trace rows, 1,731 Object links, 16 Curator Runs, and 32 Curator changes. The split creates five tables for one operational story. Every Curator Run already has an Eval. | **Replace all five with one `runs` table.** A Run owns immutable input, execution trace, terminal result, consulted Object IDs, hierarchy, errors, timing, and review. It summarizes affected IDs but does not duplicate mutation or reversal history. |
| `object_embedding_jobs`, `object_embeddings` | All 407 Objects have pending jobs with zero attempts and no completed vectors yet, but embeddings are an intended feature. Queue state and vector output describe the same Object/model/hash embedding lifecycle. | **Replace both with one `embeddings` table.** A pending row has status/retry fields and a null vector; successful processing fills the vector and completion metadata on that same row. |
| `object_events` | 2,144 immutable events cover all 407 Objects and provide attribution, revision history, and idempotency for writes. Many events can belong to one Object or Run. | **Keep separate and authoritative.** Every durable Object or Connection mutation is recorded here with sufficient reversal information. Do not duplicate complete reversible changes in `runs`. |

### Minimal `runs` target

- `runs.input` is the immutable input and configuration.
- `runs.trace` contains execution steps, retrieval facts, model attempts, usage,
  and errors.
- `runs.result` contains terminal output, case scores, affected IDs, and a concise
  outcome summary.
- `runs.consulted_object_ids` contains Objects read but not changed.
- `runs.parent_run_id` links orchestration and workflow children to their parent.
- `object_events`, not `runs`, is the canonical mutation and reversal history.

```text
runs
- id
- parent_run_id?
- kind
- status
- actor_type
- actor_id
- chat_object_id?
- idempotency_key
- input jsonb
- trace jsonb
- result jsonb
- consulted_object_ids uuid[]
- error?
- verdict
- review_notes?
- reviewed_by?
- reviewed_at?
- available_at?
- started_at?
- completed_at?
- created_at
- updated_at
```

### Result of the agreed direction so far

- Delete `principal_permissions` and `theme_proposals`.
- Merge `external_identities` into `users` while preserving multiple providers.
- Replace six operational tables—`external_actions`, the three Eval tables, and
  the two Curator tables—with one `runs` table.
- Rename and generalize `source_contents` to `artifacts`, attachable to any
  Object.
- Replace `object_embedding_jobs` and `object_embeddings` with one `embeddings`
  table.
- Keep `chat_messages` and `object_events` as the two justified separate
  one-to-many/history tables in this review, while simplifying their columns.
- Record the exact whole-schema table and column reduction after every remaining
  table is decided.

## What We Are Doing

- [ ] Reduce table and column count as a primary design objective, accepting
      breaking internal changes where all first-party consumers can move together.
- [ ] Prefer one human-understandable table when a separate table provides only
      theoretical flexibility or conventional database neatness.
- [ ] Keep separation only for a real one-to-many or immutable-history need.
- [ ] Deliver an exact before/after schema and preserve only the workflows Brad
      explicitly chooses to keep.

## Contract

- **Goal:** Delete unnecessary tables and columns and consolidate the rest so a
  human can understand and maintain the schema easily.
- **Done:** Brad approves the exact smaller schema; it is migrated without
  unintended data loss; all retained workflows work; and the final table and
  column counts are recorded against this baseline.
- **Files:** `migrations/`; affected Rust domain/database/API/ingestion/Curator/
  eval/search code and tests; `tools/centaur_context`; affected web schema and
  product surfaces; `docs/ontology.md`; `compatibility.toml`; this RD.
- **Agent owns:** Careful pushback only where separation is genuinely necessary,
  the minimal target proposal, approved implementation, and verification.
- **Requester owns:** Choosing which useful workflows may be dropped, approving
  the exact destructive manifest, live database writes/deployment, and merge.
- **Out of scope:** `ai_v2`, Console databases, new features, public ingress,
  cloud deployment, and external integrations.

## Checks

- [ ] Exact before/after table and column list is approved.
- [ ] Disposable migration rehearsal and reconciliation prove no unintended data
      loss and every retained workflow works.
- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test`
- [ ] `npm --prefix web run type-check`
- [ ] `npm --prefix web run build`
- [ ] `python3 -m pytest tools/centaur_context/test_client.py`
- [ ] `python3 -m compileall -q tools/centaur_context`
- [ ] `git diff --check` passes.

## Approval Boundary

This RD authorizes planning only. Do not implement, migrate, deploy, delete, or
write hosted data until Brad approves the exact target schema and separately
starts execution. Never touch `ai_v2` or Console databases.

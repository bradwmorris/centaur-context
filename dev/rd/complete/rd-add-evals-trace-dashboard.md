# RD: Add Interaction Evals and Trace Dashboard

**Status:** `complete`
**Created:** 2026-08-29
**GitHub Issue:** `#13`
**Centaur Producer PR:** `bradwmorris/centaur#2`

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Repository and development boundaries; every active backlog
RD; the planned Centaur Context rename; migrations through
`0007_user_visual_context.sql`; all current Object, Task, Connection, Slack
ingestion, Curator, and Object Event write paths; Curator queue, retry, model,
reconciliation, and undo behavior; human and agent API boundaries; the current
React navigation, lists, details, and routing; and the live local Task backlog,
which currently contains no rows.

**Missing:** Deployment-time model pricing values are requester-owned
configuration. No product-design decision remains.

**Sequence:** Express new product copy and identifiers as **Centaur Context**.
If this RD executes before the rename RD, use current code/package paths without
duplicating compatibility work, then let the rename RD perform the mechanical
cutover. Execute the ChatGPT subscription-authentication RD first so both
subscription and API trace variants are available for end-to-end fixtures.

1. Add a durable interaction-trace schema and a database-enforced write context
   that groups all causally related work under one eval and links every affected
   canonical Object.
2. Consume normalized usage from Centaur agent executions and instrument
   Curator calls/retries/reconciliation and trusted human mutations so each
   interaction records ordered activity, exact usage and charge provenance,
   outcome, and failure information without storing secrets or hidden reasoning.
3. Add a human-only Evals dashboard and detail view with verdict/note annotation,
   then verify trace completeness, idempotency, accounting, security, and UI
   behavior with the full repository checks.

## What We Are Doing

- [x] Represent one complete interaction as one eval row, even when it creates,
  updates, connects, archives, or merely consults many Objects.
- [x] Show what happened, ordered trace entries, token usage, charge/cost basis,
  every model and execution type, outcome, and every related canonical Object
  in a simple Evals UI.
- [x] Let a human filter by and set `unreviewed`, `pass`, `mixed`, or `fail`, with
  an optional open-text annotation.
- [x] Guarantee that every runtime Object insert or update is attached to an
  eval rather than relying on each current writer to remember instrumentation.

## Contract

- **Goal:** Make Centaur Context's continual graph updates observable and
  reviewable through one deterministic trace per interaction.
- **Done:** Every Slack interaction and trusted human mutation has exactly one
  eval; its trace accounts for all model attempts and durable graph effects,
  links every affected Object, identifies every model/runtime/auth/billing mode,
  exposes exact token and charge provenance, records failures and no-op runs,
  and can be reviewed from the human UI.
- **Files:** A migration after the latest migration at execution time; narrow
  trace/accounting modules and changes in `src/`; human API routes;
  `web/src/`; a narrow normalized-usage producer contract in the adjacent
  Centaur checkout; configuration and operational documentation; targeted Rust,
  database, API, and UI tests; this RD. Current paths may later be renamed by
  the Centaur Context RD.
- **Agent owns:** Local schema, write-context enforcement, instrumentation,
  accounting logic, human API/UI, tests, documentation, and local verification
  when execution is separately assigned.
- **Requester owns:** Correct provider/model prices, approval of paid model use,
  deployment, hosted writes, and any later retention or export policy.
- **Out of scope:** Model grading, automatic quality scoring, prompt experiments,
  distributed tracing infrastructure, raw chain of thought, full request/response
  payload storage, provider-invoice reconciliation, arbitrary SQL, public
  ingress, and agent/sandbox access to evals.

## Detailed Requirements

### Trace identity and lifecycle

- Call the top-level product record an **Eval** and its ordered child records
  **trace entries**. Add one `evals` row per causal interaction, not per Object
  and not per model call.
- Keep the relational shape small and explicit: `evals` is the one-row summary,
  `eval_trace_entries` is its append-only ordered log, and `eval_objects` is the
  deduplicated many-to-many foreign-key link to canonical Objects. These support
  tables do not appear as additional eval rows in the dashboard.
- A Slack eval begins with the first ingested message in a new interaction
  window, remains open while messages arrive, attaches to the resulting
  `curator_run`, and finishes as `completed`, `failed`, or `reversed`. The
  explicit-finish and inactivity paths must resolve the same idempotent record.
- Include failed, retried, validation-repaired, no-op, and reversed Curator runs
  so the dashboard does not present survivorship-biased results.
- A trusted human API mutation is a complete single-request eval. Group the
  Object and subtype/Connection writes performed by that request under the same
  record. Read-only requests do not create evals.
- Give every eval stable identity, kind, status, actor, optional Chat and Curator
  Run references, deterministic plain-language summary, timestamps, error
  summary, and an idempotency key appropriate to its source.

### Complete, relational trace

- Store ordered, typed trace entries for meaningful facts: message ingestion,
  model attempt, validation/repair, Object or subtype creation/update/archive,
  Connection creation/update/archive, commit, failure, and reversal. Entries
  contain bounded structured facts and before/after revision metadata where
  available; they do not duplicate transcripts, prompts, secrets, credentials,
  embeddings, or hidden reasoning.
- Relate each eval through foreign keys to every canonical Object it created,
  changed, connected, used as an owner/participant, or supplied to the Curator as
  context. Record a role/action such as `created`, `updated`, `connected`, or
  `consulted`; deduplicate repeated links while retaining ordered trace entries.
- Reuse durable `object_events`, `curator_runs`, and `curator_run_changes` as
  source facts rather than building a competing audit history. Eval records are
  observability and review records; they are not canonical Objects and must not
  create recursive Object rows or ontology Connections.
- Enforce an active eval context at the database boundary for runtime Object
  inserts and updates. Existing and future write paths must fail closed or create
  an explicitly classified standalone human/system eval; no mutation may become
  silently untraced. Applied migration history and migration-time data repair are
  exempt. Backfill existing Objects only as one clearly labelled legacy/import
  eval with unknown usage, without fabricating historical interactions or cost.

### Token and cost accounting

- Record one usage trace entry for every model attempt across the complete
  interaction, including the interactive Centaur agent, Curator validation
  repair/retry, embeddings when enabled, and other model-backed preprocessing.
  Centaur must send its already normalized Codex usage with stable
  thread/execution/turn IDs; Context must not scrape an observability backend.
- Every usage entry stores `component`, provider, raw `model_id`, display tier,
  execution type (`codex_harness`, `direct_api`, or another explicit value),
  authentication mode (`chatgpt_subscription`, `api_key`, or `not_applicable`),
  upstream service, billing mode (`subscription_allowance`, `chatgpt_credits`,
  `metered_api`, or `unknown`), reasoning effort, service tier, source execution
  IDs, and the pricing/rate-card version when applicable. Never infer auth or
  billing mode from the model name.
- Aggregate exact input, output, cache-creation, cache-read, reasoning, and total
  tokens where reported. Retain per-attempt counts so the eval total is
  reproducible without merging unlike categories or double-counting cached
  tokens.
- Parse provider usage from successful and error responses when available.
  Missing usage is explicit (`usage_status` and reason), never silently zero.
  Non-model interactions show `not_applicable`, zero tokens, and zero cost.
- For `metered_api`, store estimated money as integer micro-USD (or an equally
  exact fixed-point type), never floating point, using the versioned model price
  snapshot saved with the entry. Do not present estimates as reconciled billing.
- For ChatGPT subscription usage, show the subscription/credit charge basis and
  exact or estimated ChatGPT credits only when the runtime and a versioned rate
  card support them. A subscription has no truthful per-trace billed USD value:
  display `included/credit usage; per-trace USD unavailable`, never `$0`. An
  optional API-equivalent estimate must be separate and clearly labelled.
- If usage, pricing, credits, or billing basis is unavailable, mark that field
  incomplete rather than guessing or recalculating history from today's rates.
- Dashboard totals count each eval once. They must never multiply usage by the
  number of related Objects.

### Human dashboard and annotation

- Add **Evals** to the existing human navigation. The default list is newest
  first and shows time, source/actor, Chat when present, summary, status, every
  model/type used, total tokens, charge basis plus USD/credits or an explicit
  unavailable state, affected-Object count, and verdict.
- Use unambiguous labels such as `OpenAI · GPT-5.6 Sol · Codex harness · ChatGPT
  subscription` and `OpenAI · GPT-4.1 mini · direct API · metered API key`.
  Multiple usage sources appear as separate badges/rows, not one misleading
  concatenated model label.
- Provide bounded pagination and filters for date, eval kind, run status,
  verdict, component, provider, model, execution type, auth mode, billing mode,
  and affected Object. Do not load the entire trace table into browser memory.
- The detail view shows aggregate usage/cost, pricing provenance, ordered trace,
  failures/retries, and navigable canonical Object links with action/role.
- Annotation is human-only mutable metadata: verdict defaults to `unreviewed`;
  notes are optional bounded plain text; save annotator identity and timestamp.
  Updating annotation must not alter trace facts or create another eval.
- Mount eval read/annotation routes only on the existing trusted human listener.
  Do not expose them through the read-only agent API, ingestion/Curator listeners,
  standard agent client, or sandbox credentials.

## Checks

- [x] Database tests prove one eval per Slack window and human mutation,
  idempotent finish/inactivity/retry behavior, complete Object linkage, stable
  trace ordering, and rejection or explicit classification of unscoped runtime
  Object writes.
- [x] Curator tests cover multiple creates/updates/Connections, consulted
  candidates, no-op, validation repair, retry, partial provider usage, failure,
  commit, and undo/reversal without duplicated evals or accounting.
- [x] Accounting tests use fixed agent-plus-Curator fixtures to prove category-
  safe token aggregation, versioned API-price and ChatGPT-credit arithmetic,
  rounding, incomplete metadata, non-model interactions, and totals that do not
  multiply across Objects.
- [x] API/security tests prove bounded pagination/filtering, annotation
  validation and concurrency behavior, immutable trace facts, and absence of
  eval routes from every non-human listener.
- [x] UI tests cover mixed Sol-via-subscription and direct-API examples, all
  model/type/auth/billing filters and labels, pass/mixed/fail annotation and
  notes, USD/credit/unavailable states, long/error traces, Object navigation,
  loading/empty/error states, accessibility, and narrow layouts.
- [x] The repository-root verification suite and `git diff --check` pass.

## Verification Results

- `cargo fmt --check`, Clippy with warnings denied, and the complete Rust test
  suite pass.
- The database contract passes against an isolated PostgreSQL 16 plus pgvector
  database named `centaur_context_test`; migration 9 also passed its focused
  legacy-backfill and trace-order checks.
- Web type-checking, all 35 UI tests, and the production build pass.
- All 11 Python client tests and bytecode compilation pass.
- The adjacent Slack producer passes type-checking, seven focused usage/sink
  tests, Helm lint, and `git diff --check`.
- The full Slackbot test suite retains pre-existing timing failures reproduced
  from unmodified `main`; the changed usage and interaction-sink suites pass.

## Approval Boundary

This RD is planning-only until execution is separately assigned. It authorizes
no paid model calls, provider configuration change, deployment, hosted writes,
public ingress, external tracing service, export, retention deletion, credential
change, or modification of another logical database. Evals remain inside the
application-owned database and trusted human UI; every external action requires
explicit requester approval.

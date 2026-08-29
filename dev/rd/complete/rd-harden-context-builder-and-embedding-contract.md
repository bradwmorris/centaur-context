# RD: Harden the Context Builder and Embedding Contract

**Status:** `complete`
**Created:** 2026-08-28
**Completed:** 2026-08-29
**GitHub Issue:** #8

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** migrations through `0007_user_visual_context.sql`; the API,
search, embedding, configuration, and database code; authenticated
principal/thread client headers; database/API tests; the completed attribution
RD; the public context RD; and the cited retrieval implementations.

**Missing:** none

**Sequence:** Execute the description RD first so embeddings and budgets target
the description contract.

1. Bind agent `get-context` to a canonical Chat and authenticated Centaur thread,
   then anchor retrieval on that Chat and its participants.
2. Extend the context packet with bounded essential subtype state and enforce a
   serialized token/character budget in addition to the ten-Object limit.
3. Version the canonical Object embedding-input format, distinguish document
   and query embedding modes, and make PostgreSQL text-search configuration
   reusable beyond hard-coded English.
4. Add migration, API, retrieval-quality, fallback, budget, security, and
   standard-client tests; run the repository checks and record completion
   evidence.

## What We Are Doing

- [x] Require or explicitly resolve the current canonical Chat when an
  interactive agent asks Centaur OS to build context.
- [x] Return the minimum Task, Chat, User, Entity, or Memory subtype state needed
  to make each retrieved Object operationally useful.
- [x] Bound the complete context packet by both Object count and serialized
  context size.
- [x] Make embedding regeneration deterministic across input-template and model
  changes, including providers with different query/document input modes.
- [x] Remove hard-coded English as a universal public-product assumption while
  retaining fast, reliable full-text fallback.

## Contract

- **Goal:** Turn the current hybrid Object search into a conversation-aware,
  bounded, provider-neutral Context Builder suitable for the public Centaur OS
  contract.
- **Done:** `get-context` uses an authorized canonical Chat anchor, returns no
  more than ten useful Objects within a fixed context budget, includes bounded
  subtype state, rebuilds embeddings whenever their semantic input contract
  changes, supports query/document embedding modes, and performs configured
  full-text search when embeddings are missing or unavailable.
- **Files:** `0008_context_builder_embedding_contract.sql`; `src/api.rs`,
  `src/search.rs`, `src/embeddings.rs`, `src/config.rs`,
  `src/db.rs`; `tools/centaur_os`; deployment/example configuration; targeted
  Rust, database, API, and Python-client tests; this RD.
- **Agent owns:** Schema/API/client changes, context selection and budgeting,
  embedding versioning and modes, configurable search, migrations, tests, and
  local verification.
- **Requester owns:** Selecting or paying for an embedding provider/model, approving
  hosted configuration or writes, and approving deployment.
- **Out of scope:** A general Object `body`, raw-transcript embeddings, chunking,
  Chroma deployment, ontology or Curator write-contract changes, multi-hop
  search, and new external integrations.

## Detailed Requirements

### 1. Canonical Chat input

- Agent `get-context` requires a canonical `chat_object_id` in addition to the
  current message/query. Validate that it resolves to one active Chat subtype.
- Bind it to the existing request context: keep the bearer token and
  `X-Centaur-Principal-Id`, and require the stored provider/workspace/channel/
  thread identity to match normalized `X-Centaur-Thread-Key`. Reject missing or
  mismatched bindings; do not add another agent authentication system.
- Always consider the Chat, its canonical participating Users, and directly
  connected active Objects as deterministic candidates. Do not rely on semantic
  similarity to rediscover the conversation already in progress.
- General `search-objects` remains usable without a Chat and remains distinct
  from conversation-aware context building.

### 2. Essential subtype state

- Return one typed, compact subtype projection rather than dumping raw subtype
  rows. Examples include Task status/priority/owner/due date, Chat provider and
  thread identity, User kind/display identity, Entity subtype, and Memory event
  time.
- Keep subtype state subordinate to the canonical Object ID, type, title, and
  description. Do not duplicate canonical text or expose private provider data
  that the agent does not need.

### 3. Context budget

- Preserve the ten-Object maximum and add a deterministic serialized
  token/character budget covering Objects, subtype state, relevance rationale,
  and included Connections.
- Select and trim at semantic boundaries. Prefer fewer complete descriptions
  over many truncated fragments; never mutate stored descriptions to satisfy
  the packet budget.
- Report omitted-result counts and allow explicit follow-up search/read calls.

### 4. Versioned embedding contract

- Define one canonical formatter such as `centaur-object-v1` for the embedding
  document assembled from Object type, title, and description.
- Store the format version with each embedding and include it in stale detection
  and rebuild selection. A formatter, model, dimensions, or provider-mode change
  must queue a rebuild without changing the canonical Object.
- Let the embedding adapter distinguish `search_document` and `search_query`
  inputs while supporting providers where both use the same request shape.
- Keep embeddings as rebuildable derived records, never the source of truth.

### 5. Configurable text-search language

- Replace the unconditional PostgreSQL `english` configuration with one safe,
  installation-level setting. Use an explicitly documented language-neutral
  default unless the installer selects a supported language configuration.
- Validate the configured value against a server-side allowlist; never
  interpolate an unrestricted configuration identifier into SQL.
- Full-text retrieval must continue working when embeddings are disabled,
  queued, stale, failing, or rate-limited.

## Checks

- [x] API/client tests cover valid, missing, inactive, wrong-type, and
  thread-mismatched Chat Object IDs, required principal/thread headers, and
  preserve Chat-free `search-objects`.
- [x] Database/retrieval tests prove Chat/participant anchoring, bounded subtype
  projections, deterministic ordering, and one-hop graph relevance.
- [x] Budget tests cover long descriptions, many Connections, subtype state,
  Unicode, omitted-result reporting, and the ten-Object maximum.
- [x] Embedding tests prove format/model/dimension/mode changes queue rebuilds,
  stale vectors cannot match, retries remain safe, and full-text fallback works.
- [x] Text-search tests cover the neutral default, one configured language,
  invalid configuration rejection, names/identifiers, and non-English text.
- [x] The standard Python client and CLI remain compatible with the authenticated
  read-only API.
- [x] The complete repository verification suite and `git diff --check` pass.

## Verification Results

- Rust formatting and Clippy with warnings denied passed.
- The full Rust suite passed: 17 library tests, 10 API/auth tests, the curator
  evaluation, the disposable-database contract, and documentation tests.
- Migration `0008` and the full database contract passed from a fresh
  PostgreSQL/pgvector database named `centaur_os_test_issue_8`.
- All 33 web tests, TypeScript type-checking, and the production web build
  passed.
- All eight standard Python client tests and Python bytecode compilation passed.
- `git diff --check` passed.
- The optional package audit reaches its existing private-assumption check and
  flags the unchanged tracked file `dev/rd/rd-fork-centaur-os-multi-agent-poc.md`;
  that baseline file is outside this RD and unchanged from `origin/main`.

## Approval Boundary

This RD authorizes only local Centaur OS schema, API, retrieval, configuration,
client, and test changes when separately assigned for execution. It does not
authorize adding an Object body, changing the ontology, querying another
logical database, exposing a database DSN, calling or paying for an embedding
provider, public ingress, deployment, hosted writes, new external integrations,
credential changes, or deletion.

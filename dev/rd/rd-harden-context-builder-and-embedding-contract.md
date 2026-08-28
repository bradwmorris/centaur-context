# RD: Harden the Context Builder and Embedding Contract

**Status:** `backlog`
**Created:** 2026-08-28

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** `migrations/0005_hybrid_object_search.sql`, `src/api.rs`,
`src/search.rs`, `src/embeddings.rs`, `src/config.rs`, `src/db.rs`, the standard
Python agent client and CLI, database/API contract tests, the public shared
context RD, and Chroma's Foundation, Context-1, Context Rot, agentic-memory, and
open Foundation API/search implementations.

**Missing:** none

1. Make `get-context` conversation-aware by accepting and validating the
   canonical Chat Object ID, then use that Chat and its participants as
   deterministic context anchors before text retrieval.
2. Extend the context packet with bounded essential subtype state and enforce a
   serialized token/character budget in addition to the ten-Object limit.
3. Version the canonical Object embedding-input format, distinguish document
   and query embedding modes, and make PostgreSQL text-search configuration
   reusable beyond hard-coded English.
4. Add migration, API, retrieval-quality, fallback, budget, security, and
   standard-client tests; run the repository checks and record completion
   evidence.

## What We Are Doing

- [ ] Require or explicitly resolve the current canonical Chat when an
  interactive agent asks Centaur OS to build context.
- [ ] Return the minimum Task, Chat, User, Entity, or Memory subtype state needed
  to make each retrieved Object operationally useful.
- [ ] Bound the complete context packet by both Object count and serialized
  context size.
- [ ] Make embedding regeneration deterministic across input-template and model
  changes, including providers with different query/document input modes.
- [ ] Remove hard-coded English as a universal public-product assumption while
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
- **Files:** A narrow migration following `0005_hybrid_object_search.sql` if
  required; `src/api.rs`, `src/search.rs`, `src/embeddings.rs`, `src/config.rs`,
  `src/db.rs`; `tools/centaur_os`; deployment/example configuration; targeted
  Rust, database, API, and Python-client tests; this RD.
- **Agent owns:** Schema/API/client changes, context selection and budgeting,
  embedding-input versioning, provider-neutral request modes, configurable text
  search, migrations, tests, and local verification.
- **Requester owns:** Selecting or paying for an embedding provider/model, approving
  hosted configuration or writes, and approving deployment.
- **Out of scope:** Adding a general Object `body`, embedding raw Slack
  transcripts, Object chunking, deploying Chroma, changing the canonical
  ontology, agentic multi-hop search, adding new external integrations, and
  changing the Context Curator write contract.

## Detailed Requirements

### 1. Canonical Chat input

- `get-context` accepts a canonical `chat_object_id` in addition to the current
  message/query. Validate that it resolves to one active Chat subtype and that
  the authenticated interaction is permitted to use it.
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

- [ ] API/client tests cover valid, missing, inactive, wrong-type, and
  unauthorized Chat Object IDs and preserve Chat-free `search-objects`.
- [ ] Database/retrieval tests prove Chat/participant anchoring, bounded subtype
  projections, deterministic ordering, and one-hop graph relevance.
- [ ] Budget tests cover long descriptions, many Connections, subtype state,
  Unicode, omitted-result reporting, and the ten-Object maximum.
- [ ] Embedding tests prove format/model/dimension/mode changes queue rebuilds,
  stale vectors cannot match, retries remain safe, and full-text fallback works.
- [ ] Text-search tests cover the neutral default, one configured language,
  invalid configuration rejection, names/identifiers, and non-English text.
- [ ] The standard Python client and CLI remain compatible with the authenticated
  read-only API.
- [ ] The complete repository verification suite and `git diff --check` pass.

## Approval Boundary

This RD authorizes only local Centaur OS schema, API, retrieval, configuration,
client, and test changes when separately assigned for execution. It does not
authorize adding an Object body, changing the ontology, querying another
logical database, exposing a database DSN, calling or paying for an embedding
provider, public ingress, deployment, hosted writes, new external integrations,
credential changes, or deletion.

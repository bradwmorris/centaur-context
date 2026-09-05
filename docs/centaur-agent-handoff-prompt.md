# Centaur + Centaur Context Agent Handoff Prompt

Paste everything below into a new agent chat.

---

You are joining me as a senior product, architecture, and engineering collaborator on Centaur and Centaur Context. Treat this prompt as your initial orientation, then inspect the linked public documentation and local repository before making technical claims or proposing changes. Preserve the distinction between verified implementation, current operational posture, and planned work.

## The mission

We are building an operating model in which AI agents can do durable, shared, auditable work inside infrastructure controlled by their team.

Centaur is the agent control plane. It provides durable agent turns, isolated Kubernetes sandboxes, approved tools, credential-safe outbound calls, durable workflows, thin Slack/API clients, and organization-specific overlays. Its purpose is to make production agents recoverable, governable, and useful across real systems instead of leaving each Slackbot or integration to invent its own agent platform.

Centaur Context is a separate but complementary local-first shared context and operations application for Centaur users and agents. It is the durable semantic workspace: a canonical graph of the things the organization knows and acts on, plus controlled ingestion, curation, retrieval, provenance, and human oversight.

The intended relationship is:

```text
Humans / Slack / API clients
        |
        v
Centaur control plane
  - durable turns and event streams
  - sandbox assignment and execution
  - approved tool and workflow runtime
  - principal permissions and secret-safe egress
        |
        | authenticated HTTP API and approved overlay tool
        v
Centaur Context
  - canonical Objects and explained Connections
  - Tasks, Chats, Users, Entities, Memories
  - Slack interaction ingestion
  - atomic Context Curator
  - hybrid search and bounded context retrieval
  - immutable audit history and human controls
        |
        v
separate PostgreSQL database: centaur_context
```

Centaur orchestrates agents; Centaur Context gives those agents and their users a shared, durable, inspectable model of context and work. Do not collapse the two systems or assume that Centaur Context owns Centaur's session/runtime data.

## Centaur: public platform context

Start with these sources:

- Product overview: https://centaur.run/what-is-centaur
- Architecture: https://centaur.run/architecture
- Overlays: https://centaur.run/extend/overlay
- Tools: https://centaur.run/extend/tools
- Workflows: https://centaur.run/extend/workflows
- Security: https://centaur.run/security
- Configuration: https://centaur.run/reference/configuration

The important Centaur concepts are:

1. **Durable agent turns.** Postgres records the user turn, runtime assignment, execution request, streamed events, terminal state, and final-delivery obligation. Clients reconnect to/replay the event stream rather than owning execution state in memory.
2. **Isolated execution.** A conversation is assigned to a Kubernetes sandbox pod running a harness such as Amp, Claude Code, Codex, or pi-mono. The API owns assignment, serialization, cancellation, recovery, and release. Harness quirks stay behind a stable message contract.
3. **Approved capabilities.** Tools are Python plugins discovered from ordered `TOOL_DIRS`. Their `[project.scripts]` commands become local CLI shims inside sandboxes. Agents normally discover tools with `centaur-tools list`, inspect `<tool> --help`, and invoke the CLI.
4. **Credential-safe egress.** Sandboxes see placeholders, not raw long-lived upstream credentials. Iron-proxy binds actual values to allowed hosts and request locations and substitutes them on matching outbound requests. Tool metadata declares required grants.
5. **Durable workflows.** Python handlers run through Centaur's durable workflow engine. Checkpointed steps, sleeps, external-event waits, child workflows, tool calls, and agent turns survive process restarts. External side effects belong behind durable steps.
6. **Thin product surfaces.** Slackbot validates Slack signatures and owns Slack-specific rendering/delivery, while the API owns durable execution. External clients use the same control-plane primitives.
7. **Ordered overlays.** The base Centaur repository remains generic. Separate repos contribute organization-specific tools, workflows, skills, personas, prompt fragments, and sandbox files. Later sources can override earlier names. Production sources should be pinned when reproducibility matters.
8. **Production shape.** Kubernetes sandboxes, Postgres as source of truth, controlled egress, and optional logs/metrics/dashboards. Centaur is deliberately more infrastructure than a demo chat loop because it targets shared, recoverable production work.

## Centaur Context: repository and current shipped state

Work from the Centaur Context repository root.

Read these first and treat them as authoritative for repository work:

- `AGENTS.md` — ownership and safety boundaries plus mandatory verification
- `README.md` — product contract, architecture, integrations, and local development
- `compatibility.toml` — explicit release/API/ontology/schema compatibility
- `docs/installation.md` — guarded installation procedure
- `docs/operations.md` — backup, restore, upgrade, rollback, and uninstall
- `dev/AGENTS.md` — requirements-document workflow
- `dev/rd/*.md` — current backlog specifications; these are plans, not shipped features

Current repository baseline:

- The primary branch is `main`; inspect `git status` and `git log` for the
  current checkout rather than relying on a recorded commit hash.
- The implemented sequence is: preserved POC baseline; canonical graph migration; deterministic Slack ingestion; read-only Context Builder; atomic Context Curator; completed MVP human surfaces; packaged operations.
- `dev/` contains the tracked development workflow and backlog. Its RDs are
  plans and are not evidence that the described work has shipped.
- Release contract: Centaur Context `0.3.0`, HTTP API `v1`, ontology `v2`, database schema version `10`, standard tool version `0.3.0`.
- Deployment profile: single organization, local machine or trusted private network. Verified container architecture is `linux/arm64`; builds declare `linux/amd64` and `linux/arm64` support.

### Product and ontology

Centaur Context centers everything important on one canonical `objects` record with exactly one primary kind:

- `task`
- `chat`
- `user`
- `entity`
- `memory`

Every Object has a mandatory explicit description, optimistic revision, actor metadata, provenance, timestamps, an optional archive time, and optional protection from curator changes. The API derives active/archived lifecycle from that archive time. Task, Chat, User, Entity, Memory, Source, Note, and Theme records are one-to-one subtype rows keyed by the same canonical Object UUID. They are not separate competing identities.

Connections join canonical Objects and must include both a controlled kind and a plain-language explanation. The MVP kinds are:

- `involves`
- `about`
- `related_to`
- `depends_on`
- `derived_from`

Object Events are immutable audit records. The graph is intended to be explainable: never add a mysterious edge, treat a display name as identity, or erase provenance to simplify a UI.

Memories are event-shaped records, not a generic dumping ground for notes or model thoughts. Chats and Users remain canonical Objects. Provider identities live separately so two people with the same display name are never merged.

### Service implementation

The server is Rust using Axum, Tokio, SQLx, Serde, and PostgreSQL 16 with pgvector. The React/TypeScript/Vite UI compiles to static assets served by the same Rust process.

The process exposes four distinct listeners and trust boundaries:

- Human UI/API on port `8080`, intended to be reached only through localhost port-forwarding. It has the management/read surfaces under `/api/v2`.
- Agent API on port `8081`, ClusterIP-only and bearer-authenticated. It exposes only read operations: `GET /api/v2/context`, `GET /api/v2/search/objects`, and `GET /api/v2/objects/{id}`.
- Chat ingestion API on port `8082`, protected by a distinct token and exact Slack workspace/channel allowlist.
- Internal Context Curator API on port `8083`, protected by a third token and isolated from agent sandboxes and Slack transport by the supplied network policy.

Human `/api/v2` surfaces include metadata; Object listing/creation/read/update; Connection listing/creation/update/archive; Task listing/creation/read/update; Chat Messages; Users with embedded identities; Run listing/detail/review/Undo; search; context; and Object Events. Unknown API versions fail closed rather than being silently reinterpreted.

The database is always the separate logical database `centaur_context`, owned through the least-privilege application role `centaur_context_app`. Migration and operations code refuses unrelated database names except disposable test databases containing `centaur_context_test`.

### Slack interaction ingestion

Centaur's Slack transport posts a provider-neutral transcript envelope to `POST /api/v2/ingest/slack/interactions`. Centaur Context does not read Centaur's private session tables.

For each approved Slack thread, ingestion:

- creates or reuses one canonical Chat Object;
- creates or reuses canonical human and agent User Objects using Slack provider identities;
- stores ordered Chat Messages idempotently by Slack message ID;
- creates explained `involves` Connections for participants; and
- queues only the not-yet-queued message range for later curation.

An explicit `interaction_finished` boundary queues immediately. Otherwise, inactivity queues after a configurable interval whose default is ten minutes. A later continuation reuses the Chat and starts a new message range. Interactive agents do not directly write inferred context.

### Context Curator

The Curator is a separate background concern. It claims one queued message window, retrieves candidate Objects without using connection-count popularity, asks an optional OpenAI-compatible model for a bounded structured reconciliation plan, validates that plan independently, and commits the entire accepted plan in one short transaction.

Important invariants:

- Exactly one primary event Memory is required per completed interaction window.
- Every proposed change cites exact supporting Chat Message IDs.
- Task creation requires explicit confirmation, not an inferred suggestion.
- Updates use optimistic revisions.
- Only the five Connection kinds are allowed.
- Every changed Object receives a `derived_from` link to the source Chat.
- Protected records and Chat/User mutation by the Curator are rejected.
- Validation failure commits no partial graph writes.
- Each run retains model and prompt version, proposed and committed plan, result, changes, immutable events, and a before/after journal.
- Whole-run Undo is a compensating transaction: it archives records created by the run and restores prior values using new revisions. It never deletes source messages or audit history, and it refuses to overwrite later edits.

There is no default or automatic external model provider. Curator processing is enabled only when every required OpenAI-compatible setting is supplied. Without them, runs stay queued for evaluation or submission through the authenticated internal Curator API.

### Search, embeddings, and agent context

Canonical Object title and description are always indexed with PostgreSQL full-text search using an allowlisted installation-level configuration and a language-neutral default. If configured, an OpenAI-compatible embedding provider adds pgvector semantic retrieval. Reciprocal-rank fusion combines text and vector candidates; the Context Builder requires an authenticated canonical Chat, anchors its participants and direct Connections, includes compact subtype state, and returns at most ten Objects inside a deterministic serialized-size budget. Plain Object search remains Chat-independent and does not use connection-count popularity; Context Builder uses it only as a small importance signal.

Embeddings and embedding rows are rebuildable derived search data, never canonical business truth. Their versioned document format, model, dimensions, and query/document mode participate in stale detection. Object writes queue regeneration. If embeddings are absent, stale, incomplete, or failing, retrieval falls back to full text.

The public standard agent tool lives in `tools/centaur_context` because it is part of the public API contract. It provides only:

- `centaur-context get-context`
- `centaur-context search-objects`
- `centaur-context read-object`
- `centaur-context search-sources`
- `centaur-context list-sources`
- `centaur-context read-source`
- `centaur-context read-source-content`
- `centaur-context search-notes`
- `centaur-context read-note`
- `centaur-context create-note` (separate Note-write grant only)

The general agent listener has no write, deletion, SQL, arbitrary-request, or application-policy command. A separate internal listener and credential grant only idempotent Note creation; it requires principal and thread attribution and does not grant other writes. The server enforces these restrictions even if a sandbox bypasses the CLI. Requests carry bearer placeholders plus Centaur principal and thread identity headers. Real credentials are injected through Centaur's supported secret boundary.

The intended Centaur integration is a pinned overlay tool source plus authenticated Slack post-response ingestion. No Centaur semantic-version range is claimed yet; compatibility is contract-based.

### Human workspace

The current human UI has navigation for Objects, Tasks, Chats, Users, Entities, Memories, Sources, Notes, Themes, and Runs. It exposes the canonical ontology rather than raw database concepts. Object/Task details include incoming and outgoing Connections and activity. Chat details include messages. User details include provider identities. curator Run detail shows its source window/messages, model/prompt information, result, committed changes, failure state, and guarded whole-run Undo.

The workspace is deliberately local/private. Deployment manifests are reviewable artifacts, not authorization to apply them.

### Packaging and operations

The repository includes:

- a multi-architecture Docker build;
- guarded Kubernetes install/uninstall scripts;
- separate database bootstrap/drop scripts;
- backup and restore with checksum/metadata validation;
- a ClusterIP Service and Centaur Context-scoped NetworkPolicy;
- example Secret and provider-egress manifests;
- package-contract validation;
- forward-only migration and rollback guidance.

Installation requires an explicit Kubernetes context and namespace, a pinned image, PostgreSQL 16 with pgvector, four distinct API credentials of at least 32 characters, and exact approved Slack workspace/channel pairs. The local deployment gate expects context `kind-centaur-lab`, more than 15 GiB free disk, and a fresh validated backup before applying anything. Secrets must not be put into source, prompts, command arguments, or sandbox-visible DSNs.

Never infer from the existence of deployment files that deployment, public ingress, publishing, or hosted mutation has been approved.

## Current planned direction (not yet implemented)

Five requirements documents exist in `dev/rd/`, all with backlog status and complete/ready execution plans:

1. **Canonical Object ID navigation.** Make `Object ID` visible, copyable, deep-linkable, and correctly distinguished from supporting-record IDs everywhere.
2. **Clean type/source/user/Connection visuals.** Introduce consistent accessible labels, evidence-backed Slack source markers, deterministic avatars, correct user attribution, and directional navigable Connection presentation.
3. **Harden Context Builder and embedding contract.** Make context explicitly Chat-aware, add bounded subtype projections and a serialized context budget, version embedding input, support document/query embedding modes, and replace hard-coded English search assumptions with validated configuration.
4. **Read-only schema visualizer.** Add a human-only, allowlisted, paginated view of Centaur Context-owned tables, columns, constraints, relationships, and rows—never arbitrary SQL or cross-database access.
5. **Stronger Object descriptions and list snippets.** Enforce a more useful canonical-description contract and show consistent accessible snippets in every primary list without creating a competing notes/body field.

These RDs do not authorize implementation by themselves. Execution must be
requested separately. Follow `dev/AGENTS.md`: inspect first, keep scope narrow,
preserve unrelated changes, and use its issue, branch, verification, PR, and
merge-approval workflow during execution.

## Non-negotiable boundaries

- This repository owns only the `centaur_context` logical database. Never query, migrate, copy, or repurpose Centaur's `ai_v2` or Console databases.
- Agents use the authenticated HTTP API. Never give a sandbox a database DSN.
- Keep the ontology centered on canonical Objects, one-to-one subtypes, explained Connections, and immutable Object Events.
- Organization-specific agents, prompts, workflows, retention rules, and business policies belong in a private overlay, not the reusable base product.
- Do not copy code, credentials, data, or schema wholesale from The AGI Post.
- Do not add public ingress, cloud deployment, external integrations, hosted writes, new credentials, spending, publishing, repository-visibility changes, or destructive data operations without explicit repository-owner approval.
- Preserve user work and unrelated changes. Do not treat untracked files as disposable.
- Prefer the smallest compatible change and keep the public `/api/v2` plus standard tool contract stable unless a deliberate versioned change is requested.
- Distinguish human controls, Curator writes, ingestion writes, and agent reads. Do not widen the agent surface casually.
- Never call external model or embedding providers merely because configuration support exists. Provider choice, credentials, and cost require explicit approval.

## How to work with me

1. Begin by restating the relevant part of the architecture and what you verified from current sources. Do not claim that backlog items are shipped.
2. For repository tasks, read `AGENTS.md`, the relevant code/migrations/tests/docs, and any applicable RD completely before editing.
3. Make reasonable local, reversible assumptions but stop for any choice that materially changes product direction, ontology, security, deployment, external state, or cost.
4. Be evidence-led. Cite exact repository paths, API routes, migrations, tests, or public documentation behind important claims.
5. If asked only to diagnose, review, explain, or plan, do not implement or mutate external systems.
6. If asked to implement an RD, keep its status and checks honest and do only the documented scope.
7. Before handing off repository changes, run the repository-required verification:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
npm --prefix web run type-check
npm --prefix web run build
python3 -m pytest tools/centaur_context/test_client.py
python3 -m compileall -q tools/centaur_context
```

Database integration tests require `TEST_DATABASE_URL` and must target a disposable database whose name contains `centaur_context_test`. Report skipped environment-dependent checks explicitly. Also run any narrower RD-specific checks and `git diff --check`.

## Your initial response

After receiving this context:

1. Confirm the Centaur-versus-Centaur-Context boundary in your own words.
2. State what is implemented today versus what is backlog.
3. Name any source you could not access or any assumption you had to make.
4. Then ask what outcome I want to work on, unless I included a concrete task after this prompt.

When I provide a concrete task, use this context as constraints—not as permission to broaden scope.

---

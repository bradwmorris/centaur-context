# RD 83: Codex desktop Context bridge

GitHub Issue: [#83](https://github.com/bradwmorris/centaur-context/issues/83)

Status: core checks passed; an opt-in adopter backend is deployed. Native desktop
hook review and final desktop acceptance remain pending in PR #88.
The GitHub Issue contains the published design and acceptance contract.

## Outcome

Work performed in the Codex desktop app contributes to the appropriate Centaur Context instance without moving execution into Centaur. Automatically preserve the conversation as a canonical Chat and extract selective, concise Memories of meaningful decisions and evidenced work. Expose exactly `context_search`, `context_read`, and `context_apply` through MCP so the user can explicitly request other record creation or changes.

Ordinary conversation must not automatically create Tasks, Notes, Entities, Sources, or Themes. Chat identity, participant attribution, supporting Connections, immutable Events, and reviewable Runs remain system-managed bookkeeping.

This is a create-issue deliverable for a subsequent execute-issue run. The design below is the initial RD; issue creation does not install hooks, start capture, change credentials, deploy services, or enable inference.

## Current evidence and related work

Inspected local Context revision `bc471fa8853c56dfa3aecf98dbc49bf5f1c50fde`:

- `docs/centaur-integration.md` already defines pre-execution context retrieval and a completed-interaction sink.
- `src/ingest.rs` currently implements Slack-specific ingestion, identity resolution, and inactivity selection. Codex must have its own provider identity; do not fabricate Slack IDs.
- `tools/centaur_context/` and the universal HTTP contract already provide the three required operations. Chats and Memories are system-owned and cannot be created through ordinary `context_apply`.
- `src/curator.rs` enforces append-only Memory creation, explained Memory connections, and human-authored evidence. Assistant completion prose alone cannot prove that work happened.
- #78 owns concise event Memory quality and a separately validated committed-outcome evidence path. Reuse/coordinate that capture contract; do not duplicate its extractor or bring its background maintenance scope into this issue. Basic Chat capture and MCP development can proceed independently; truthful completed-work Memories depend on the evidence contract.
- Current official Codex docs describe local MCP servers and lifecycle hooks including `UserPromptSubmit`, `Stop`, and `Interrupt`. Hooks expose session/turn identity; full transcript format is explicitly not stable. Verify the actual desktop/runtime version rather than assuming CLI documentation proves desktop behavior: [hooks](https://learn.chatgpt.com/docs/hooks), [MCP](https://learn.chatgpt.com/docs/extend/mcp).

## RD: proposed implementation

### 1. A small local Codex bridge

Provide reusable local capture integration and an MCP adapter over Context's authenticated HTTP API. Prefer a local STDIO MCP server and supported lifecycle hooks; reuse the existing client and contract. Package installation, configuration, diagnostics, and removal together where practical. No new permanent agent runtime or event-bus platform.

Use prompt and turn-completion events for incremental capture, with interruption handling and bounded local delivery recovery. Saving must not depend on the model voluntarily calling a tool. Validate hook review/trust and event delivery on the supported desktop version. If that version cannot support reliable capture, report the limitation and settle the adapter design before claiming completion.

Capture the user-visible human/assistant conversation and narrowly selected result evidence, with ordered timestamps, stable host/session/turn/message IDs, repository identity, and actual human/agent attribution. Exclude system/developer instructions, hidden reasoning, credentials, unrelated sessions, and wholesale tool-output dumps. Specify text-only or attachment support honestly. Prefer structured hook fields; isolate and version-test any transcript reader. Unsupported transcript versions must be visible failures, not silent partial success.

### 2. Canonical Chat ingestion and reliable delivery

Extend the existing ingestion/curation machinery with a bounded Codex/provider-neutral contract. Retain Chat, User, Event, Run, and message semantics, including correct provenance for subsequent MCP writes. Do not rewrite the Slack pipeline or create parallel knowledge tables.

Use stable idempotency and message/event identity across repeated hooks, overlapping processes, resumes, restarts, and replay. Handle forks explicitly so inherited messages are not misrepresented as new events. Bound the local queue, retention, payload size, request timeout, and retry backoff. A Context outage must not block the Codex response; queued/failing/synced states must be inspectable. Queue contents remain local private data and cannot be committed to repositories.

### 3. Concise Memory capture only

Feed captured interactions to the existing append-only curator and the capture improvements from #78. Prefer one useful sentence, with a second only when necessary, within the canonical description limit. Retain meaningful decisions, commitments, and concrete evidenced outcomes. Zero Memories is valid; do not summarize every turn or turn operational noise into durable knowledge.

Keep evidence and a derivation link to the source Chat/turn. Distinguish discussion, requested work, verified completion, and unresolved work. Use a narrow authenticated, independently validated receipt path for actual code/test/PR outcomes, coordinated with #78; an assistant statement, pasted log, supplied URL, or caller-controlled `trusted` flag is insufficient by itself. Preserve the existing human-message evidence validator.

A deployment with automatic curation disabled must visibly report that capture is Chat-only until an explicitly configured compatible Memory path is enabled. A disabled curator is not successful Memory delivery. Do not enable unrelated background maintenance as a side effect.

### 4. Exactly three interactive MCP tools

Expose `context_search`, `context_read`, and `context_apply` using the existing canonical schemas, limits, principal permissions, revisions, and idempotency semantics. Reuse the generated Context guidance. Keep infrastructure capture endpoints separate from model-facing tools.

Search/read lets Codex use prior context. `context_apply` allows user-requested record changes, such as adding a Note or Task, subject to the existing field/assignment/issue requirements. The automatic capture path cannot exercise that ordinary write authority. Instructions must distinguish requests to save/change records from brainstorming; an already explicit user request is sufficient without a redundant confirmation ceremony. Never grant arbitrary SQL or direct Memory writes through MCP.

### 5. Explicit routing and private configuration

Support separate organization and personal Context instances. An allowlisted repository and its worktrees map to one configured instance; an explicit session binding handles intentionally selected contexts. Bind capture and all three tools to the same target, with separate scoped credentials. Confirm repository identity across worktrees rather than relying on a folder-name substring.

Keep real repository paths, endpoints, identities, credentials, and adopter policy in private overlay/local configuration. Core examples and tests use synthetic instances. A session touching another repository must not silently switch or broadcast its conversation. Shared-core and mixed-domain sessions require an explicit destination; absent one, report capture as unconfigured and send nothing. Transcript content cannot change routing or permissions.

Use the existing authenticated HTTP boundary and a trusted local transport/broker. Do not place bearer tokens in model context, tool responses, repository files, or agent command environments; never expose a database DSN. Document the trusted-host boundary honestly. Reuse private/loopback connectivity; no public ingress is part of this proposal.

## Scope, delivery, and review

1. Verify the installed desktop hook/MCP capabilities and record the supported version and payload contract.
2. Complete an explicit design/security review of capture selection, host trust, credentials, identity, routing, retry storage, and outcome verification, as required by AGENTS.md. Update this RD with the settled choices before activating the integration.
3. Implement the generic bridge, missing ingestion support, and synthetic tests in this repository; coordinate the capture seam with #78.
4. Document installation, hook trust, target selection, sync status, recovery, disable/removal, and compatible deployment configuration. Disable capture without deleting existing Chats or Memories.
5. Prove desktop-to-Context-to-agent retrieval against isolated synthetic instances. Private adopter rollout and inference settings belong in their overlay tracking and must follow those repositories' Issue/Task rules. Public core work remains tracked here, not as an organization-private engineering Task.

The future executor uses an isolated `codex/<issue-number>-<slug>` branch after updating local main, follows CONTRIBUTING.md, self-reviews, and opens a linked PR. Merge only with passing required checks and resolved findings under the repository's normal policy. Do not overwrite concurrent work.

## Non-goals

- Move the Codex harness into Centaur or add a remote-execution orchestrator.
- Automatically create domain records beyond Memories and necessary capture/provenance bookkeeping.
- Add background dreaming/legacy cleanup, another ontology, new dashboards, or an embeddings requirement.
- Import historical Codex sessions, capture every project on the machine, or mirror private data between instances.
- Expose public endpoints, database access, or blanket administrative/workflow credentials.
- Change private overlays or live inference policy through a public-core PR.

## Acceptance checks

- [ ] In a supported Codex desktop version, a synthetic conversation becomes one correctly attributed canonical Chat with ordered messages and resumable session provenance.
- [ ] A meaningful explicit decision produces a concise Memory linked to the correct Chat; verified completed work is described accurately. Brainstorming, requests, interruptions, and forged completion claims do not become successful outcomes.
- [ ] Routine status/retry/test noise creates no durable Memory. Automatic capture creates no Task, Note, Entity, Source, or Theme; required participant identity bookkeeping is bounded and attributable.
- [ ] The next Codex session and a Centaur agent can find and read the captured Memory and its supporting Chat through existing retrieval paths.
- [ ] MCP exposes exactly the three universal tools. An explicit user request can create a valid Note/Task through `context_apply`, with correct target, provenance, revision checks, replay behavior, and readback. System-owned Objects remain denied.
- [ ] Two synthetic instances prove organization/personal isolation, including worktrees, overlapping sessions, a denied wrong-target write, and unmapped/mixed sessions with no implicit routing or fan-out.
- [ ] Repeated hooks, reconnects, restarts, resumes/forks, and retries do not duplicate messages, Runs, or event Memories. Outages preserve bounded pending delivery and show an actionable sync state without blocking the answer.
- [ ] Unsupported desktop/transcript versions, missing credentials, untrusted hooks, disabled curation, and rejected payloads cannot be reported as successful Chat-and-Memory sync.
- [ ] Credential handling, capture filtering, request bounds, spoofed identity/evidence, and stored-content injection are covered by meaningful tests; the design/security review is recorded.
- [ ] Existing Slack capture, universal Context operations, and canonical history remain compatible. Required CONTRIBUTING.md checks pass; database integration tests use only disposable databases whose names contain `centaur_context_test`.
- [ ] Record actual end-to-end desktop evidence and supported versions. Publishing an MCP server or passing mocked hook tests alone does not complete this issue.

## Remaining decisions

- The destination for shared-core and mixed-domain conversations remains an adopter choice. Keep them unconfigured until explicitly selected; this does not block implementation of the generic routing contract.
- Verified runtime is 0.153.4 with paginated transcripts. Capture is text-only and begins at activation; native Desktop acceptance remains pending.
- The narrow Git receipt adapter records only observed commit changes. It deliberately cannot attest tests, merge, deployment, or general task completion.

## Execution design and security review — 2026-09-22

The owner authorized end-to-end execution. Work is isolated from concurrent
maintenance, media, UI composition and personal-operations changes.

- Verified desktop runtime: Codex Desktop 26.903.61454, bundled Codex 0.153.4.
  A disposable app-server protocol probe demonstrated that tool calls include
  trusted client metadata `_meta.threadId`; server processes do not receive
  `CODEX_THREAD_ID`. Routing must use that metadata plus a hook-created session
  binding, never a model-supplied destination or a process-global last session.
- Versioned transcript coverage uses only completed `UserMessage` and
  `AgentMessage` items for the current thread and new capture window. These
  carry stable item and turn IDs in the installed desktop's paginated history.
  All reasoning, instructions, tools, and unrelated thread items are excluded.
  Attachment content is not ingested in v1; coverage limitations are explicit.
- Add an opt-in private Codex listener with distinct capture and universal-tool
  credentials. Its configuration fixes the host, canonical human identity and
  exact allowed repository aliases. The server derives principal/thread identity;
  the model cannot supply it. Existing listeners and Slack behavior are unchanged.
- A local mode-0700 state directory holds immutable-target session bindings and
  a bounded SQLite delivery queue; token files are mode 0600 outside repositories.
  The local host/account and reviewed bridge code are trusted. These file modes
  do not isolate secrets from malicious full-access software running as that user.
  No credential is inserted into model context, tool output or shell environment.
- Context #84 landed the #78 capture/maintenance implementation. Reuse its concise
  chat rules and trusted Context Object Events. External code outcomes are a
  separately bounded bridge-verification path; do not grant Memory-write or
  maintenance operations through the three interactive tools, and do not accept
  assistant prose as a verified result.
- Add a provider allowlist to automatic curator claims so a personal deployment
  can enable Codex curation while leaving historical/ongoing Slack curation and
  background dreaming disabled. Token, payload, replay and route failures fail
  closed for delivery while ordinary Codex responses continue.
- Install only against explicitly configured destinations. No public endpoint,
  history import, cross-instance copying, or change to operational databases.
  Live restarts require coordination with the owners of each active deployment.

Review conclusion: the above boundary supports implementation without new public
network exposure or agent database access. Test identity/replay/routing, outcome
verification, privacy filtering, recovery and actual desktop delivery before
activation and closure. Remaining choices and evidence belong in this RD.


### Runtime and receipt verification update

The bundled 0.153.4 runtime executed reviewed hooks through its native trust
flow in an isolated Codex home. A real synthetic turn emitted SessionStart,
UserPromptSubmit and Stop with matching UUIDs and transcript paths. The resulting
paginated transcript contained only the expected visible message item shapes and
completed-turn marker. This establishes actual lifecycle behavior; final Desktop
activation against Context is still required.

Code outcome coverage is intentionally factual: an observed worktree gained new
Git commits with verified object hashes and first-parent continuity from a trusted
pre-turn checkpoint. It does not prove authorship, tests, merge, deployment or task
completion. Receipts use a distinct codex-capture actor, standard reversible Events,
and the existing memory_capture Run kind. They stay outside current Memory dreaming
admission. The server retains references and proof digests rather than raw author
or signature data. No new interactive Memory-write capability is exposed.

### Isolated end-to-end evidence — 2026-09-22

Using the actual bundled Codex 0.153.4 runtime, reviewed native hook trust and an
installed bridge package (not direct calls to mocked capture handlers):

- A fresh real turn spooled its human/assistant visible messages and completed
  marker into one canonical Chat in a disposable Context instance.
- A controlled local model fixture exercised the real curator validation/commit
  pipeline and produced one concise Memory with its originating Chat connection.
  This verifies data plumbing; real subscription inference remains a live adopter
  rollout check and is not claimed from that fixture.
- A fresh runtime session retrieved that Memory with the actual MCP client, then
  read its Connections and Events. An explicit Note write succeeded through MCP;
  repeating the same operation key reported replay without another Note. An
  explicit Task with a fixture owner and deadline was also created/read through
  MCP with its automatic Chat provenance connection.
- The normal Centaur Python agent client independently found/read the Memory and
  its Chat connection through the standard authenticated agent listener.
- A second disposable Context instance had separate credentials, canonical human
  and repository alias. It could not read the first instance's Chat/Memory and
  rejected its repository alias. Stopping/restarting only the second fixture
  service preserved the offline message and delivered it once after recovery.
- A real HTTP test exposed and fixed a transport mismatch hidden by earlier
  method mocks. A successful delivery cannot clear a transcript parsing error.
- Remote CI passed Rust lint/tests (including real disposable PostgreSQL), web
  checks, package/contract checks, Python tests and ARM64 composition/stock builds.
  Further fixes receive the same checks before merge.

Fixture databases: centaur_context_test_codex_a and centaur_context_test_codex_b.
No live instance was written or restarted during these checks. The installer is
still confined to a temporary Codex home. Native Desktop activation and adopter
subscription inference remain outstanding; this is not a completion declaration.


### Adopter rollout and remaining activation boundary

Both CI jobs passed for head `af6221add5481afa1fbd7b4286f6866c9f08b91a` and
merge candidate `0abddaae4cb2ea73abe772e4fc0371e88832e785`. Local targeted checks
include 71 Python tests. The checked ARM64 image was adopted by one explicitly
configured private instance. Archive checksum, OCI config/manifest linkage,
container runtime filesystem layers and clean stock UI revision were verified.
The private listener's authenticated contract read passed. A real dedicated
subscription-inference check returned the expected empty Memory-only plan for
synthetic status noise. This proves the deployed inference capability, not a
saved real-user Memory.

The bridge package, separate credentials, owned MCP entry, five hooks and bounded
recovery service are installed outside the repository. Native trust has not been
bypassed. The running desktop uses bundled runtime 0.153.4 (application
26.901.51231); the separately installed 26.903.61454 bundle supplied the isolated
runtime verification. Both emit the supported paginated format.

Computer-use automation refuses control of the desktop app itself. The owner
must review the installed hook definitions through the documented CLI `/hooks`
flow before desktop activation. No actual production desktop capture, historical
import, or completed deployment acceptance is claimed while that review is
pending. Keep the issue and PR open, then verify a new desktop turn, its actual
curation result and later retrieval before merge/final image adoption.

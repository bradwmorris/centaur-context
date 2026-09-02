# 7 — RD: Repeatable Slack RSI Flow

**Status:** `in_progress`
**Created:** 2026-09-02
**Issue:** [#80](https://github.com/bradwmorris/centaur-context/issues/80)
**Dependency:** Land and synchronize the Context, Centaur, and Enyu baseline
branches from RD #78 before execution begins.

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** RD #78 live Slack evidence; interaction and workflow Runs;
Source/Note read, write, and linkage contracts; retrieval and Run UI; local
Kubernetes deployment. The Source currently exists as Object
`c364005f-4cc9-58d2-8ed7-8ed4bce24460`; its identity and full dependency set
must be verified before each reset.

**Current execution:** Candidate fixes are implemented on Issue #80 branches.
Step 5's exact Object is selected from step 4 during each run. The first
approved reset and one diagnostic take are complete; a new clean reset and two
consecutive clean takes are still required.

1. Build a trusted reset operation that discovers earlier residue, emits a
   dry-run manifest, verifies ownership, and removes only this eval's approved
   Source and generated records.
2. Send the same five Slack messages in order in one Rez thread, waiting for a
   terminal response and durable state after each message. Substitute step 4's
   selected Object into step 5 without otherwise changing the script.
3. After each step, inspect Slack and Context for answer quality, Runs, traces,
   usage, consulted/affected Objects, records, and connections. Record defects
   and concrete improvements.
4. Fix the highest-value problems at their owning layer, add focused regression
   coverage, deploy the candidate, reset the fixture, and repeat the complete
   five-step flow from the beginning.
5. Once the full flow works well, repeat it from another clean reset and produce
   final evidence and video narration notes.

## What We Are Doing

- [ ] Make this five-step workflow reliable instead of running another broad
  matrix of individual bot updates.
- [ ] Make repeated takes safe: each take starts from a verified clean fixture
  without deleting unrelated Objects, shared RSI research, or external data.
- [ ] Prove Rez routes every new Source through the ingestion workflow, can
  edit the resulting Source, create a grounded Note, find relevant RSI
  material, and create or edit Connections without duplicating ingestion
  output.
- [ ] Capture per-step observations and talking points for a Slack/UI video.

## Fixed Five-Step Slack Flow

Run these messages in order in the same Slack thread:

1. `can you add this conversation as source`
   `https://youtu.be/hY6S__xeCjg?si=g_3DaHSPnJDxDNvG`
2. `hey @Rez (enyu researcher) in that recent invest like the best podcast, what does she say about recursive self-improvement?`
3. `can you create a note on this`
4. `what’s the other most interesting stuff we’ve recently added on RSI?`
5. `can you link the new note to <most relevant Object from step 4>?`

Step 5 must clearly name the selected Object and the evaluator must record its
ID. Choose the most substantively relevant existing Object returned in step 4,
not merely the first or newest. If none is defensible, fail the step; do not
invent a target.

## Baseline Take — 2026-09-02

The manually-run baseline establishes that steps 1–4 are functionally useful
and step 5 is blocked by a missing, deliberately ungranted capability.

- Source ingestion created one protected Source and eight ingestion
  connections: the originating Chat plus seven useful links to Sarah Guo,
  Invest Like the Best, Conviction, OpenAI, Anthropic, Andrej Karpathy, and the
  Agents Theme. Preserve this extraction behavior.
- Rez gave a grounded recursive-self-improvement answer, created Note
  `179d663e-97c1-4136-9377-a92ac4888383`, and returned a useful set of recent
  RSI Objects. The Note has exactly two durable links: protected
  `derived_from` to the Source and protected `about` from the Slack Chat.
- The final request created no connection or Object Event. Rez accurately
  reported that its live Context capability can create Notes/Tasks and attach
  source provenance or Themes, but cannot connect an existing Note to an
  existing Object.

Measured parent interaction Runs:

| Step | Duration | Tokens (cache-read) | Tool calls / failures | Result |
| --- | ---: | ---: | ---: | --- |
| Source ingestion | 2m39s | 72,682 (66,304) | 10 / 0 | Source + 8 connections |
| Source question | 3m01s | 40,702 (31,488) | 19 / 9 | Grounded answer |
| Create Note | 1m58s | 40,689 (39,424) | 14 / 6 | Note + 2 links |
| Retrieve RSI | 37s | 64,390 (58,496) | 13 / 0 | 23 Objects consulted |
| Link Note | 22s | 66,745 (65,664) | 1 / 0 | Unsupported; no mutation |

The 72,682 figure on Run `01a0600f-3332-73f3-9604-2ab8e46b7fd5` is accurate
provider-reported usage for one model attempt, not the total for the five-step
flow and not usage per tool call. It comprises 71,307 input and 1,375 output
tokens, including 66,304 cache-read tokens. Six of its ten persisted tool calls
are Source-readiness polls; tool execution is only a small share of its 2m39s
duration. The model attempt took about 87 seconds and post-commit polling
accounts for most of the remaining delay. The Run UI should expose input,
cache-read, output, and reasoning separately and distinguish workflow calls
from inner agent commands before cost or efficiency is judged from the headline
numbers.

The baseline also exposed avoidable work: attempts to use disallowed or
unavailable Context and Slack tools; three failed `read-artifact` invocations,
including a real `source_id` client bug; a rejected provenance field followed
by retry; repeated individual reads during RSI retrieval; and Curator retries
for deterministic protected-object, empty-plan, and no-change outcomes. Child
Curator Runs reported another 432,405 mostly cached tokens across the four
post-ingestion interactions. A valid no-op must complete as `no_changes`, and
deterministic validation failures must not trigger another model attempt.

### Connection Capability Decision

Rez must not receive a direct Source-create capability. Every new Source is
created through the existing ingestion workflow so artifact processing,
canonicalization, extraction, provenance, and initial Connection reconciliation
always run. Rez receives first-class abilities to start that workflow, edit a
completed Source, and create or edit Connections between existing Objects.
Ingestion and Rez's later edits must use the same canonical mutation rules for
identity, validation, idempotency, revisions, and Events.

When asked to add a Source, Rez has exactly one path: call the ingestion
workflow. Source ingestion owns automatic extraction and reconciliation of the
Source's initial inferred Connections. Rez waits for its terminal result and
uses the returned Source and Connection IDs; it does not independently recreate
relationships inferred from the same artifact. Rez uses Source editing or
Connection mutation only after ingestion completes and only for an explicit
user-directed change, or for a relationship absent from the ingestion result.

Connection creation is an idempotent upsert against one canonical identity:
relationship kind plus endpoint IDs and direction, with normalized endpoint
ordering for symmetric kinds. If ingestion and Rez assert the same
relationship, the second operation returns `reused` or safely enriches the
existing record rather than inserting another Connection. Distinct evidence or
descriptions are preserved as assertions/provenance on that canonical
Connection instead of becoming parallel edges. A stable request idempotency key
also makes exact command replay a no-op.

Expose `enyu-source-ingest` as the only Source-create command and
`enyu-context-mutate edit-source`, `connect`, and `edit-connection` as signed
workflow triggers. Source and Connection
edits require an expected revision and patch only explicitly named mutable
fields; stale edits return a conflict for reread rather than overwriting newer
pipeline or user work. Protected provenance, identity fields, object types, and
system-managed ingestion assertions remain immutable to Rez.

Authorization remains role-scoped: Rez may mutate Sources and Connections but
cannot use those operations to update unrelated Object bodies, archive Objects,
or bypass target visibility. Every operation validates active endpoints,
rejects self-links and invalid kinds, records actor and origin, emits Object
Events and affected IDs in the Run, and supports an inspectable dry-run where a
change is ambiguous.

The step-5 command therefore creates or reuses the requested Note-to-Object
Connection through the same path used by ingestion. The acceptance test must
also ask Rez to add an already-ingested relationship and prove the result is one
canonical Connection with both origins/evidence retained—not two edges.

Before the next take, also fix the `read-artifact` client contract, align Rez's
prompt with its granted tools, make Curator no-op reconciliation successful,
reduce readiness polling, and provide batch/richer retrieval reads. Runs must
record sanitized subcommands, error classes, duration, and disaggregated usage
so headline tool and token counts are interpretable.

## Reset Contract

- Seed discovery with the canonical video URL and known Source ID; validate its
  identity, provenance, and graph membership before deletion.
- The dry run must list every targeted Object, subtype, artifact, embedding,
  connection, Chat/message, Run, idempotency record, and dependency, with its
  reason for inclusion.
- The baseline Source is revision 9 and has later update Events plus inbound
  links from two older Tasks and the new Note. Reset discovery must distinguish
  eval residue from shared or older records; the known Source ID alone is not
  sufficient authority to cascade-delete its current graph.
- Delete only the Source and records proven to come from this eval's marked
  threads/Runs. Retrieved pre-existing RSI Objects must survive; only
  eval-created connections to them may be removed.
- Abort on unexpected inbound dependencies, ambiguous ownership, changed seed
  identity, or targets outside canonical `centaur_context` or legacy-named
  `centaur_os`. Never inspect or modify `ai_v2` or Console databases.
- Execute from trusted operator tooling. Agents and sandboxes continue to use
  authenticated HTTP APIs and must never receive a database DSN.
- Post-check that manifested residue is absent and protected RSI Objects remain.
  The reset must be safe to rerun when already clean.

## Per-Take Review and Video Notes

For each step, capture the message, expected behavior, Slack response, UI pages
to show, durable state and Runs, confusing moments, latency/failures, and a
concise video explanation. Keep narration separate from machine evidence.

After every take, rank improvements by impact. After a failed step, reset and
restart at step 1; do not hand-create state or weaken acceptance criteria.

## Contract

- **Goal:** Reliably recreate the fixed five-step Slack-to-Context RSI workflow
  from a clean fixture, with high-quality answers, correct durable linkage, and
  shared idempotent Source/Connection mutation for ingestion and Rez.
- **Done:** Two consecutive takes pass from verified clean resets: one Source
  per take; grounded answer; one Source-linked Note; useful pre-existing RSI
  retrieval; intended Note connection; readable Runs; and explanatory notes.
- **Files:** This RD; bounded Context reset/eval/report tooling; reusable Centaur
  instrumentation; private Enyu prompts, workflows, pins, and fixture data.
- **Agent owns:** Reset design, Slack operation, evidence, scoped fixes, tests,
  local deployment, clean reruns, and draft video notes.
- **Requester owns:** Approval of the first manifested destructive reset,
  disputed semantic or step-5 relevance judgments, final narration, publishing,
  and merge approval.
- **Out of scope:** The earlier broad Rez/Ed golden matrix; unrelated bot
  scenarios; deleting shared RSI research; direct agent database access;
  `ai_v2` or Console databases; public ingress; external integrations; or
  production deployment.

## Checks

- [ ] Reset dry run and execution operate from the same immutable manifest or
  fail if the graph changes between them.
- [ ] Reset is idempotent and its post-check proves a clean fixture plus intact
  baseline RSI Objects.
- [ ] Each Slack message correlates by eval marker, workspace, channel, thread,
  and Run IDs rather than timing alone.
- [ ] Evidence separates response quality, retrieval, mutations, connections,
  workflow outcome, traces, errors, latency, and usage for all five steps.
- [ ] Rez cannot create a Source directly: every add request starts ingestion;
  after completion Rez can edit the Source and create/edit Connections.
- [ ] Repeating an ingestion-created relationship through Rez reuses one
  canonical Connection and preserves both origins/evidence.
- [ ] `read-artifact` accepts its documented Artifact ID argument, Rez's prompt uses
  only granted tools and valid fields, and the five-step trace contains no
  avoidable command-discovery or corrected-schema retries.
- [ ] Curator records valid no-op reconciliation as `no_changes`; protected,
  empty-plan, and unchanged inputs do not trigger another model attempt.
- [ ] Source readiness avoids repeated fixed polling, and RSI retrieval uses
  batch or sufficiently rich search/read responses instead of one command per
  Object where possible.
- [ ] Run detail exposes sanitized inner command names, error class and duration,
  plus separate input, cache-read, output, and reasoning usage; workflow polling
  is distinguishable from substantive agent tool calls.
- [ ] Two consecutive clean-reset takes satisfy the complete done state.
- [ ] Focused regressions and relevant repository checks pass.
- [ ] `git diff --check` passes.

## Approval Boundary

Planning this RD authorizes no implementation or deletion. During execution,
the first destructive reset requires Brad's approval of its exact dry-run
manifest; later resets may reuse that approved scope only when identity and
dependency checks match exactly. Any expanded deletion set requires fresh
approval. Publishing, public ingress, production deployment, new external
integrations, credentials, unconfigured spending, PR merge, and deletion of
evidence remain separately approval-gated.

## Execution Journal — Candidate 1

- Context now has a dedicated listener for the exact
  `workflow-enyu-context-mutation` principal. It permits expected-revision
  Source edits, idempotent Connection create/reuse, and expected-revision
  Connection edits; it cannot create Sources.
- Connection creation reuses the active canonical edge and additively records a
  distinct assertion instead of inserting a parallel edge.
- Rez starts a signed zero-model mutation workflow. The write credential stays
  on that workflow; Rez keeps only ingestion, Note/Task creation, reads, and
  signed workflow-trigger grants.
- Source readiness is one bounded client operation rather than up to twelve
  visible workflow calls. `read-artifact` now takes one Artifact ID.
- Curator syntactic/effective no-ops complete without repair; protected or
  archived targets are skipped instead of causing deterministic retries.
- Run detail separates token categories, classifies readiness polling, and
  records sanitized tool names, duration, and error class.
- `scripts/reset_rsi_eval_fixture.py` verifies the local cluster and exact
  database/URL, computes the transitive eval dependency closure, and requires
  its current manifest hash plus an exact approval phrase before deletion.

First reset manifest (generated 2026-09-02):

- SHA-256: `56fcd6b731d104dec797dd2735b9d33206d45135fc6ae8981d71a021a1bcb0ea`
- Objects: 7 (the Source, two prior eval Tasks, the new Note, and three Curator
  Memories created from those eval interactions)
- Connections: 17; Runs: 22; Artifacts: 1; Embeddings: 20; Run-owned Events: 34
- Shared targets survive, including Slack Chats, Sarah Guo, Invest Like the
  Best, Conviction, OpenAI, Anthropic, Andrej Karpathy, and the Agents Theme.
- Execute only after Brad approves this exact scope. Any state change produces
  a different hash and requires a fresh manifest and approval.

## Diagnostic Take — 2026-09-02

- The reset completed and Source workflow
  `01a061b8-59f2-7280-8360-fda93dbd5dde` created Source
  `60a93eb0-5250-5074-9e2a-5b9550e53b7b`. Readiness completed after 70.643
  seconds with all 14 embeddings present. Its useful ingestion-created
  relationships were preserved.
- Steps 2–4 produced a grounded RSI answer, Note
  `2f890c6d-328b-4ea2-95e5-c8f0572a883c` with exactly its two intended initial
  links, and a useful recent-RSI list.
- Step 5 exposed three independent deployment defects: the HMAC rule was not
  scoped to the mutation path, the webhook token was absent, and the workflow
  principal lacked its specialized Context-write and Slack roles. Each failed
  before or after a bounded mutation and was repaired without duplicating the
  Note or Source.
- A fourth defect was semantic: the first successful mutation chose a different
  Ryan Greenblatt video (`fc63ac35-8340-56db-a5e0-9064f28cb046`) instead of the
  podcast URL cited in step 4. That one incorrect Connection and its Object
  Event were surgically deleted. Rez's guidance now requires preserving and
  verifying the exact cited URL/ID and scopes retry idempotency to the current
  triggering message.
- The unambiguous repair run `01a061f7-cade-7405-880b-f97e34f38cb6` created
  exactly one `related_to` Connection,
  `83d5cd3d-22ac-48a2-a691-ff92285c8094`, from the Note to Source
  `ee67e8c4-c7ea-5231-8626-9f76da35b7ba` at canonical URL
  `https://www.youtube.com/watch?v=-RXD4bTuFTo`. Slack posted the terminal
  completion receipt.
- This was a diagnostic repair take, not a clean pass: step 5 required retries
  and an explicit URL. The next take must begin with the approved surgical
  reset and prove the normal five-message wording resolves the step-4 target
  without correction.

Next reset manifest after the diagnostic take:

- SHA-256: `b7c44095b19c3bdfd9970e2ef94f01495341da4546215437668ff809bf4bcae7`
- Exact fixture closure: 2 Objects, 12 Connections, 6 Runs, 1 Artifact, 15
  Embeddings, 15 Run-owned Events, and 15 Events targeting fixture records.
- The closure contains only Source `60a93eb0-5250-5074-9e2a-5b9550e53b7b`
  and Note `2f890c6d-328b-4ea2-95e5-c8f0572a883c`; shared RSI targets survive.
- This hash has not been executed and requires fresh approval because the graph
  changed during the diagnostic retries.

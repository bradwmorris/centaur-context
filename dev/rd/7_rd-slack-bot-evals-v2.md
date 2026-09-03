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
Kubernetes deployment. The fixture identity and full dependency set must be
verified before each reset.

**Current execution:** Candidate fixes are implemented on Issue #80 branches.
The latest complete five-step attempt exposed remaining provenance, thread
context, and efficiency defects, so it is not a clean pass. Candidate 3 fixes
are implemented and awaiting deployment plus a newly approved surgical reset.
Two consecutive clean takes are still required.

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
- Brad approved this exact hash and the reset completed successfully before the
  next take.

## Functional Rerun — 2026-09-03

- Workflow `01a063f8-6d0a-726b-b950-f53a6cf876f0` created Source
  `5b870a30-7257-55ca-b9cb-bb51d6a1f333` with ten links and 14 ready embeddings.
- The source answer and recent-RSI retrieval were useful and grounded. Note
  `738920bb-ea04-4fa3-88da-a6c49dbee50e` was created with its two intended
  initial links.
- The unmentioned step-3 reply did not dispatch to Rez. A tagged retry was
  required, so the fixed five-message script did not pass unchanged.
- Step 5 resolved the cited Ryan Greenblatt Source
  `ee67e8c4-c7ea-5231-8626-9f76da35b7ba`, not decoy
  `fc63ac35-8340-56db-a5e0-9064f28cb046`, and created exactly one Connection
  `41220a6f-aa9e-4589-bd31-abb8268ca3f3`.

## Surgical Efficiency Fixes — Next Candidate

1. **Route thread replies once.** After Rez answers a thread, dispatch later
   human replies in that thread to Rez without requiring another mention.
   Mentioned replies must deduplicate by Slack message timestamp.
2. **Make the stored Source authoritative.** Resolve it from the current
   Chat/ingestion receipt and read its artifact. Do not invoke
   `company_context`, global tool discovery, web search, or YouTube when that
   complete artifact answers the question.
3. **Remove ingestion refetches.** The analysis worker already has the transcript.
   Remove its search tools or prohibit refetching; never send a placeholder key.
4. **Stop help-command discovery.** Put exact `read-source`, `read-artifact`,
   `create-note`, and `enyu-context-mutate connect` forms in Rez's instructions.
   A normal take must contain no `centaur-tools list` or `--help` calls.
5. **Align Note provenance.** Document accepted keys in CLI help and the Note
   template. Add a regression proving the first create request is
   valid; no rejected `passage_start`/`passage_end` retry is allowed.
6. **Trust mutation receipts.** Treat successful create/mutation responses as
   authoritative. Re-read only after an ambiguous response, conflict, or
   explicit verification request.
7. **Compact between Slack turns.** Preserve Slack-visible messages and a small
   state record containing Source, Note, and selected-candidate IDs; exclude old
   help text, transcripts, and tool output from later model inputs.
8. **Use one rich RSI retrieval.** Step 4 should return titles, IDs, canonical
   URLs, excerpts, and recency in one bounded result so step 5 can reuse the
   selected ID without searching again.
9. **Keep step 5 deterministic.** Read the Note and exact cited Source once,
   then invoke the zero-model mutation workflow once with message-scoped
   idempotency. Require one terminal receipt, one edge, and zero decoy edges.
10. **Set trace acceptance limits.** Step 2 permits bounded Source/artifact
    reads; step 3 one Note create; step 4 one rich retrieval; step 5 two reads
    and one trigger. Any discovery call, schema retry, refetch, or failed tool
    call fails the take even when the final answer is correct.

## Candidate 2 Implementation — 2026-09-03

- Centaur commit `baa8350adb73bc011ebdd5f1c59c900a79fb5bea`
  adds two deployment-gated Slackbot behaviors. Rez executes unmentioned human
  replies only after the prior subscribed-thread turn has completed, and each
  Rez execution uses a fresh harness session rebuilt from Slack-visible thread
  history. The latter retains visible Source/Note/candidate IDs without carrying
  forward earlier help text, transcript payloads, or tool output.
- Enyu commit `0556e202581f1ca2ecd32ebd705430cc306ca169`
  moves supplied-transcript analysis to the narrower ingestion-workflow
  principal, explicitly prohibits source refetch/tool use in that analysis,
  and gives Rez exact command recipes and receipt-trust rules for all five
  steps. Deployment pin commit `172060a` selects that overlay.
- Context commit `943840098626b69b74d37514f79fa7ad559ac6ad`
  documents the four accepted provenance keys and rejects unsupported keys in
  the client before a request is sent. Source search already returns the rich
  step-4 packet required here: Object ID, title, canonical URL, current Artifact
  ID, excerpt, and created/updated timestamps.
- Focused Slack routing/session tests, Enyu workflow/overlay tests, Context
  client tests, Helm rendering, type checking, and `git diff --check` pass. One
  unrelated pre-existing Slackbot test for clearing a harness-rejected sticky
  model times out when run alone and is not changed or hidden by this candidate.
- Local Helm revision 109 is deployed. Researcher startup confirms
  `execute_subscribed_replies_enabled=true` and
  `fresh_session_per_turn_enabled=true`; the editor retains both defaults as
  false. The Context UI is live at `http://127.0.0.1:8180/objects`.

Candidate 2 reset manifest (generated and executed 2026-09-03):

- SHA-256: `9fd1f8effef9dbbb379560b7705f83066cd7860d1bb33a48ecbaaaaf2ada7f09`
- Exact fixture closure: 2 Objects, 13 Connections, 7 Runs, 1 Artifact, 15
  embeddings, 16 Run-owned Events, and 16 Events targeting fixture records.
- The only Objects are Source `5b870a30-7257-55ca-b9cb-bb51d6a1f333`
  and Note `738920bb-ea04-4fa3-88da-a6c49dbee50e`. Shared RSI targets,
  including Source `ee67e8c4-c7ea-5231-8626-9f76da35b7ba`, survive.
- Brad approved this exact hash and the reset completed successfully before the
  Candidate 2 take.

## Candidate 2 Take — 2026-09-03

- Step 1 ran in Slack root `1788417818.785919`. Workflow
  `01a06602-ad00-74e4-8b79-d522878d7e0d` created canonical Source
  `fb3b4e29-929d-5099-a961-fb97a1d8f3ab`, one Artifact, 14 ready embeddings,
  and five useful ingestion Connections. The terminal receipt arrived only
  after readiness; this preserved the desired ingestion extraction behavior.
- Steps 2–5 ran in Slack thread `1788418028.442589`. Step 2 answered the RSI
  question accurately with the stored transcript and a 12:43 citation, but
  consumed 337,898 cumulative provider tokens. It made three help calls, one
  failed oversized Artifact read, and seven 20,000-character chunk reads.
- The Curator then double-dipped. It created duplicate Source
  `a7308830-6b00-4f15-b8b7-f0290ac3a827` from Rez's answer and Memory
  `f8f305f6-86e4-45c4-abc0-e4193d9bcadf` supported only by that assistant
  answer. Step 3 consequently created Note
  `c418dbf6-208e-4ced-ba9f-d234cefefbda` against the duplicate Source rather
  than canonical ingestion Source `fb3b4e29-929d-5099-a961-fb97a1d8f3ab`.
- The first unmentioned step-3 message did not dispatch because the live Rez
  Slack app subscribed only to `app_mention`. The undelivered message was
  deleted, `message.channels` and `message.groups` were enabled in Slack, and
  the identical message then dispatched. Both app manifests now declare those
  events and have a regression assertion.
- Step 3 used 134,636 cumulative tokens and performed two avoidable help calls
  before one successful Note create. Step 4 returned four useful RSI items and
  selected the 15%-versus-9% calibration result, but used 313,672 cumulative
  tokens across tool listing, four help/discovery calls, and four searches. Its
  visible answer omitted Object IDs required by the next turn.
- Step 5 used 222,376 cumulative tokens and created no Connection. Rez tried a
  direct Slack thread read, received `invalid_auth`, then repeated discovery and
  broad Context search before asking Bradley to restate the targets.
- Root cause: fresh-per-turn Slack context deliberately excluded Rez's own
  visible replies. The new turn therefore contained prior human prompts but not
  Rez's step-4 answer, making “these items” impossible to resolve. This take is
  a failure even though steps 1–4 produced useful visible output.

## Candidate 3 Implementation — 2026-09-03

1. **Retain visible assistant state.** Fresh-per-turn Rez sessions now rebuild
   from both human messages and Rez's visible Slack replies through the current
   trigger. Stateful sessions continue excluding self messages so history is
   not duplicated. A focused Slack emulator regression proves both behaviors.
2. **Make Source ownership deterministic.** The Curator can no longer create a
   Source under any circumstances; Source creation remains ingestion-only.
   Worker plans drop Source proposals and dependent Connections before normal
   plan validation, producing a no-op instead of a second model attempt.
3. **Reject assistant-derived durable claims.** Curator Memory and Task creates
   must cite only human-authored messages. Assistant-derived proposals and
   their dependent Connections are dropped before reconciliation, with a
   deterministic-filter trace; direct reconciliation retains a validation
   backstop.
4. **Make the bounded recipes prominent.** Rez's top-level prompt now states
   that one RSI turn performs one substantive recipe and must never begin with
   tool listing, help, Slack-history, or alternate-search commands. Visible
   answers must include Object IDs needed by the next turn.
5. **Persist normal thread events.** Both Rez and Ed Slack manifests include
   `message.channels` and `message.groups`, matching the live Rez repair and
   preventing an app reinstall from regressing unmentioned thread replies.

Candidate 3 is not yet a passing take. It must be committed, deployed from the
three feature worktrees, then exercised from a fresh exact reset manifest. The
expanded fixture now includes the canonical Source, Curator duplicate Source,
assistant-derived Memory, and Note, so the old reset hash must not be reused.

Candidate 3 deployment and reset checkpoint:

- Context commit `7791c9ab6d8387707ae397993f4b58af91aa3d16`, Centaur
  commit `e907441306c985c588cfba350f8f6204524fb30a`, and Enyu content
  commit `2f2653e60e277b81ab54c17773d3f74b815f4fb2` are pushed. Enyu
  deployment commits `a02ed88` and `002491f` pin those revisions and preserve
  the existing 5 GiB Postgres claim instead of attempting an unrelated resize.
- Candidate Slackbot and Context images were rebuilt from their feature
  worktrees and loaded into the local `centaur-lab` cluster. Helm revision 112
  is deployed and the Rez, Ed, and Enyu Context rollouts are healthy.
- New dry-run manifest SHA-256:
  `448d463bb81057bb955f119b6472ef952ed9f693e80596ca4253be723b369e58`.
- Exact expanded closure: 4 Objects, 10 Connections, 6 Runs, 1 Artifact, 17
  embeddings, 15 Run-owned Events, and 15 Events targeting fixture records.
  The four Objects are canonical Source
  `fb3b4e29-929d-5099-a961-fb97a1d8f3ab`, duplicate Curator Source
  `a7308830-6b00-4f15-b8b7-f0290ac3a827`, assistant-derived Memory
  `f8f305f6-86e4-45c4-abc0-e4193d9bcadf`, and Note
  `c418dbf6-208e-4ced-ba9f-d234cefefbda`.
- This deletion set is larger than the prior approved manifest. It has not been
  executed and requires fresh approval of this exact hash under the reset
  contract.

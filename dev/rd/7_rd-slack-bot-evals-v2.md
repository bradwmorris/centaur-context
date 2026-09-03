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

**Current execution:** Candidate 4 completed all five actions and proved the
bounded recipes, Note creation, retrieval, and Connection mutation. The take
also exposed and fixed artifact-read authorization, optional UUID
serialization, Note-to-Chat linkage instructions, and sandbox-capacity
headroom. Because those fixes were applied during the take, a clean reset and
two consecutive clean post-fix takes are still required.

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

Candidate 3 was not yet a passing take at this checkpoint. It had to be
committed, deployed from the three feature worktrees, then exercised from a
fresh exact reset manifest. The expanded fixture included the canonical Source,
Curator duplicate Source, assistant-derived Memory, and Note, so the old reset
hash could not be reused.

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
- This deletion set was larger than the prior approved manifest. Brad approved
  the exact hash and the reset completed before the Candidate 3 take.

## Candidate 3 Take — 2026-09-03

- The approved reset manifest
  `448d463bb81057bb955f119b6472ef952ed9f693e80596ca4253be723b369e58`
  executed successfully.
- Step 1 ran at Slack root `1788421657.150639`. Workflow
  `01a0663d-4a36-7ce4-ad09-4071b6792c43` created Source
  `c0296c58-cb79-5c29-8e09-d8c1c06327f9`, one Artifact, 14 Source
  embeddings, and eight useful ingestion Connections. This preserves the good
  related-Object behavior and confirms Rez still routes Source creation through
  ingestion.
- The Curator incorrectly created redundant Task
  `97934cf5-82d5-4f18-8638-c9f7e959ce48` (run
  `415feec6-0abc-4a91-b7f3-506bd63f2d35`) by restating the already-completed
  ingestion command. No duplicate Source was created.
- Steps 2–5 ran in Slack thread `1788421897.373899`. Step 2 returned the correct
  transcript-grounded answer with the 12:43 passage. Step 3 created Note
  `63829ae4-b3d8-4e26-ac04-3431923c3087` with only the intended `derived_from`
  Source link and Chat `about` link. Step 4 returned four useful recent RSI
  ideas, though the visible response omitted their Object IDs.
- The exact step-5 wording, `@Rez (enyu researcher) can you link our new note to
  these items`, succeeded without a corrective user message. Workflow
  `01a06647-d5e6-7d1d-88be-5c7eb0eeb1b5` created exactly one Connection,
  `0a46db2a-74c6-4ce3-88be-25974228fecb`, from the new Note to existing Note
  `70a7ce79-e61e-53b3-80f3-b759f35ad252`. It did not recreate ingestion links.

Exact parent-run efficiency evidence:

| Step | Run | Input / output tokens | Tool calls / failures |
| --- | --- | ---: | ---: |
| 2 | `c57dc643-3d1f-4eac-b423-ccffc0002ea3` | 31,187 / 193 | 13 / 4 |
| 3 | `9e63a9f5-cf97-4448-afbd-3951f8aa8dc3` | 28,217 / 51 | 5 / 0 |
| 4 | `bd8d92e0-b8ac-4d15-84ca-e7a787491fff` | 40,648 / 190 | 18 / 0 |
| 5 | `53d0dfe6-ca70-4c7b-9f1d-f9db3ddb7b83` | 44,165 / 67 | 19 / 2 |

The user-visible flow is now functional, but this is not a clean efficiency
pass. The live sandbox set `AGENT_PERSONA=researcher` and
`CENTAUR_PERSONA_SOURCE_PATH`, yet its Codex `thread/start` request did not
include the persona prompt. Rez therefore saw generic instructions and spent
calls discovering tools instead of following the bounded five-step recipes.

## Candidate 4 Implementation And Deployment — 2026-09-03

- Context commit `1c080873a660fa8858d7d4eadd19f062e2591f34` deterministically
  rejects Curator Tasks that merely restate an explicit Source-ingestion
  command. The prefilter and reconciliation backstop prevent the redundant Task
  without changing ingestion's initial related Objects or Connections.
- Centaur commit `ee7792d21014294191029aa86f20955dca32c32a` reads the selected
  persona's `PROMPT.md` and injects it as Codex `developerInstructions` at
  `thread/start`. A configured missing or empty prompt fails closed rather than
  silently falling back to the generic agent.
- Enyu deployment commit `f60cf7c` pins those two revisions and sandbox image
  `centaur-agent:rd80-rsi-flow`. Follow-up commits `4f50ac0`, `3081fa0`, and
  `e3e862f` wire the existing private-overlay token and make the unchanged local
  console image deterministic. Commit `a2e93aa` updates the deployment-pin
  regression.
- Helm revision 118 is deployed. The repo cache is ready at Centaur
  `ee7792d2`, Enyu `2f2653e6`, and Context `1c080873`; Rez, Ed, API, Console,
  and Context are healthy. The Context UI returns HTTP 200 at
  `http://127.0.0.1:8180/objects`.
- Verification: 12 focused Context Curator tests pass; the focused Centaur
  persona-injection integration test passes; all 69 Enyu tests plus 15 subtests
  pass; Helm renders successfully; and `git diff --check` passes.

Candidate 4 reset manifest (generated and executed 2026-09-03):

- SHA-256: `ada5077db1dab337f17f547039bac77f9cebc06db6a2d54216d5fc41fd72b2eb`
- Exact closure: 3 Objects, 12 Connections, 7 Runs, 1 Artifact, 16 embeddings,
  16 Run-owned Events, and 16 Events targeting fixture records.
- The only Objects are Source `c0296c58-cb79-5c29-8e09-d8c1c06327f9`,
  redundant Task `97934cf5-82d5-4f18-8638-c9f7e959ce48`, and Note
  `63829ae4-b3d8-4e26-ac04-3431923c3087`. Pre-existing RSI Objects survive;
  only this take's Connections to them are included.
- Brad approved this exact hash, and the transaction removed the stated closure
  while preserving the shared RSI Objects.

## Candidate 4 Take And Surgical Fixes — 2026-09-03

- The first post-reset ingestion attempt failed before Source creation because
  the local lab used the short `centaur-iron-proxy:latest` image name. The exact
  failed Slack/DB closure was deleted after approval. Enyu commit `16b1715`
  selects the cached GHCR repository; the clean step-1 root is
  `1788430463.597139`.
- Step 1 completed through workflow `01a066c3-a580-757d-a1ff-d678b9cf3fd5`.
  Source `74c0dd73-de79-56e8-9d48-9bb9d4952e30` has one complete transcript,
  14 embeddings, and the five desired initial links: Sarah Guo, Invest Like the
  Best, Conviction, Agents, and the originating Slack Chat. There is one active
  canonical Source for the URL and no redundant ingestion Task.
- Steps 2–5 used Slack thread `1788430649.955449`. Step 2 found the exact Source
  with one search, but its one bounded Artifact read failed because the narrow
  grant omitted `/api/v2/artifacts/*`. Enyu commit `7e880f2` adds only the v1/v2
  Artifact GET paths; a live read of artifact
  `304c00cc-00aa-5fe7-b118-bc7e683304b4` now succeeds.
- Step 3 initially hit leaked sandbox capacity, then exposed a client contract
  bug: omitted `originating_chat_object_id` was serialized as `""`, causing HTTP
  422. Context commit `66a8b47` preserves JSON `null` for omitted optional Note
  and Task UUIDs, with 21 client tests passing. Enyu commits `6a8ea58` and
  `e9bf8a3` reduce the warm pool from three to one under the unchanged total
  limit of four and preserve the pinned API image structure. After deployment,
  one create call produced Note `719f301a-fb21-4118-8683-a6fadb81fe73`.
- The created Note has the correct `derived_from` Source link but no Chat link.
  The persona had incorrectly claimed that link was automatic while omitting
  the argument. Enyu commit `7e880f2` now requires the trusted Context packet's
  `Current Slack Chat Object ID` via `--originating-chat-object-id`, so both
  links are created in the Note transaction on the next take.
- Step 4 used exactly one `search-sources` call and returned the two strongest
  direct RSI Sources plus two clearly labelled adjacent Sources, all with IDs
  and URLs. Step 5 read the exact Note and RSI Simulator Source once each, then
  triggered one mutation. Workflow `01a066d5-35bf-705c-a537-3b8937b26dd1`
  created exactly one `related_to` Connection,
  `ec47eb97-3671-4c77-a6d1-350e4f5ea458`; no ingestion relationship was
  duplicated.
- Helm revision 123 is deployed from the feature worktrees. Repo cache pins
  Centaur `ee7792d2`, Enyu `7e880f2`, and Context `66a8b47`. The protected
  `/Users/bradleymorris/Desktop/dev/centaur` checkout was not changed.

Successful-path model usage and substantive tools:

| Step | Run | Total / cache-read tokens | Substantive tools |
| --- | --- | ---: | ---: |
| 1 | `206e587e-07ea-4244-ab70-7a8109f386d2` | 27,440 / 26,880 | 1 |
| 2 | `266d77f8-9fe2-420e-960a-41e178f38ce2` | 28,241 / 27,648 | 2, one failed |
| 3 | `a396dee7-02ca-4740-b3d4-dcc31b969e7b` | 27,992 / 26,752 | 1 |
| 4 | `96bf254a-866a-45e3-83c8-7b71ddece236` | 30,684 / 26,880 | 1 |
| 5 | `c67c667b-ab80-443b-8e64-92f65b51bafd` | 30,086 / 28,032 | 3 |

The UI's large token number is accurate as total model input, but 136,192 of
the 144,443 successful-path tokens were cache reads. Only 7,395 input tokens
were not cache reads, plus 856 output tokens. Tool tracing currently undercounts
commands chained in one shell invocation, so the sandbox shim audit is the
authoritative step-5 count. The take is functional but not clean because two
capacity attempts and one pre-fix 422 attempt remain in the Slack thread; do not
use it as the final video take.

## Candidate 5 Clean Reset, Retake, And Timestamp Fix — 2026-09-03

- Brad approved the exact reset generated by the trusted
  `scripts/reset_rsi_eval_fixture.py` path. Manifest SHA-256
  `a392e78e140a69c509d2164d35afa74ce3fa2dd25d3561c634efc6e8755f1923`
  removed only the preceding take's 2 Slack Chats, Source, Note, 11
  Connections, 17 Runs, 1 Artifact, 17 embeddings, 16 Run-owned Events, and
  16 Messages. The four shared RSI Objects were verified intact.
- Step 1 ran at Slack root `1788433478.117559`. Ingestion workflow
  `01a066f1-a639-7ec8-a2aa-4530cb6a323d` created canonical Source
  `7c57669a-0782-52ea-8466-50b8b0c9e9b3`, complete transcript Artifact
  `40f5fa1c-777d-5f18-b0fc-e5cfd73b1942`, and 14/14 ready embeddings. Its
  initial links are useful and unchanged: Sarah Guo, Andrej Karpathy,
  Anthropic, Conviction, Invest Like the Best, OpenAI, Agents, and the
  originating Slack Chat. No Task or duplicate Source was created. Step 2 did
  not begin until readiness completed.
- Steps 2–5 ran in Slack thread `1788433650.580309`. Step 2 used one Source
  search and one Artifact read to give a concise transcript-grounded RSI
  answer. Step 3 used one Note create to produce Note
  `5346dc0d-1cdb-4166-a84c-f6c48bec71f3`, with exactly the intended Chat
  `about` Note and Note `derived_from` Source links. Step 4 used one search and
  returned the two strongest direct RSI Sources with IDs: RSI Simulator
  `8d3ff281-1ae0-53e4-973d-153b56f6da3c` and The Economics of Recursive
  Self-Improvement `46f174c1-38ef-5958-a3de-a8238fe8f174`.
- The first exact step-5 attempt at `1788434099.648799` failed safely instead
  of guessing: Rez lacked the current triggering message timestamp required
  for mutation idempotency. Centaur follow-up PR commit
  `c1d778195335304888f079aa74a33d5d5636b869` now includes the exact trigger
  timestamp in the trusted per-turn Context packet and explicitly forbids
  substituting the thread-root timestamp. Enyu commit `47dfee6` pins that
  landable commit. Healthy Helm revision 124 runs the content-identical
  pre-cherry-pick build `344ff40e`; focused Slack tests (4), type checking, and
  all 20 Enyu overlay tests pass.
- Retrying the identical fifth message at `1788434562.939289` succeeded.
  Workflow `01a06702-3310-71e6-bb3f-df887416dc5c` and Context mutation run
  `cdbe9992-b72b-4f9e-9b07-2ec0f27707a4` created exactly one `related_to`
  Connection, `11f486f7-1205-4864-b4c8-52d10c9b1e2c`, from the new Note to
  RSI Simulator. It did not duplicate an ingestion-created relationship. One
  target per mutation turn is the intended bounded behavior.

Successful parent-run efficiency evidence (the repaired step-5 retry is used):

| Step | Run | Input / cache-read / uncached | Output | Substantive tools |
| --- | --- | ---: | ---: | ---: |
| 1 | `4500e816-54e5-462b-b074-40786aaa41a5` | 27,698 / 27,008 / 690 | 54 | 1 |
| 2 | `a854caa7-fde2-4e5d-bd57-675e71a312cf` | 33,497 / 27,776 / 5,721 | 179 | 2 |
| 3 | `abeb878f-b9a6-42b6-86dd-1b5b6b07a93b` | 28,125 / 26,752 / 1,373 | 47 | 1 |
| 4 | `6b44a11f-644f-4e07-a352-dbe0c0a5dd3b` | 30,196 / 26,880 / 3,316 | 291 | 1 |
| 5 | `ceb427d2-5132-4828-8364-838672f86667` | 30,568 / 28,032 / 2,536 | 189 | 2 |

Across those five Slack parent runs, gross input was 150,084 tokens, of which
136,448 (90.9%) were cache reads; fresh input was 13,636 tokens. The UI's gross
token figure is therefore accurate but is not a measure of fresh model work.
Every action used the bounded recipe with no failed substantive tool call.
Ingestion itself is intentionally separate and heavier: 42,583 input tokens,
five deterministic pipeline stages, and a 70.5-second readiness wait.

This is a successful diagnostic retake, not the first clean acceptance pass,
because step 5 required a code fix and retry. The next candidate must start
from a newly approved surgical reset and complete the exact five messages once,
without intervention. Two consecutive clean-reset passes remain the acceptance
bar.

Candidate 6 reset preview (not executed):

- Manifest SHA-256:
  `bcab64f8137a8598974374c48b87ce201f663b56008b374a0b208142e5c4baad`.
- Exact database closure: the 2 current Slack Chats, Source, Note, 15
  Connections, 14 Runs, 1 Artifact, 17 embeddings, 20 Run-owned Events, and 20
  Events targeting fixture records. Shared Objects remain outside the Object
  deletion set. The corresponding Slack roots are `1788433478.117559` and
  `1788433650.580309`; deletion awaits explicit approval of this exact preview.

## Video Narration Notes

1. Add the podcast once; ingestion owns transcript processing, canonical Source
   creation, embeddings, and initial related Objects.
2. Ask Rez a grounded question; show the Slack answer and the Source/Artifact
   used in the UI.
3. Ask for a Note; show that the Note is durably linked to both the Source and
   the Slack conversation.
4. Ask for other recent RSI work; show retrieval across existing Objects and
   preserve the returned IDs for the next action.
5. Ask Rez to link the new Note to the most relevant returned Object; show the
   single idempotent Connection in the UI and explain that it does not duplicate
   ingestion-created relationships.

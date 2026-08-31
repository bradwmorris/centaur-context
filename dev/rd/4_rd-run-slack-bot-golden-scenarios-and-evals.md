# 4 — RD: Run and Review Real Slack Interactions

**Status:** `scoped`
**Created:** 2026-08-30
**Dependencies:** Active priorities 1 and 3; successful schema-17 and `/api/v2`
cutover; completed embedding and complete-capture rollout from RD1; deployed Enyu
Researcher (`Rez`) and Editor (`Ed`) workflows.

## Execution Plan

**Status:** `still needs work`

**Basis checked:** The consolidated `runs`, `artifacts`, `embeddings`, and
authoritative `object_events` schema; trusted-human Runs API/UI; Slack interaction
ingestion and Curator; Context Builder; Source intake; current Enyu persona,
Source-ingestion, article-publication, and publication-email contracts; the two
real Sources selected below; and Hamel Husain and Shreya Shankar's guidance to
evaluate real product behavior, inspect complete domain-specific traces, start
with binary checks and human error analysis, and classify the first failing
workflow stage.

**Current mismatch:** Schema 17 stores a Slack interaction, Curator execution,
Source intake, and external action as separate Run rows. That is not the product
model required here. For this Slack surface, one human interaction must have one
Run row. All work caused by it must appear as ordered trace evidence and linked
durable effects on that same Run.

**Missing:** Priorities 1 and 3 completion; the one-Run interaction
instrumentation described here; a pre-capture Entity-resolution and confirmation
step; embedding rollout/backfill; the focused Evals dashboard; Brad's later
article/email instructions; and approval for deployment, publication, or email
actions. Brad has approved the executing agent operating the signed-in Slack UI
for Tests 001/002, starting their Source workflows, and answering the expected
MTS creation question with `Yes, create MTS and continue.`

1. Preflight the deployed system, then have the executing agent control the real
   signed-in Slack UI and perform Tests 001/002 exactly as a user would. Do not
   substitute API calls or synthetic messages for this first pass.
2. After each interaction, inspect the resulting Run and durable records. Fix an
   immediately blocking, obvious, self-contained defect directly on a clean,
   current `main`; verify and push it before continuing. Use the normal issue/
   branch/PR path for broader, risky, coordinated, or multi-commit changes.
3. Build the minimal Evals dashboard incrementally from the evidence the first
   real Runs actually produce, while making one interaction the single causal Run
   and instrumenting missing stages. Add no eval table.
4. Encode each proven interaction as one private executable module, replay after
   approved deployment of fixes, and continue the same observe→fix→inspect loop
   without deleting canonical data.

## What We Are Doing

- [ ] One row in `runs` represents one real Slack interaction from its first user
  message until everything it started is terminal.
- [ ] That Run makes the conversation, retrieval, decisions, workflow activity,
  writes, Artifacts, embeddings, usage, replies, errors, retries, and curation
  examinable in one place.
- [ ] Every test is a normal piece of work Brad would actually ask Rez or Ed to
  perform, encoded as one private module with exact interaction steps and checks.
- [ ] A separate Evals screen provides finer-grained inspection of Runs without a
  second datastore or a parallel notion of execution.

## Simple Product Model

The unit is an **interaction**, not a model call, workflow, database mutation, or
test case. One interaction may do any combination of:

1. Converse in Slack, retrieve Context, answer, and later curate the conversation.
2. Start and complete one or more workflows:
   - capture a Source;
   - create or publish an article;
   - create, approve, and send a publication email.

All of that belongs to the same Run. A workflow engine may retain its own external
execution ID, but Context stores that ID as evidence on the interaction Run; it
does not create another Context Run for the same causal interaction. Likewise,
Curator, intake, embedding, and external-action steps append to the interaction
trace and attach their Object Events to its Run ID.

The Run is the causal spine, not a duplicate data dump. Full Slack messages stay
in Chat Messages and full article/transcript text stays in Artifacts. The Run
contains stable IDs, hashes, counts, states, excerpts where safe, and links needed
for the dashboard to reconstruct the complete interaction.

### Interaction boundary

- Open the Run on the first Slack message in a new interaction window.
- Keep it open while Rez or Ed asks clarifying questions, waits for Brad, invokes
  workflows, waits for approvals/readback, and posts final replies.
- Complete it only when every activity it started is terminal and its final Slack
  response has been captured. `done`, `finished`, or the approved inactivity rule
  may then run curation on the same Run.
- Duplicate Slack deliveries and workflow callbacks update the same Run
  idempotently.
- A later, distinct request opens a new Run, even when it uses Sources created by
  an earlier Run.

## Required First Execution

The agent executing this RD does not merely prepare scripts for Brad. It operates
the signed-in Slack UI itself using the available browser-control capability,
types the exact Test 001 and Test 002 messages, reads Rez's replies, follows the
threads, waits for workflow completion, and then inspects the trusted-human Run
and Object views. It must not claim an interaction occurred from a local function
call, fixture, or direct workflow/API invocation.

For Test 002, Brad's approval in this RD authorizes the agent to send the exact
reply `Yes, create MTS and continue.` only after Rez correctly reports that Ryan
Greenblatt exists and MTS does not. Any different proposed new Entity, ambiguous
match, changed Source, destructive action, publication, email send, credential
change, or deployment requires Brad's further direction.

Run the first pass in this order:

1. Record deployed Context/Enyu revisions, model and embedding configuration,
   Slack workspace/channel, and the existing Source/Entity preflight without
   writing anything.
2. Run Test 001 through Slack. Wait for its workflow and interaction to become
   terminal. Inspect its complete Run, Source, Artifact, Connections, embeddings,
   messages, events, and final response before starting Test 002.
3. Classify the first failing stage. If a small code defect prevents useful
   continuation, fix it immediately under the direct-main rule below. Do not
   weaken the expected interaction.
4. Add or refine only the smallest dashboard slice needed to make Test 001 easy
   to understand from one screen.
5. Run Test 002 through Slack, including the approved MTS confirmation. Inspect
   it completely, fix immediate blockers, and add the missing Entity-resolution,
   approval, transcript, and embedding views to the dashboard.
6. Repeat each interaction only when the relevant fix is deployed. A deliberate
   repeat is a new Run and must reuse the canonical Source; a duplicated Slack
   delivery remains on the original Run.

### Direct-main rule for immediate fixes

Brad explicitly authorizes direct-to-`main` fixes discovered during these first
interactions only when the change is obvious, self-contained, reversible, and
small enough for one reviewed commit. Before editing, the agent must use the
canonical checkout, confirm it is clean, run `git pull --ff-only`, and preserve
all unrelated work. It must run proportionate targeted tests plus the required
repository checks, inspect its diff, commit, and push the verified fix.

If `main` is dirty, cannot fast-forward, the cause is uncertain, the change spans
repositories or migrations, or it needs multiple commits/meaningful design, the
agent must not mix it into `main`; it records the failure on the Run and uses the
normal issue/branch/PR workflow. Direct-to-main approval does not authorize
deployment. The agent may prepare and push the fix, but must obtain explicit
deployment approval before validating that fix against the live system.

## What One Run Must Expose

### Input

- Slack workspace, channel, thread, initiating message, actor, timestamps, and
  exact user request.
- Deployed application/model/prompt/workflow versions.
- Optional test module ID/version, attached by the trusted runner after exact
  workspace/channel/thread correlation so the Slack request remains natural.

### Ordered trace

Use bounded typed entries sufficient to reconstruct the path:

- message received and response posted;
- Context query, lexical/Object-vector/Artifact-chunk candidates, fused ranking,
  selected Objects and exact Artifact spans;
- agent decision, selected tool/workflow, and why clarification was or was not
  required;
- Source existence check and Entity matches/candidates;
- human confirmation requested and received;
- workflow started, external workflow ID, meaningful steps, approval waits,
  retries, callbacks, failure, and completion;
- Artifact capture/hash/size/outcome and Source current-version decision;
- embedding configuration, expected chunks, queued/completed/failed counts,
  offsets, target hash, latency, usage, and degraded fallback;
- Curator planning/validation/commit and all model/provider usage.

Never store credentials, hidden reasoning, full private prompts, vectors, or full
Artifact bodies in the trace.

### Durable effects and result

- `object_events.run_id` for every Object or Connection mutation caused by the
  interaction.
- `consulted_object_ids` for every Object actually supplied to an agent.
- Result references to created/reused Source, Artifact, Entity, Connection, Chat,
  Message, publication, and external-action identities.
- Final workflow states, final Slack reply, errors, first failing stage, and
  automatic test checks when the Run is a named test.
- One existing human verdict (`unreviewed`, `pass`, `mixed`, or `fail`) and one
  optional review note. Brad does not have to grade every individual line.

## Source-Capture Decision Flow

Every new Source request follows this exact sequence before capture:

1. Normalize the URL and check whether the Source already exists by canonical
   URI and, once fetched, exact content hash.
2. If it exists, do not duplicate it. Rez identifies the existing Source and
   stops. Refresh or recapture requires a separate explicit request.
3. If it does not exist, inspect authoritative metadata and identify the main
   creator, publisher/channel, participant, and subject Entities that warrant
   durable links.
4. Search Context for each proposed Entity and reject same-name decoys.
5. Automatically reuse and connect confident existing Entities.
6. For every important missing Entity, Rez explains what it is and asks one
   direct question: `Should I create this new Entity?`
7. Do not start the write workflow while confirmation is outstanding. A refusal
   omits the Entity and records the decision; silence produces no write.
8. After approval, create the approved Entity, Source, explained Connections,
   and complete immutable Artifact under the same interaction Run, then index
   the current complete Artifact.

The current Enyu Source workflow cannot do steps 3–7: it can connect only existing
Object IDs, omits uncertain links, starts immediately, and cannot pause to create
an approved Entity. This is a prerequisite product change, not an eval exception.

## Test Modules and Runner

Do not build a large abstract “golden dataset” for the MVP. Build a small library
of named real-interaction modules in the private Enyu overlay:

```text
evals/
  runner.py
  shared/                 # Slack operation, Run collection, common assertions
  tests/
    test_001_dwarkesh_article_capture.py
    test_002_mts_youtube_capture.py
    test_003_cross_source_questions.py
    test_004_create_article.py
    test_005_create_publication_email.py
    test_006_replay_permissions_and_failures.py
```

Each module contains the exact messages the executing agent sends as Brad's
approved operator, required starting state,
expected dialogue, expected final outcome, automatic checks, and any short human
questions. Shared code correlates the real Slack interaction, finds the one Run,
collects trusted HTTP evidence, computes checks, and writes the structured check
summary back into that same Run's result. No module receives a database DSN.

Every module reports the same sections:

1. Request and routing.
2. Source/Entity resolution or Context retrieval.
3. Clarification and approval.
4. Workflow execution.
5. Canonical writes and Object Events.
6. Artifact completeness.
7. Embedding/indexing or retrieval.
8. Slack outcome and curation.
9. Idempotency, usage, latency, and first failure.

Start with agent-operated Slack: the executing agent types the approved exact
interaction in the real UI, watches the complete response, and then the runner
collects and checks the Run. Automate repeated Slack operation only after these
first traces reveal which failures actually matter.

## Minimal Evals Dashboard

Add **Evals** beside **Runs** in the trusted human UI. It is a finer-grained view
of the same rows and uses the same Run detail/API plus a bounded eval projection.
It creates no `evals`, `eval_cases`, `eval_suites`, or check-results table.

### List view

Show one row per interaction Run:

- time, actor/bot, natural-language request, and capabilities used;
- status, overall verdict, duration, model/provider, tokens/cost basis;
- Source/workflow/publication/email outcome;
- automatic checks passed/failed when it is a named test;
- first failing stage and unreviewed/error/retry indicators.

Filter by date, Rez/Ed, capability, status, verdict, workflow, model, test module,
failure stage, and affected/consulted Object. Ordinary production interactions
remain inspectable even when they are not named tests.

### Detail view

Render one interaction on one screen in these sections:

1. **Conversation** — exact ordered Slack messages and final reply.
2. **Context used** — queries, candidates by retrieval mode, fused ranks, selected
   Objects, Artifact spans, and what reached the model.
3. **Decisions** — route/tool choice, Source existence result, Entity matches,
   clarification, and Brad's approval/refusal.
4. **Workflow** — workflow IDs, steps, waits, retries, callbacks, and terminal
   result.
5. **Database effects** — created/reused/changed Objects, explained Connections,
   Object Events, and links to their normal detail pages.
6. **Artifact and embeddings** — current Artifact, kind, hash, bytes, capture
   outcome, chunk coverage, model/dimensions/config, failures, and readiness.
7. **Usage and timing** — model attempts, tokens, billing basis, estimated cost,
   stage timings, and total duration.
8. **Checks and review** — expected versus observed facts, green/red automatic
   checks, first failure, one overall verdict selector, and one optional note.

The screen must show failure and partial state, not only successful final output.
It must link rather than duplicate full transcripts and Artifact bodies.

Do not wait to design the entire dashboard before running Slack. After Test 001,
ship the smallest useful list and detail view for conversation, workflow, Source,
Artifact, Connections, embeddings, and errors. After Test 002, add the missing
Entity-resolution and human-confirmation detail. Later question, publication, and
email Runs add their own sections only when real traces demonstrate the required
shape. The existing generic Runs screen remains available throughout.

## Test 001 — Real Dwarkesh Article Capture

**Starting state:** The canonical URL and exact article do not already exist.
Dwarkesh Patel, OpenAI, and Hugging Face exist as active Entities with their exact
IDs frozen by preflight; same-name decoys, if any, are also frozen.

**Brad sends to Rez:**

```text
Add this to Context:
https://www.dwarkesh.com/p/openai-huggingface
```

**Expected interaction:** Rez first checks Source and Entity existence. Because
the Source is new and all three Entities exist, Rez asks no unnecessary question,
states that it will reuse them, starts exactly one Source workflow, returns the
workflow receipt, and later posts the terminal Source ID in the same Slack thread.

**Hard checks:**

- One interaction Run contains the Slack, workflow, intake, Artifact, embedding,
  Object Event, completion, and curation evidence; no child/parallel Context Run
  represents the same activity.
- Source title is exactly `The Rise and Fall of Agent Civilizations` and the
  canonical URI is the normalized supplied URL.
- Source kind is `article`; byline is `Dwarkesh Patel`; publisher is the agreed
  canonical publisher value frozen before execution.
- Description is 50–150 direct words that identifies the Source, author, OpenAI
  agent events, Hugging Face incident, and report-synthesis context. It contains
  no invented claim, generic placeholder, or copied article fragment. Length and
  required concepts are automatic checks; faithfulness/usefulness is one binary
  human check included in the overall review.
- Source→Dwarkesh Patel uses `involves` with an author-specific explanation;
  Source→OpenAI and Source→Hugging Face use `about` with specific explanations.
- Exact complete article text is the current immutable `article_text` Artifact;
  its SHA-256, bytes, capture method/version/time, language, and completeness
  outcome reconcile.
- Deterministic expected chunk count and offsets are calculated from the frozen
  Artifact and chunker version. Every current chunk is completed for the exact
  hash/model/dimensions/config; missing, failed, stale, or superseded ranked rows
  are zero.
- A deliberate later repeat is a new interaction Run with its own normal Slack
  messages, but it reports the existing Source and creates no new Source, Entity,
  Connection, Artifact, embedding row/job, Object Event, or workflow write. A
  duplicate delivery of the original Slack event creates neither a new Run nor a
  duplicate Message.

## Test 002 — Real MTS YouTube Capture

**Starting state:** The normalized watch URL and transcript do not already exist.
Ryan Greenblatt exists as the exact active Entity frozen by preflight. MTS does
not exist under an equivalent identity or alias.

**Brad sends to Rez:**

```text
Add this to Context:
https://youtu.be/N9lye22ce48?si=4e_Ed8YB3nA_AzqI
```

**Expected Rez question before workflow start:**

```text
I found the existing Entity Ryan Greenblatt. I couldn't find MTS, the publisher
and interview channel for this video. Should I create a new Entity called MTS
and connect it to the Source?
```

**Brad replies:**

```text
Yes, create MTS and continue.
```

Rez then starts exactly one workflow, returns its receipt, captures the complete
public English transcript, and posts the terminal Source ID in the same thread.

**Hard checks:**

- No workflow or durable write begins before Brad's approval; the question,
  answer, decision, and resumed execution are visible on the same interaction Run.
- Source title is exactly `1,200 AI Agents Colluded to Hack Hugging Face | Ryan
  Greenblatt`; kind is `video`; canonical URI is
  `https://www.youtube.com/watch?v=N9lye22ce48`; publisher is `MTS`.
- Description is 50–150 direct words identifying MTS, Ryan Greenblatt as
  interviewee, agent coordination/reward hacking, and control/alignment context,
  without copying show notes or inventing claims.
- Existing Ryan Greenblatt is reused. Exactly one active Entity titled `MTS` is
  created after approval, described as the MTS technology/news/interview show and
  publishing channel. The Source has specific `involves` Connections to Ryan as
  interviewee and MTS as publisher/channel. Existing OpenAI/Hugging Face subject
  Entities are connected with `about` when confirmed by preflight.
- YouTube URL variants normalize to the same watch URL. Show notes are not treated
  as the transcript.
- One complete immutable English `transcript` Artifact stores the captured
  transcript with exact SHA-256, bytes, language, capture metadata, and outcome.
- Every expected transcript chunk is current and embedded for the exact
  hash/model/dimensions/config with valid character offsets and no gaps beyond
  the defined overlap policy.
- Refusal at the MTS prompt creates no MTS Entity and starts no workflow. A later
  repeat creates its own interaction Run, reports the existing Source, and starts
  no capture workflow; duplicate delivery of an existing Slack event remains on
  its original Run and creates no duplicate Message.

## Tests 003–006 — Same Real Work Journey

- **003 — Cross-source questions:** In new Rez and Ed interactions, ask practical
  questions whose answers require the captured article, the video transcript,
  and then both together. Freeze exact supporting Artifact spans after Brad
  inspects the captures. Measure candidate ranks, Context inclusion, evidence
  use, answer faithfulness, uncertainty, latency, and context bytes.
- **004 — Create article:** Brad gives Ed a real brief using Tests 001/002 as the
  cited Sources. The same interaction Run must show retrieval, drafting,
  validation, preview, exact approval, publication readback, and durable effects.
- **005 — Create publication email:** Brad asks Ed to email the published Test
  004 article. The Run must show bounded fields, preview, recipient set, exact
  approval binding, send, provider callbacks, and delivery result without storing
  recipient addresses or email bodies in trace metadata.
- **006 — Replay, permissions, and failures:** Repeat the same real requests to
  prove Source reuse, duplicate-delivery safety, Ed/Rez role denial, approval
  refusal, missing captions, embedding degradation, retry visibility, and honest
  partial failure.

Brad supplies the natural-language brief, desired article outcome, email wording,
and the most useful cross-source questions before Tests 003–005 are frozen. Do
not invent artificial prompts merely to make the system pass.

## Scoring and Operating Loop

- Automatic checks are binary and concrete: expected ID, exact title/URI/hash,
  no duplicate, required trace stage, current embedding coverage, evidence rank,
  or terminal state.
- Human review is simple: read the interaction and detailed checks, then choose
  one overall `pass`, `mixed`, or `fail` and optionally explain why. The dashboard
  records the first failing stage automatically; Brad is not required to grade
  every line.
- A named test passes only when every hard check passes and Brad's overall verdict
  is `pass`. Fluency cannot compensate for incorrect data, retrieval, or writes.
- Run once to understand behavior, inspect the complete trace, add or correct
  checks for observed meaningful failures, fix the product, and replay. Never
  weaken an expectation to make a defect pass.
- The first successful live capture creates permanent canonical evidence.
  Subsequent runs prove reuse and idempotency; they do not delete and recreate it.
- Re-run affected modules after changes to routing, prompts, models, workflows,
  schema, capture, embeddings, retrieval, curation, or external integrations.

## Checks

- [ ] Database/API tests prove one Slack interaction has one Run across
  clarification, workflow, callback, intake, Object Events, embedding state,
  curation, retries, failure, and completion.
- [ ] Concurrent append/replay tests prove ordered trace facts and usage cannot be
  lost or duplicated.
- [ ] Source-resolution tests cover existing Source, confident Entity reuse,
  same-name decoy, missing-Entity approval/refusal/silence, and no write before
  approval.
- [ ] Article/video tests prove metadata, descriptions, exact Artifact
  completeness, URL normalization, captions, hashes, chunk coverage, indexing,
  and replay.
- [ ] Retrieval tests separately expose lexical, Object-semantic,
  Artifact-chunk-semantic, and fused candidates plus selected evidence spans.
- [ ] Evals UI/API tests cover every detail section, failure/partial states,
  filtering, links, automatic checks, one overall review, authorization,
  pagination, accessibility, and narrow layouts.
- [ ] No eval table, eval-only Run hierarchy, agent database access, trusted-human
  credential exposure, or duplicate full transcript/Artifact storage is added.
- [ ] Both repositories' targeted tests, repository-root verification, and
  `git diff --check` pass.

## Approval Boundary

Brad has explicitly approved the executing agent controlling the signed-in Slack
UI for Tests 001/002, fetching and capturing those two selected Sources through
the existing workflow, using the already configured model/embedding providers,
performing their expected hosted writes, and approving creation of MTS exactly as
specified. He has also approved small qualifying fixes directly on `main` under
the rule above. This does not authorize another Source, another new Entity,
credential or provider changes, backfill, deletion, public ingress, deployment,
publication, email preview/send, or additional spend outside the existing
configuration; those require explicit approval. The Evals/Run-review surface
remains trusted-human-only, and agents/sandboxes receive neither its credential
nor a database DSN.

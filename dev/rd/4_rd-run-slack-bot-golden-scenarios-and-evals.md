# 4 — RD: Run Slack Bot Golden Scenarios and Evals

**Status:** `scoped`
**Created:** 2026-08-30
**Dependencies:** Active priority 3; completed consolidated-schema,
canonical-data, Editor-publishing, and Paradigm-corpus RDs; successful schema-17
and `/api/v2` cutover; user-facing bots are `Rez` (the stable Enyu researcher
identity) and `Ed` (the stable editor identity).

## Execution Plan

**Status:** `still needs work`

**Basis checked:** Active priority 3; completed consolidated-schema,
canonical-data, Editor-publishing, and Paradigm-corpus RDs; schema-17 `runs`,
`artifacts`, `embeddings`, and authoritative `object_events`; `/api/v2`; Slack
interaction snapshots and literal `done`/`finished`; Chat-aware hybrid Context
Builder; Source intake; Curator reconciliation; Runs UI; Enyu roles and live
acceptance record. Closure curates a primary Memory, not a Note, and interactive
bots cannot write Context.

**Missing:** Priority 3 completion; successful schema-17 live cutover; Brad's
private article and YouTube selection; a frozen manifest containing expected and
decoy Object IDs, Artifact hashes, and evidence spans; and Brad's approval for
live Slack/model/hosted-write use.

1. Freeze a private golden manifest and preflight the deployed revisions,
   corpus hashes, identities, permissions, search/embedding health, and clean
   Slack test channel.
2. Drive versioned, copy-paste-exact Slack scripts through the real UI, using a
   unique suite marker in every root message; finish each thread with the sole
   message `done` and wait for terminal external workflow, `slack_interaction`,
   `intake`, and child `curator` Run states.
3. From an operator-side local runner, collect Slack transcript plus trusted
   human `/api/v2/runs/{id}` evidence, Artifacts, Chat Messages, and Object Events
   into one redacted evidence bundle; score deterministic facts automatically
   and review answer quality separately.
4. Replay idempotency cases, classify failures by stage, rerun only after the
   fixture/version changes, and publish a concise pass/fail matrix. File
   implementation defects separately rather than weakening expected results.

## What We Are Doing

- [ ] Prove Rez can ingest one article and one YouTube transcript, reuse the
  correct existing identities, and avoid duplicates or attractive decoys.
- [ ] Prove a Rez conversation closes into correctly linked canonical records
  and one complete `slack_interaction` Run with one child `curator` Run.
- [ ] Prove Ed retrieves the required existing evidence through the intended
  search/context path, answers faithfully, and produces complete Run evidence.
- [ ] Make the suite repeatable by an agent operating Slack without database
  credentials, timestamps copied by hand, or subjective inspection as the only
  oracle.

## Contract

- **Goal:** Validate the complete human→Slack→bot/workflow→Context→Curator→Run
  loop against known answers after priority 3.
- **Done:** Every required scenario passes its hard invariants and reviewed
  rubric twice, exact replay creates no duplicates, every failure is attributable
  to a stage, and the evidence bundle identifies deployed and fixture versions.
- **Files:** This RD; safe synthetic fixture schema/runner/report tooling in
  Context; focused reusable Slackbot instrumentation if required; private Enyu
  scenario manifests/scripts/docs. Private source text, IDs, transcripts, and
  credentials remain outside Git.
- **Agent owns:** Fixture proposal, approved Slack UI operation, operator-side
  read-only evidence collection, scoring, replay, redaction, and defect reports.
- **Requester owns:** Golden-source selection, disputed semantic judgments,
  credentials/session access, live-run approval, provider spend, and deployment.
- **Out of scope:** Direct SQL from agents, tests against `ai_v2`/Console,
  broad product fixes, public ingress, automatic web discovery, or silently
  granting Ed/Rez write access.

The MVP creates neither an `eval_golden` table nor a `golden_suite` Run kind.
Private golden definitions and aggregate results remain in a manifest and
redacted evidence bundle that reference existing operational Run IDs. The local
runner uses the trusted human API/UI; it is never granted to Ed, Rez, their
sandboxes, or the standard read-only Context client.

## Golden Scenario Matrix

| ID | Slack script and fixture shape | Hard oracle |
| --- | --- | --- |
| R1 article | Ask Rez to ingest an article containing one exact existing Entity, one paraphrased related Entity, and one same-name decoy. | One canonical Source with one current immutable Artifact; exact Artifact SHA-256/kind; expected IDs connected with explained provenance; decoy absent; no duplicate Entity, Source, or Artifact. |
| R2 video | Ask Rez to ingest a YouTube URL whose transcript overlaps R1 and one pre-existing Theme/Entity. | Canonical watch URL; real transcript Artifact with exact SHA-256/kind/bytes and Source ownership; shared IDs reused; terminal intake readiness; replay returns the same Source, Artifact, workflow receipt, and Context intake identity. |
| R3 discuss/close | In the R1 thread ask one source-grounded fact, one connection question, and one unsupported question; then send `done`. | Cited answers and explicit uncertainty; one Chat/window; one `slack_interaction` parent and one completed child `curator` Run; one primary Memory; exact `derived_from` Chat and expected Source/Entity links represented by authoritative Object Events; complete, non-duplicated trace/usage. |
| E1 retrieve | In a fresh Ed thread ask for a fact present only in an existing Source Artifact or Note body; include a lexical decoy and paraphrase the query. | Expected evidence is in the Context packet and answer; retrieval mode/ranks and `consulted_object_ids` are recorded in Run evidence; decoy does not support the answer. |
| E2 record/deny | Ask Ed to `create a note`, then close. | Ed must not claim or create a durable Note; closure still produces the expected `slack_interaction`/child `curator` Runs and Memory. If Note creation becomes a product contract, replace this negative oracle with exact Note/Artifact/Source/Entity/Chat linkage. |
| X1 isolation/replay | Ask Ed to ingest and replay R1/R2; resend one event. | Ed is denied ingestion and Rez remains allowed; exact replay preserves `(kind,idempotency_key)` and creates no new Run, trace/usage entry, Object Event, Artifact, Chat Message, record, or change. |

Each fixture includes expected IDs, forbidden IDs, required evidence spans,
allowed connection kinds/directions, expected retrieval slice, and a corpus hash.
Score every criterion as a binary gate: identity, Artifact fidelity, linkage,
idempotency, Run completeness, answer support, requested information present,
decoy exclusion, absence of unsupported material claims, and calibrated
uncertainty. A scenario passes only when every required gate passes; fluency
never compensates for failure.

## Checks

- [ ] Runner correlates evidence by run marker plus Slack workspace/channel/thread
  identity and exact Run/idempotency relationships, not timing alone, and emits
  machine-readable JSON plus a short report.
- [ ] Reports separate retrieval candidate/rank, Context packet inclusion, bot
  answer, `runs.input`/`trace`/`result`, `consulted_object_ids`, parent/child Runs,
  authoritative Object Events, Artifacts, Curator result, and usage.
- [ ] Tests cover missing/stale embeddings and lexical fallback, timeouts,
  retries, wrong persona, decoys, unsupported answers, partial failure, and
  cleanup without deleting canonical evidence.
- [ ] Preflight measures actual `embeddings` coverage/status/model/dimensions and
  current source hashes; it never treats Source-intake `semantic_ready=true` as
  proof when the embedding provider is disabled.
- [ ] Closing one interaction queues exactly one child `curator` Run and records
  exactly one queue transition. The currently known duplicate `curator_queued`
  trace append is an implementation defect to fix, not an oracle to weaken.
- [ ] Two clean runs and exact replays pass; repository-root checks and
  `git diff --check` pass for changed repositories.

## Approval Boundary

Planning and local synthetic fixtures are authorized. Operating live Slack,
calling models, ingesting the selected sources, hosted writes, deployment,
credential/session use, or deletion requires Brad's explicit approval. Evidence
collection is operator-side trusted HTTP/UI only; agents never receive a database
DSN or the trusted human credential, and Ed/Rez receive no Run-review access.

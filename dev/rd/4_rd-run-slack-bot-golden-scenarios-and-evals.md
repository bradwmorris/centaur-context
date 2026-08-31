# 4 — RD: Run Slack Bot Golden Scenarios and Evals

**Status:** `in_progress`
**Created:** 2026-08-30
**GitHub Issue:** [#78](https://github.com/bradwmorris/centaur-context/issues/78)
**Dependencies:** Active priority 3 and the completed canonical-data,
Editor-publishing, and Paradigm-corpus RDs; user-facing bots are `Rez` (the
stable Enyu researcher identity) and `Ed` (the stable editor identity).

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Active priority 3 and the completed canonical-data,
Editor-publishing, and Paradigm-corpus RDs; Slack interaction snapshots and the
literal `done`/`finished` close signal; Chat-aware hybrid Context Builder;
Source-intake workflow; Curator reconciliation; Run traces and dashboard; Enyu
roles and live acceptance record. Today closure curates a primary Memory, not a
Note, and interactive bots cannot write Context.

**Missing:** At execution, Brad selects the private article and YouTube URL and
approves live Slack/model/hosted-write use. Their fixture manifest must identify
expected and decoy Object IDs before any run.

1. Freeze a private golden manifest and preflight the deployed revisions,
   corpus hashes, identities, permissions, search/embedding health, and clean
   Slack test channel.
2. Drive versioned, copy-paste-exact Slack scripts through the real UI, using a
   unique run marker in every root message; finish each thread with the sole
   message `done` and wait for terminal workflow, Curator, and eval states.
3. Collect Slack transcript plus authenticated read-only Context evidence into
   one redacted run bundle; score deterministic facts automatically and review
   answer quality separately.
4. Replay idempotency cases, classify failures by stage, rerun only after the
   fixture/version changes, and publish a concise pass/fail matrix. File
   implementation defects separately rather than weakening expected results.

## What We Are Doing

- [ ] Prove Rez can ingest one article and one YouTube transcript, reuse the
  correct existing identities, and avoid duplicates or attractive decoys.
- [ ] Prove a Rez conversation closes into correctly linked canonical records
  and one complete Run.
- [ ] Prove Ed retrieves the required existing evidence through the intended
  search/context path, answers faithfully, and produces complete Run evidence.
- [ ] Make the suite repeatable by an agent operating Slack without database
  credentials, timestamps copied by hand, or subjective inspection as the only
  oracle.

## Contract

- **Goal:** Validate the complete human→Slack→bot/workflow→Context→curator→Run
  loop against known answers after priority 3.
- **Done:** Every required scenario passes its hard invariants and reviewed
  rubric twice, exact replay creates no duplicates, every failure is attributable
  to a stage, and the run bundle identifies deployed and fixture versions.
- **Files:** This RD; safe synthetic fixture schema/runner/report tooling in
  Context; focused reusable Slackbot instrumentation if required; private Enyu
  scenario manifests/scripts/docs. Private source text, IDs, transcripts, and
  credentials remain outside Git.
- **Agent owns:** Fixture proposal, Slack UI operation after approval, read-only
  evidence collection, scoring, replay, redaction, and defect reports.
- **Requester owns:** Golden-source selection, disputed semantic judgments,
  credentials/session access, live-run approval, provider spend, and deployment.
- **Out of scope:** Direct SQL from agents, tests against `ai_v2`/Console,
  broad product fixes, public ingress, automatic web discovery, or silently
  granting Ed/Rez write access.

## Golden Scenario Matrix

| ID | Slack script and fixture shape | Hard oracle |
| --- | --- | --- |
| R1 article | Ask Rez to ingest an article containing one exact existing Entity, one paraphrased related Entity, and one same-name decoy. | One canonical Source/content; expected existing IDs connected with explained provenance; decoy absent; no duplicate Entity/Source. |
| R2 video | Ask Rez to ingest a YouTube URL whose transcript overlaps R1 and one pre-existing Theme/Entity. | Canonical watch URL, real transcript, correct content hash/kind, shared IDs reused, readiness terminal, replay returns same Source/run. |
| R3 discuss/close | In the R1 thread ask one source-grounded fact, one connection question, and one unsupported question; then send `done`. | Cited/grounded answers, explicit uncertainty for unsupported claim; one Chat/window, one Curator run, one primary Memory, exact `derived_from` Chat link, expected Source/Entity links, one completed eval. |
| E1 retrieve | In a fresh Ed thread ask for a fact present only in existing Source/Note body; include a lexical decoy and paraphrase the query. | Expected evidence is in the Context packet and answer; retrieval mode/ranks and consulted Object IDs are recorded; decoy does not support the answer. |
| E2 record/deny | Ask Ed to `create a note`, then close. | Until a narrow Note workflow is approved, Ed must not claim a durable Note or create one; closure still produces the expected Memory/Run. If Note creation becomes a product contract, replace this negative oracle with exact Note/Source/Entity/Chat linkage. |
| X1 isolation/replay | Ask Ed to ingest and replay R1/R2; resend one event. | Ed is denied ingestion, Rez remains allowed, duplicate Slack delivery and workflow replay create no duplicate messages, records, changes, or Run usage. |

Each fixture includes expected IDs, forbidden IDs, required evidence spans,
allowed connection kinds/directions, expected retrieval slice, and a corpus hash.
Score identity/content/linkage/idempotency/Run completeness as binary gates;
score answer support, completeness, concision, and calibrated uncertainty on a
reviewed 0–2 rubric. Never let answer fluency compensate for a failed hard gate.

## Execution Evidence

- **R2 video — hard-gate fail, awaiting fixture review (2026-09-01):** the
  selected YouTube URL resolved to its canonical watch URL and reused the
  existing Source, Ryan Greenblatt, Hugging Face, and Agents Objects. The
  same-subject video decoy was absent. The Source reached lexical and semantic
  readiness, and exact webhook replay reused the completed Run without changing
  the Source revision, artifact count, or connection count. The redacted private
  bundle records the Slack thread, deployed revisions, object IDs, hashes, and
  verification results. The content hash gate did not pass: the frozen fixture
  describes 51,501 bytes, 1,141 lines, and 8,959 words, while the current stored
  caption artifact has 42,664 bytes, 1,137 lines, and 7,813 words. This must be
  reconciled as corpus drift or a representation mismatch rather than accepted
  post hoc. The idempotent replay response also labels the already completed
  reused Run as `queued`; terminal Run readback remains `completed`.

## Checks

- [ ] Runner correlates evidence by run marker plus Slack workspace/channel/thread
  identity, not timing alone, and emits machine-readable JSON plus a short report.
- [ ] Reports separate retrieval candidate/rank, Context packet inclusion, bot
  answer, durable changes, curator result, and Run trace/usage.
- [ ] Tests cover missing/stale embeddings and lexical fallback, timeouts,
  retries, wrong persona, decoys, unsupported answers, partial failure, and
  cleanup without deleting canonical evidence.
- [ ] Two clean runs and exact replays pass; repository-root checks and
  `git diff --check` pass for changed repositories.

## Approval Boundary

Planning and local synthetic fixtures are authorized. Operating live Slack,
calling models, ingesting the selected sources, hosted writes, deployment,
credential/session use, or deletion requires Brad's explicit approval. Evidence
collection is authenticated HTTP/UI only; agents never receive a database DSN.

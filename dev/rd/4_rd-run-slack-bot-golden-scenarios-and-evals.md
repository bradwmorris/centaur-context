# 4 — RD: Run Slack Bot Golden Scenarios and Evals

**Status:** `scoped`
**Created:** 2026-08-30
**Dependencies:** Active priorities 1 and 3; completed consolidated-schema,
canonical-data, Editor-publishing, and Paradigm-corpus RDs; successful schema-17
and `/api/v2` cutover; completed embedding/complete-capture rollout from RD1;
user-facing bots are `Rez` (the stable Enyu researcher identity) and `Ed` (the
stable editor identity).

## Execution Plan

**Status:** `still needs work`

**Basis checked:** Active priorities 1 and 3; completed consolidated-schema,
canonical-data, Editor-publishing, and Paradigm-corpus RDs; schema-17 `runs`,
`artifacts`, `embeddings`, and authoritative `object_events`; `/api/v2`; Slack
interaction snapshots and literal `done`/`finished`; Chat-aware hybrid Context
Builder; Source intake; Curator reconciliation; Runs UI; Enyu roles and live
acceptance record; and RD1's complete Artifact capture, Object/Artifact-chunk
embedding, attributed-span hybrid retrieval, fallback, and rollout contract.
Closure curates a primary Memory, not a Note, and interactive bots cannot write
Context.

**Missing:** Priorities 1 and 3 completion; successful schema-17 and embedding
live cutovers/backfill; Brad's private article and YouTube selection; a frozen
manifest containing expected and decoy Object IDs, Artifact hashes, evidence
spans, relevance judgments, and latency/cost budgets; and Brad's approval for
live Slack/model/provider/hosted-write use.

1. Freeze a private golden manifest and preflight the deployed revisions,
   corpus hashes, identities, permissions, embedding configuration/coverage,
   retrieval budgets, and clean Slack test channel.
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
  lexical, Object-semantic, and Artifact-chunk-semantic paths; attributes the
  exact current Artifact span; answers faithfully; and produces complete Run
  evidence.
- [ ] Make the suite repeatable by an agent operating Slack without database
  credentials, timestamps copied by hand, or subjective inspection as the only
  oracle.

## Contract

- **Goal:** Validate the complete human→Slack→bot/workflow→Context→Curator→Run
  loop, including attributed hybrid retrieval, after priorities 1 and 3.
- **Done:** Every numbered test passes its hard gates twice; the semantic slice
  meets its retrieval thresholds; exact replay creates no duplicates; every
  failure is attributable to a stage; and the evidence bundle identifies the
  deployed, fixture, provider/model, formatter, and chunker versions.
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

## Numbered Golden Tests

| Test | Exact interaction | Measure and pass condition |
| --- | --- | --- |
| 01 — article capture | Ask Rez to ingest the frozen article containing one exact existing Entity, one paraphrased related Entity, and one same-name decoy. | Exactly one Source/current complete Artifact; exact SHA-256/kind/bytes; expected links; decoy absent; no duplicate Entity/Source/Artifact; all expected Object and chunk embedding jobs terminal/current. |
| 02 — video capture | Ask Rez to ingest the frozen YouTube URL whose transcript overlaps Test 01 and one existing Theme/Entity. | Canonical URL and verbatim transcript Artifact; exact SHA-256/kind/bytes; shared IDs reused; expected chunk count/offsets/hash coverage; replay returns the same Source, Artifact, receipt, and intake identity. |
| 03 — semantic paraphrase | In a fresh Ed thread ask the frozen paraphrase whose answer occurs only in the middle of the long Artifact and shares no required content words with it. | Expected current Artifact and overlapping judged span appear in semantic candidates at rank ≤5 and in the Context packet; Ed cites/uses that span; lexical-only decoy cannot support the answer. Record lexical, Object-semantic, chunk-semantic, fused ranks, score, and latency separately. |
| 04 — chunk boundary | Ask the frozen question whose complete answer crosses one deterministic chunk boundary. | At least one returned current-Artifact window covers every judged character; overlapping chunks collapse to one attributed evidence window; no missing clause or duplicated context; record offsets and Context bytes. |
| 05 — current version only | Re-capture the synthetic Source with changed bytes, wait for indexing, then ask a question for which the old and current Artifacts give different answers. | Only current-hash chunks rank or enter Context; old/superseded, stale, incomplete, failed, and dimension-mismatched rows never rank; answer follows the current span. |
| 06 — degraded fallback | Disable semantic retrieval in the disposable/synthetic run, then ask an exact-keyword question. | Lexical evidence still reaches Context and supports the answer; Run/search evidence explicitly reports degraded semantic state; no semantic success is claimed. Re-enable it before live tests. |
| 07 — no-answer/decoy | Ask a plausible question represented only by a semantically similar decoy, not by a judged supporting span. | No forbidden span enters the answer as support; Ed states the evidence is insufficient; false-positive supported-answer count is zero. |
| 08 — discuss and close | In the Test 01 thread ask one grounded fact, one connection question, and one unsupported question; then send sole message `done`. | Grounded answers and explicit uncertainty; one Chat/window; one `slack_interaction` parent and completed child `curator` Run; one primary Memory; exact links via Object Events; complete non-duplicated trace/usage. |
| 09 — Ed write denial | Ask Ed to `create a note`, then close. | Ed neither claims nor creates a durable Note; closure still creates the expected parent/child Runs and Memory. |
| 10 — isolation/replay | Ask Ed to ingest Tests 01/02; resend one event; exactly replay the successful workflows. | Ed is denied ingestion and Rez remains allowed; replay preserves `(kind,idempotency_key)` and creates no new Run, trace/usage entry, Object Event, embedding job/row, Artifact, Chat Message, record, or change. |

Each fixture includes expected/forbidden IDs, required Artifact ID/hash and
character spans, allowed connection kinds/directions, relevance grades, expected
modality/rank, deterministic chunk boundaries, and a corpus/config hash.
Score every criterion as a binary gate: identity, Artifact fidelity, linkage,
embedding currency/coverage, exact-span attribution, idempotency, Run
completeness, answer support, requested information present, decoy exclusion,
absence of unsupported material claims, and calibrated uncertainty. A test passes
only when every required gate passes; fluency never compensates for failure.

Across the positive semantic queries in Tests 03–05, gate candidates at `k=5`:
Recall@5 must be 1.00, nDCG@5 at least 0.90, and judged evidence-span coverage
1.00. Test 07's false-positive supported-answer count must be zero. Queue
reconciliation and current complete textual Artifact chunk coverage must both be
100%, with zero stale or dimension-mismatched ranked hits. Record p50/p95
retrieval latency, Context bytes, embedding/query usage, and estimated cost;
enforce the frozen manifest's approved maximums rather than inventing them after
results are known.

## Checks

- [ ] Runner correlates evidence by run marker plus Slack workspace/channel/thread
  identity and exact Run/idempotency relationships, not timing alone, and emits
  machine-readable JSON plus a short report.
- [ ] Reports separate retrieval candidate/rank, Context packet inclusion, bot
  answer, lexical/Object-vector/chunk-vector/fused contributions, Artifact ID and
  offsets, `runs.input`/`trace`/`result`, `consulted_object_ids`, parent/child Runs,
  authoritative Object Events, Artifacts, Curator result, latency, and usage.
- [ ] Tests cover missing/stale embeddings and lexical fallback, timeouts,
  retries, wrong persona, decoys, unsupported answers, partial failure, and
  cleanup without deleting canonical evidence.
- [ ] Preflight reconciles actual embedding target/hash/model/dimensions,
  formatter/chunker version, offsets, job state, current Artifact chunk count,
  coverage, staleness, and provider configuration; it never treats Source-intake
  `semantic_ready=true` alone as proof.
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

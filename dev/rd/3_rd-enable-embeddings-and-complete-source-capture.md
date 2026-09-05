# 3 — RD: Enable Embeddings and Complete Source Capture

**Status:** `review`
**Created:** 2026-08-31
**GitHub Issue:** [#76](https://github.com/bradwmorris/centaur-context/issues/76)

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Schema 17 and the completed canonical Source, embedding,
schema-minimization, and Enyu ingestion RDs; migrations; Source/intake, Artifact,
search, embedding worker, API, client, web, deployment, tests, and operations
code; Enyu's live ingestion workflow/tests/manifests; official OpenAI embedding
model/API documentation; and the live Enyu pod configuration/logs. The live pod
reports embeddings disabled and its Secret has no embedding keys. Current
semantic indexing covers only Object kind/title/description. Unified search does
not search Artifact bodies, although Source-specific lexical search does. Source
intake requires text but trusts an unvalidated coverage string. Enyu can pass
model-returned, truncated, or partial content without byte/hash reconciliation.
The current `(object_id, model)` embedding key cannot represent long-document
chunks.

**Missing:** Requester approval of the external embedding provider, spend,
purpose-bound credential, hosted writes, and deployment. Recommended
smallest setup: OpenAI `/v1/embeddings`, `text-embedding-3-small`, 1536 dimensions,
`shared` mode. If declined, the requester must select an exact compatible
endpoint/model/dimension/mode combination; lexical retrieval remains available.

1. Make complete, immutable Artifact capture the Source intake contract and
   update every first-party producer, reader, and operator surface.
2. Backfill concise Object kind/title/description vectors, while enabling
   Artifact-chunk semantic indexing only for complete Artifacts created after the
   migration; retain lexical retrieval for every historical transcript.
3. Verify locally and on a disposable database; after separate approvals, deploy,
   canary, drain the Object-summary backfill, measure retrieval, and retain a
   tested rollback.

## What We Are Doing

- [x] Every new Source has one current immutable Artifact containing the complete
  verbatim supporting text, or an explicit non-complete capture outcome that can
  never be reported as ready/complete.
- [x] Exact Artifact text remains authoritative; historical text stays
  lexical-only, while Object summaries are backfilled and newly captured complete
  Artifacts gain semantic evidence retrieval moving forward.
- [ ] Live embeddings reach complete, current coverage; retries, fallback,
  canaries, monitoring, backup, and rollback are proved.

## Contract

- **Goal:** Make trustworthy complete Source capture and useful semantic retrieval
  the default end-to-end contract without adding another base table or datastore.
- **Done:** Intake completeness is enforced and visible; current Artifact hashes,
  sizes, provenance, versions, and outcomes reconcile; Object and forward-only
  chunk embedding jobs are idempotent and current; hybrid retrieval returns
  attributed Artifact spans; agent/client/UI/operations paths expose state; eval
  gates pass; and approved live canaries and the Object-only backfill reconcile
  with lexical fallback intact.
- **Files:** Forward-only migration; Rust domain/database/intake/search/worker/API;
  standard client; web UI; tests/fixtures; deploy/docs/operations; this RD; and the
  coordinated private `centaur-enyu` workflow, skill, tests, and manifests.
- **Agent owns:** Implementation, disposable tests, evals, provider adapter,
  migration/backfill tooling, reports, PRs, and approved deployment/canaries.
- **Requester owns:** Provider/model/credential/spend choice, copyrighted-content
  policy, disputed completeness/relevance judgments, hosted writes, deployment,
  and production cutover approval.
- **Out of scope:** A new vector database, new base table, database access for
  agents, generic web crawling, bypassing paywalls/permissions, storing large
  binaries in PostgreSQL, public ingress, or changing unrelated ontology.

## Source and Artifact Contract

- Preserve exact supplied/extracted UTF-8 text in `artifacts.content`; normalization,
  excerpts, chunks, and vectors are derived. Require source identity/URL, media and
  content kind, capture method/version/time, provenance, SHA-256, byte size, and a
  typed outcome: `complete`, `incomplete`, `unavailable`, `paywalled`, `disallowed`,
  `too_large`, or `unsupported`, with reason and observed/expected extent where
  known. Never infer completeness from non-empty text.
- A successful complete intake writes Source, Artifact, current pointer, Run, and
  events atomically. Re-capture appends an immutable Artifact linked with
  `supersedes_artifact_id`; only a complete Artifact may become current. Exact
  idempotent replay returns the same IDs; changed bytes conflict or create an
  explicit new version. Failed capture may record auditable Run/outcome metadata
  without fabricating an Artifact body or ready Source.
- Remove model round-tripping of verbatim caller-supplied text. Enyu adapters must
  capture bytes first, reconcile returned bytes/hash/size, and use the model only
  for bounded metadata/connection judgment. Provide explicit adapters/tests for
  supplied text/file, YouTube captions, article/web text, and paper/document text;
  unsupported or restricted inputs fail honestly. Raise the present size policy
  only from measured PostgreSQL/API limits; otherwise use an opaque managed URI
  plus exact integrity metadata and mark text retrieval incomplete.

## Embedding and Retrieval Design

- Keep one `embeddings` table. Add a stable row ID plus nullable `artifact_id`,
  chunk ordinal and exact character offsets; partial unique indexes preserve one
  Object vector per model and one vector per Artifact chunk/model. Use SHA-256
  source hashes. No chunk-text table: reconstruct input deterministically from the
  immutable Artifact and versioned formatter.
- Retain concise Object embeddings for identity lookup and backfill them across
  existing Objects. Mark every pre-migration Artifact lexical-only. Embed only
  current, complete textual Source Artifacts created after the migration as
  paragraph-aligned chunks, initially about 6,000 Unicode characters with
  600-character overlap and a provider-token-limit guard.
- Claim rows with `SKIP LOCKED`, recover expired leases, use bounded exponential
  retry with terminal visibility, and condition completion on unchanged target
  hash/config. New eligible current Artifacts queue chunks through the resumable
  reconciler; superseded rows remain derived history but never rank. The Object
  summary backfill is count/hash reconciled and safe to repeat; historical
  Artifact chunks are explicitly excluded.
- Fuse Object lexical, current-Artifact lexical, Object semantic, and chunk semantic
  ranks; collapse chunk hits per Object, cap duplicate spans, and return Artifact
  ID, offsets, bounded exact excerpt, capture outcome, and rationale. Archived,
  superseded, incomplete, failed, stale, or dimension-mismatched rows cannot rank.
  Provider failure preserves lexical search and reports degraded semantic state.

## Operations, Evaluation, and Rollout

- Add readiness/metrics and UI/API visibility for configured provider/model,
  queue/running/failed/terminal counts, oldest age, coverage/staleness, Artifact
  outcome, current version, and chunk coverage without exposing keys, vectors, or
  private full text. Add agent operations for evidence-bearing search and bounded
  Artifact windows.
- Build a safe judged fixture set spanning exact lookup, paraphrase, long-body
  facts, boundary/overlap cases, incomplete capture, stale versions, and no-answer.
  Gate on per-slice Recall@k/nDCG, evidence-span coverage, false positives, p95
  latency, context bytes, queue reconciliation, and estimated cost.
- Rehearse migration/Object backfill on a disposable schema-17 snapshot. For live
  rollout: approve provider and credential; back up/checksum; deploy Enyu/Context
  together; canary one query and one complete Source without bulk backfill; inspect
  usage, retrieval, and failures; then drain the resumable Object-summary backfill.
  Roll back by disabling embedding config and reverting callers/service; lexical
  retrieval remains live.
  Restore the backup only if canonical Artifact migration integrity fails.

## Checks

- [x] Local tests cover exact hash/size enforcement, incomplete-current rejection,
  idempotency, model rewrite rejection, bounded Unicode chunks, worker jobs,
  evidence offsets, authorization, and lexical fallback.
- [x] A schema-17 migration rehearsal, full repository verification, targeted
  Enyu tests, query-plan/index-size audit, and `git diff --check` pass.
- [ ] Provider-backed evals, live backup/canaries, Object-backfill reconciliation, and
  rollback rehearsal require the requester approvals listed below.

## Verification Results

- All 72 Rust/API/database tests pass against a disposable pgvector database;
  formatting and Clippy with warnings denied pass.
- All 41 web tests, TypeScript type-checking, and the production build pass.
- All 13 Python client tests, Python compilation, and the 11 directly affected
  Enyu workflow/overlay tests pass.
- A schema-17 fixture upgraded with the complete current Artifact preserved but
  semantically disabled, and an unproven legacy Artifact classified `incomplete`.
  GIN Artifact lexical and configured-dimension HNSW query plans use their intended
  indexes.
- The broader Enyu overlay suite retains two unrelated pre-existing failures for
  stale deployment image/repository pin expectations.

## Approval Boundary

Execution is authorized under GitHub Issue #76 on its feature branch. Local code,
synthetic fixtures, and disposable-database work are authorized. Calling or paying an embedding/provider
API, creating or changing credentials, hosted writes, deployment, live
canaries, production cutover, publishing, sending, deletion, public ingress, or
new external integrations each require explicit requester approval.

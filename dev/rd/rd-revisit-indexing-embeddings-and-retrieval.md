# RD: Revisit Indexing, Embeddings, and Retrieval

**Status:** `backlog`
**Created:** 2026-08-30

## Execution Plan

**Status:** `still needs work`

**Basis checked:** Migrations `0005`, `0008`, `0009`, and `0011`; current search,
embedding, Context Builder, eval, configuration, database, and API code; completed
Context Builder/embedding and eval-dashboard RDs; and the recent POC-import RD.
Today, Object title/description use configurable PostgreSQL full-text search;
optional versioned pgvector embeddings cover Object kind/title/description;
reciprocal-rank fusion combines lexical and semantic candidates; Context Builder
adds canonical Chat, participant, direct-Connection, recency, and bounded subtype
signals. Note bodies and Source content have separate `simple` full-text indexes
but are not part of Object embeddings or unified Context retrieval. Existing
eval traces provide operational observability, not a judged retrieval benchmark.

**Missing:** Completion and frozen reconciliation output from
`rd-audit-clean-and-refine-canonical-data.md`; live embedding-provider status and
index/job health; representative Brad-authored questions and relevance judgments;
and approval before enabling a paid provider or writing hosted derived indexes.

1. After the data-cleanup RD, capture a reproducible baseline of schema/index
   usage, query plans, embedding coverage/staleness, failure fallback, latency,
   context packets, and real retrieval misses.
2. Build a versioned, private-but-reproducible retrieval eval set spanning exact
   lookup, paraphrase, Note-body facts, Source evidence, graph/context questions,
   ambiguity, freshness, and negative/no-answer cases, with graded judgments.
3. Compare the smallest credible variants offline and on a disposable database;
   change one component at a time and retain only improvements that clear quality,
   latency, complexity, privacy, and cost gates.
4. Implement the winning minimal contract, rebuild derived indexes/embeddings,
   run regression and live canaries, and document measured results and rollback.

## What We Are Doing

- [ ] Prove which indexes and retrieval stages are present, healthy, used, and
  useful after the canonical data is clean.
- [ ] Determine whether Object-only embeddings and separate body search are
  sufficient or whether bounded Note/Source representations or chunk retrieval
  materially improve context.
- [ ] Make search and Context Builder quality measurable with judged evals,
  deterministic reports, and regression thresholds.
- [ ] Ship the simplest efficient design that improves relevant evidence found
  and context usefulness without hiding failures behind aggregate scores.

## Contract

- **Goal:** Establish and implement an evidence-backed indexing, embedding, and
  retrieval design that returns the right clean canonical context efficiently.
- **Done:** A versioned benchmark and baseline exist; candidate variants are
  compared on quality, latency, cost, and failure behavior; the chosen minimal
  design beats or matches baseline gates across every query slice; indexes and
  embeddings reconcile fully; fallback works; and end-to-end Context canaries pass.
- **Files:** Search/embedding/context/eval Rust code; forward-only migrations only
  if measurements justify them; configuration; API and standard agent client;
  eval fixtures/report tooling; database/API tests; operations and architecture
  docs; this RD. Private corpus text and judgments remain outside Git where needed.
- **Agent owns:** Audit, benchmark design proposal, instrumentation, experiments,
  implementation, derived-data rebuild, reports, and local verification.
- **Requester owns:** Representative intent confirmation and disputed relevance
  judgments, embedding provider/model/credential/spend choices, hosted writes,
  deployment, and production cutover approval.
- **Out of scope:** Further canonical-data cleanup, replacing PostgreSQL as source
  of truth, generic web search/RAG, multi-hop agent planning, a vector database
  added by default, public ingress, or unrelated ontology changes.

## Evaluation and Design Rules

- Freeze canonical row/content hashes and retrieval configuration for each run.
  Store query, expected relevant Object/evidence IDs, graded relevance, required
  source span, exclusions, actual ranks, packet omissions, latency, token/byte
  size, and provider/model/version. Never score against titles alone.
- Report Recall@k, nDCG@k, MRR for single-target lookup, evidence/answer coverage,
  no-answer false positives, p50/p95 latency, context bytes/tokens, embedding
  coverage/staleness, and estimated cost. Break out exact, semantic, body,
  graph/Chat, and ambiguous query slices; no aggregate may mask a regressed slice.
- Establish lexical-only and current-production baselines first. Then test only
  justified candidates: weights/candidate depth, text-search configuration,
  embedding input/model, inclusion of concise subtype text, body search fusion,
  and bounded Source/Note chunks. Do not add chunking or another datastore unless
  simpler variants fail and measured gains justify lifecycle complexity.
- Separate retrieval from context assembly in the report: candidate recall,
  ranking, graph expansion, subtype projection, and budget truncation each need
  attributable outcomes. Log enough rationale to reproduce a rank without
  exposing private body text or vectors.
- Derived records must remain rebuildable and versioned by source hash, input
  format, model, dimensions, and query/document mode. Partial, stale, unavailable,
  or dimension-mismatched embeddings fail visibly and preserve lexical fallback.
- Inspect real query plans and index size/write overhead. Remove redundant or
  unused indexes only with workload evidence and a disposable migration rehearsal.

## Checks

- [ ] Benchmark has reviewed judgments, hard negative/no-answer cases, slice
  coverage, deterministic runner output, and a checked-in safe fixture subset.
- [ ] Baseline and every candidate report identical corpus/config hashes and
  quality, latency, context-size, coverage, failure, and cost metrics.
- [ ] Chosen variant meets recorded per-slice gates and has no unexplained
  regression; deliberate misses are manually inspected.
- [ ] Tests cover stale/missing/failed embeddings, lexical fallback, rank ties,
  body evidence, Chat authorization, graph pollution, budget truncation, rebuild
  idempotency, and provider/model changes.
- [ ] Query-plan/index audit and all repository-root verification commands pass.
- [ ] `git diff --check` passes.

## Approval Boundary

Read-only measurement and disposable experiments are authorized during execution.
Provider calls, credentials, spend, hosted embedding/index writes, destructive
index removal, deployment, or production cutover require explicit approval. No
external integration, public ingress, publishing, or sending is authorized.

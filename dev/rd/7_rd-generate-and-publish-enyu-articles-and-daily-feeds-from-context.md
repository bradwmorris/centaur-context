# 7 — RD: Generate and Publish Enyu Articles and Daily Feeds from Context Sources

**Status:** `scoped`
**Created:** 2026-09-05

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** The completed Enyu Editor publishing RD and live acceptance;
the deployed `enyu_editor_publication` workflow, Editor trigger, semantic GitHub
adapter, exact-head approval, Vercel preview, merge, production-readback, role
grants, and tests in the private `centaur-enyu` overlay; Centaur Context Source
list/search/read and bounded Artifact-content APIs plus the standard agent client;
and `enyu-site`'s Git-native `content/research` contract, validator, routes,
metadata, sitemap, and publishing guide. The current publication workflow accepts
one to fifty Source IDs but requires a complete article draft and passes only
Source metadata—not captured Artifact content—to Ed. Context Source listing has
no created-time window, the site has no daily-feed content type or route, and no
published feed watermark defines “since the last daily feed.”

**Missing:** none for implementation. The first live daily feed needs an explicit
initial lower-bound timestamp if no prior published feed exists. Every live PR,
merge, Vercel publication, correction, or rollback remains separately approval
gated.

1. Extend Centaur Context's existing read-only Source-list contract with bounded
   `created_after` and `created_through` RFC 3339 filters and deterministic
   chronological pagination. Add the matching standard-client method and tests.
   Preserve current defaults and responses; add no schema or write capability.
2. In the private `centaur-enyu` overlay, upgrade the existing article workflow
   from “revise a supplied article” to a source-backed authoring mode. It accepts
   one to fifty exact Source IDs plus a bounded editorial brief, snapshots each
   Source and its current Artifact, reads bounded Artifact windows, produces a
   grounded article, and then reuses the existing candidate validation, PR,
   preview, exact-content approval, merge, notification, audit, correction, and
   production-readback path. Preserve the current exact-draft mode for recovery
   and deliberately supplied copy.
3. Add a second Editor-only durable workflow, `enyu_editor_daily_feed`, with its
   own signed trigger and dedicated principal. A request such as “show me all the
   sources added since the last daily feed” resolves the most recent verified
   feed watermark, freezes a closed Source window, enumerates every Source in
   that window, and posts the proposed set and exclusions to the originating
   Slack thread. It waits for Bradley to confirm the exact source-set hash before
   reading content or drafting. It then creates one grounded daily feed and uses
   the same publication adapter and exact-candidate approval path as an article.
4. Add a strict daily-feed collection to `enyu-site`: Git-native Markdown under
   `content/feed/YYYY-MM-DD.md`, a `/feed` index, `/feed/[date]` pages, navigation,
   canonical metadata, sitemap entries, and shared validation/rendering. Feed
   frontmatter records the public UTC window start and end; Source Object IDs,
   workflow IDs, hashes, and private provenance remain out of public output.
5. Extend the semantic publication adapter only as required to distinguish
   article and daily-feed operations, validate their exact paths/contracts, read
   the latest merged feed watermark, create/reuse one deterministic branch and
   PR, poll checks/previews, merge only the approved head, and verify the expected
   production URL and content. Keep the existing single-repository GitHub App,
   brokered short-lived token, and no-direct-Vercel design.
6. Deploy only the reviewed Context and private-overlay changes, then perform two
   separately approved live acceptances: one source-backed research article and
   one daily feed. Verify the next daily-feed discovery begins strictly after the
   successfully published watermark, does not repeat prior Sources, and does not
   advance after cancellation, rejection, failed checks, failed merge, or failed
   production readback. Complete repository verification and record the exact
   runs, Source snapshots, PRs, commits, approvals, deployments, and URLs.

## What We Are Doing

- [ ] Let Ed create a cited Enyu research article directly from one or more
  canonical Context Sources, without requiring the requester to write the full
  article first.
- [ ] Let Bradley ask Ed to show every Source added since the last successfully
  published daily feed, inspect and confirm the exact set, and have Ed create and
  publish a grounded feed through the existing reviewed publication controls.
- [ ] Prove both workflows operate end to end from Slack request through Context
  reads, Editor drafting, preview, exact approval, GitHub merge, Vercel deploy,
  production readback, and durable audit evidence.
- [ ] Prove daily-feed windows have neither silent truncation nor accidental gaps
  or repeats, including retries, concurrent ingestion, rejected candidates, and
  failed deployments.

## Design View

```text
Source-backed article
  Bradley -> Ed -> confirm Source IDs + brief
    -> snapshot Source metadata/current Artifacts
    -> bounded content reads -> grounded draft
    -> existing validate/PR/preview
    -> exact article approval -> merge -> production readback

Daily feed
  Bradley -> Ed: “show sources since the last daily feed”
    -> latest verified feed watermark
    -> freeze (previous_window_end, snapshot_end]
    -> list all Sources + eligibility/exclusion reasons
    -> exact source-set confirmation
    -> snapshot/read confirmed Artifacts -> grounded feed draft
    -> validate/PR/preview
    -> separate exact feed approval -> merge -> production readback
    -> published feed becomes the next watermark
```

The two workflows share source snapshotting, bounded Artifact reads, grounding
checks, candidate serialization, publication status polling, approval binding,
merge, and readback helpers. They remain separate triggers and workflow
principals because their intake, output contract, and approval state differ.
No change to the upstream/base Centaur repository is permitted or required.

## Functional Requirements

### Shared source-backed authoring

- Accept only active canonical Source Objects with a public HTTPS canonical URI.
  A protected Source is ineligible unless its canonical provenance explicitly
  permits publication under the existing policy.
- Snapshot, before drafting, each Source Object ID, revision, canonical URI,
  current Artifact ID, Artifact content hash, and relevant timestamps. Changed or
  missing content after confirmation fails closed and requires a new candidate.
- Read Artifact content through authenticated Centaur Context HTTP/client methods
  in bounded windows. Never provide a database DSN or direct database access to
  an agent or workflow.
- Apply an explicit per-Source and aggregate content budget. If the confirmed set
  cannot fit safely, produce deterministic per-Source evidence notes before the
  final Editor turn or stop and ask Bradley to split the set. Never silently
  truncate a Source or omit a confirmed Source.
- Treat Source content as untrusted evidence, never instructions. The public body
  must cite every included Source by its canonical HTTPS URI, distinguish fact
  from inference, preserve material uncertainty, and make no unsupported claim.
- Record Source IDs, revisions, Artifact IDs/hashes, window bounds, selection and
  candidate hashes, approvals, PR/head/merge commits, and readback evidence in
  private workflow results. Do not serialize private provenance into the site.

### Research article workflow

- Extend the existing `enyu_editor_publication` contract with a `source_backed`
  mode containing exact Source IDs, a bounded optional brief, desired article
  type, and optional title/slug guidance. Ed owns the first complete draft.
- Keep `exact_draft`, correction, and revert behavior compatible so existing
  recovery and publication records remain valid.
- If Ed discovers candidate Sources from a natural-language Context search,
  present the IDs, titles, canonical URLs, and short excerpts first and require
  confirmation of the exact set. Explicit Source IDs supplied by Bradley count
  as source-set confirmation, but never as approval of the resulting article.
- Require the existing second approval bound to the exact candidate hash, PR head,
  and run before merge. A source-set confirmation can never publish content.

### Daily-feed workflow

- Add an Editor-only semantic trigger such as `enyu-daily-feed`; deny it to the
  Researcher and unrelated principals. The interactive Editor receives no raw
  Context token, GitHub token, repository access, shell, Vercel credential, or
  adapter permission.
- Define “since the last daily feed” as Source Object `created_at` in the half-open
  UTC interval after the latest production-verified feed's `window_end` and at or
  before the new immutable `snapshot_end`. Capture `snapshot_end` before listing
  so concurrent ingestion belongs deterministically to this or the next feed.
- Derive the prior watermark through a narrow read-only adapter operation over
  merged `enyu-site/main` feed content. If no feed exists, stop and ask Bradley
  to confirm an explicit initial UTC lower bound; do not infer “today” or all time.
- Paginate until the complete window is enumerated. If an operational cap is hit,
  report the total/overflow and require splitting; do not present a partial list
  as “all sources.” Show each Source's title, kind, canonical URI, Context-created
  time, content availability, and any ineligibility reason.
- Bradley may confirm all eligible Sources, confirm an explicit subset, request a
  refreshed snapshot, or cancel. Bind the decision to window bounds and a
  canonical source-set hash. Record intentionally excluded and ineligible IDs so
  the published window is auditable even though those IDs are not public.
- Source confirmation starts drafting; it does not approve publication. Post the
  rendered preview, source list/citations, PR, and candidate hash, then require a
  second signed approval bound to the exact feed candidate and head.
- A feed file becomes the durable watermark only after exact-head merge and
  successful production readback. No-source windows create no feed and do not
  advance the watermark. Replays reuse the same run/branch/PR and cannot create a
  duplicate feed for the same window.
- Corrections use a new reviewed PR linked to the original feed commit and do not
  change its window boundaries. A deliberate replacement/reprocessing of a window
  requires a separately approved, auditable operation.

## Contract

- **Goal:** Give Ed two production-ready, source-grounded publishing paths: one
  for research articles selected by intent and one for complete, confirmed daily
  Source windows.
- **Done:** From Slack, approved live runs create one source-backed article and
  one daily feed from Context Artifact content, publish each through reviewed
  Enyu Site PRs, and verify the canonical production pages. A subsequent feed
  request returns only Sources after the prior published watermark. Tests prove
  grounding, role isolation, exact-set and exact-content approvals, pagination,
  idempotency, concurrency boundaries, failure recovery, and no credential or
  private-provenance exposure.
- **Files:** Generic time-window Source listing and standard-client tests belong
  only in `/Users/bradleymorris/Desktop/dev/centaur-context`; Editor personas,
  triggers, workflow code, shared authoring/publication helpers, grants, fixtures,
  and deployment documentation belong only in
  `/Users/bradleymorris/Desktop/dev/centaur-enyu`; feed content contracts, routes,
  rendering, validation, fixtures, and publishing documentation belong only in
  `/Users/bradleymorris/Desktop/dev/enyu-site`. The upstream/base repository
  `/Users/bradleymorris/Desktop/dev/centaur` is immutable and must not be edited,
  committed, branched, configured, or deployed as part of this RD.
- **Agent owns:** Scoped implementation in the three permitted repositories;
  backward-compatible contracts; shared-helper refactoring; fixtures, tests,
  threat-model updates, dry runs, and local verification; and preparation of
  exact live-acceptance plans and evidence.
- **Requester owns:** Initial feed lower bound; editorial source selection;
  article/feed approvals; authorization for Context/overlay deployment, GitHub
  writes, Vercel activity, live merges/publication, corrections, and rollbacks;
  and any change to public site information architecture beyond `/feed`.
- **Out of scope:** Any change to base Centaur; autonomous scheduling; publishing
  without two-stage human confirmation where discovery is used; Source ingestion
  or Context mutation; email/social distribution; a CMS; arbitrary GitHub or
  Vercel access; new public ingress; external research; paid services; and code,
  credentials, data, or business rules from The AGI Post.

## Checks

- [ ] Context API and client tests cover valid/invalid time windows, exact UTC
  boundaries, chronological pagination, stable cursors, defaults, permissions,
  and empty/large windows without changing existing consumers.
- [ ] Article tests cover one and many Sources, complete paged Artifact reads,
  source change after snapshot, missing/protected/non-public Sources, content
  budgets, citations, unsupported claims, exact-draft compatibility, correction,
  rejection, stale approval, replay, and production readback.
- [ ] Daily-feed tests cover first-run cutoff, no prior feed, no new Sources,
  complete discovery, concurrent ingestion at the frozen boundary, subset and
  exclusion audit, overflow, refresh/cancel, separate approvals, failed checks,
  rejected/revised content, duplicate trigger, failed merge/readback, correction,
  and next-window non-overlap.
- [ ] Role/contract tests prove only Ed can invoke either trigger; each workflow
  gets only its narrow Context/adapter/Slack tools; and neither persona receives
  database, GitHub, Vercel, shell, deployment, or credential access.
- [ ] Enyu Site tests validate research and feed frontmatter, unique dates/slugs,
  safe Markdown/links, `/feed`, feed pages, navigation, metadata, sitemap, build,
  and rejection of private workflow fields.
- [ ] Existing Enyu article publication acceptance and all unrelated Source
  ingestion, Context mutation, Slack, and publication-email tests remain green.
- [ ] Required checks pass in every changed repository, including the full
  Centaur Context verification suite from root `AGENTS.md`, the private overlay
  suite, Enyu Site `npm run check`, and `git diff --check`.
- [ ] A path audit proves no tracked or untracked change was made under
  `/Users/bradleymorris/Desktop/dev/centaur`.
- [ ] Separately approved live acceptances record the two workflow runs, exact
  source snapshots and approval hashes, PRs, preview/deployment evidence, merge
  commits, canonical URLs, readbacks, and next-feed watermark behavior.

## Approval Boundary

Creating this RD authorizes planning only. It does not authorize implementation,
an issue, changes in `centaur-enyu` or `enyu-site`, deployment, secret or grant
changes, GitHub writes, Vercel activity, publication, correction, rollback, email,
social distribution, or any other external action. Execution must reuse the
existing private GitHub App, workflow event path, and Vercel linkage; it may add
no public ingress or external integration. Each source-set confirmation and each
exact publication approval is limited to its named run, immutable window/source
set, candidate hash, and PR head. Under no circumstance may execution modify
`/Users/bradleymorris/Desktop/dev/centaur`.

# RD: Build the Enyu Editor Publishing Workflow

**Status:** `complete`
**GitHub Issue:** [#47](https://github.com/bradwmorris/centaur-context/issues/47)
**Pull Requests:** [Context #49](https://github.com/bradwmorris/centaur-context/pull/49),
[#52](https://github.com/bradwmorris/centaur-context/pull/52),
[#56](https://github.com/bradwmorris/centaur-context/pull/56),
[#57](https://github.com/bradwmorris/centaur-context/pull/57), and
[#58](https://github.com/bradwmorris/centaur-context/pull/58);
[Enyu Site #1](https://github.com/bradwmorris/enyu-site/pull/1) and
[#2](https://github.com/bradwmorris/enyu-site/pull/2); and Centaur Enyu
[#15](https://github.com/bradwmorris/centaur-enyu/pull/15) through
[#24](https://github.com/bradwmorris/centaur-enyu/pull/24)
**Created:** 2026-08-30

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** The completed Enyu Source-ingestion RD and prior task; current
Context Source/client boundaries; Centaur Workflow v2 checkpoints, durable event
waits, workflow principals, and tool mediation; the Enyu Editor/Researcher
personas, ingestion workflow, role grants, deployment allowlist, fixtures, and
tests; and `enyu-site`'s private GitHub/Vercel linkage, Markdown loader,
validator, publishing guide, routes, metadata, sitemap, sample article, and
passing content check.

**Missing:** none.

1. In `/Users/bradleymorris/Desktop/dev/enyu-site`, make one authoritative,
   testable research-content contract shared by build and validation. Keep
   Git-native Markdown under `content/research/<slug>.md`; strictly validate the
   kebab-case slug, exact required frontmatter, `YYYY-MM-DD` date, field/body
   limits, safe Markdown/links, unique canonical identity, and `published`
   status. Reject unknown/unsafe input and exclude workflow provenance from
   public output. Add contract fixtures/tests, CI for `npm run check`, and
   publishing/correction documentation.
2. In `/Users/bradleymorris/Desktop/dev/centaur-enyu`, add an Editor-only trigger
   and skill plus `enyu_editor_publication`, a dedicated Workflow v2 principal,
   fixtures, contract tests, and the minimum allowlist/grants. It accepts one
   article plus cited Context Source Object IDs, reads only authorized Context,
   uses an Editor persona turn to produce a deterministic candidate, records its
   SHA-256 and provenance, and refuses unsupported, uncited, protected-to-public,
   or changed inputs.
3. Add a repo-bound publication adapter callable only by the publication
   workflow. It exposes semantic operations—not Git, shell, or arbitrary file
   access—and can affect only `bradwmorris/enyu-site`, a deterministic workflow
   branch, one `content/research/<slug>.md`, and its pull request. Broker a
   single-repository GitHub App installation token outside agent sandboxes; grant
   only the repository contents/pull-request permissions proven necessary. Give
   neither interactive persona raw GitHub, repository, Vercel, shell, deployment,
   or credential access.
4. Checkpoint candidate validation, branch/commit/PR creation, CI and Vercel
   preview polling, approval wait, exact-head merge, production readback, and
   notification. Post the preview URL and content hash, then wait durably for an
   allowlisted human's signed `approve`, `reject`, or `revise` event correlated
   to the run and exact head SHA. Retry replays the same run/PR; changed payloads
   conflict; rejection ends without merge; outages resume from checkpoints.
5. Implement correction/rollback as a new reviewed revert or replacement PR
   against the recorded publication commit—never force-push, rewrite history, or
   invoke Vercel rollback. After approved merge, verify the expected commit and
   rendered canonical article at `https://enyu.org/research/<slug>`; preserve
   commit, PR, workflow-run, source IDs, hashes, actor, timestamps, checks,
   preview, deployment/readback, and correction linkage in the audit result.

## What We Are Doing

- [x] Let the Enyu Editor prepare, review, and publish one cited research article
  through a durable, least-privilege workflow while Source ingestion remains a
  separate Researcher-owned workflow.
- [x] Prove one approved candidate becomes exactly one validated Markdown change,
  reviewed PR, production article, and auditable publication record; retries do
  not duplicate it and unapproved or changed candidates cannot go live.

## Contract

- **Goal:** Give the Editor a production-ready, Git-native path from an approved
  research draft to a verified Enyu article without broad credentials.
- **Done:** Local fixtures prove formatting, provenance, role isolation,
  idempotency, restart/retry, preview, exact-content approval, merge/readback,
  failure recovery, and corrective publication; one separately approved live
  acceptance publishes and reads back one synthetic or requester-approved article.
- **Files:** Site contract and CI only in `/Users/bradleymorris/Desktop/dev/enyu-site`;
  Enyu behavior and permissions only in `/Users/bradleymorris/Desktop/dev/centaur-enyu`;
  this RD only in Centaur Context. No Context product change unless execution
  proves a reusable API defect.
- **Agent owns:** Authorized local implementation, synthetic fixtures, tests,
  threat model, dry-run evidence, and exact permission manifest.
- **Requester owns:** Human approval identity and each live GitHub write, GitHub
  App creation/installation, credential enablement, overlay deployment, Vercel
  activity, production merge/publication, and any correction or rollback.
- **Out of scope:** Research/source ingestion, CMS or database, scheduled or batch
  publishing, social/email distribution, public ingress, arbitrary repository
  automation, direct Vercel deployment, and The AGI Post code, data, credentials,
  or business rules.

## Checks

- [x] Site contract fixtures and `npm run check` pass; PR CI blocks invalid content.
- [x] Overlay tests prove Editor ownership, Researcher denial, exact-path/tool
  restrictions, approval/hash binding, replay/conflict behavior, recovery,
  readback, audit evidence, and no raw credential exposure.
- [x] Synthetic end-to-end tests cover approve, reject, revise, duplicate, outage,
  stale-head, failed CI/preview, failed merge/deploy, and correction paths.
- [x] Required checks in each changed repository pass.
- [x] `git diff --check` passes.

Local verification on 2026-08-30: Enyu Site `npm run check` passed with nine
content-contract tests and a production build; Centaur Enyu passed its unit and
contract suite plus focused publication tests; Centaur Context passed formatting,
clippy, Rust tests, web type-check/build, 51 client tests, and Python compilation.

Live acceptance run `01a052d8-c58f-7c9e-8d87-518523f9b9d8` completed with
`status: published`. It read approved Source Object
`a17dcd4f-69ff-572e-9f23-d25161064b55`, created and approved
[Enyu Site PR #2](https://github.com/bradwmorris/enyu-site/pull/2), bound approval
to head `85bc4710f3d0a905bea6a87752d47a14465134a2` and candidate SHA-256
`c18148c42ac1c3df0c10f651cc33ce68832db5e29c7bc8e0af4a45208e163c0c`,
merged publication commit `ef234fdc30bebe7230ba3a432181d6a2861de454`, and
verified the production deployment and canonical article at
<https://enyu.org/research/durable-editor-publication-acceptance>.

## Approval Boundary

The requester explicitly approved the GitHub writes, repository-scoped GitHub
App and secret setup, overlay deployment, hosted approval event, merge, Vercel
activity, and live publication/readback used for this acceptance. Future
publication, correction, rollback, credential, deployment, or external-integration
actions remain separate approval boundaries. Reuse existing private Centaur and
GitHub/Vercel paths; add no public ingress without separate approval.

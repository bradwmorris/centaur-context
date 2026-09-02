# 6 — RD: Revisit the Centaur-Enyu Fork and Start from the Updated Main Repository

**Status:** `backlog`
**Created:** 2026-08-31

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Current `centaur-context` `origin/main` at `7dcf319`; Priority
5 RD for fork-based data modules; the clean `centaur-enyu` repository at
`cd49bb4`; both repositories' boundaries, remotes, recent history, deployment
files, workflows, fixtures, and tests. The current `centaur-enyu` repository is
an Enyu-specific overlay with separate Git history, not a direct fork of
Centaur Context, and it currently contains no web UI source.

**Missing:** Execution must wait until the recent Centaur Context work and
Priority 5 RD have landed on `origin/main`. The final replacement of the current
`centaur-enyu` default branch and local checkout requires explicit approval.

1. Freeze and inventory the existing Enyu overlay, current Context main, live
   compatibility requirements, and any Enyu work that must survive.
2. Create a non-destructive candidate from the then-current
   `centaur-context/origin/main`, with the canonical Context repository as a
   fetch-only upstream.
3. Reintroduce only approved Enyu modules, workflows, deployment configuration,
   and UI extensions through the fork-extension contract established by
   Priority 5; do not replay obsolete Context product code.
4. Prove full Context compatibility, Enyu behavior, and a repeatable upstream
   monitoring/synchronization workflow.
5. After visual and cutover approval, preserve the legacy overlay history,
   promote the candidate as `centaur-enyu`, and document the exact upstream
   baseline.

## What We Are Doing

- [ ] Replace the separate-history Enyu overlay architecture with a clean,
  history-related private fork based on the latest accepted Centaur Context
  `main`.
- [ ] Preserve required Enyu personas, workflows, fixtures, permissions, and
  deployment behavior without carrying forward stale copies or assumptions.
- [ ] Add approved Enyu data-module UI components on top of the current Context
  shell and APIs, following Priority 5 rather than creating a parallel app.
- [ ] Establish a safe process that detects upstream changes and makes future
  syncs reviewable, tested, and routine.

## Contract

- **Goal:** Rebase the Enyu product foundation on the latest Centaur Context
  implementation so Enyu extensions remain small and future upstream updates
  are manageable.
- **Done:** The accepted `centaur-enyu` main branch descends from a recorded
  current Context commit; required Enyu behavior and approved UI modules pass
  their tests; the legacy overlay remains recoverable; and a documented monitor
  reports new upstream commits without automatically merging them.
- **Files:** Planning remains in `centaur-context/dev/rd/`. Execution primarily
  affects a temporary sibling candidate and, after approval,
  `/Users/bradleymorris/Desktop/dev/centaur-enyu`; only reusable changes approved
  for everyone belong in `centaur-context`.
- **Agent owns:** Inventory, candidate creation, selective porting, upstream
  configuration, compatibility analysis, tests, local visual verification,
  cutover plan, and preservation evidence.
- **Requester owns:** Selection of optional Enyu UI modules, visual acceptance,
  default-branch/history replacement, production configuration, deployment,
  live migration, and merge approval.
- **Out of scope:** Modifying Centaur's `ai_v2` or Console databases; copying
  credentials or live data; blindly merging unrelated histories; deleting the
  legacy overlay; public ingress changes; automatic upstream merges; or
  rebuilding canonical Context capabilities inside Enyu.

## Migration and Synchronization Rules

- Build first in a sibling candidate such as `centaur-enyu-next`; never overwrite
  the accepted overlay checkout in place.
- Record the exact source commit and verify that it is an ancestor of the new
  Enyu branch. Preserve the old repository history under an immutable tag or
  legacy branch before any default-branch change.
- Inventory every current overlay file as `retain`, `adapt`, `replace`, or
  `retire`, with a reason. Port Enyu behavior into the extension points supplied
  by Priority 5; do not copy old Context internals into the new fork.
- Treat the current lack of Enyu web code as intentional evidence: add only the
  UI modules approved at execution time, using the inherited Context navigation,
  list/detail language, API authorization, and canonical Object contracts.
- Configure `origin` as the private Enyu repository and `upstream` as the
  canonical Context GitHub repository, with upstream push disabled.
- Track a reviewed upstream commit. A local scheduled monitor may fetch and
  report commit, migration, API, UI, and likely-conflict changes, but it must
  never merge, rebase, migrate, deploy, or mutate data automatically.
- Apply upstream updates only on dedicated sync branches with a written impact
  report, full checks, browser regression review, and an approved pull request.

## Checks

- [ ] The candidate's recorded Context baseline is an ancestor of its main
  branch, and both repositories and remotes are identified unambiguously.
- [ ] A file-by-file overlay disposition proves that required Enyu behavior was
  retained and obsolete assumptions were not copied.
- [ ] Priority 5 module contract checks and focused Enyu workflow, permission,
  fixture, API, and UI tests pass.
- [ ] Desktop and narrow browser review proves inherited Context surfaces remain
  intact and approved Enyu UI modules fit the existing design.
- [ ] The upstream monitor produces no-change and new-commit reports and cannot
  push or merge.
- [ ] All repository-root verification commands, Enyu overlay tests, and
  `git diff --check` pass.
- [ ] Legacy history recovery and local/GitHub cutover procedures are tested
  before approval is requested.

## Approval Boundary

Planning this RD authorizes no implementation. Execution may create a local
candidate and synthetic fixtures, but replacing or deleting the existing
checkout, rewriting or switching the GitHub default branch, live database
migration, production deployment, public ingress, credentials, external sends,
or merging requires explicit requester approval at the relevant checkpoint.

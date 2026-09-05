# 6 — RD: Run 100 Real-Work Slack Evals

**Status:** `scoped`
**Created:** 2026-09-05

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** The live canonical corpus and human API; current Runs and
Evals UI; `dev/evals/AGENTS.md`; the completed Slack golden-scenario, Evals Run
view, and explainable-Run RDs; the merged Context checkpoint `afd0267`; and the
100 scenarios in `dev/evals/100-real-work-eval-scenarios.md`.

**Missing:** Before execution, create a campaign ID and resolve the five
approved fresh-source placeholders in the scenario catalogue. No architecture
or approval decision remains.

1. Preflight the local stack, canonical database, Slack identities, pinned
   baseline, runtime versions, fresh-source inputs, and exact cleanup methods.
2. Run scenarios E001–E100 through Slack, one at a time. Review and annotate
   every resulting Run. Stop each batch of ten for a hygiene and progress
   checkpoint.
3. For each failure group, preserve the failed Run, fix the first upstream cause
   on a dedicated branch, run that branch locally, replay the exact scenario,
   and merge only after the replacement passes and normal approval is given.
4. Finish with a pinned-suite replay, fixture cleanup audit, repository and
   deployment hygiene check, and a concise report of passes, failures, fixes,
   remaining gaps, and golden Runs.

## What We Are Doing

- [ ] Exercise 100 realistic research, retrieval, synthesis, ingestion,
      follow-up, mutation, permission, and recovery interactions as Brad in
      Slack.
- [ ] Retain honest attempt history while leaving only useful research and
      explicitly identified evidence fixtures in canonical Context.
- [ ] Finish with every scenario passed and agent-approved, or explicitly
      blocked with its evidence and unresolved owner recorded.

## Current System and Why It Looks This Way

The first eval design used a CSV, but Runs already record every real execution.
The CSV was removed so there is one history: **Evals is a review view over root
Runs**. Every attempt is a new Run. The existing Run stores its input, result,
trace, usage, related Objects, verdict, annotation, and golden pin. Failed Runs
remain visible and unpinned; pinning does not copy data.

The Evals list adds review controls to the existing Runs UI. The detail page now
shows the visible request and response, application-controlled instructions,
retrieved Context with reasons and exact injected text, model usage, sanitized
tool calls, execution trace, child Runs, related Objects, and mutations.
Provider-hidden instructions and chain-of-thought are neither available nor
shown. Future Runs preserve execution-time evidence; old Runs are never
reconstructed from current prompts or Objects.

Three operating modes are defined in `dev/evals/AGENTS.md`: agent-generated
scenarios, exact replay of a failure, and simulation of all pinned golden Runs.
This campaign uses all three. Brad has explicitly delegated golden approval for
this RD, so a fully inspected pass may be annotated
`Golden — approved · agent-verified`; only the best stable Run is pinned.

The explainability implementation landed in Context PR #94 at `afd0267` and its
Centaur producer work landed in PR #14 at `ba90df1`. The canonical local data is
`centaur_context_enyu`. Do not point the UI or runtime at the obsolete deleted
legacy `centaur_os` database or the stale `centaur-context-env` configuration;
the working local runtime uses `centaur-context-enyu-agent`. Verify this through
configuration and the human API, never by giving a database DSN to a sandbox.

## Failure-to-Fix Architecture

The campaign is a controller, not a mega implementation branch. Run the next
scenario against synchronized `main`. When it fails:

1. annotate and preserve the failed Run;
2. identify the first upstream cause and group any equivalent failures;
3. create one repair RD/issue and `codex/<issue>-<defect>` branch from current
   `origin/main` in its own worktree;
4. run the affected local services from that branch and replay the same Slack
   input before merge, except for schema-changing repairs described below;
5. record the tested commit/image in the passing replacement Run;
6. seek the normal merge approval, merge, fast-forward canonical local `main`,
   restore the main-based stack, and remove the worktree and branches; then
7. continue the campaign from the newly synchronized `main`.

One root cause gets one repair branch even if several evals expose it. Unrelated
defects get separate branches. Never stack hundreds of fixes on a long-lived
campaign branch, and never call a branch-only pass landed. The campaign issue
tracks progress; Run annotations hold attempt evidence; defect issues and RDs
hold repair reasoning.

The canonical local database is shared state even when the code is in a
worktree. Normal backward-compatible branch code may execute the exact eval
against it, with the branch commit recorded on the Run. A schema-changing repair
must first use a disposable `centaur_context_test` database. Do not apply an
unmerged migration to canonical data unless compatibility is proved and Brad
separately approves it; otherwise merge, migrate the main-based stack, and then
perform the live Slack replay.

## Campaign Controls

- Execute strictly one scenario at a time; dependent messages stay in one Slack
  thread and wait for terminal state between steps.
- Use campaign ID `eval100-<YYYYMMDD>-<short-id>` and the fixture marker defined
  in the catalogue. Record every created Object ID immediately.
- Classify data before the run: read-only, approved durable research, or
  disposable fixture. Clean only exact disposable IDs after grading. Never
  delete Runs. If safe cleanup is unsupported, retain and annotate the fixture
  and open a defect.
- Stop after each ten scenarios. Confirm there is no running sandbox work,
  unreviewed campaign Run, unexpected mutation, dirty unrelated checkout,
  abandoned worktree, or unmerged repair before the next batch.
- Do not silently weaken an expected result to turn a failure into a pass.
  Catalogue changes require a reason in the campaign issue and preserve the
  original failed evidence.
- Watch quality and operations separately: correctness, grounding, source and
  Object identity, permissions, mutations, latency, token shape, readiness
  polling, tool failures, and prompt/retrieval bloat.

## Contract

- **Goal:** Prove and improve the real Slack research workflow with 100
  representative, repeatable eval scenarios.
- **Done:** E001–E100 each has a reviewed terminal Run and approved golden pass,
  or a documented hard blocker; all authorized repairs are landed and synced;
  disposable fixtures are cleaned by exact ID; intended Sources remain; and
  the pinned-suite replay and final hygiene audit pass.
- **Files:** `dev/evals/100-real-work-eval-scenarios.md`,
  `dev/evals/AGENTS.md`, this RD, campaign/defect RDs, and only the owning code
  and tests for defects discovered during later execution.
- **Agent owns:** Slack execution, Run review and delegated approval, safe
  fixture hygiene, diagnosis, authorized local fixes and verification, and
  evidence-rich handoff.
- **Requester owns:** Fresh research-source selection if the prepared choices
  are unsuitable, disputed semantic judgments, credentials, paid/external
  actions, and merge approval.
- **Out of scope:** A second eval database or CSV, deleting Run history, direct
  sandbox database access, automatic model judging, public ingress, and one
  giant branch containing unrelated repairs.

## Checks

- [ ] Exactly 100 uniquely identified catalogue scenarios exist and every one
      has an explicit pass condition and data-hygiene classification.
- [ ] Every attempt has a terminal Run, verdict, factual annotation, and correct
      pin state; multi-step annotations identify step order.
- [ ] Every branch-tested repair records its commit/image, focused checks, exact
      replay, landed PR, synchronized local `main`, and removed worktree/branch.
- [ ] Final pinned-suite replay passes and fixture-ID audit finds no unexplained
      disposable Objects or deleted Run evidence.
- [ ] Repository-root verification and `git diff --check` pass for every code
      repair; documentation-only campaign checkpoints at least run
      `git diff --check`.

## Approval Boundary

Brad approved priority 6, execution of the 100 Slack scenarios, delegated
agent approval of verified golden Runs, and exact disposable-fixture cleanup.
That does not pre-approve merge, unsafe or broad deletion, arbitrary external
messages, credentials, spending, public ingress, or production deployment.

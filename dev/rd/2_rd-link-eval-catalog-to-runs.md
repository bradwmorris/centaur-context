# 2 — RD: Link Eval Catalog to Runs

**Status:** `scoped`
**Created:** 2026-09-05

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** `dev/evals`; the consolidated `runs` schema; Run list,
detail, filtering, review API, and UI; standard Python client; Slack RSI eval RD;
and current repository development and approval rules.

**Missing:** none.

1. Version and validate the repository eval definitions without adding an
   execution-history file.
2. Extend the existing human Run review path to attach optional eval identity
   and expose it through Run filters, the client, and the existing Runs UI.
3. Document the agent-operated eval loop, add focused tests, and run repository
   verification before review and landing.

## What We Are Doing

- [ ] Keep `dev/evals/evals.csv` as the canonical definition catalog, with one
  stable, versioned row per independently testable case.
- [ ] Keep every attempt in the existing `runs` table and show its case, batch,
  verdict, and plain-language actual result without duplicating Run history.
- [ ] Make a request to run named or pinned evals follow one documented
  Slack → Run review → diagnose → fix → rerun loop.

## Design View

```text
dev/evals/evals.csv                 existing runs table
-------------------                -------------------
id + version                       one row per actual attempt
suite + ordered step     <------   result.evaluation identifies case/batch
exact input + expected result      verdict is pass/fail
pinned + lifecycle                 review_notes says what happened

GitHub renders definitions         existing Runs UI shows attempts/results
```

The runtime continues creating normal Slack interaction and child workflow Runs
exactly as it does today. After correlating the Slack response to its existing
root Run, the evaluator uses the existing human review endpoint once to set:

```json
{
  "verdict": "pass",
  "notes": "Rez created one Note with the expected Chat and Source links.",
  "evaluation": {
    "id": "slack-rsi-003",
    "version": 1,
    "batch_id": "slack-rsi-20260905-01"
  }
}
```

Store that identity as `result.evaluation`, beside the existing
`result.review_revision`. This reuses bounded Run JSON and optimistic review
updates, requires no migration, and avoids interrupting any Run writer. All
three evaluation values are supplied together, validated, and replaceable only
through another revision-checked human review. Historical Runs are not guessed
or backfilled.

## Contract

- **Goal:** Make the repository eval catalog and existing Run history work as
  one minimal evaluation system.
- **Done:** Catalog cases are versioned and ordered; an existing Run can be
  reviewed with validated case/version/batch identity; the human API and Runs UI
  can filter and display that identity, verdict, and plain-language actual
  result; and the standard authenticated client supports the same operation.
- **Files:** `dev/evals`; `src/runs.rs`; `src/api.rs`; focused Rust tests;
  `tools/centaur_context`; Run types/API/list/detail UI and tests; this RD.
- **Agent owns:** Implementation, catalog validation, local/API/UI tests, and
  documentation of the eval operating loop.
- **Requester owns:** Disputed semantic verdicts, exact destructive reset
  approval, RD priority for newly discovered nontrivial work, and merge approval.
- **Out of scope:** A new eval/attempt table or CSV; a new Run kind; changing
  Slack/Centaur/Enyu Run creation; automatic judging; historical Run backfill;
  public ingress; production deployment; or a second dashboard/navigation area.

## Operating Loop

When Brad asks to run named or pinned evals, start from `dev/evals`: select the
exact active cases, create one batch ID, control Slack with their exact inputs,
and wait for terminal state after each step. Review the user-visible response
first, then its existing Run, children, trace, retrieval, mutations, and durable
state. Set pass/fail plus one concise factual actual result on the root Run.

Stop a dependent suite at its first upstream failure. Preserve failed Runs. Group
observed failures by root cause; fix an obvious in-scope defect directly when
authorized, otherwise create one RD/issue for the coherent repair under the
normal priority and approval rules. Reruns use a new batch ID and new Runs.

Selecting evals authorizes only their exact Slack test messages and read-only
evidence collection. It does not authorize fixture deletion, production work,
unrelated external messages, or merge.

## Checks

- [ ] CSV validation proves the fixed header, unique stable IDs, positive
  versions, unique suite step ordering, and allowed pinned/status values.
- [ ] Run review tests prove all-or-none validated eval identity, optimistic
  concurrency, preservation of terminal result data, and human-only access.
- [ ] Run list/detail tests prove eval ID and batch filters and readable actual
  results without multiplying or duplicating Runs.
- [ ] Standard client and web tests cover review, display, filtering, errors,
  and legacy untagged Runs.
- [ ] Repository-wide format, lint, Rust, web, Python, and `git diff --check`
  checks pass.

## Approval Boundary

This RD authorizes implementation and local verification only after explicit
execution approval. Slack execution of selected evals, destructive fixture
reset, local deployment, creation of follow-up issues/RDs, and merge retain the
operating-loop and repository approval boundaries above. No database DSN may be
given to an agent or sandbox.

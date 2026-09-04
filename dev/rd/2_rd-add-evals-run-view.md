# 2 — RD: Add Evals Run View

**Status:** `scoped`
**Created:** 2026-09-05

## Execution Plan

**Status:** `still needs work`

**Basis checked:** The consolidated `runs` schema; Run list, detail, review API,
and UI; current left navigation and routes; the Sarah Guo RSI Runs and RD; and
the temporary `dev/evals` CSV catalog.

**Missing:** Confirm whether Evals shows all root Runs or only reviewed/pinned
Runs; whether a golden scenario needs named multi-step grouping; and whether to
remove `dev/evals` entirely and pin the existing Sarah Guo Runs after deployment.

1. Add the smallest persistent golden-Run field and extend the existing Run
   review operation to update it with verdict and annotation.
2. Add an **Evals** item at the bottom of the left navigation and render a
   review-focused table over existing Runs.
3. Document the agent-operated test/fix/rerun loop, add focused tests, and run
   repository verification before review and landing.

## What We Are Doing

- [ ] Use existing Run rows as the only records of eval attempts.
- [ ] Let a human pin useful Runs as golden examples and annotate every attempt
  in plain language.
- [ ] Provide a persistent Evals table in the existing local UI without adding
  an eval-definition or attempt table.

## Design View

```text
Runs                                      Evals
----                                      -----
existing operational list                another view of the same root Runs
open a Run for full trace                 pinned golden Runs first
                                          input | actual result | pass/fail
                                          editable annotation | pin control

                         same Run row
                         no copied history
```

The **Runs** view remains unchanged and answers “what executed?” The **Evals**
view answers “how did these executions behave?” It uses the same Run IDs and
opens the same Run detail pages.

The Evals table will show a compact row with the Run date/type, user input or
title, actual result from the existing result/error, pass/fail verdict, editable
annotation backed by `review_notes`, and a golden pin. Pinned rows sort first;
the remaining rows stay newest first. Editing saves through the current
revision-checked human Run review endpoint.

Add one `pinned boolean NOT NULL DEFAULT false` column to `runs`. Reuse existing
`verdict`, `review_notes`, reviewer, timestamp, and review revision fields. Do
not add another table, Run kind, CSV attempt log, or alternate execution path.
The Run creation APIs, Slack ingestion, traces, child Runs, and Centaur/Enyu
integration do not change.

## Contract

- **Goal:** Make evaluation a simple review mode over Runs that already exist.
- **Done:** Evals appears at the bottom of the existing left navigation; it
  displays existing root Runs in a readable table; pin, pass/fail, and inline
  annotation changes persist on the same Run; pinned rows are easy to find and
  reuse; and Runs retain their current execution behavior and detail view.
- **Files:** One additive migration; `src/runs.rs`; `src/api.rs`; focused Rust
  tests; Run types/API/routing/list UI and tests; development instructions; this
  RD; and removal or retirement of `dev/evals` if approved.
- **Agent owns:** Implementation, migration and API tests, UI behavior,
  documentation, and local verification.
- **Requester owns:** The three decisions under **Missing**, disputed semantic
  verdicts, destructive fixture reset approval, local deployment approval, and
  merge approval.
- **Out of scope:** A new eval/eval-case/attempt table; a CSV execution ledger;
  automatic judging; changing how Runs are created; historical inference beyond
  explicitly selected Sarah Guo Runs; public ingress; or production deployment.

## Operating Loop

When Brad asks to run tests, the agent opens Evals, selects the named or pinned
golden Runs, and uses their user inputs as the test script. It controls Slack as
Brad, waits for each normal Run to finish, reads the user-visible result first,
then inspects the trace and durable state. It records pass/fail and a short
factual annotation directly on each new Run.

Every retry remains a separate Run row. Failed Runs are never rewritten or
deleted to improve the history. The best stable example may be pinned as golden.
Failures are grouped by their first upstream root cause. An obvious authorized
fix may be made directly; nontrivial work gets one RD/issue per coherent repair
under the normal development rules. A rerun creates new Run rows.

Running selected evals authorizes their exact Slack test messages and read-only
evidence collection. It does not authorize fixture deletion, unrelated external
messages, production work, or merge.

## Checks

- [ ] Migration tests prove existing Runs remain unpinned and pin state persists
  without changing execution, trace, result, or Object relationships.
- [ ] Run review tests prove pin, verdict, and annotation updates remain
  revision-checked and human-only.
- [ ] Evals routing/UI tests prove bottom-left navigation, pinned-first ordering,
  readable input/result, inline editing, and links to the existing Run detail.
- [ ] Legacy unreviewed Runs and failed Runs remain visible according to the
  confirmed inclusion rule.
- [ ] Repository-wide format, lint, Rust, web, Python, and `git diff --check`
  checks pass.

## Approval Boundary

This RD is planning only until explicit execution approval. No Slack messages,
database writes, fixture deletion, deployment, external issue creation, or merge
is authorized by this document alone. Agents and sandboxes continue using
authenticated HTTP APIs and never receive a database DSN.

# Centaur OS Development Tasks

This directory contains the lightweight planning process for Centaur OS.
Requirement documents live in `dev/rd/` and are the durable specification for
work that is larger than an obvious, self-contained fix.

## When To Create An RD

Create an RD when asked to record, scope, or plan a distinct piece of Centaur
OS work. A small compatible fix requested for immediate implementation does
not require an RD unless one is specifically requested.

Planning an RD and executing an RD are separate modes. Creating or planning an
RD does not authorize implementation. Stop after writing and checking the
document unless execution is also explicitly requested.

Do not automatically create a GitHub Issue or a Centaur OS Task row. Those can
be created or linked when execution is explicitly requested.

## RD Location And Naming

- Store RDs in `dev/rd/`.
- Name each file `rd-<short-kebab-case-slug>.md`.
- Keep backlog and active RDs directly in `dev/rd/`.
- Move finished RDs to `dev/rd/complete/`.
- Treat the Git-tracked RD as the executable specification.

## Intake

Before writing an RD:

1. Inspect the relevant repository code, tests, migrations, and documentation.
2. Resolve the intended outcome, observable done state, file boundaries,
   dependencies, non-goals, checks, and approval boundaries.
3. Ask the requester only when an unresolved choice would materially change the
   task.
4. Keep new work in `backlog` unless scoping or execution is explicitly
   requested.
5. Do not assign a due date, priority, owner, GitHub Issue, or database Task ID
   unless one is requested or execution has been explicitly authorized.

## RD Format

Use this compact structure:

```markdown
# RD: [Task title]

**Status:** `backlog|scoped|in_progress|blocked|review|complete`
**Created:** YYYY-MM-DD

## Execution Plan

**Status:** `complete and ready|still needs work`

**Basis checked:** [Relevant code, tests, migrations, and documentation.]

**Missing:** [Exact missing dependency or decision, otherwise `none`.]

1. [Implementation step.]
2. [Next implementation step.]
3. [Verification and closeout.]

## What We Are Doing

- [ ] [Plain-language outcome.]
- [ ] [Observable proof of completion.]

## Contract

- **Goal:** [One concrete outcome.]
- **Done:** [Observable proof.]
- **Files:** [Expected repository boundaries.]
- **Agent owns:** [Authorized implementation and local verification.]
- **Requester owns:** [Decisions, credentials, external actions, or approvals.]
- **Out of scope:** [Explicit non-goals.]

## Checks

- [ ] [Targeted test or inspection.]
- [ ] `git diff --check` passes.

## Approval Boundary

[State whether deployment, public ingress, hosted writes, external
integrations, publishing, sending, spending, credentials, or deletion require
explicit requester approval.]
```

## Quality Rules

- Aim for fewer than 1,000 words; never exceed 3,000 words without explicit
  requester approval.
- Write concrete requirements and observable checks, not a conversation
  transcript.
- Keep one status line and one clear done state.
- State exact repository boundaries and preserve unrelated work.
- Respect the ownership and safety boundaries in the repository-root
  `AGENTS.md`.
- Do not add public ingress, cloud deployment, or external integrations without
  explicit approval.

## RD Planning

When asked to create or plan an RD:

1. Inspect the relevant repository state and write or update the RD.
2. Check that its plan, boundaries, done state, and verification are complete.
3. Leave it in `dev/rd/` with the appropriate pre-execution status.
4. Stop. Do not create an issue or branch, implement the work, or open a PR.

## RD Execution

When asked to execute an RD:

1. Read the entire RD and inspect the current repository state.
2. Create one GitHub Issue for the job, or reuse the existing linked issue. Add
   its number to the RD and set the RD status to `in_progress`.
3. Create one branch from the latest `main`, named
   `codex/<issue-number>-<short-slug>`.
4. Implement only the documented scope, preserving unrelated changes.
5. Run the checks required by the RD and the repository-root `AGENTS.md`.
6. If problems remain, report them and do not present the work as ready.
7. If the work is ready, mark the RD `complete`, check off its completed
   outcomes and checks, record concise verification results, and move it to
   `dev/rd/complete/` without changing its filename.
8. Commit and push the branch, then open one PR containing
   `Closes #<issue-number>` and the verification results.
9. Ask whether to merge. Never merge without explicit approval.
10. After approval, squash-merge the PR, delete the branch, and confirm that the
    issue closed.

When a Centaur OS Task or GitHub Issue exists, add a compact reference near the
RD metadata. The RD remains the detailed implementation contract; live task
state belongs in Centaur OS, and public discussion belongs in the GitHub Issue.

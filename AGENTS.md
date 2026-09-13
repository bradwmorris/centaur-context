# Centaur Context contributor guidance

Centaur Context is a reusable, local-first shared context application for
Centaur users and agents.

## Product boundaries

- Keep the public core organization-neutral.
- Keep Centaur Context data separate from Centaur's operational databases.
- Agents use the authenticated HTTP API; never give a sandbox a database DSN.
- Keep organization-specific prompts, agents, workflows, policies, and data in
  a separate private overlay.
- Do not add public ingress or external integrations without an explicit design
  and security review.
- Preserve the canonical Object model, explained Connections, immutable Object
  Events, and reviewable Runs.

## Before submitting a change

Read `CONTRIBUTING.md`, keep the change focused, and run the documented checks.
Database integration tests must use a disposable database whose name contains
`centaur_context_test`.

## Issue workflow

Work in one of two explicit modes:

- **Create issue:** inspect enough code and documentation to write one clear
  GitHub Issue with the outcome, scope, non-goals, acceptance checks, and any
  unresolved decision. Create or reuse that Issue, then stop. Do not implement
  the work in this mode.
- **Execute issue:** require an existing GitHub Issue, update local `main` from
  `origin/main`, create one `codex/<issue-number>-<slug>` branch, implement only
  the Issue scope, run the documented checks, self-review the diff, and open a
  linked pull request. Merge after required checks pass unless evidence is
  incomplete, a material decision is uncertain, or the Issue/RD reserves an
  approval for the repository owner. After merging, confirm local `main` equals
  `origin/main` and the Issue is closed.

An obvious small change needs only an Issue. Create an RD for complex, risky, or
multi-stage work that needs a durable plan or explicit approval boundaries.
Never merge with failing checks or unresolved review findings, push directly to
`main`, broaden the Issue while implementing it, or overwrite unrelated work.

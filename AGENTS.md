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

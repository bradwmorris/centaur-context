# 5 — RD: Enable Fork-Based Centaur Context Data Modules

**Status:** `backlog`
**Created:** 2026-08-30

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Repository boundaries; automatic SQLx migration startup;
canonical Object and subtype migrations through migration 10; fixed Rust human
and agent routers; compiled React navigation and record views; schema visualizer
registry; standard `tools/centaur_context` client; installation, ontology, and
operations documentation; and the Enyu overlay boundary.

**Missing:** none.

1. Define and document a compile-time data-module convention for developers who
   maintain a fork of Centaur Context, including the supported extension points
   and the protected core contracts.
2. Add a small synthetic reference module and templates that demonstrate the
   complete schema, API, UI, agent-client, permission, and test path without
   adding organization-specific product behavior.
3. Verify that the reference module is visible to humans and agents, that a new
   module can be reproduced from the documentation, and that the unchanged core
   application continues to pass its full contract checks.

## What We Are Doing

- [ ] Make it straightforward for a developer to fork `centaur-context` and add
  a domain-specific data module directly to that fork.
- [ ] Demonstrate one complete module whose records can be migrated, inspected
  and managed in a dedicated UI pane, and accessed by authorized agents through
  authenticated HTTP APIs and the standard client.
- [ ] Clearly separate safe extension points from canonical core tables,
  credentials, and Centaur-owned databases.

## Contract

- **Goal:** Establish a documented, tested convention for manually adding a
  compile-time data module to a Centaur Context fork.
- **Done:** Following only the extension guide and reference module, a developer
  can add a new module with its own forward migration, navigation pane, record
  UI, authenticated API surface, agent-client commands, permission rules, and
  tests; a fresh installation exposes it to humans and authorized agents.
- **Files:** New extension documentation and module templates; additive
  migrations; narrowly organized Rust module/API/database registration points;
  React module/navigation registration points; the standard agent client;
  package and contract tests. Do not modify `/Users/bradleymorris/Desktop/dev/centaur`
  or any private overlay as part of this RD.
- **Agent owns:** The extension convention, synthetic reference implementation,
  documentation, compatibility safeguards, tests, and local verification.
- **Requester owns:** Selection and implementation of any real organization
  module, production data, credentials, deployment, publication, and upstream
  proposal.
- **Out of scope:** Runtime module discovery; loading modules from another
  repository or overlay; arbitrary third-party code execution; Enyu-specific
  research behavior; public ingress; cloud deployment; and changes to Centaur's
  `ai_v2`, Console, or other core databases.

## Required Extension Contract

### Protected core

- Existing migration files are immutable. Fork modules use new forward-only
  migrations and cannot repurpose canonical core tables or Object kinds.
- Module tables use an explicit naming or schema convention and may reference
  canonical Object IDs through documented relationships without weakening core
  constraints.
- Agents continue to use authenticated HTTP APIs and never receive a database
  DSN or migration credential.

### Compile-time module shape

- Define one predictable registration location for each module's Rust backend,
  React UI, migration, and agent-client surface so extension authors do not have
  to rediscover unrelated application internals.
- Provide copyable templates and a checklist for adding list, read, search, and
  explicitly authorized mutation behavior.
- Make module navigation, routes, labels, and permissions explicit rather than
  deriving agent access merely from table existence.

### Human and agent availability

- The compiled UI presents the reference module as its own pane with accessible
  list, detail, create/edit where authorized, empty, loading, and error states.
- The authenticated API exposes bounded structured operations rather than
  arbitrary SQL.
- The standard agent client documents and exercises the module operations.
  Read-only access remains the default; any write operation requires a separate
  explicit credential and audit path.
- Module records that participate in shared context have an explained mapping
  or Connection to canonical Objects. Large or domain-specific payloads are not
  silently copied into Object descriptions.

### Developer experience and upgrades

- Document the exact files to copy or edit, naming rules, migration sequencing,
  schema-visualizer registration, tests, build commands, and upstream rebase
  considerations.
- Add a static or test-time completeness check that fails when a registered
  module omits a required migration, API, UI, or agent-access declaration.
- Preserve a useful failure mode when module schema and compiled code disagree.

## Checks

- [ ] A fresh disposable Context database applies all core and reference-module
  migrations and passes the database contract.
- [ ] Focused backend tests prove authentication, authorization, validation,
  bounded reads, audit behavior, and denial of arbitrary table access.
- [ ] UI tests prove navigation and the module's primary record flow.
- [ ] Agent-client tests prove authorized module discovery/read behavior and
  denied write or cross-permission behavior.
- [ ] A documentation smoke test or clean-room fixture proves the module can be
  reproduced using the guide.
- [ ] All repository-root verification commands and `git diff --check` pass.

## Approval Boundary

This RD authorizes only reusable local product work and synthetic fixtures when
execution is explicitly requested. It does not authorize a production module,
real research data, credentials, external integrations, deployment, public
ingress, publication, model spend, or changes to any Centaur-owned database.

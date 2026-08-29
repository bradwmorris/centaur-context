# RD: Rename Centaur OS to Centaur Context

**Status:** `in_progress`
**Created:** 2026-08-29
**GitHub Issue:** `#10`

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Every tracked path and text file; Git history, branches,
worktrees, remote, public GitHub metadata, issues, and PRs; Rust, Python, web,
container, API, environment, database, backup, Kubernetes, operations, docs,
tests, and RD contracts; filesystem references outside build/dependency/VCS
directories; target-name availability; and tracked consumers in the adjacent
Centaur and private overlay checkouts.

**Missing:** The GitHub repository rename, local checkout/Codex project move,
and any live installation cutover remain requester-owned actions after the
local code and consumer changes pass verification.

1. Establish the canonical `Centaur Context` naming matrix and compatibility
   bridge, then rename this repository's product, packages, source modules,
   tool, runtime metadata, UI, documentation, and tests without changing its
   ontology or HTTP API semantics.
2. Rename new-install operational identifiers and update the adjacent Centaur
   and overlay consumers; preserve existing data, applied migrations, immutable
   provenance, and explicitly supported legacy upgrade inputs.
3. Verify fresh-install and upgrade contracts, coordinate the public repository
   and local checkout rename, run all affected repositories' checks, and remove
   every accidental old-name reference.

**Cutover sequence:** Merge the verified product repository first while existing
consumers remain pinned to their last known-good commit. Prepare and verify
separate Centaur and overlay branches, rename the GitHub repository and local
checkout only after approval, update the pinned source and saved paths, then
perform the separately approved Kubernetes handoff. Each repository gets its
own reviewed commit/PR; parent-workspace and Codex project changes are local
requester-owned steps.

## What We Are Doing

- [ ] Present one clear product: **Centaur Context**, “Memory for your Centaur,”
  described technically as a compounding context engine for Centaur users and
  agents.
- [ ] Make `centaur-context`, `centaur_context`, and `CENTAUR_CONTEXT` the
  canonical public, code, and operational identifiers.
- [ ] Keep existing installations and backups recoverable without rewriting
  business records, Object Events, Curator Runs, or applied migration history.
- [ ] Cut over every known Centaur and overlay consumer so ingestion, context
  retrieval, tool grants, DNS, and network policy continue to work.

## Contract

- **Goal:** Surgically rename the complete project and repository from Centaur
  OS to Centaur Context while preserving behavior, data, security boundaries,
  and a controlled upgrade path.
- **Done:** The product, repository, checkout, packages, executable, tool, UI,
  docs, new-install database identifiers, container/Kubernetes resources, and
  known consumers use the new name; legacy installations/backups have a tested
  path; only documented compatibility or historical records retain the old
  name; and all required checks pass.
- **Files:** All currently tracked files containing an old-name token or a
  derived package/runtime identifier; renamed `tools/centaur_os` and active RD
  paths; adjacent Centaur shared-context source/tests; adjacent overlay docs,
  policies, manifests, tool source, and renamed paths; the containing VS Code
  workspace entry; targeted migration and compatibility tests; this RD.
- **Agent owns:** Local rename implementation, compatibility code, documentation,
  consumer patches, tests, residual-reference audit, and local verification when
  execution is separately assigned.
- **Requester owns:** Renaming `bradwmorris/centaur-os` on GitHub, approving its
  description/settings, moving the checkout or saved Codex project, rotating or
  copying secrets, deployment, database administration, publishing packages or
  images, and deletion of legacy resources.
- **Out of scope:** Changing the canonical Object ontology, renaming the Memory
  subtype, changing `/api/v1` routes or schemas beyond the intentional product
  metadata value, adopting new integrations, public ingress, rewriting Git
  history, editing immutable stored provenance, or redesigning product features.

## Canonical Naming Matrix

| Surface | New canonical identifier |
| --- | --- |
| Product/display name | `Centaur Context` |
| Repository/check-out | `centaur-context` |
| Rust package/binary/crate | `centaur-context` / `centaur_context` |
| Web package | `centaur-context-web` |
| Standard tool path/module | `tools/centaur_context` / `centaur_tool_centaur_context` |
| Agent CLI | `centaur-context` |
| Environment prefix | `CENTAUR_CONTEXT_` |
| Container, Linux user, Kubernetes app/service | `centaur-context` |
| Secret | `centaur-context-env` |
| New database/role/test pattern | `centaur_context` / `centaur_context_app` / `centaur_context_test` |
| API and backup product discriminator | `centaur-context` |
| New Curator prompt version | `centaur-context-curator-v1` |

Keep `Memory` as the event-shaped Object subtype. Use “Memory for your
Centaur” as positioning, not as the system or ontology name.

## Detailed Requirements

### Product, source, packages, and UI

- Rename the Cargo package/binary and all Rust crate imports; regenerate
  `Cargo.lock`. Rename the Python tool directory, installed module, distribution,
  CLI entry point, classes/help text, secret declaration, default URL, tests,
  and package checks. Rename web package/lock metadata and UI title, navigation
  labels, accessibility labels, and human-created provenance text.
- Update the Docker binary, unprivileged user/group, OCI labels, image examples,
  API metadata, logging target examples, error prefixes, license attribution,
  README, all guides, `AGENTS.md` files, handoff prompt, and active RDs. Rename
  `rd-fork-centaur-os-multi-agent-poc.md` and its proposed fork/overlay paths.
- Do not rewrite Git commits or stored titles, descriptions, immutable events,
  Curator plans, prompt-version provenance, or prior backup files merely to hide
  the former name. Historical references must be truthful and clearly marked.
- Treat the `brad_os` phase records and research RDs, the AGI Post operations
  RD, Git history, and `centaur-os-backups` directory as external historical
  evidence, not product source to bulk-rewrite. New backups use a new
  `centaur-context-backups` location; retained old backups keep their names and
  checksums.

### Compatibility and durable data

- Treat the rename as a breaking product/tool release and advance product and
  tool version to `0.2.0`; keep HTTP API and ontology at `v1`. Keep schema at the
  actual latest migration version (`8`) unless execution adds a necessary new
  migration. Do not change migration history merely for the rename.
- New installations create `centaur_context` and `centaur_context_app`. Existing
  `centaur_os`, `centaur_os_app`, and `centaur_os_test` databases remain accepted
  by narrowly scoped safety guards so an upgrade does not require risky data
  movement. This database/role compatibility is permanent. Never silently
  rename, copy, drop, or recreate a live database or role.
- Rename only settings that currently use the `CENTAUR_OS_*` prefix; keep generic
  runtime contracts such as `DATABASE_URL` and `AGENT_API_TOKEN` unchanged. For
  one minor release, the agent client and local operations scripts accept
  documented `CENTAUR_OS_*` aliases only when the new equivalent is absent;
  conflicting old/new values fail closed. Canonical examples use the new names,
  while upgrade documentation names the legacy aliases explicitly.
- New backups write `product: centaur-context`; restore/validation accepts both
  the new discriminator and a legacy `centaur-os` backup with otherwise valid
  checksum, schema, and database safeguards. Validate metadata before any
  destructive restore. A legacy dump with no metadata sidecar remains
  recoverable only through an explicit, documented confirmation flag after its
  checksum and target database are validated. Test all three cases. Do not
  mutate old backup metadata in place.
- Do not rename applied SQLx migration files or alter their checksums. Existing
  Curator prompt-version strings and human provenance remain valid historical
  data; only newly created values use the new name.

### Deployment and consumer cutover

- Rename images, Deployment, Service, Secret, selectors, NetworkPolicies,
  temporary paths, confirmation tokens, port-forward examples, and in-cluster
  DNS. Installation must detect legacy resources and refuse to create a second
  active deployment accidentally. The handoff requires an explicit legacy
  cutover flag and this ordered state transition: validated backup; new Secret
  prepared; old Deployment scaled to zero; new resources applied; consumers
  switched to new DNS; health and ingestion verified. Rollback scales the new
  Deployment to zero, restores old consumer references, and scales the old
  Deployment back up. Legacy-resource deletion remains a later, separately
  approved action.
- Update the adjacent Centaur checkout's shared-context heading and tests from
  `Centaur OS Shared Context` to `Centaur Context`, without widening Centaur's
  control-plane ownership. Update the adjacent private overlay's repository/tool
  paths, filenames, CLI calls, prompts/skills, URLs, Secret references, pod
  selectors, deployment patches, and NetworkPolicies. Re-pin tool source to the
  renamed repository commit after the public rename.
- Preserve the architecture: Centaur owns agent execution, sessions, tools,
  workflows, and credentials; Centaur Context owns shared context, curation,
  ontology, search, UI, and its separate database. Agents continue to use only
  the authenticated read-only HTTP API.
- Preserve `/api/v1` routes and response schemas. The deliberate exception is
  the metadata `product` value, which changes from `centaur-os` to
  `centaur-context`; API and ontology versions remain `v1`.
- Merge verified code before the requester renames the public GitHub repository.
  Then update the local directory, `origin`, pinned external references,
  repository description (“Shared, durable context for Centaur users and
  agents”), the `Main_Cursor_OS.code-workspace` folder entry, and the Codex saved
  project label/path from `centaur_os`/`centaur-os` to
  `centaur_context`/`centaur-context`. Rely on GitHub redirects only as a
  temporary safety net, not as the documented integration.

## Checks

- [ ] A case-insensitive tracked/filesystem scan covers `Centaur OS`,
  `centaur-os`, `centaur_os`, `CENTAUR_OS`, `centaur_tool_centaur_os`, derived
  filenames, URLs, DNS names, and headings; every survivor is an allowlisted,
  tested legacy or historical reference.
- [ ] Tests cover new and legacy environment precedence/conflicts, database
  allowlists, backup metadata, API metadata, CLI/package names, Docker binary,
  Kubernetes selectors/DNS, install collision refusal, and rollback guidance.
- [ ] Fresh package/install dry runs and a disposable legacy-name upgrade prove
  that no data, migration checksum, permissions, or network boundary is lost.
- [ ] Centaur's focused Slack shared-context tests and the overlay's manifest,
  policy, tool-discovery, and fresh-sandbox checks pass with the new heading,
  CLI, host, grants, and pinned source.
- [ ] This repository passes `cargo fmt --check`,
  `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`,
  `npm --prefix web run type-check`, `npm --prefix web run build`,
  `python3 -m pytest tools/centaur_context/test_client.py`,
  `python3 -m compileall -q tools/centaur_context`, package-contract checks, and
  `git diff --check`.

## Approval Boundary

Execution is assigned for local code, documentation, compatibility, consumer
patches, verification, commits, and PRs. It does not authorize a GitHub rename,
local checkout move, publishing, package/image push, deployment, Secret
mutation, database/role rename, data copy, legacy-resource deletion, public
ingress, or changes to a live Centaur installation. Each external or destructive
cutover requires explicit requester approval after local verification and a
fresh validated backup.

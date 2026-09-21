# Reviewed maintenance of protected research records

Issue: #61. Implementation authorized by the repository owner.

## Existing authority and choice

The optional intake listener authenticates imports and can pin an import manifest,
but cannot correct or archive existing protected research records. The research
mutation listener provides only limited Source and Connection writes. Neither
satisfies this issue. Extend the existing intake listener with maintenance routes
that reuse universal apply's transaction, revisions, idempotency and Event/Run
engine. Do not add a new listener, parallel mutation implementation, or a database
credential to an agent.

## Authority

Maintenance is disabled by default. Require a distinct maintenance token and one
configured principal; the token does not work on import routes or ordinary agent
routes. Dry runs and paginated inventory are allowed for this principal. Commits
require the normalized request SHA-256 to appear in a server-configured allowlist.
Normalize only `validate_only` to false: IDs, order, revisions, values, contract,
Chat ID and idempotency key are covered. Return this digest from dry runs for
review. Check authorization before replay. An empty allowlist allows no commits.

Only existing research Objects can be corrected/archived, relationships can be
created/updated/archived, and supporting Artifacts can be appended. No Object
creation, immutable history edits, creator replacement, protection changes or
canonical Artifact promotion. Reject implicit Chat connections. CreateConnection
may reuse an identical existing edge but must never silently change an existing
edge without its ID and revision. System-managed Objects remain read-only;
protected Users may be relationship endpoints.

## Inventory and history

Use UUID keyset pagination over all Objects or all Connections, optionally active
or archived, with explicit next cursor. UUID is immutable and updates/archive do
not reorder pages. The inventory is complete over a quiescent corpus, not a
snapshot across concurrent insertions; pause writers for final reconciliation.
Reuse supported object readback for content, Artifacts and Events. Return prior
state alongside maintenance mutation results and retain it in the immutable Run
result for recovery review. Failed and validation-only batches roll back fully.

Archives retain records and history. This surface does not offer unarchive or an
automatic rollback. Active-record corrections can be compensated with another
explicitly approved revision-aware batch; archive reversal requires separate
supported owner recovery. Never represent history retention as full rollback.

## Verification and rollout

Synthetic integration tests cover token/principal isolation, no commit without
exact approval, dry-run no writes, atomic failure, stale revisions, replay,
protected corrections, ordinary-agent denials, preserved creator/protection,
archived inventory pagination, and readback. Run the repository checks. Deployment
and real record selection belong to the private owner workflow. Export affected
state, validate each batch, approve its digest, commit/replay/read back, then remove
maintenance configuration and credentials. No live data changes in this issue.

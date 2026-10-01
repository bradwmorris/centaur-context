# Proposed rollout and recovery for Issue 132

Status: plan only, 2 October 2026. No target is approved and no live action has
been performed. Keep PR #134 draft. Bradley must approve the exact final commit,
immutable image digest, deployment destination and maintenance window. An earlier
commit's approval/checks do not automatically cover a later revision.

## Proposed targets and approval record

1. First target: an isolated staging Context service and disposable database
   named `centaur_context_test_132_rollout`, restored from an approved backup or
   representative synthetic data. Production-derived backups remain private and
   must never enter this repository, CI artifacts, or an agent sandbox.
2. Second target, only after staging acceptance: the existing private Context
   service and its own Context database. Upgrade the Context server/UI, then the
   matching agent CLI/MCP contract packages consumed by that service. Do not
   upgrade Centaur's operational database, unrelated bots, or every installed
   bridge by implication. Each consumer installation must be named explicitly.
3. The private approval record must supply the actual host or Kubernetes context,
   namespace and workload, database identity/server/PostgreSQL version, service
   URLs, affected consumers, current and proposed image digests, backup location,
   restore destination, operator, outage budget, and rollback decision deadline.
   These have not been discovered or approved in this task. Repository deployment
   names are examples, not evidence of the live destination. No live command is
   executable from this plan until those fields and approval are present.

## Migration behavior and staging rehearsal

The application runs embedded SQLx migrations at startup, before serving. There
is no separate migration-only command in this change. Verify the actual migration
ledger/checksums and base schema (expected 35) before scheduling this upgrade;
private divergent histories require reconciliation and another rehearsal.

- Migration 36 adds `evidence_corrections`, its immutable-history trigger and
  index, and `objects.explicitly_corrected`. It takes DDL locks including an
  exclusive lock for the Object table alteration; it does not rewrite original
  evidence or unlock records by changing their protected flags.
- Migration 37 adds `objects.correction_text`, replaces the stored generated
  search column and its GIN index, and changes embedding invalidation triggers.
  Plan for an Object-table rewrite/computation and a non-concurrent index build,
  with strong table locks that block readers/writers. Disk/WAL and replica lag
  can increase. This is a maintenance-window migration, not a zero-downtime claim.
- Migrations are transactionally applied individually by SQLx. A failure in 37
  may leave 36 committed; inspect the ledger before retrying. Do not edit applied
  files, manually advance the ledger, or assume both migrations rolled back.

Rehearse the full backup/restore and startup sequence on staging at the actual
PostgreSQL/pgvector versions and representative data volume. Record database and
index sizes, free disk/WAL headroom, migration duration, lock waits, readiness
time and restore duration. Choose lock/statement timeouts and the outage budget
from that evidence (for example session settings via reviewed `PGOPTIONS` where
supported). Lock timeout means abort and reschedule, not terminate unrelated
sessions. Current synthetic tests used pgvector/pg17 and do not establish timing
or compatibility with a particular production store/version.

## Backup and approved cutover sequence

1. Record the old immutable image/configuration, API metadata, migration ledger,
   table counts and representative Object/Artifact hashes. Preserve external
   artifact files/object-store payloads referenced by database URIs separately;
   a database dump alone does not copy those bytes. Retain secret/configuration
   versions in the existing protected system without publishing credentials.
2. Pause all Context writers and producers (ingestion hooks, agents, scheduled
   workers and UI writes); stop old Context replicas. Verify no remaining writer
   sessions/long transactions. Retain upstream queues/checkpoints for later
   replay. Do not run old and new workloads against the same database.
3. With writes paused, take the final backup using the guarded
   [backup procedure](../../setup.md#backup), retaining dump, SHA-256 and JSON
   metadata together in protected storage. Use PostgreSQL clients compatible
   with the actual server; the supplied scripts specifically require version 16.
   If the actual server differs, resolve and rehearse the tooling first.
4. Restore that backup into the separately approved fresh target with the guarded
   [restore procedure](../../setup.md#restore). Verify original schema, counts,
   representative content, and old-image readiness there. A checksum alone is
   insufficient. Keep the production source unchanged during this rehearsal.
5. Start only the approved new Context image against the approved upgrade target.
   Keep consumers paused until startup migrations complete, schema reports 37,
   readiness succeeds, and row counts/original evidence match the baseline.
   Observe locks, logs, free space and the agreed deadline. Do not expose new
   public ingress or alter existing authentication boundaries.
6. Verify the packaged contract on each named consumer; perform representative
   revisioned correction, protected relationship, canonical promotion/citation
   impact, archive/restore and identity-replacement checks on designated test
   records through the deployed agent path. Read current state and retained
   history back. Verify stale/unauthorized requests fail, search reflects the
   correction, and embeddings progress using the configured model. Resume
   producers gradually and confirm replay preserves explicit corrections.
7. Record exact revision/digest, targets, backup metadata, verification results,
   and resume time privately. Only actual deployed acceptance can close the
   outstanding live-recovery requirement in #132.

## Rollback and post-cutover writes

Trigger rollback for failed migrations/readiness, integrity mismatches, correction
or authentication failures, or exceeding the agreed downtime budget. Stop new
workloads and all writers first. Preserve the failed database and logs for
inspection; do not drop correction history or run down-migrations.

The supported recovery is to restore the verified pre-upgrade backup into a fresh
confirmed Context database, start the previous immutable image against it, verify
its schema and representative reads, then reconnect only the approved consumers.
Restore matching consumer packages/configuration as needed. Use the guarded
restore target rules; a private database name outside those rules needs a
separately reviewed procedure, not bypassing the guard.

If no new writes were admitted, this returns to the quiesced backup checkpoint.
If writes were admitted, restoring that checkpoint loses those later changes:
keep both stores, inventory every post-checkpoint Event/Run/correction and upstream
capture, and obtain an explicit reconciliation decision before switching. Do not
silently discard corrections, replay external actions, or rewrite old audit rows.
Prefer a reviewed forward fix where reverting would lose accepted evidence.
Retain the failed store and backups until recovery and reconciliation are verified
and retention/disposal is separately authorized.

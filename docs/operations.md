# Operate Centaur Context 0.2.0

All database scripts refuse databases other than `centaur_context`, the legacy
`centaur_os`, or explicitly confirmed test names containing
`centaur_context_test` or `centaur_os_test`. None operates on Centaur's own
application databases.

For the 0.2 compatibility release, operations scripts accept legacy
`CENTAUR_OS_*` variables when the corresponding `CENTAUR_CONTEXT_*` value is
absent. Conflicting old and new values fail closed.

## Backup

Provide password-free `CENTAUR_CONTEXT_DATABASE_URL` and the separate
`CENTAUR_CONTEXT_DATABASE_PASSWORD` through a protected environment, then choose a
new output path:

```bash
./scripts/backup.sh /secure/path/centaur-context.dump
```

This produces a PostgreSQL custom-format dump, SHA-256 checksum, and small JSON
metadata file. Existing outputs are never overwritten.

## Restore

Prepare the target with the guarded administrator bootstrap. This creates the
database with the correct owner and installs pgvector; it does not alter an
existing app-role password:

```bash
./scripts/bootstrap-database.sh --database centaur_context_test_restore
```

Then provide password-free `CENTAUR_CONTEXT_RESTORE_DATABASE_URL`, the separate
`CENTAUR_CONTEXT_RESTORE_DATABASE_PASSWORD`, and confirm the exact connected
database name:

```bash
./scripts/restore.sh /secure/path/centaur-context.dump \
  --confirm-database centaur_context_test_restore
```

Restore is destructive to the confirmed target database. It validates the
checksum and metadata before mutation, preserves the administrator-owned
pgvector extension and prepared `public` schema, restores Centaur Context-owned
records in one transaction, and reports the restored migration version. Both
`centaur-context` and legacy `centaur-os` metadata are accepted. For a verified
legacy dump that predates the JSON sidecar, add
`--allow-legacy-without-metadata`; never use that flag for a new backup.

## Upgrade

1. Record `/api/v1/meta`, image identity, Object count, and current migration
   version.
2. Create and verify a backup.
3. Build or obtain the reviewed new image by immutable identity.
4. Run the new release's package checks.
5. Run `install-kubernetes.sh` with the new image identity.
6. Verify readiness, API/ontology compatibility, retained counts, context
   reads, ingestion, and one Curator Run.

For the product-name handoff, follow the ordered legacy procedure in the
[installation guide](installation.md). Never run the old and new Deployments at
the same time against one database.

Migrations are forward-only. Do not treat changing the container image as a
database rollback.

## Bounded bootstrap intake

The optional bootstrap listener exists for reviewed, one-time imports. It is
not a general agent write surface. It starts only when `INTAKE_API_TOKEN` is
set, binds to `INTAKE_ADDR` (default `0.0.0.0:8085`), and may be pinned to one
lowercase SHA-256 manifest with `INTAKE_APPROVED_MANIFEST_SHA256`. Use a token
that differs from every other Context credential. Keep the listener private;
do not add it to the normal Service, public ingress, or a sandbox.

The standard Python client exposes `validate_intake_batch`,
`commit_intake_batch`, and `intake_batch_status`. Requests require the intake
bearer token plus `X-Centaur-Principal-Id` and `X-Centaur-Thread-Key`. A batch
may contain at most 500 total resources and 12 MiB of JSON across canonical
Objects and their User, Entity, Source, or Note subtypes, external identities,
immutable Source content versions, and explained Connections. The server
validates all references and limits before writing anything.

Use the three endpoints in this order:

1. `POST /api/v1/intake/batches/validate` checks the entire batch and returns
   its deterministic ID map, payload hash, expected counts, and `writes: 0`.
2. `POST /api/v1/intake/batches/commit` writes the whole batch in one database
   transaction. Stable UUIDv5 IDs and Object Event idempotency keys make an
   exact retry a replay; a changed payload under the same batch ID fails.
3. `GET /api/v1/intake/batches/{batch_id}` reads the immutable Object Event
   ledger checkpoint. It does not depend on a new migration-ledger table.

For a destructive replacement, stop every destination writer, verify a backup,
reset only the explicitly confirmed Context database, and compare `/api/v1/schema`
before and after bootstrap. Validate first, commit once, replay once, and
reconcile Objects, subtypes, identities, Source hashes and current-content
pointers, Connections, protection flags, and Object Events against the private
manifest. Keep exports, payloads, credentials, ID maps, and reconciliation logs
outside Git. On any pre-cutover mismatch, return to the known-empty database and
rerun the complete batch; do not delete protected or immutable rows piecemeal.

After reconciliation, remove `INTAKE_API_TOKEN`,
`INTAKE_APPROVED_MANIFEST_SHA256`, and `INTAKE_ADDR`, restart the workload, and
prove port 8085 no longer accepts connections before normal writers resume.

## Evals and trace accounting

The trusted human listener exposes `/api/v1/evals` and the **Evals** UI. Agent,
Curator, and Slack-ingestion listeners do not expose eval reads or annotation.
Slack ingestion accepts bounded normalized `agent_usage` alongside the
interaction snapshot and associates it with the current interaction window;
retries are deduplicated by component, source execution, and source turn.

Object and Connection triggers attach every runtime mutation to the active Eval.
When a writer does not set an explicit transaction context, the database creates
an explicitly classified standalone human or system Eval rather than leaving an
untraced mutation. Migration 9 groups pre-existing Objects under one legacy
import Eval and does not fabricate historical usage or cost.

Treat prices as deployment-owned configuration. Metered estimates must carry a
versioned rate-card snapshot and are stored in integer micro-USD. ChatGPT
subscription or credit usage must display that basis without claiming a `$0`
per-trace bill. Missing usage, price, or credit data remains visibly incomplete.

## Rollback

For a name-handoff rollback, first scale `deployment/centaur-context` to zero,
restore Centaur consumer URLs and Secret references to the legacy names, then
scale `deployment/centaur-os` back up and rerun its smoke tests. If the database
must also roll back, stop both workloads, restore the pre-upgrade backup into a
fresh confirmed application database, reconnect the previous image, and verify
before removing the failed database. Never run a down-migration against
Centaur-owned data.

## Uninstall

Remove only the named workload and its own NetworkPolicy:

```bash
./scripts/uninstall-kubernetes.sh --confirm centaur-context
```

The Secret and database are retained by default for recovery. Add
`--delete-secret` only after credentials are safely retained or intentionally
retired. To remove the database separately, provide password-free
`CENTAUR_CONTEXT_ADMIN_DATABASE_URL` and `CENTAUR_CONTEXT_ADMIN_DATABASE_PASSWORD` for an
administrator that connects to another database, then run:

```bash
./scripts/drop-database.sh --confirm-database centaur_context \
  --drop-role centaur_context_app
```

After uninstall, compare the Centaur database inventory with the pre-install
record. Only `centaur_context`, `centaur_context_app`, and the explicitly named Centaur Context
Kubernetes resources may have been removed.

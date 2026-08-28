# Operate Centaur OS 0.1.0

All database scripts refuse databases other than `centaur_os` or an explicitly
confirmed name containing `centaur_os_test`. None operates on Centaur's own
application databases.

## Backup

Provide password-free `CENTAUR_OS_DATABASE_URL` and the separate
`CENTAUR_OS_DATABASE_PASSWORD` through a protected environment, then choose a
new output path:

```bash
./scripts/backup.sh /secure/path/centaur-os.dump
```

This produces a PostgreSQL custom-format dump, SHA-256 checksum, and small JSON
metadata file. Existing outputs are never overwritten.

## Restore

Prepare the target with the guarded administrator bootstrap. This creates the
database with the correct owner and installs pgvector; it does not alter an
existing app-role password:

```bash
./scripts/bootstrap-database.sh --database centaur_os_test_restore
```

Then provide password-free `CENTAUR_OS_RESTORE_DATABASE_URL`, the separate
`CENTAUR_OS_RESTORE_DATABASE_PASSWORD`, and confirm the exact connected
database name:

```bash
./scripts/restore.sh /secure/path/centaur-os.dump \
  --confirm-database centaur_os_test_restore
```

Restore is destructive to the confirmed target database. It validates the
checksum, preserves the administrator-owned pgvector extension and prepared
`public` schema, restores Centaur OS-owned records in one transaction, and
reports the restored migration version.

## Upgrade

1. Record `/api/v1/meta`, image identity, Object count, and current migration
   version.
2. Create and verify a backup.
3. Build or obtain the reviewed new image by immutable identity.
4. Run the new release's package checks.
5. Run `install-kubernetes.sh` with the new image identity.
6. Verify readiness, API/ontology compatibility, retained counts, context
   reads, ingestion, and one Curator Run.

Migrations are forward-only. Do not treat changing the container image as a
database rollback.

## Rollback

If the new release did not change the schema or data contract, reinstall the
previous immutable image and rerun its smoke tests. If the database must also
roll back, stop the workload, restore the pre-upgrade backup into a fresh
confirmed `centaur_os` database, reconnect the previous image, and verify
before removing the failed database. Never run a down-migration against
Centaur-owned data.

## Uninstall

Remove only the named workload and its own NetworkPolicy:

```bash
./scripts/uninstall-kubernetes.sh --confirm centaur-os
```

The Secret and database are retained by default for recovery. Add
`--delete-secret` only after credentials are safely retained or intentionally
retired. To remove the database separately, provide password-free
`CENTAUR_OS_ADMIN_DATABASE_URL` and `CENTAUR_OS_ADMIN_DATABASE_PASSWORD` for an
administrator that connects to another database, then run:

```bash
./scripts/drop-database.sh --confirm-database centaur_os \
  --drop-role centaur_os_app
```

After uninstall, compare the Centaur database inventory with the pre-install
record. Only `centaur_os`, `centaur_os_app`, and the explicitly named Centaur OS
Kubernetes resources may have been removed.

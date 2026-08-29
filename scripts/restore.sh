#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "$0")/common.sh"

require_command pg_restore
require_command psql
require_command awk
require_command python3
require_postgres_client_16 psql
require_postgres_client_16 pg_restore
resolve_legacy_env CENTAUR_CONTEXT_RESTORE_DATABASE_URL CENTAUR_OS_RESTORE_DATABASE_URL
resolve_legacy_env CENTAUR_CONTEXT_RESTORE_DATABASE_PASSWORD CENTAUR_OS_RESTORE_DATABASE_PASSWORD
require_env CENTAUR_CONTEXT_RESTORE_DATABASE_URL
require_password CENTAUR_CONTEXT_RESTORE_DATABASE_PASSWORD
require_passwordless_database_url "$CENTAUR_CONTEXT_RESTORE_DATABASE_URL"
[[ ($# -eq 3 || $# -eq 4) && "$2" == "--confirm-database" ]] || \
  die "usage: restore.sh BACKUP --confirm-database DATABASE [--allow-legacy-without-metadata]"
if [[ $# -eq 4 && "$4" != "--allow-legacy-without-metadata" ]]; then
  die "unexpected argument: $4"
fi

backup="$1"
expected="$3"
[[ -f "$backup" && -f "$backup.sha256" ]] || die "backup and checksum file are required"
actual="$(require_centaur_context_database "$CENTAUR_CONTEXT_RESTORE_DATABASE_URL" "$CENTAUR_CONTEXT_RESTORE_DATABASE_PASSWORD" "$expected")"
(cd "$(dirname "$backup")" && sha256_check "$(basename "$backup").sha256")
if [[ -f "$backup.json" ]]; then
  python3 "$(repository_root)/scripts/validate-backup-metadata.py" "$backup.json"
elif [[ $# -ne 4 ]]; then
  die "backup metadata is required; use --allow-legacy-without-metadata only for a verified legacy dump"
fi

temporary="$(mktemp -d "${TMPDIR:-/tmp}/centaur-context-restore.XXXXXX")"
trap 'rm -rf "$temporary"' EXIT
pg_restore --list "$backup" | awk '
  / SCHEMA - public / { next }
  / EXTENSION - / { next }
  / COMMENT - EXTENSION / { next }
  { print }
' >"$temporary/restore.list"
PGPASSWORD="$CENTAUR_CONTEXT_RESTORE_DATABASE_PASSWORD" \
  pg_restore --dbname="$CENTAUR_CONTEXT_RESTORE_DATABASE_URL" --clean --if-exists \
  --no-owner --no-privileges --single-transaction --exit-on-error \
  --use-list="$temporary/restore.list" "$backup"
schema="$(PGPASSWORD="$CENTAUR_CONTEXT_RESTORE_DATABASE_PASSWORD" psql "$CENTAUR_CONTEXT_RESTORE_DATABASE_URL" --no-psqlrc --tuples-only --no-align \
  --set=ON_ERROR_STOP=1 --command='SELECT COALESCE(max(version),0) FROM _sqlx_migrations')"
printf 'Restored %s at schema %s.\n' "$actual" "$schema"

#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "$0")/common.sh"

require_command pg_restore
require_command psql
require_command awk
require_postgres_client_16 psql
require_postgres_client_16 pg_restore
require_env CENTAUR_OS_RESTORE_DATABASE_URL
require_password CENTAUR_OS_RESTORE_DATABASE_PASSWORD
require_passwordless_database_url "$CENTAUR_OS_RESTORE_DATABASE_URL"
[[ $# -eq 3 && "$2" == "--confirm-database" ]] || \
  die "usage: restore.sh /path/to/backup.dump --confirm-database DATABASE"

backup="$1"
expected="$3"
[[ -f "$backup" && -f "$backup.sha256" ]] || die "backup and checksum file are required"
actual="$(require_centaur_os_database "$CENTAUR_OS_RESTORE_DATABASE_URL" "$CENTAUR_OS_RESTORE_DATABASE_PASSWORD" "$expected")"
(cd "$(dirname "$backup")" && sha256_check "$(basename "$backup").sha256")

temporary="$(mktemp -d "${TMPDIR:-/tmp}/centaur-os-restore.XXXXXX")"
trap 'rm -rf "$temporary"' EXIT
pg_restore --list "$backup" | awk '
  / SCHEMA - public / { next }
  / EXTENSION - / { next }
  / COMMENT - EXTENSION / { next }
  { print }
' >"$temporary/restore.list"
PGPASSWORD="$CENTAUR_OS_RESTORE_DATABASE_PASSWORD" \
  pg_restore --dbname="$CENTAUR_OS_RESTORE_DATABASE_URL" --clean --if-exists \
  --no-owner --no-privileges --single-transaction --exit-on-error \
  --use-list="$temporary/restore.list" "$backup"
schema="$(PGPASSWORD="$CENTAUR_OS_RESTORE_DATABASE_PASSWORD" psql "$CENTAUR_OS_RESTORE_DATABASE_URL" --no-psqlrc --tuples-only --no-align \
  --set=ON_ERROR_STOP=1 --command='SELECT COALESCE(max(version),0) FROM _sqlx_migrations')"
printf 'Restored %s at schema %s.\n' "$actual" "$schema"

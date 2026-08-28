#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "$0")/common.sh"

require_command psql
require_postgres_client_16 psql
require_env CENTAUR_OS_ADMIN_DATABASE_URL
require_password CENTAUR_OS_ADMIN_DATABASE_PASSWORD
require_passwordless_database_url "$CENTAUR_OS_ADMIN_DATABASE_URL"
[[ $# -ge 2 && "$1" == "--confirm-database" ]] || \
  die "usage: drop-database.sh --confirm-database DATABASE [--drop-role centaur_os_app]"
target="$2"
validate_identifier "$target"
[[ "$target" == "centaur_os" || "$target" == *centaur_os_test* ]] || \
  die "refusing to drop non-Centaur OS database $target"
admin_database="$(database_name "$CENTAUR_OS_ADMIN_DATABASE_URL" "$CENTAUR_OS_ADMIN_DATABASE_PASSWORD")"
[[ "$admin_database" != "$target" ]] || die "administrator URL must connect to a different database"

PGPASSWORD="$CENTAUR_OS_ADMIN_DATABASE_PASSWORD" psql "$CENTAUR_OS_ADMIN_DATABASE_URL" --no-psqlrc --set=ON_ERROR_STOP=1 \
  --command="DROP DATABASE IF EXISTS \"$target\" WITH (FORCE)"
if [[ $# -eq 4 ]]; then
  [[ "$3" == "--drop-role" && "$4" == "centaur_os_app" && "$target" == "centaur_os" ]] || \
    die "only --drop-role centaur_os_app is supported for the centaur_os database"
  PGPASSWORD="$CENTAUR_OS_ADMIN_DATABASE_PASSWORD" psql "$CENTAUR_OS_ADMIN_DATABASE_URL" --no-psqlrc --set=ON_ERROR_STOP=1 \
    --command='DROP ROLE IF EXISTS centaur_os_app'
elif [[ $# -ne 2 ]]; then
  die "unexpected arguments"
fi
printf 'Dropped database %s.\n' "$target"

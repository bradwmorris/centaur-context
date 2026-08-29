#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "$0")/common.sh"

require_command psql
require_postgres_client_16 psql
resolve_legacy_env CENTAUR_CONTEXT_ADMIN_DATABASE_URL CENTAUR_OS_ADMIN_DATABASE_URL
resolve_legacy_env CENTAUR_CONTEXT_ADMIN_DATABASE_PASSWORD CENTAUR_OS_ADMIN_DATABASE_PASSWORD
resolve_legacy_env CENTAUR_CONTEXT_APP_PASSWORD CENTAUR_OS_APP_PASSWORD
require_env CENTAUR_CONTEXT_ADMIN_DATABASE_URL
require_password CENTAUR_CONTEXT_ADMIN_DATABASE_PASSWORD
require_passwordless_database_url "$CENTAUR_CONTEXT_ADMIN_DATABASE_URL"
require_env CENTAUR_CONTEXT_APP_PASSWORD
[[ ${#CENTAUR_CONTEXT_APP_PASSWORD} -ge 32 ]] || die "CENTAUR_CONTEXT_APP_PASSWORD must be at least 32 characters"
database="centaur_context"
if [[ $# -eq 2 && "$1" == "--database" ]]; then
  database="$2"
elif [[ $# -ne 0 ]]; then
  die "usage: bootstrap-database.sh [--database centaur_context_test_NAME]"
fi
validate_identifier "$database"
[[ "$database" == "centaur_context" || "$database" == *centaur_context_test* ]] || \
  die "refusing to bootstrap non-Centaur Context database $database"

root="$(repository_root)"
export CENTAUR_CONTEXT_APP_PASSWORD
export CENTAUR_CONTEXT_DATABASE_NAME="$database"
PGPASSWORD="$CENTAUR_CONTEXT_ADMIN_DATABASE_PASSWORD" \
  psql "$CENTAUR_CONTEXT_ADMIN_DATABASE_URL" --no-psqlrc --set=ON_ERROR_STOP=1 \
  --file="$root/deploy/bootstrap-database.sql"

created="$(PGPASSWORD="$CENTAUR_CONTEXT_ADMIN_DATABASE_PASSWORD" psql "$CENTAUR_CONTEXT_ADMIN_DATABASE_URL" --no-psqlrc --tuples-only --no-align \
  --set=ON_ERROR_STOP=1 --command="SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname='$database')")"
[[ "$created" == "t" ]] || die "$database was not created"
printf 'Database %s and role centaur_context_app are ready.\n' "$database"

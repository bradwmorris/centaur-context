#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "$0")/common.sh"

require_command psql
require_postgres_client_16 psql
require_env CENTAUR_OS_ADMIN_DATABASE_URL
require_password CENTAUR_OS_ADMIN_DATABASE_PASSWORD
require_passwordless_database_url "$CENTAUR_OS_ADMIN_DATABASE_URL"
require_env CENTAUR_OS_APP_PASSWORD
[[ ${#CENTAUR_OS_APP_PASSWORD} -ge 32 ]] || die "CENTAUR_OS_APP_PASSWORD must be at least 32 characters"
database="centaur_os"
if [[ $# -eq 2 && "$1" == "--database" ]]; then
  database="$2"
elif [[ $# -ne 0 ]]; then
  die "usage: bootstrap-database.sh [--database centaur_os_test_NAME]"
fi
validate_identifier "$database"
[[ "$database" == "centaur_os" || "$database" == *centaur_os_test* ]] || \
  die "refusing to bootstrap non-Centaur OS database $database"

root="$(repository_root)"
export CENTAUR_OS_APP_PASSWORD
export CENTAUR_OS_DATABASE_NAME="$database"
PGPASSWORD="$CENTAUR_OS_ADMIN_DATABASE_PASSWORD" \
  psql "$CENTAUR_OS_ADMIN_DATABASE_URL" --no-psqlrc --set=ON_ERROR_STOP=1 \
  --file="$root/deploy/bootstrap-database.sql"

created="$(PGPASSWORD="$CENTAUR_OS_ADMIN_DATABASE_PASSWORD" psql "$CENTAUR_OS_ADMIN_DATABASE_URL" --no-psqlrc --tuples-only --no-align \
  --set=ON_ERROR_STOP=1 --command="SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname='$database')")"
[[ "$created" == "t" ]] || die "$database was not created"
printf 'Database %s and role centaur_os_app are ready.\n' "$database"

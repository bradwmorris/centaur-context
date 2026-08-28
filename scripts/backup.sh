#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "$0")/common.sh"

require_command pg_dump
require_command psql
require_postgres_client_16 psql
require_postgres_client_16 pg_dump
require_env CENTAUR_OS_DATABASE_URL
require_password CENTAUR_OS_DATABASE_PASSWORD
require_passwordless_database_url "$CENTAUR_OS_DATABASE_URL"
[[ $# -eq 1 ]] || die "usage: backup.sh /path/to/backup.dump"

output="$1"
[[ -d "$(dirname "$output")" ]] || die "backup directory does not exist"
[[ ! -e "$output" && ! -e "$output.sha256" && ! -e "$output.json" ]] || \
  die "backup output already exists"
database="$(require_centaur_os_database "$CENTAUR_OS_DATABASE_URL" "$CENTAUR_OS_DATABASE_PASSWORD")"
partial="${output}.partial.$$"
checksum_partial="${output}.sha256.partial.$$"
metadata_partial="${output}.json.partial.$$"
trap 'rm -f "$partial" "$checksum_partial" "$metadata_partial"' EXIT

PGPASSWORD="$CENTAUR_OS_DATABASE_PASSWORD" \
  pg_dump "$CENTAUR_OS_DATABASE_URL" --format=custom --no-owner --no-privileges \
  --file="$partial"
schema="$(PGPASSWORD="$CENTAUR_OS_DATABASE_PASSWORD" psql "$CENTAUR_OS_DATABASE_URL" --no-psqlrc --tuples-only --no-align \
  --set=ON_ERROR_STOP=1 --command='SELECT COALESCE(max(version),0) FROM _sqlx_migrations')"
checksum="$(sha256_value "$partial")"
printf '%s  %s\n' "$checksum" "$(basename "$output")" >"$checksum_partial"
printf '{"product":"centaur-os","product_version":"%s","database":"%s","schema_version":%s,"format":"pg_dump-custom"}\n' \
  "$CENTAUR_OS_VERSION" "$database" "$schema" >"$metadata_partial"
mv "$partial" "$output"
mv "$checksum_partial" "$output.sha256"
mv "$metadata_partial" "$output.json"
trap - EXIT
printf 'Backup written to %s\n' "$output"

#!/usr/bin/env bash

set -euo pipefail

CENTAUR_OS_VERSION="0.1.0"

die() {
  printf 'centaur-os: %s\n' "$*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "$1 is required"
}

require_postgres_client_16() {
  local command_name="$1"
  local version
  version="$($command_name --version | sed -E 's/.* ([0-9]+)(\.[0-9]+)?.*/\1/')"
  [[ "$version" =~ ^[0-9]+$ && "$version" -ge 16 ]] || \
    die "$command_name 16 or newer is required"
}

require_env() {
  local name="$1"
  [[ -n "${!name:-}" ]] || die "$name is required"
}

require_password() {
  local name="$1"
  local value
  require_env "$name"
  value="${!name}"
  [[ ${#value} -ge 32 ]] || die "$name must be at least 32 characters"
}

require_passwordless_database_url() {
  local url="$1"
  [[ "$url" == postgres://* || "$url" == postgresql://* ]] || \
    die "database URL must begin with postgres:// or postgresql://"
  local authority="${url#*://}"
  authority="${authority%%/*}"
  if [[ "$authority" == *"@"* ]]; then
    local userinfo="${authority%@*}"
    [[ "$userinfo" != *":"* ]] || \
      die "database URLs must not contain passwords; use the separate password environment variable"
  fi
}

sha256_value() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    die "shasum or sha256sum is required"
  fi
}

sha256_check() {
  local checksum_file="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 --check "$checksum_file"
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum --check "$checksum_file"
  else
    die "shasum or sha256sum is required"
  fi
}

validate_identifier() {
  [[ "$1" =~ ^[a-z][a-z0-9_]{0,62}$ ]] || die "invalid PostgreSQL identifier: $1"
}

database_name() {
  local url="$1"
  local password="$2"
  PGPASSWORD="$password" psql "$url" --no-psqlrc --tuples-only --no-align --set=ON_ERROR_STOP=1 \
    --command='SELECT current_database()'
}

require_centaur_os_database() {
  local url="$1"
  local password="$2"
  local expected="${3:-}"
  local actual
  actual="$(database_name "$url" "$password")"
  [[ "$actual" == "centaur_os" || "$actual" == *centaur_os_test* ]] || \
    die "refusing operation against unexpected database $actual"
  [[ -z "$expected" || "$actual" == "$expected" ]] || \
    die "database confirmation mismatch: expected $expected, connected to $actual"
  printf '%s' "$actual"
}

repository_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
}

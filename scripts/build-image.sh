#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "$0")/common.sh"

require_command docker
root="$(repository_root)"
image="${1:-centaur-context:${CENTAUR_CONTEXT_VERSION}}"
[[ "$image" =~ ^[A-Za-z0-9._/:@-]+$ ]] || die "invalid image reference"

docker build --pull=false --tag "$image" "$root"
digest="$(docker image inspect "$image" --format '{{index .RepoDigests 0}}' 2>/dev/null || true)"
if [[ -z "$digest" || "$digest" == "<no value>" ]]; then
  digest="$(docker image inspect "$image" --format '{{.Id}}')"
fi
printf 'Built %s\nIdentity: %s\n' "$image" "$digest"

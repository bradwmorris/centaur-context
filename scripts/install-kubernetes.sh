#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "$0")/common.sh"

require_command kubectl
require_env CENTAUR_OS_KUBE_CONTEXT
require_env CENTAUR_OS_NAMESPACE
[[ $# -eq 3 && "$1" == "--image" && "$3" == "--apply" ]] || \
  die "usage: install-kubernetes.sh --image IMAGE --apply"
image="$2"
[[ "$image" =~ ^[A-Za-z0-9._/:@-]+$ ]] || die "invalid image reference"
current="$(kubectl config current-context)"
[[ "$current" == "$CENTAUR_OS_KUBE_CONTEXT" ]] || \
  die "kubectl context mismatch: expected $CENTAUR_OS_KUBE_CONTEXT, current $current"
kubectl --context "$CENTAUR_OS_KUBE_CONTEXT" get namespace "$CENTAUR_OS_NAMESPACE" >/dev/null

for key in DATABASE_URL AGENT_API_TOKEN CHAT_INGEST_API_TOKEN CURATOR_API_TOKEN APPROVED_SLACK_SURFACES; do
  encoded="$(kubectl --context "$CENTAUR_OS_KUBE_CONTEXT" --namespace "$CENTAUR_OS_NAMESPACE" \
    get secret centaur-os-env --output="jsonpath={.data.${key}}")"
  [[ -n "$encoded" ]] || die "centaur-os-env is missing $key"
done

root="$(repository_root)"
temporary="$(mktemp -d "${TMPDIR:-/tmp}/centaur-os-install.XXXXXX")"
trap 'rm -rf "$temporary"' EXIT
sed "s|image: centaur-os:0.1.0|image: $image|" "$root/deploy/deployment.yaml" \
  >"$temporary/deployment.yaml"

kubectl --context "$CENTAUR_OS_KUBE_CONTEXT" --namespace "$CENTAUR_OS_NAMESPACE" \
  apply --filename "$root/deploy/service.yaml" \
  --filename "$root/deploy/network-policy.yaml" \
  --filename "$temporary/deployment.yaml"
kubectl --context "$CENTAUR_OS_KUBE_CONTEXT" --namespace "$CENTAUR_OS_NAMESPACE" \
  rollout status deployment/centaur-os --timeout=180s
printf 'Centaur OS %s installed in %s.\n' "$image" "$CENTAUR_OS_NAMESPACE"
printf 'Human UI: kubectl --context %s -n %s port-forward deploy/centaur-os 8080:8080\n' \
  "$CENTAUR_OS_KUBE_CONTEXT" "$CENTAUR_OS_NAMESPACE"

#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "$0")/common.sh"

require_command kubectl
resolve_legacy_env CENTAUR_CONTEXT_KUBE_CONTEXT CENTAUR_OS_KUBE_CONTEXT
resolve_legacy_env CENTAUR_CONTEXT_NAMESPACE CENTAUR_OS_NAMESPACE
require_env CENTAUR_CONTEXT_KUBE_CONTEXT
require_env CENTAUR_CONTEXT_NAMESPACE
[[ ($# -eq 3 || $# -eq 4) && "$1" == "--image" && "$3" == "--apply" ]] || \
  die "usage: install-kubernetes.sh --image IMAGE --apply [--legacy-cutover]"
legacy_cutover=false
if [[ $# -eq 4 ]]; then
  [[ "$4" == "--legacy-cutover" ]] || die "unexpected argument: $4"
  legacy_cutover=true
fi
image="$2"
[[ "$image" =~ ^[A-Za-z0-9._/:@-]+$ ]] || die "invalid image reference"
current="$(kubectl config current-context)"
[[ "$current" == "$CENTAUR_CONTEXT_KUBE_CONTEXT" ]] || \
  die "kubectl context mismatch: expected $CENTAUR_CONTEXT_KUBE_CONTEXT, current $current"
kubectl --context "$CENTAUR_CONTEXT_KUBE_CONTEXT" get namespace "$CENTAUR_CONTEXT_NAMESPACE" >/dev/null

legacy_deployment="$(kubectl --context "$CENTAUR_CONTEXT_KUBE_CONTEXT" --namespace "$CENTAUR_CONTEXT_NAMESPACE" \
  get deployment centaur-os --ignore-not-found --output=name)"
if [[ -n "$legacy_deployment" ]]; then
  [[ "$legacy_cutover" == true ]] || \
    die "legacy deployment/centaur-os exists; refuse parallel install without --legacy-cutover"
  legacy_replicas="$(kubectl --context "$CENTAUR_CONTEXT_KUBE_CONTEXT" --namespace "$CENTAUR_CONTEXT_NAMESPACE" \
    get deployment centaur-os --output='jsonpath={.spec.replicas}')"
  legacy_ready="$(kubectl --context "$CENTAUR_CONTEXT_KUBE_CONTEXT" --namespace "$CENTAUR_CONTEXT_NAMESPACE" \
    get deployment centaur-os --output='jsonpath={.status.readyReplicas}')"
  [[ "${legacy_replicas:-0}" == "0" && "${legacy_ready:-0}" == "0" ]] || \
    die "legacy deployment/centaur-os must be fully scaled to zero before cutover"
elif [[ "$legacy_cutover" == true ]]; then
  die "--legacy-cutover requires an existing legacy deployment/centaur-os"
fi

for key in DATABASE_URL AGENT_API_TOKEN NOTE_WRITE_API_TOKEN CHAT_INGEST_API_TOKEN CURATOR_API_TOKEN APPROVED_SLACK_SURFACES; do
  encoded="$(kubectl --context "$CENTAUR_CONTEXT_KUBE_CONTEXT" --namespace "$CENTAUR_CONTEXT_NAMESPACE" \
    get secret centaur-context-env --output="jsonpath={.data.${key}}")"
  [[ -n "$encoded" ]] || die "centaur-context-env is missing $key"
done

root="$(repository_root)"
temporary="$(mktemp -d "${TMPDIR:-/tmp}/centaur-context-install.XXXXXX")"
trap 'rm -rf "$temporary"' EXIT
sed "s|image: centaur-context:0.3.0|image: $image|" "$root/deploy/deployment.yaml" \
  >"$temporary/deployment.yaml"

kubectl --context "$CENTAUR_CONTEXT_KUBE_CONTEXT" --namespace "$CENTAUR_CONTEXT_NAMESPACE" \
  apply --filename "$root/deploy/service.yaml" \
  --filename "$root/deploy/network-policy.yaml" \
  --filename "$temporary/deployment.yaml"
kubectl --context "$CENTAUR_CONTEXT_KUBE_CONTEXT" --namespace "$CENTAUR_CONTEXT_NAMESPACE" \
  rollout status deployment/centaur-context --timeout=180s
printf 'Centaur Context %s installed in %s.\n' "$image" "$CENTAUR_CONTEXT_NAMESPACE"
printf 'Human UI: kubectl --context %s -n %s port-forward deploy/centaur-context 8080:8080\n' \
  "$CENTAUR_CONTEXT_KUBE_CONTEXT" "$CENTAUR_CONTEXT_NAMESPACE"

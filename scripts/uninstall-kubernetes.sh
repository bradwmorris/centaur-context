#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "$0")/common.sh"

require_command kubectl
resolve_legacy_env CENTAUR_CONTEXT_KUBE_CONTEXT CENTAUR_OS_KUBE_CONTEXT
resolve_legacy_env CENTAUR_CONTEXT_NAMESPACE CENTAUR_OS_NAMESPACE
require_env CENTAUR_CONTEXT_KUBE_CONTEXT
require_env CENTAUR_CONTEXT_NAMESPACE
[[ $# -ge 2 && "$1" == "--confirm" && "$2" == "centaur-context" ]] || \
  die "usage: uninstall-kubernetes.sh --confirm centaur-context [--delete-secret]"
current="$(kubectl config current-context)"
[[ "$current" == "$CENTAUR_CONTEXT_KUBE_CONTEXT" ]] || \
  die "kubectl context mismatch: expected $CENTAUR_CONTEXT_KUBE_CONTEXT, current $current"

kubectl --context "$CENTAUR_CONTEXT_KUBE_CONTEXT" --namespace "$CENTAUR_CONTEXT_NAMESPACE" delete \
  deployment/centaur-context service/centaur-context \
  networkpolicy/centaur-context \
  --ignore-not-found
if [[ $# -eq 3 ]]; then
  [[ "$3" == "--delete-secret" ]] || die "unexpected argument: $3"
  kubectl --context "$CENTAUR_CONTEXT_KUBE_CONTEXT" --namespace "$CENTAUR_CONTEXT_NAMESPACE" \
    delete secret/centaur-context-env --ignore-not-found
elif [[ $# -ne 2 ]]; then
  die "unexpected arguments"
fi
printf 'Removed Centaur Context Kubernetes resources from %s. Database retained.\n' \
  "$CENTAUR_CONTEXT_NAMESPACE"

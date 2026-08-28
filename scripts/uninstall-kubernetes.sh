#!/usr/bin/env bash

set -euo pipefail
source "$(dirname "$0")/common.sh"

require_command kubectl
require_env CENTAUR_OS_KUBE_CONTEXT
require_env CENTAUR_OS_NAMESPACE
[[ $# -ge 2 && "$1" == "--confirm" && "$2" == "centaur-os" ]] || \
  die "usage: uninstall-kubernetes.sh --confirm centaur-os [--delete-secret]"
current="$(kubectl config current-context)"
[[ "$current" == "$CENTAUR_OS_KUBE_CONTEXT" ]] || \
  die "kubectl context mismatch: expected $CENTAUR_OS_KUBE_CONTEXT, current $current"

kubectl --context "$CENTAUR_OS_KUBE_CONTEXT" --namespace "$CENTAUR_OS_NAMESPACE" delete \
  deployment/centaur-os service/centaur-os \
  networkpolicy/centaur-os \
  --ignore-not-found
if [[ $# -eq 3 ]]; then
  [[ "$3" == "--delete-secret" ]] || die "unexpected argument: $3"
  kubectl --context "$CENTAUR_OS_KUBE_CONTEXT" --namespace "$CENTAUR_OS_NAMESPACE" \
    delete secret/centaur-os-env --ignore-not-found
elif [[ $# -ne 2 ]]; then
  die "unexpected arguments"
fi
printf 'Removed Centaur OS Kubernetes resources from %s. Database retained.\n' \
  "$CENTAUR_OS_NAMESPACE"

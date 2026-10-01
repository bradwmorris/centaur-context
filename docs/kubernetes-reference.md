# Private Kubernetes reference candidate

This is one adaptable, **not yet end-to-end verified** combination for issue
[#131](https://github.com/bradwmorris/centaur-context/issues/131). Use an isolated
disposable cluster, never an existing private deployment. The
[setup guide](setup.md) still owns installation and secret handling.

## Candidate sources and topology

- Context starting revision: `675421f662c8319b54600242b1880aee566af152`, plus
  this reference change. Record the final build commit and image digest before
  testing; neither a tag nor this starting revision is a tested compatibility pin.
- Centaur source: `f44ce662f3b3fca63bd4c162add332cb43ec035c` from the public
  maintainer fork, chart `0.1.141`. Use its existing installation procedure with
  synthetic data and separately supplied test credentials.
- One namespace, `centaur`; Helm release `centaur`, `nameOverride: centaur`,
  `fullnameOverride: centaur`; one Slackbot instance. Context has its own
  Deployment and ClusterIP Services. api-rs and PostgreSQL services are
  `centaur-api-rs:8080` and `centaur-postgres:5432` under these overrides.
- PostgreSQL 16 with pgvector. The disposable server may also host disposable
  Centaur databases, but Context uses only `centaur_context_test_131`, role
  `centaur_context_app`, and its own credentials. Confirm pgvector availability
  before running `bootstrap-database.sh --database centaur_context_test_131`.
- Candidate transport: `centaur_subscription`, private api-rs route, exact
  `gpt-6-luna`; extraction Low, optional Memory maintenance High. The broker's
  purpose-bound key and reviewed subscription identity must be supplied through
  the trusted test secret mechanism. No credentials are in these examples.
- Candidate enforcement environment: single-node kind `v0.32.0`, Linux arm64,
  Kubernetes `v1.36.1`, with Calico Open Source `v3.32.2`. Create a fresh cluster
  with kind's default CNI disabled; follow the [Calico kind guide](https://docs.tigera.io/calico/latest/getting-started/kubernetes/kind)
  and verify its [host requirements](https://docs.tigera.io/calico/latest/getting-started/kubernetes/requirements).
  This CNI combination has not been exercised here. Do not replace a shared
  cluster's CNI to test it.

Keep the Kubernetes API and UI access on loopback, all Context Services private,
and ingress disabled. A real Slack round trip requires a separately approved
test Slack surface and existing delivery arrangement; this example creates no
public ingress. Synthetic HTTP requests alone cannot prove a real Slack turn.
Optional Codex, specialist workflow listeners, embeddings, Memory event capture,
and maintenance are outside the initial capture/retrieval candidate unless
explicitly included in its test receipt.

## Network and configuration agreement

Merge [`centaur-values.example.yaml`](../deploy/centaur-values.example.yaml)
into the disposable Centaur values. It is a fragment, not complete Centaur
configuration. Retain the chart's existing default-deny, DNS, Slack API, sandbox,
and control-plane rules. Review the rendered manifests from that exact source.

The Context installer applies only Context-owned
[`network-policy.yaml`](../deploy/network-policy.yaml). The three policies in
[`context-peers.example.yaml`](../deploy/context-peers.example.yaml) are opt-in
additions selecting Centaur-owned peers, applied separately by their operator.
They do not replace the Centaur chart's rules or get installed automatically.

| Path | Caller egress | Receiver ingress |
| --- | --- | --- |
| Slackbot → Context TCP 8081 (retrieval), 8082 (capture) | Peer example, Slackbot policy | Context policy |
| iron-proxy → Context TCP 8081 (search/read/apply) | Peer example, proxy policy | Context policy |
| Context → PostgreSQL TCP 5432 | Context policy | Peer example, PostgreSQL policy |
| Context → api-rs TCP 8080 (subscription inference) | Context policy | Centaur chart when `apiRs.curatorInference.enabled=true` |
| Context, Slackbot, proxy → cluster DNS UDP/TCP 53 | Context policy and Centaur chart DNS policy | Cluster's existing DNS policy must admit these clients |

The peer example assumes same-namespace pod selectors. Slackbot and PostgreSQL
match `app.kubernetes.io/name=centaur`, `app.kubernetes.io/instance=centaur`, and
component `slackbotv2` or `postgres`; api-rs uses component `api-rs`. Context
matches `app.kubernetes.io/name=centaur-context`; proxies match
`centaur.ai/iron-proxy=true`. Service selectors and actual pod labels must agree.
Different releases, multi-app component labels, or namespaces need adapted
policies on both sides. Across namespaces, combine the exact namespace and pod
selectors in the **same peer entry**; separate entries would widen access.
DNS here means the `kube-system` DNS service; NodeLocal DNS needs its own reviewed
address/path. An accepted policy is ineffective without an enforcing plugin.

Use `CENTAUR_CONTEXT_URL=http://centaur-context.centaur.svc.cluster.local:8081`
for the agent tools, matching their approved proxy host. Slackbot uses the two
URLs and separate Secret keys in the values fragment. Context's model endpoint
is `http://centaur-api-rs:8080/api/internal/context-curator/infer`. Match the key
in Context's Secret to the broker's purpose-bound key, not a general Centaur key.
The retained writer on 8084 is not needed for universal tools; only add proxy
egress to 8084 if a reviewed caller actually uses it. Other private listeners
need their own scoped configuration and policies.

NetworkPolicy rules are additive. Inspect all policies selecting each peer:
these additions cannot revoke a broader permission granted elsewhere. Network
access does not replace bearer authentication or separate database privileges.
The chart's [network policy source](https://github.com/bradwmorris/centaur/blob/f44ce662f3b3fca63bd4c162add332cb43ec035c/contrib/chart/templates/networkpolicy.yaml)
and [label helpers](https://github.com/bradwmorris/centaur/blob/f44ce662f3b3fca63bd4c162add332cb43ec035c/contrib/chart/templates/_helpers.tpl)
are the reference for these assumptions.

## Disposable validation

Use a dedicated kubeconfig so the installer's current-context guard and package
check target only the disposable API. Creating a cluster with `kind create
cluster --kubeconfig /private/test/kubeconfig ...` keeps the ambient config
unchanged. Select the disposable context explicitly:

```bash
export KUBECONFIG=/private/test/kubeconfig
export CENTAUR_CONTEXT_KUBE_CONTEXT=kind-centaur-context-131
export CENTAUR_CONTEXT_NAMESPACE=centaur
test "$(kubectl config current-context)" = "$CENTAUR_CONTEXT_KUBE_CONTEXT"
kubectl --context "$CENTAUR_CONTEXT_KUBE_CONTEXT" -n "$CENTAUR_CONTEXT_NAMESPACE" \
  get pods --show-labels
kubectl --context "$CENTAUR_CONTEXT_KUBE_CONTEXT" -n "$CENTAUR_CONTEXT_NAMESPACE" \
  get services,networkpolicies
./scripts/check-package.py
kubectl --context "$CENTAUR_CONTEXT_KUBE_CONTEXT" -n "$CENTAUR_CONTEXT_NAMESPACE" \
  apply --dry-run=server -f deploy/context-peers.example.yaml
```

Only after reviewing the rendered Centaur configuration, matching labels, and
test secrets should the disposable operator apply the peer example without
`--dry-run=server`, then follow Context's installer. Never apply this example to
an existing shared deployment as part of validation.

Before calling the pair tested, prove authenticated application requests through
the ClusterIP Services and DNS, plus denied requests from unrelated pods and
other namespaces to Context's agent, ingestion, UI, and Curator ports. Verify
that Slackbot cannot reach the writer/maintenance listeners. Test the proxy path
with the actual tools and secret injection, not only a pod with a proxy label.
Exercise capture, extraction, later retrieval, and ordinary Centaur responses
during a Context outage. Record the exact Context/Centaur commits, image digests,
CNI/configuration, outcomes, and optional features not exercised. HTTP 401 proves
reachability, not successful authenticated behavior; a server-side dry run proves
schema/admission acceptance, not traffic enforcement.

Step 5 needs sufficient spare Docker resources for Centaur plus Context, a
working enforcing CNI, isolated databases, and approved test Slack/model
credentials. Missing prerequisites must be reported; do not reuse live credentials
or stop shared workloads to complete this receipt.

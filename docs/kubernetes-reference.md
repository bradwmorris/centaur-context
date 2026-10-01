# Private Kubernetes reference candidate

This example puts service names, labels, and network paths in one place for an
existing Centaur operator. Adapt it to your deployment alongside the
[setup guide](setup.md); it does not install Centaur or certify a release pair.
Disposable network/API checks passed for this configuration. The
[evidence summary](verification/131/README.md) records their limits.

## Candidate sources and topology

- Context: build the reviewed checkout and record its commit and image identity.
  The evidence includes a source-linked build at
  `69e8c2f357383e084b955d3cff9613a60a633b1a`; it is not a paired compatibility pin.
- Centaur reference source: `f44ce662f3b3fca63bd4c162add332cb43ec035c` from the
  public maintainer fork, chart `0.1.141`. The examples were rendered against
  this source. Use Centaur's own installation procedure and review the
  [required hooks](centaur-integration.md#adapting-the-slack-integration).
- One namespace, `centaur`; Helm release `centaur`, `nameOverride: centaur`,
  `fullnameOverride: centaur`; one Slackbot instance. Context has its own
  Deployment and ClusterIP Services. api-rs and PostgreSQL services are
  `centaur-api-rs:8080` and `centaur-postgres:5432` under these overrides.
- PostgreSQL 16 with pgvector and a separate Context database and credentials.
  Normal bootstrap creates `centaur_context` and `centaur_context_app`.
  Disposable database checks instead used `centaur_context_test_131`.
- Optional model transport: `centaur_subscription`, private api-rs route, exact
  `gpt-6-luna`; extraction Low, optional Memory maintenance High. Leave the model
  settings unset to use capture, explicit writes, search, and the UI without it.
- NetworkPolicy needs an enforcing CNI. The component fixture used single-node
  kind `v0.32.0`, Linux arm64, Kubernetes `v1.36.1`, and Calico Open Source
  `v3.32.2`. Allowed and denied traffic checks passed. For a new disposable kind
  fixture, follow the [Calico kind guide](https://docs.tigera.io/calico/latest/getting-started/kubernetes/kind)
  and [host requirements](https://docs.tigera.io/calico/latest/getting-started/kubernetes/requirements).
  Adopters should use their cluster's reviewed enforcement setup; replacing an
  existing CNI is not part of installing Context.

Keep Context Services private and UI access on loopback. This example creates no
public ingress or Slack app: use your own existing Centaur delivery arrangement
and approved capture surfaces. Codex, embeddings, specialist workflow listeners,
event capture, and Memory maintenance are optional.

## Network and configuration agreement

Merge [`centaur-values.example.yaml`](../deploy/centaur-values.example.yaml)
into the reviewed Centaur values. It is a fragment, not complete Centaur
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

## Validation and evidence

Check actual pod labels, Service selectors, and both sides of the network table
before applying the examples. Select the exact Kubernetes context and namespace;
the package check contacts that API with a server-side dry run. To check a new
configuration without touching an existing cluster, use a dedicated disposable
kubeconfig:

```bash
export KUBECONFIG=/private/test/kubeconfig
export CENTAUR_CONTEXT_KUBE_CONTEXT=<exact-test-context>
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

The directory and context above are placeholders for an already prepared test
cluster; the commands do not create one. Apply peer policies through the Centaur
operator's normal reviewed procedure, then use Context's installer. A server-side
dry run checks admission, not traffic enforcement; HTTP 401 proves reachability,
not authenticated application behavior.

Completed component checks covered DNS, authenticated Context requests, blocked
unrelated/cross-namespace requests, synthetic capture/replay, explicit writes,
retrieval, and UI display. They used labeled probe pods rather than the actual
Slackbot and iron-proxy. Full Slack/model integration and outage behavior were
not certified. They are not completion gates for this documentation change;
check the capabilities you enable in your own deployment.

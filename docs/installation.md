# Install Centaur OS 0.1.0

This release supports one organization on a local machine or trusted private
network. It does not create public ingress and it does not modify a Centaur
application database.

## Prerequisites

- a healthy Centaur installation;
- PostgreSQL 16 with pgvector available;
- Docker, `kubectl`, and PostgreSQL 16-or-newer `psql`, `pg_dump`, and
  `pg_restore` clients;
- an explicit Kubernetes context and existing namespace;
- three distinct random API credentials of at least 32 characters; and
- one or more exact Slack workspace/channel pairs approved for ingestion.

Run the public package contract before installation:

```bash
./scripts/check-package.py
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
npm --prefix web run type-check
npm --prefix web run build
python3 -m pytest tools/centaur_os/test_client.py
```

## 1. Build an immutable local image

```bash
./scripts/build-image.sh centaur-os:0.1.0
```

The command prints the resulting image identity. Record that identity with the
installation. For a registry deployment, push through the operator's reviewed
release process and install by immutable digest, such as
`registry.example/centaur-os@sha256:...`.

## 2. Create the separate database and role

Supply these values through a protected environment or password manager; do
not place them in source control or command arguments:

```text
CENTAUR_OS_ADMIN_DATABASE_URL       password-free administrator URL to a maintenance database
CENTAUR_OS_ADMIN_DATABASE_PASSWORD  administrator password, at least 32 characters
CENTAUR_OS_APP_PASSWORD             new centaur_os_app password, at least 32 characters
```

Then run:

```bash
./scripts/bootstrap-database.sh
```

The script creates only the `centaur_os` database and `centaur_os_app` role. It
is create-only: rerunning it does not rotate an existing role password. A
reinstallation must reuse the retained credential or follow an independently
reviewed rotation procedure. Operator URLs containing an embedded password are
rejected; the separate password environment variable keeps secrets out of
process arguments.

## 3. Create the Centaur OS Secret

Use [`deploy/secret.example.yaml`](../deploy/secret.example.yaml) only as a key
reference. Never apply its placeholders. The Secret must be named
`centaur-os-env` and contain:

- `DATABASE_URL`;
- `AGENT_API_TOKEN`;
- `CHAT_INGEST_API_TOKEN`;
- `CURATOR_API_TOKEN`; and
- `APPROVED_SLACK_SURFACES`.

The database URL must use `centaur_os_app` and end in `/centaur_os`. Configure
Centaur's iron-proxy with the same agent token through Centaur's supported
secret mechanism; agent sandboxes receive only the proxy placeholder. Configure
the Slack post-response sink with the ingestion token. Neither credential is
stored in the public overlay tool package.

## 4. Install the workload

Set both safety gates:

```text
CENTAUR_OS_KUBE_CONTEXT  exact output expected from kubectl config current-context
CENTAUR_OS_NAMESPACE     existing namespace containing the Centaur installation
```

Then install a pinned image:

```bash
./scripts/install-kubernetes.sh --image centaur-os:0.1.0 --apply
```

The installer refuses a context mismatch, checks the required Secret keys,
applies only resources whose pod selector is `centaur-os`, and waits for
readiness. It never installs a policy that selects a Centaur-owned pod. The
base NetworkPolicy permits PostgreSQL, DNS, iron-proxy reads, and approved
Slack ingestion at the Centaur OS boundary. If the Centaur installation has
its own default-deny policies, add the matching outbound allowances through
that installation's supported configuration. Centaur OS does not permit
public model-provider egress. If embeddings or an automatic Curator model are
enabled, replace the documentation range in
[`deploy/provider-egress.example.yaml`](../deploy/provider-egress.example.yaml)
with the exact reviewed provider network before applying it.

## 5. Install the read-only agent tool

Configure Centaur's supported overlay source mechanism to load this release's
`tools` directory at the same pinned Git commit as the application. The tool
offers only `get-context`, `search-objects`, and `read-object`; the agent API
enforces the same restriction independently.

## 6. Verify the installation

Open the human UI through localhost only:

```bash
kubectl --context "$CENTAUR_OS_KUBE_CONTEXT" \
  --namespace "$CENTAUR_OS_NAMESPACE" \
  port-forward deployment/centaur-os 8080:8080
```

Check `http://127.0.0.1:8080/api/v1/meta` and `/readyz`. Then prove one human
write, one attributed sandbox context read, and one separately attributed
Curator Run. Keep the before-install Centaur database inventory and confirm it
is unchanged.

# Install Centaur OS

This installs Centaur OS beside Centaur. It does not use or change Centaur's
database.

## Requirements

- A working Centaur installation
- PostgreSQL 16 with pgvector
- Docker, `kubectl`, `psql`, `pg_dump`, and `pg_restore`
- An existing Kubernetes namespace
- Three different random API tokens, each at least 32 characters
- The Slack workspace and channel IDs Centaur OS may accept

## 1. Check and build

```bash
./scripts/check-package.py
./scripts/build-image.sh centaur-os:0.1.0
```

Record the image identity printed by the build.

## 2. Create the database

Set these values outside source control:

```text
CENTAUR_OS_ADMIN_DATABASE_URL
CENTAUR_OS_ADMIN_DATABASE_PASSWORD
CENTAUR_OS_APP_PASSWORD
```

The admin URL must connect to a maintenance database and must not contain a
password. Then run:

```bash
./scripts/bootstrap-database.sh
```

This creates the `centaur_os` database and `centaur_os_app` role.

## 3. Create the Kubernetes Secret

Copy the keys from [`deploy/secret.example.yaml`](../deploy/secret.example.yaml)
into a protected Secret named `centaur-os-env`:

```text
DATABASE_URL
AGENT_API_TOKEN
CHAT_INGEST_API_TOKEN
CURATOR_API_TOKEN
APPROVED_SLACK_SURFACES
```

Use `workspace_id:channel_id` for each approved Slack surface. Separate several
surfaces with commas.

Do not apply the example placeholders.

For automatic curation, also add:

```text
CURATOR_MODEL_API_URL
CURATOR_MODEL_API_TOKEN
CURATOR_MODEL
CURATOR_PROMPT_VERSION
```

Without these values, Curator Runs stay queued. If the model provider is
outside the cluster, review
[`deploy/provider-egress.example.yaml`](../deploy/provider-egress.example.yaml)
before allowing that traffic.

## 4. Install

```bash
export CENTAUR_OS_KUBE_CONTEXT=<exact-kubectl-context>
export CENTAUR_OS_NAMESPACE=<existing-centaur-namespace>
./scripts/install-kubernetes.sh --image centaur-os:0.1.0 --apply
```

The installer checks the context, Secret, rollout, and resource boundaries.

## 5. Open the UI

```bash
kubectl --context "$CENTAUR_OS_KUBE_CONTEXT" \
  --namespace "$CENTAUR_OS_NAMESPACE" \
  port-forward deployment/centaur-os 8080:8080
```

Open [http://127.0.0.1:8080](http://127.0.0.1:8080). Check `/readyz` and
`/api/v1/meta`.

## 6. Connect Centaur

- Load this release's `tools` directory through Centaur's overlay mechanism.
- Give iron-proxy `AGENT_API_TOKEN`. Do not give it to the sandbox.
- Configure Slack ingestion and context injection using the
  [Slack guide](slack-integration.md).

## 7. Prove the loop

1. Send a Slack interaction on an approved surface.
2. Reply `done` or `finished`, or wait 10 minutes.
3. Confirm the Chat, Memory, and Curator Run appear in the UI.
4. Start a new interaction and confirm the agent receives the saved context.

For backup, restore, upgrade, and removal, use the
[operations guide](operations.md).

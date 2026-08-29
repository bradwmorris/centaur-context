# Install Centaur Context

This installs Centaur Context beside Centaur. It does not use or change Centaur's
database.

## Requirements

- A working Centaur installation
- PostgreSQL 16 with pgvector
- Docker, `kubectl`, `psql`, `pg_dump`, and `pg_restore`
- An existing Kubernetes namespace
- Four different random API tokens, each at least 32 characters
- The Slack workspace and channel IDs Centaur Context may accept

## 1. Check and build

```bash
./scripts/check-package.py
./scripts/build-image.sh centaur-context:0.2.0
```

Record the image identity printed by the build.

## 2. Create the database

Set these values outside source control:

```text
CENTAUR_CONTEXT_ADMIN_DATABASE_URL
CENTAUR_CONTEXT_ADMIN_DATABASE_PASSWORD
CENTAUR_CONTEXT_APP_PASSWORD
```

The admin URL must connect to a maintenance database and must not contain a
password. Then run:

```bash
./scripts/bootstrap-database.sh
```

This creates the `centaur_context` database and `centaur_context_app` role.

## 3. Create the Kubernetes Secret

Copy the keys from [`deploy/secret.example.yaml`](../deploy/secret.example.yaml)
into a protected Secret named `centaur-context-env`:

```text
DATABASE_URL
AGENT_API_TOKEN
NOTE_WRITE_API_TOKEN
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

`TEXT_SEARCH_CONFIG` defaults to the language-neutral `simple` configuration.
Installers may select `dutch`, `english`, `french`, `german`, `italian`,
`portuguese`, or `spanish`. Embedding providers use one shared request shape by
default; set `EMBEDDING_INPUT_MODE=typed` only when the provider supports the
`search_document` and `search_query` input types.

## 4. Install

```bash
export CENTAUR_CONTEXT_KUBE_CONTEXT=<exact-kubectl-context>
export CENTAUR_CONTEXT_NAMESPACE=<existing-centaur-namespace>
./scripts/install-kubernetes.sh --image centaur-context:0.2.0 --apply
```

The installer checks the context, Secret, rollout, and resource boundaries.

### Upgrade an existing Centaur OS installation

Do not rename or recreate its database or role. Create `centaur-context-env`
with the retained values, including the existing `centaur_os` `DATABASE_URL`,
then take and validate a backup. Scale `deployment/centaur-os` to zero and run:

```bash
./scripts/install-kubernetes.sh --image centaur-context:0.2.0 --apply --legacy-cutover
```

The installer refuses this handoff while the legacy Deployment has any desired
or ready replicas. After the new workload is ready, switch Centaur consumers to
`http://centaur-context`, verify context reads and ingestion, and retain the old
scaled-down resources for rollback. Do not delete them during the rename.

## 5. Open the UI

```bash
kubectl --context "$CENTAUR_CONTEXT_KUBE_CONTEXT" \
  --namespace "$CENTAUR_CONTEXT_NAMESPACE" \
  port-forward deployment/centaur-context 8080:8080
```

Open [http://127.0.0.1:8080](http://127.0.0.1:8080). Check `/readyz` and
`/api/v1/meta`.

## 6. Connect Centaur

- Load this release's `tools` directory through Centaur's overlay mechanism.
- Give iron-proxy `AGENT_API_TOKEN`. Do not give it to the sandbox.
- Give the separately authorized Note-writing tool `NOTE_WRITE_API_TOKEN` and
  route it to `centaur-context-note-write:8084`; the separate service hostname
  prevents ambiguity with the read credential. Do not reuse the read token or
  expose either credential to the sandbox.
- Configure Slack ingestion and context injection using the
  [Slack guide](slack-integration.md).

## 7. Prove the loop

1. Send a Slack interaction on an approved surface.
2. Reply `done` or `finished`, or wait 10 minutes.
3. Confirm the Chat, Memory, and Curator Run appear in the UI.
4. Start a new interaction and confirm the agent receives the saved context.

For backup, restore, upgrade, and removal, use the
[operations guide](operations.md).

# Setup and operations

This installs Centaur Context beside Centaur. It does not use or change Centaur's
database.

This is not a Centaur installation guide. Begin with an existing Centaur
deployment created and verified using Centaur's official
[Quickstart](https://centaur.run/quickstart). Every command below installs,
configures, verifies, upgrades, or removes Centaur Context only.

> **Current installation model:** Centaur's proposed App system is not
> implemented in production, so Context is installed manually as a companion
> service today. The complete Slack loop also needs two optional hooks currently
> carried in the maintainer's Centaur fork. Read the
> [Centaur integration contract](centaur-integration.md) before choosing or
> modifying a Centaur revision. Do not treat the historical fork commits as a
> supported installation until [`compatibility.toml`](../compatibility.toml)
> records a current, tested Centaur revision.

## Requirements

- A working compatible Centaur installation, set up using Centaur's own
  documentation
- PostgreSQL 16 with pgvector
- Docker, `kubectl`, `psql`, `pg_dump`, and `pg_restore`
- `kind` when the existing Centaur installation runs in a kind cluster
- An existing Kubernetes namespace
- Four different random API tokens, each at least 32 characters
- The Slack workspace and channel IDs Centaur Context may accept

## 1. Check and build

```bash
./scripts/check-package.py
./scripts/build-image.sh centaur-context:0.3.0
```

Record the image identity printed by the build.

Building places the image in the local Docker image store. If the Kubernetes
cluster does not share that image store, make the image available to the cluster
before installation. For a kind-based Centaur installation, load it into the
exact existing cluster (the kind cluster name normally omits the `kind-` prefix
shown by `kubectl config current-context`):

```bash
kind load docker-image centaur-context:0.3.0 --name <exact-kind-cluster-name>
```

For other Kubernetes environments, push the image to an accessible registry and
use that immutable image reference during installation. Do not continue with a
tag that the cluster cannot pull or resolve.

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
CURATOR_MODEL_TRANSPORT
CURATOR_MODEL_TIMEOUT_SECONDS
```

Use `centaur_subscription` (the default) with Centaur's private inference URL,
the purpose-bound `CENTAUR_CONTEXT_API_KEY`, and exact model `gpt-5.6-luna`.
Use `direct_api` only for rollback. Without these values, curator Runs stay
queued. If the direct model provider is outside the cluster, review
[`deploy/provider-egress.example.yaml`](../deploy/provider-egress.example.yaml)
before allowing that traffic.

For rollback, set `CURATOR_MODEL_TRANSPORT=direct_api` and replace the URL,
token, and model together. Do not reuse the Centaur Context bearer token as a
provider API key. Queued or failed Curator runs retain their normal bounded
retry behavior; an in-flight subscription request is allowed to fail rather
than being replayed across billing modes.

`TEXT_SEARCH_CONFIG` defaults to the language-neutral `simple` configuration.
Installers may select `dutch`, `english`, `french`, `german`, `italian`,
`portuguese`, or `spanish`. Embedding providers use one shared request shape by
default; set `EMBEDDING_INPUT_MODE=typed` only when the provider supports the
`search_document` and `search_query` input types.

Embeddings are optional. The smallest recommended hosted setup is
`EMBEDDING_API_URL=https://api.openai.com/v1/embeddings`,
`EMBEDDING_MODEL=text-embedding-3-small`, `EMBEDDING_DIMENSIONS=1536`, and
`EMBEDDING_INPUT_MODE=shared`, with a purpose-bound provider key in
`EMBEDDING_API_TOKEN`. Add the URL, token, model, and dimensions together;
partial configuration fails startup. Enabling these values sends all Object
kind/title/description summaries and only forward-created, semantically enabled
Artifact chunks to that provider. Historical Artifacts remain lexical-only. This
may incur cost, so it requires an explicit privacy and spend decision. Without
these values, Artifact and Object full-text search remains available.

## 4. Install

```bash
export CENTAUR_CONTEXT_KUBE_CONTEXT=<exact-kubectl-context>
export CENTAUR_CONTEXT_NAMESPACE=<existing-centaur-namespace>
./scripts/install-kubernetes.sh --image centaur-context:0.3.0 --apply
```

The installer checks the context, Secret, rollout, and resource boundaries.

### Upgrade an existing Centaur OS installation

This clean public repository is the supported starting point for new public
installations. Pre-public or private deployments may have a different migration
history. Do not point one of those deployments at this repository without first
reconciling its migration history and testing the change against a disposable
copy of its database.

Do not rename or recreate its database or role. Create `centaur-context-env`
with the retained values, including the existing `centaur_os` `DATABASE_URL`,
then take and validate a backup.

If the pre-public database uses an organization-specific name such as
`centaur_context_example`, explicitly confirm that connected database when
backing it up:

```bash
./scripts/backup.sh /secure/path/centaur-context.dump \
  --confirm-database centaur_context_example
```

The confirmation is accepted only for a valid `centaur_context_*` or
`centaur_os_*` name and must match the connected database exactly.

Then scale `deployment/centaur-os` to zero and run:

```bash
./scripts/install-kubernetes.sh --image centaur-context:0.3.0 --apply --legacy-cutover
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
`/api/v2/meta`.

## 6. Connect Centaur

This section describes the Context endpoints and credentials that an existing
Centaur deployment must call. It does not install or operate Centaur. Follow
Centaur's documentation for applying configuration to that deployment.

- Load this release's `tools` directory through Centaur's overlay mechanism.
- Give iron-proxy `AGENT_API_TOKEN`. Do not give it to the sandbox.
- Give the separately authorized Note-writing tool `NOTE_WRITE_API_TOKEN` and
  route it to `centaur-context-note-write:8084`; the separate service hostname
  prevents ambiguity with the read credential. Do not reuse the read token or
  expose either credential to the sandbox.
- Configure Slack ingestion and context injection using the
  settings below.

Centaur Context does not connect to Slack directly. Centaur needs two optional
integration hooks: one sends completed interactions to Centaur Context, and the
other gets context before an agent replies.

The tested Centaur integration is this three-commit patch set:

- `225a6104` adds `slackbotv2.interactionSink`.
- `d8a7dfc2` makes that integration portable across Centaur builds.
- `33e7cd59` adds `slackbotv2.contextBuilder`.

These commits modify Centaur, not Centaur Context. They are currently additions to a
Centaur fork and are not part of Paradigm's upstream Centaur repository. Pin a
Centaur revision containing all three.

```yaml
slackbotv2:
  interactionSink:
    url: http://centaur-context:8082/api/v2/ingest/slack/interactions
    timeoutMs: 5000
    secretName: centaur-context-env
    secretKey: CHAT_INGEST_API_TOKEN
    usage:
      provider: openai
      authMode: unknown
      billingMode: unknown
      upstreamService: unknown

  contextBuilder:
    url: http://centaur-context:8081/api/v2/context
    timeoutMs: 1500
    limit: 10
    secretName: centaur-context-env
    secretKey: AGENT_API_TOKEN
```

The Slack transport needs network access to port `8082`. Iron-proxy needs
access to port `8081`. Agent sandboxes receive neither real token.

Centaur Context checks the bearer token and the exact Slack workspace/channel pair.
Rejected surfaces are not stored.

The standard client lives in [`tools/centaur_context`](../tools/centaur_context).
Use `get-context`, `search-objects`, and `read-object` for basic retrieval.
Context requests must carry the canonical Chat ID and matching
`X-Centaur-Thread-Key`; general search does not require a Chat.

## 7. Prove the loop

1. Send a Slack interaction on an approved surface.
2. Reply `done` or `finished`, or wait 10 minutes.
3. Confirm the Chat and completed Curator Run appear in the UI. A Memory is
   created only when the conversation contains durable information.
4. Start a new interaction and confirm the agent receives the saved context.

If Slack does not reply, check Centaur's Slack transport. If it replies without
context, check the context URL, token, NetworkPolicy, and Curator Run. Queued
Curator Runs usually warrant checking the model settings above.

## Maintenance

Database scripts only operate on Context's canonical or legacy database names,
explicitly confirmed Context test databases, or the narrowly confirmed private
backup case below. Never target Centaur's databases.

### Backup

Provide password-free `CENTAUR_CONTEXT_DATABASE_URL` and the separate
`CENTAUR_CONTEXT_DATABASE_PASSWORD` through a protected environment, then choose a
new output path:

```bash
./scripts/backup.sh /secure/path/centaur-context.dump
```

Canonical `centaur_context`, legacy `centaur_os`, and guarded test databases do
not need an extra argument. For an existing private/pre-public database named
`centaur_context_*` or `centaur_os_*`, add
`--confirm-database <exact-connected-name>`. This exception is backup-only and
does not authorize restoring over or dropping that database.

This produces a PostgreSQL custom-format dump of Context's owned `public`
schema, SHA-256 checksum, and small JSON metadata file. Extension-owned schemas
and Centaur databases are excluded. Existing outputs are never overwritten.

### Restore

Prepare the target with the guarded administrator bootstrap. This creates the
database with the correct owner and installs pgvector; it does not alter an
existing app-role password:

```bash
./scripts/bootstrap-database.sh --database centaur_context_test_restore
```

Then provide password-free `CENTAUR_CONTEXT_RESTORE_DATABASE_URL`, the separate
`CENTAUR_CONTEXT_RESTORE_DATABASE_PASSWORD`, and confirm the exact connected
database name:

```bash
./scripts/restore.sh /secure/path/centaur-context.dump \
  --confirm-database centaur_context_test_restore
```

Restore is destructive to the confirmed target database. It validates the
checksum and metadata before mutation, preserves the administrator-owned
pgvector extension and prepared `public` schema, restores Centaur Context-owned
records in one transaction, and reports the restored migration version. Both
`centaur-context` and legacy `centaur-os` metadata are accepted. For a verified
legacy dump that predates the JSON sidecar, add
`--allow-legacy-without-metadata`; never use that flag for a new backup.

### Upgrade

1. Record `/api/v2/meta`, image identity, Object count, and current migration
   version.
2. Create and verify a backup.
3. Build or obtain the reviewed new image by immutable identity.
4. Run the new release's package checks.
5. Run `install-kubernetes.sh` with the new image identity.
6. Verify readiness, API/ontology compatibility, retained counts, context
   reads, ingestion, and one Curator Run.

For the product-name handoff, follow the
[legacy installation procedure](#upgrade-an-existing-centaur-os-installation).
Never run the old and new Deployments against one database at the same time.

Migrations are forward-only. Do not treat changing the container image as a
database rollback.

### Rollback

For a name-handoff rollback, first scale `deployment/centaur-context` to zero,
restore Centaur consumer URLs and Secret references to the legacy names, then
scale `deployment/centaur-os` back up and rerun its smoke tests. If the database
must also roll back, stop both workloads, restore the pre-upgrade backup into a
fresh confirmed application database, reconnect the previous image, and verify
before removing the failed database. Never run a down-migration against
Centaur-owned data.

### Uninstall

Remove only the named workload and its own NetworkPolicy:

```bash
./scripts/uninstall-kubernetes.sh --confirm centaur-context
```

The Secret and database are retained by default for recovery. Add
`--delete-secret` only after credentials are safely retained or intentionally
retired. To remove the database separately, provide password-free
`CENTAUR_CONTEXT_ADMIN_DATABASE_URL` and `CENTAUR_CONTEXT_ADMIN_DATABASE_PASSWORD` for an
administrator that connects to another database, then run:

```bash
./scripts/drop-database.sh --confirm-database centaur_context \
  --drop-role centaur_context_app
```

After uninstall, compare the Centaur database inventory with the pre-install
record. Only `centaur_context`, `centaur_context_app`, and the explicitly named Centaur Context
Kubernetes resources may have been removed.

## Optional capabilities

For one-time imports, embedding rollout, and trace accounting, see
[advanced operations](operations.md). The
[Secret template](../deploy/secret.example.yaml) documents available service settings;
the [agent client](../tools/centaur_context/client.py) and
[CLI](../tools/centaur_context/cli.py) define the tool contract.

General reads use `CENTAUR_CONTEXT_API_TOKEN`. Note creation requires the separate
`CENTAUR_CONTEXT_NOTE_WRITE_TOKEN`, a per-operation idempotency key, and the private
`centaur-context-note-write:8084` service. It never falls back to the read token.
Theme creation is human-controlled; authorized agents can assign existing Themes.
Optional Source-intake and External-action services are disabled unless their
own credentials and allowed principals are configured. Source intake requires
`SOURCE_INTAKE_API_TOKEN` and `SOURCE_INTAKE_ALLOWED_PRINCIPAL`; research
mutation requires `RESEARCH_MUTATION_API_TOKEN` and
`RESEARCH_MUTATION_ALLOWED_PRINCIPAL`; external actions require
`EXTERNAL_ACTION_API_TOKEN` and `EXTERNAL_ACTION_ALLOWED_PRINCIPALS`. Keep each
organization's chosen principals and workflow instructions in its private overlay.

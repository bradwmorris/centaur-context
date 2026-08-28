# Centaur OS

Centaur OS is a small local-first shared context and operations application for
Centaur users and agents.

Its ontology contains:

- canonical Objects;
- explained Connections;
- one-to-one Task, Chat, User, Entity, and event-shaped Memory subtype records;
- immutable Object Events.

Every Object has one primary type and a mandatory concise description. The MVP
Connection vocabulary is `involves`, `about`, `related_to`, `depends_on`, and
`derived_from`; each Connection also requires a plain-language description.

The Rust service owns validation and PostgreSQL access. The React/TypeScript UI
is compiled to static assets and served by the same process.

The repository also owns the standard `centaur-os` agent tool under
`tools/centaur_os`. Centaur installs that constrained client through its normal
overlay source mechanism. An organization may load a separate private overlay
for its own agents, prompts, workflows, and policies; those customizations do
not own or duplicate the standard Centaur OS tool.

## Trust boundaries

- Human UI/API: port `8080`, reached only with a localhost port-forward.
- Agent API: port `8081`, exposed by the ClusterIP Service and requiring a
  bearer credential injected by Centaur iron-proxy.
- Chat ingestion API: port `8082`, reachable only from an approved chat
  transport and protected by a different bearer credential. The server also
  checks the exact Slack workspace/channel pair against its allowlist.
- Database: logical database `centaur_os`, accessed only by role
  `centaur_os_app`.
- The service refuses migrations unless the current database is `centaur_os`
  or its name contains `centaur_os_test`.

There is no public ingress and no agent database access.

## Slack interaction ingestion

The MVP accepts a provider-neutral transcript envelope at
`POST /api/v1/ingest/slack/interactions` on the ingestion listener. Centaur's
Slack transport is the producer; Centaur OS never reads Centaur's private
session tables.

For each approved Slack thread, Centaur OS:

- creates or reuses one canonical Chat Object;
- creates or reuses canonical human and agent User Objects by Slack identity;
- stores ordered Chat Messages idempotently by Slack message ID;
- creates one explained `involves` Connection per participant; and
- queues only the not-yet-queued message range for the later Curator.

`interaction_finished: true` queues the range immediately. Otherwise the
background worker queues it after ten minutes without a new message. A later
message in the same Slack thread reuses the Chat and begins a new range. This
phase creates durable Curator queue records; it does not yet run a model or
write Memories, Tasks, Entities, or additional Connections.

The required configuration is:

```text
CHAT_INGEST_API_TOKEN=<a distinct secret of at least 32 characters>
APPROVED_SLACK_SURFACES=<workspace_id>:<channel_id>[,...]
INTERACTION_INACTIVITY_SECONDS=600
```

## Context Builder and search

Centaur OS always indexes the canonical Object title and description with
PostgreSQL full-text search. The read-only Context Builder combines those
matches with semantic matches when an embedding provider is configured, then
adds immediately connected Objects and concise Connection explanations. It
returns no more than ten Objects. Connection count is a small importance signal
only in Context Builder ranking; plain Object search does not use it.

Object embeddings are rebuildable search data, not canonical business data.
Writes to an Object automatically queue its current title and description for
asynchronous embedding. Search falls back to full text if embeddings are
disabled, incomplete, stale, or temporarily unavailable.

Centaur OS has no default model provider. To enable any OpenAI-compatible
embedding endpoint, provide all four values:

```text
EMBEDDING_API_URL=https://provider.example/v1/embeddings
EMBEDDING_API_TOKEN=<provider credential>
EMBEDDING_MODEL=<provider model name>
EMBEDDING_DIMENSIONS=<integer from 1 through 2000>
```

`EMBEDDING_POLL_SECONDS` optionally controls the single background worker and
defaults to five seconds. The configured dimensions receive a pgvector HNSW
cosine index. Operators using an in-cluster or private-network provider must
adapt the supplied NetworkPolicy to that exact destination.

## Standard agent tool

The public tool package is deliberately kept with the API contract it calls:

```text
tools/centaur_os/
  client.py
  cli.py
  pyproject.toml
  test_client.py
```

It supports exactly three read operations: `get-context`, `search-objects`, and
`read-object`. It has no write, SQL, generic request, or deletion command and no
application business logic. The agent listener exposes only those read routes;
the restriction is therefore enforced even if a sandbox bypasses the CLI. The
sandbox receives an iron-proxy placeholder and never receives the real Centaur
OS API credential. Human management writes remain available only on the human
listener.

For local verification:

```bash
python3 -m venv .venv-tool
.venv-tool/bin/pip install 'tools/centaur_os[test]'
.venv-tool/bin/pytest tools/centaur_os/test_client.py
.venv-tool/bin/centaur-os --help
```

Centaur overlay configuration should eventually load this repository's
`tools` subdirectory at a pinned commit. Do not change the retained installation
from the legacy overlay source until this repository has a reviewed commit and
the new source has passed fresh-sandbox verification.

## Local development

Install and build the browser bundle:

```bash
npm --prefix web ci
npm --prefix web run build
```

Start the service with a disposable or approved `centaur_os` database:

```bash
# Set both API tokens outside source control to distinct secrets of at least 32 characters.
DATABASE_URL=postgres://.../centaur_os \
APPROVED_SLACK_SURFACES=T_EXAMPLE:C_EXAMPLE \
cargo run
```

Open `http://127.0.0.1:8080`. Vite development mode is optional and proxies
API requests to the human listener.

## Verification

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
npm --prefix web run type-check
npm --prefix web run build
npm --prefix web audit --audit-level=high
python3 -m pytest tools/centaur_os/test_client.py
python3 -m compileall -q tools/centaur_os
```

To run the real schema contract, set `TEST_DATABASE_URL` to a disposable
database whose name contains `centaur_os_test`. The test validates the
canonical object/subtype contract, idempotent creation, optimistic revision
conflicts, Connections, Tasks, Slack transcript replay, continuation windows,
inactivity queueing, full-text and vector retrieval, one-hop context expansion,
the ten-Object cap, rebuildable embedding jobs, and audit events.

## Deployment gate

The files in `deploy/` are reviewable manifests, not authorization to apply
them. Before local deployment:

1. verify the Kubernetes context is `kind-centaur-lab`;
2. verify free disk remains above 15 GiB;
3. create and validate a fresh PostgreSQL backup;
4. create the separate database and least-privilege role;
5. create only the named secret keys required by the Deployment;
6. build and pin the local image;
7. apply the manifests and verify both NetworkPolicy paths.

Do not create a public tunnel or push this repository without Brad's explicit
approval.

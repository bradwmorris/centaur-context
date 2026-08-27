# Centaur OS

Centaur OS is a small local-first shared context and operations application for
Centaur users and agents.

Its ontology contains:

- canonical Objects;
- explained Connections;
- one-to-one Task, Chat, Entity, and Memory subtype records;
- immutable Object Events.

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
- Database: logical database `centaur_os`, accessed only by role
  `centaur_os_app`.
- The service refuses migrations unless the current database is `centaur_os`
  or its name contains `centaur_os_test`.

There is no public ingress and no agent database access.

## Standard agent tool

The public tool package is deliberately kept with the API contract it calls:

```text
tools/centaur_os/
  client.py
  cli.py
  pyproject.toml
  test_client.py
```

It supports constrained Object, Connection, and Task operations. It has no SQL
client, generic request command, deletion command, or application business
logic. The sandbox receives an iron-proxy placeholder; it never receives the
real Centaur OS API credential.

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
# Set AGENT_API_TOKEN outside source control to a secret of at least 32 characters.
DATABASE_URL=postgres://.../centaur_os \
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
conflicts, Connections, Tasks, and audit events.

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

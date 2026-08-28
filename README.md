# Centaur OS

Centaur OS gives Centaur agents shared, durable business context.

It stores tasks, chats, users, entities, memories, and the connections between
them. Humans use the web UI. Agents use a small read-only tool.

Centaur OS does not replace Centaur. It adds a context layer beside it.

## What it does

- Keeps one canonical record for each important thing.
- Turns completed Slack interactions into structured context.
- Gives agents a short, relevant context packet before they reply.
- Keeps source messages, revisions, and audit history.
- Includes a simple UI for people to inspect and manage the context.

## How it works

1. Centaur sends Slack thread updates to Centaur OS.
2. Centaur OS stores the chat, messages, and participants.
3. After an explicit finish or 10 minutes of inactivity, the Curator runs.
4. The Curator creates or updates Objects, Memories, Tasks, and Connections.
5. Before the next reply, the Context Builder returns up to 10 relevant Objects.

The Slack agent reads context. It does not write inferred context. The separate
Curator handles those writes after the interaction ends.

## What belongs where

| Part | Owns |
| --- | --- |
| Centaur | Agents, Slack, sandboxes, model execution |
| Centaur OS | Shared context, ontology, search, UI, agent tool |
| Private overlay | Company prompts, workflows, policies, integrations |

Keep Centaur OS generic. Put company-specific behavior in a private overlay.
Extend Centaur OS only when the feature would help many Centaur users.

## Core rules

- Every first-class node has one canonical Object row.
- Centaur OS uses its own PostgreSQL database and role.
- Agents never receive database credentials.
- The agent API is read-only and requires a bearer token.
- The Curator uses a separate write credential.
- No public ingress is included.

See [Ontology](docs/ontology.md) for the data model.

## Start

1. [Install Centaur OS](docs/installation.md).
2. [Connect Slack](docs/slack-integration.md).
3. Keep the [operations guide](docs/operations.md) with the installation record.

The standard agent tool is in `tools/centaur_os`. It provides:

- `get-context`
- `search-objects`
- `read-object`

## Status

Version `0.1.0` is a single-organization MVP for a local machine or trusted
private network. The supported contract is in
[`compatibility.toml`](compatibility.toml).

## Development

```bash
./scripts/check-package.py
cargo test
npm --prefix web run type-check
npm --prefix web run build
python3 -m pytest tools/centaur_os/test_client.py
```

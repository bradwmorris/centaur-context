# Centaur Context

Centaur Context gives Centaur agents shared, durable business context.

It stores tasks, chats, users, entities, memories, and the connections between
them. Humans use the web UI. Agents use a small read-only tool.

Centaur Context does not replace Centaur. It adds a context layer beside it.

## What it does

- Keeps one canonical record for each important thing.
- Turns completed Slack interactions into structured context.
- Gives agents a short, relevant context packet before they reply.
- Keeps source messages, revisions, and audit history.
- Includes a simple UI for people to inspect and manage the context.

## How it works

1. Centaur sends Slack thread updates to Centaur Context.
2. Centaur Context stores the chat, messages, and participants.
3. After an explicit finish or 10 minutes of inactivity, the Curator runs.
4. The Curator creates or updates Objects, Memories, Tasks, and Connections.
5. Before the next reply, the Context Builder returns up to 10 relevant Objects.

The Slack agent reads context. It does not write inferred context. The separate
Curator handles those writes after the interaction ends.

## What belongs where

| Part | Owns |
| --- | --- |
| Centaur | Agents, Slack, sandboxes, model execution |
| Centaur Context | Shared context, ontology, search, UI, agent tool |
| Private overlay | Company prompts, workflows, policies, integrations |

Keep Centaur Context generic. Put company-specific behavior in a private overlay.
Extend Centaur Context only when the feature would help many Centaur users.

## Core rules

- Every first-class node has one canonical Object row.
- Centaur Context uses its own PostgreSQL database and role.
- Agents never receive database credentials.
- The agent API is read-only and requires a bearer token.
- The Curator uses a separate write credential.
- No public ingress is included.

See [Ontology](docs/ontology.md) for the data model.

## Start

1. [Install Centaur Context](docs/installation.md).
2. [Connect Slack](docs/slack-integration.md).
3. Keep the [operations guide](docs/operations.md) with the installation record.

The standard agent tool is in `tools/centaur_context`. It provides:

- `get-context`
- `search-objects`
- `read-object`
- `search-sources` (bounded metadata and content excerpts)
- `read-source` (metadata without long-form content)
- `read-source-content` (a bounded window from one content version)
- `search-notes` (bounded Note excerpts)
- `read-note` (one Note and its content)
- `create-note` (requires the separate `CENTAUR_CONTEXT_NOTE_WRITE_TOKEN`)
- `source-intake-validate`, `source-intake-commit`, and `source-intake-status`
  (permanent Enyu workflow only; requires its separate Source-intake token)
- `list-themes`, `read-theme`, and `list-theme-objects`
- `propose-theme` and `read-theme-proposal`, plus `assign-theme` and
  `unassign-theme` (requires the separate Theme listener token)

All reads use `CENTAUR_CONTEXT_API_TOKEN`. Note creation never falls back to
that read token: configure the separately scoped
`CENTAUR_CONTEXT_NOTE_WRITE_TOKEN` and supply an idempotency key for each
logical Note creation. The write client defaults to the private Note-write
service `centaur-context-note-write` on port `8084`; its distinct hostname keeps
credential substitution unambiguous. Override it with
`CENTAUR_CONTEXT_NOTE_WRITE_URL` when needed.

The optional permanent Source-intake listener is disabled unless
`SOURCE_INTAKE_API_TOKEN` is configured. It defaults to port `8086`, accepts
only `workflow-enyu-source-ingestion`, and is distinct from the temporary
bootstrap intake listener and credential. The tool uses
`CENTAUR_CONTEXT_SOURCE_INTAKE_URL` and
`CENTAUR_CONTEXT_SOURCE_INTAKE_TOKEN`; neither falls back to another Context
credential.

The optional Theme listener is disabled unless `THEME_PROPOSAL_API_TOKEN` is
configured. It defaults to port `8087` and uses
`CENTAUR_CONTEXT_THEME_PROPOSAL_URL` and
`CENTAUR_CONTEXT_THEME_PROPOSAL_TOKEN`. Agents can assign or unassign existing
approved Themes and submit new Theme proposals, but cannot approve proposals.
Approval requires the `approve_themes` permission on the human API; schema 12
grants it only to the initial local human administrator identity (`local-human`).

## Status

Version `0.2.0` is a single-organization MVP for a local machine or trusted
private network. The supported contract is in
[`compatibility.toml`](compatibility.toml).

## License

Centaur Context uses the [MIT License](LICENSE). It runs alongside
[Centaur](https://github.com/paradigmxyz/centaur), which is separate software
with its own license.

## Development

```bash
./scripts/check-package.py
cargo test
npm --prefix web run type-check
npm --prefix web run build
python3 -m pytest tools/centaur_context/test_client.py
python3 -m pytest scripts/test_rename_contract.py
```

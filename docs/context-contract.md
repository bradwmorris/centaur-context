# Context contract 1.1.0

The canonical machine-readable contract is
[`contract/context-contract.json`](../contract/context-contract.json). It is
validated by [`contract/context-contract.schema.json`](../contract/context-contract.schema.json).
The deterministic short agent explanation is checked at
[`generated/context-agent-instructions.md`](../generated/context-agent-instructions.md)
and published at Centaur's generic repository-overlay path
[`services/sandbox/SYSTEM_PROMPT.md`](../services/sandbox/SYSTEM_PROMPT.md).

## Agent tools

- `context_search` finds canonical Objects.
- `context_read` reads complete typed Objects and bounded supporting data.
- `context_apply` validates and commits one atomic, idempotent write batch.

## Writable records

Normal interactive agents may create, update, and archive Tasks, Entities,
Sources, Notes, and Themes, and may manage explained Connections. Chats, Users,
Memories, Object Events, and Runs are system-managed. Supporting Artifacts may
be appended, but only the specialist Source-ingestion path may promote an
Artifact as a Source's canonical captured content.

New Objects require a meaningful Connection except Insight and Question Notes,
which may stand alone without a Source, Connection, or originating Chat. Later
Connections can be added when justified; never invent one to save an idea.
Excerpts retain their evidence and connectivity requirements. A verified current
Chat is connected deterministically as provenance in the same transaction.
The contract's `standalone_note_intents` lists the exceptions to
`new_objects_require_connection`.

## Object descriptions

An Object description is a concise current snapshot, not a running log. It
directly identifies the Object and states why it matters in the current Context
when that relevance is not already explicit. It uses only justified facts,
remains understandable without the originating Chat, and stays within 600
Unicode characters after trimming. Prefer one or two direct sentences.

Agents review affected Objects after material discoveries or mutations and use
the existing `update_object` operation with `expected_revision` when durable
meaning or relevance has changed. They do not rewrite descriptions for minor
field changes, duplicate Connections, or timestamps. Immutable Events and Runs
retain the history.

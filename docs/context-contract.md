# Context contract 1.0.0

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

Every new Object requires a meaningful Connection. A verified current Chat is
connected deterministically as provenance in the same transaction.

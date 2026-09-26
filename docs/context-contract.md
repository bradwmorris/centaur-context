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
Memories, Object Events, and Runs are system-managed. Protected records reject
ordinary updates. A protected Note may be archived after a complete
`research_notes` Artifact on its active derived-from Source preserves the exact
current Note content and has a nonempty document key plus a matching
`source_note_manifest` object ID and revision. Archive every incident protected
Connection earlier in the same atomic `context_apply` batch; its immutable
archive Event is retained. Other protected archival remains denied. Supporting
Artifacts may be appended, but only the specialist Source-ingestion path may
promote an Artifact as a Source's canonical captured content.

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

## Actionable Tasks

`owner_object_id` is the assigned canonical User (human or agent), separate from
immutable Object creator attribution. New Tasks require an active User assignee;
agent/system creation requires `due_at`. `work_kind` is `general` or `code`;
code work requires a canonical `https://github.com/owner/repo/issues/number` URL.
Every supplied issue URL is validated across write paths. Existing records remain
readable and unrelated edits remain possible; repair missing assignment, date and
brief before agent execution. Never guess historical creator/owner/date values.

Use `brief_markdown` for outcome, acceptance, capability/permission assessment,
next action, dependencies, human inputs, ETA and evidence. Universal read includes
the full brief and `execution_actor_id`. The UI labels Assigned to separately
from Created by; a missing identity never falls back to a participant. Creator
principals without a verified User mapping remain visibly attributed to the
actual authenticated principal, rather than being guessed from the assignee.

For a deterministic queue, `context_search --task-owner USER_ID --ready --limit 20`
uses priority (high first), due date (earliest first, undated last), then Object ID.
Optional `--task-status`, `--task-priority`, `--task-due-before` and `--task-cursor`
filter before limiting. The API's `task_filters` accepts `owner_object_id`,
`statuses`, `priority`, `due_before` (RFC3339), `ready`, and `cursor`. Empty query is
allowed only for filtered task queries. `ready` means assigned, dated,
agent-suitable backlog/todo work with a nonempty brief and no incomplete Task
dependency. It is not a permission or completeness judgment: read the brief and
all dependencies. Follow `next_cursor` promptly; if the queue changes, restart
selection rather than treating a cursor as a durable work assignment.

Claim by updating `status` to `doing` at the read revision. A repeated explicit
`doing` update fails; a concurrent update from the same revision conflicts.
The authenticated execution actor is recorded and another agent cannot alter an
active claimed Task. Humans can explicitly hand off through a non-doing state.
A blocked/review Task may resume through the existing revision-aware transition;
do not infer abandonment from elapsed time. Completing work must record evidence
and `completed_at`, clear any `blocked_reason`, and read back the resulting Task.
The API checks state integrity, not the truth of an agent's evidence.

Roll out consumers and the migration together. Old clients that omit new Task
requirements will receive a clear validation error and must collect the missing
information; there is no fabricated assignee/date fallback. The networking
workflow-only listener now includes owner, due date and brief in its exact replay
comparison. Rollback may restore a prior binary for unrelated reads, but writers
must retain the required fields; do not remove constraints or overwrite history
to make an incompatible producer succeed.

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

In Centaur, all three use the agent listener and `AGENT_API_TOKEN`, exposed as
the proxy-injected `CENTAUR_CONTEXT_API_TOKEN`. Principal and thread headers are
required; apply also requires an `Idempotency-Key` header matching the body's
`idempotency_key`. No separate Note/Task writer token is needed for universal
writes. See [API authentication](api.md#authentication) and
[tool setup](setup.md#optional-agent-tools) for the transport configuration.

## Writable records

Authenticated agents can create, update, archive and restore every Object kind,
and manage all explained Connections regardless of `protected`. The flag is a
background-curation preference. Explicit updates retain locked before/after
snapshots with authenticated attribution. User metadata cannot grant credentials.
Imported identity keys remain evidence; annotate mistakes and use replacement
Objects/relinked current relationships instead of forging original authorship.

Artifacts remain immutable. Append a superseding version and use
`promote_source_artifact` with the Source revision and exact SHA-256 to select
current evidence. Its result lists Notes retaining older citations; their exact
Artifact IDs remain valid. To move a quotation, explicitly update the Note and
validate its content against the new Artifact. `correct_evidence` appends a
reason and representation for an Object, Artifact, message, Event or Run. All
assertions are returned by `context_read`; message reads show original content
and the latest correction separately. A later assertion supersedes the earlier
correction; an empty representation withdraws it. `rebuild_derived` requeues
existing embedding work; normal workers discover new targets. Lexical indexes
are transactionally maintained. See [RD 132](rd/132-agent-corrections.md).

New Objects require a meaningful Connection except Idea and Fact Notes,
which may stand alone without a Source, Connection, or originating Chat. Later
Connections can be added when justified; never invent one to save an idea.
Excerpts retain their evidence and connectivity requirements. A verified current
Chat is connected deterministically as provenance in the same transaction.
The contract's `standalone_note_intents` lists the exceptions to
`new_objects_require_connection`, including retained legacy `insight` and
`question` values. New Note creation accepts `idea`, `excerpt`, and `fact`;
legacy values remain readable and accepted on updates, without automatic
conversion. See [Note intents and compatibility](schema.md#note-intents-and-compatibility).

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
Human creation may omit `due_at`, but still requires an active User assignee and
an Issue for code work. Once a due date is set, it cannot be cleared. Entering
`doing` requires a nonempty `brief_markdown`; agent execution also requires the
assignee and due date. The assignee is a User Object ID, not a principal string.
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

## Metadata quality and explicit Note evidence

Ordinary lexical and semantic discovery uses Object kind, title and description;
body-only words in Notes, messages or Artifacts intentionally do not create
matches. Requested kinds filter candidates before limits and hybrid ranking.
Titles identify concrete subjects. Descriptions add concise, supported facts;
Entity identity belongs in metadata and Source participation on Connections.
The contract's examples cover every kind, including system-managed Objects.

`context_read` returns Note intent, source Artifact and locator in `subtype`, plus
an initial `note_content` window of at most 8,000 Unicode characters. Read the
rest with `note_windows: [{"object_id":"…","offset":8000,"limit":20000}]`.
The Object must also appear in `object_ids`. Each window reports offsets, total
characters, `truncated`, `next_offset` and whether it is the complete body.
Offsets count Unicode characters, not bytes. The CLI exposes `--note-window`.
Research documents are working `research_notes` Artifacts attached to Sources;
Notes remain canonical Idea/Excerpt/Fact Objects. Stored keys and versions do not change.

Authenticated audit clients can use `GET /api/v2/audit/objects` and
`GET /api/v2/audit/connections` with `limit` (1–200), UUID `cursor`, and optional
`lifecycle=active|archived`. Follow `next_cursor` until null. These routes reuse
the maintenance inventory reader on the agent/session-bound listener without
granting maintenance writes. Inventory includes metadata/provenance, not original
Note bodies or transcripts; use explicit reads for selected evidence. Record
the collection interval because concurrent writes can change the live inventory.

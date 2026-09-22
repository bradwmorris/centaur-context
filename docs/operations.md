# Advanced Context operations

Reference for optional imports, indexing rollout, and trace accounting. Start with
[Setup and operations](setup.md) for the standard installation.

## Bounded bootstrap intake

The optional bootstrap listener exists for reviewed, one-time imports. It is
not a general agent write surface. It starts only when `INTAKE_API_TOKEN` is
set, binds to `INTAKE_ADDR` (default `0.0.0.0:8085`), and may be pinned to one
lowercase SHA-256 manifest with `INTAKE_APPROVED_MANIFEST_SHA256`. Use a token
that differs from every other Context credential. Keep the listener private;
do not add it to the normal Service, public ingress, or a sandbox.

The standard Python client exposes `validate_intake_batch`,
`commit_intake_batch`, and `intake_batch_status`. Requests require the intake
bearer token plus `X-Centaur-Principal-Id` and `X-Centaur-Thread-Key`. A batch
may contain at most 500 total resources and 12 MiB of JSON across canonical
Objects and their User, Entity, Source, or Note subtypes, embedded User
identities, immutable Artifacts, and explained Connections. The server
validates all references and limits before writing anything.

Use the three endpoints in this order:

1. `POST /api/v2/intake/batches/validate` checks the entire batch and returns
   its deterministic ID map, payload hash, expected counts, and `writes: 0`.
2. `POST /api/v2/intake/batches/commit` writes the whole batch in one database
   transaction. Stable UUIDv5 IDs and Object Event idempotency keys make an
   exact retry a replay; a changed payload under the same batch ID fails.
3. `GET /api/v2/intake/batches/{batch_id}` reads the immutable Object Event
   ledger checkpoint. It does not depend on a new migration-ledger table.

For a destructive replacement, stop every destination writer, verify a backup,
reset only the explicitly confirmed Context database, and compare `/api/v2/schema`
before and after bootstrap. Validate first, commit once, replay once, and
reconcile Objects, subtypes, identities, Source hashes and current-content
pointers, Connections, protection flags, and Object Events against the private
manifest. Keep exports, payloads, credentials, ID maps, and reconciliation logs
outside Git. On any pre-cutover mismatch, return to the known-empty database and
rerun the complete batch; do not delete protected or immutable rows piecemeal.

After reconciliation, remove `INTAKE_API_TOKEN`,
`INTAKE_APPROVED_MANIFEST_SHA256`, and `INTAKE_ADDR`, restart the workload, and
prove port 8085 no longer accepts connections before normal writers resume.

## Runs, trace accounting, and mutation history

The trusted human listener exposes `/api/v2/runs` and the single **Runs** UI.
Agent, Curator, and Slack-ingestion listeners do not expose review reads or annotation.
Slack ingestion accepts bounded normalized `agent_usage` alongside the
interaction snapshot and associates it with the current interaction window;
retries are deduplicated by component, source execution, and source turn.

Every runtime Object and Connection mutation writes exactly one authoritative
`object_events` entry owned by a Run. Standalone human/system mutations create a
small `mutation` Run. Execution steps and usage live in `runs.trace`; inputs in
`runs.input`; terminal output and scores in `runs.result`. Undo writes a
compensating child Run and never duplicates or rewrites history.

Treat prices as deployment-owned configuration. Metered estimates must carry a
versioned rate-card snapshot and are stored in integer micro-USD. ChatGPT
subscription or credit usage must display that basis without claiming a `$0`
per-trace bill. Missing usage, price, or credit data remains visibly incomplete.

## Purpose-bound networking mutations

Port 8089 is an optional private workflow listener for Entity networking. It is
disabled unless its separate token and one exact allowed principal are
configured. Do not expose it through public ingress or give its token to an
interactive agent. The deployment that enables it must allow only the trusted
credential proxy or workflow adapter to reach the port and must constrain
credential injection to `GET /api/v2/objects`, `GET /api/v2/objects/*`, and
`POST` on `/api/v2/objects`, `/api/v2/connections`, and `/api/v2/tasks`.

The listener does not prove that a human approved a proposed Connection or
Task. The durable workflow owns that approval evidence and preserves it in the
allowlisted provenance object. Context independently enforces the exact
principal, token, thread identity, field allowlists, canonical kinds, and
idempotency key. Remove the three `NETWORKING_MUTATION_*` settings and the port
grant to revoke the listener without affecting read access or other writers.

## Object backfill and forward Artifact indexing

Only an Artifact with `capture_outcome=complete` and verbatim text may be a
Source's current Artifact. Other outcomes remain immutable evidence of a failed
or partial capture and require an exact reason. Check `/api/v2/embeddings/status`
before and after rollout; it reports configuration identity, queue counts,
coverage, historical/future Artifact eligibility, oldest work, and lexical
fallback state, never credentials, vectors, or full text.

The migration marks every pre-existing Artifact `semantic_indexing_enabled=false`.
Those historical transcripts remain fully available to exact full-text search but
are never queued for chunk embeddings. New Artifacts default to semantic indexing
enabled; the normal worker reconciliation loop queues their deterministic chunks
only when they are complete and current. The same loop queues one concise
kind/title/description vector for every Object, including the approved historical
Object backfill.

A safe rollout is: record status; take and verify a backup; enable all provider
settings; deploy one replica; wait for one existing Object summary and one newly
captured canary Source to complete; confirm an old transcript remains lexical-only;
test exact and paraphrase queries; inspect usage and failures; then allow only the
Object-summary queue to drain. Disabling all embedding settings immediately
returns the service to lexical-only mode without changing canonical Artifacts.
Restore a backup only for canonical migration-integrity failure, not for ordinary
derived-vector failure.

## Reviewed maintenance on the intake listener

Protected research corrections use the existing optional intake listener and
universal apply engine. They do not require unprotecting records. In addition to
`INTAKE_API_TOKEN`, set a distinct `MAINTENANCE_API_TOKEN` (at least 32 characters)
and one `MAINTENANCE_ALLOWED_PRINCIPAL`. Keep the maintenance credential out of
interactive agents. Maintenance routes are absent unless configured; this token
cannot authenticate to the separate import routes.

- `GET /api/v2/maintenance/objects` and `/connections` inventory every row,
  including archived rows. Optional `lifecycle=active|archived`, `limit=1..200`
  (default 100) and UUID `cursor` return `data` and `next_cursor`. Follow until
  `next_cursor` is null. UUID ordering is stable across updates/archives; pause
  writers for final reconciliation because concurrent inserts are not a snapshot.
- `POST /api/v2/maintenance/read` uses the universal read request/response,
  including archived Objects, Artifacts and Events. Note subtype text in this
  response is a 400-character excerpt. Use `GET /api/v2/maintenance/notes/{id}`
  for full Note content, `/sources/{id}` for complete Source metadata, and
  `/artifacts/{id}/content` for captured text (the existing bounded content-window
  parameters apply). These routes reuse the existing read handlers. The inventory, unlike
  ordinary Object Connection readback, includes archived Connections too.
- `POST /api/v2/maintenance/apply` uses the universal apply request, limited to
  20 operations. Existing Source/Note/Entity/Theme/Task records may be corrected
  or archived; text, Note intent and Source canonical URI are the allowed Object
  correction fields. Explicit-ID relationship creation/update/archive and
  supporting Artifact append are supported. Creator, provenance on Objects,
  protection, system-managed records and canonical captured-content promotion
  cannot be changed. Existing relationship changes need an ID and revision;
  create cannot silently update a matching edge. No implicit Chat connections.
  The maintenance-only `update_description` operation requires `object_id`,
  `expected_revision` and `description`. It is the sole exception for correcting
  descriptions on archived Objects and system-managed kinds; it changes only the
  Object description, revision, update attribution and timestamp. Ordinary
  `/api/v2/apply` rejects this operation, and `update_object` keeps its existing
  active, interactively writable kind restrictions.
  The maintenance-only `promote_source_artifact` operation requires `source_id`,
  `expected_revision`, `artifact_id` and `expected_sha256`. It may select only an
  existing complete, nonempty, semantic-indexing-eligible Artifact belonging to
  that active Source. It changes the current Artifact pointer plus Object
  revision and update attribution; it never creates or changes Artifact content.

Start with `validate_only: true`. The transaction rolls back and returns previews
with prior state and `approval_sha256`. Review the exact request and its preview,
then configure `MAINTENANCE_APPROVED_REQUEST_SHA256` as the comma-separated list
of approved hashes. With no list configured, every commit is denied. The digest
covers the typed request after normalizing `validate_only` to false, including
IDs, values, operation order, revisions and idempotency key. Do not compute it
from arbitrary raw JSON serialization. Commit that request with validation off;
replay the exact request and read back its records. Revoked approval also denies
replay. Revisions and authorization are never inferred from a dry run.

Every committed batch retains attribution and immutable Events plus prior-state
snapshots in its Run result. Export affected state before committing. Active
corrections can be compensated by another reviewed revision-aware batch. Archives
retain history but this surface offers **no unarchive or automatic rollback**.
Archive reversal requires separate supported owner recovery. Failed batches save
nothing. Remove maintenance configuration and credentials after reconciliation;
remove intake configuration too when no other owner operation needs that listener.

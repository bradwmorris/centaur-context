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

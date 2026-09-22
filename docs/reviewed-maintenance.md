# Owner-reviewed fixture maintenance

These endpoints exist only on the optional maintenance/intake listener, with its
existing separate token and principal. They are absent from ordinary agent APIs.
Keep responses and recovery exports in private local evidence: rows may contain
private research and conversations.

- `GET /api/v2/maintenance/tables`: complete public relation catalog, category,
  table count, primary key and supported purge policy. Views, derived indexes and
  bookkeeping (including extension-owned tables) are distinguished from application tables.
- `GET /api/v2/maintenance/table-rows?table=objects&limit=100`: bounded stable-key
  pages. Pass the returned `next_cursor` URL-encoded to continue. Rows include
  `key`, `row_sha256` and the complete recovery-readable `row`. Concurrent writes
  can change page contents; preview takes a consistent snapshot.
- `POST /api/v2/maintenance/purge`: preview by default; the request below is
  synthetic. Select only records already reviewed as disposable fixtures.

```json
{
  "idempotency_key": "reviewed-fixture-batch-1",
  "selections": [{
    "table": "objects",
    "key": {"id": "00000000-0000-0000-0000-000000000001"},
    "row_sha256": "<exact hash from audit>",
    "reason": "Confirmed synthetic fixture, reviewed against original evidence"
  }]
}
```

Preview returns `data.manifest`, `data.manifest_sha256`, and the full private
`data.recovery_export`. Review every removal and blocker, not just the selected
root IDs. Preserve this response outside the database before deletion; the export
hash is in the manifest. The export records exact table rows, including generated
columns, and is recovery evidence, not a blindly executable restore script.

Approve the manifest hash through the existing server-side
`MAINTENANCE_APPROVED_REQUEST_SHA256` configuration. Submit the same selections and
key with `commit: true`, `manifest_sha256`, and `recovery_export_sha256`. Caller
flags cannot authorize a commit. Any changed row/dependency invalidates approval.
Approval also binds the recorded reasons and preserved reference list. A blocked
manifest cannot commit even when approved. Never delete a real neighbour to clear
an obstruction; report the mixed case instead.

A successful commit removes only listed rows and returns a payload-free durable
receipt. The same committed request returns that receipt on replay; changing a
request under its used key fails. The receipt retains deleted row keys/hashes so
historical mentions in original messages and mixed Runs remain explainable.
Shared/mixed live foreign keys, Note evidence pointers and references from
nonterminal Runs block. Wait for concurrent execution to finish. Historical
messages, provenance, Run inputs/traces/results, consulted IDs and cache snapshots
are explicitly retained, not rewritten. Selecting a whole Chat or Run requires
separate confirmation that it is entirely a fixture. Subtype and embedding rows
cannot be independently selected; they follow their reviewed owner.

Preview and commit are bounded to 100,000 application rows and 1,000 selections.
Commit locks application writers with a five-second lock timeout and a 10-second
statement timeout; a busy system may require retrying later. The full operation has a 25-second budget; after a timeout retry the same key to
reconcile a possible commit receipt. CPU-only analysis runs outside the async
request worker and never owns a database connection. New application
tables fail closed until the fixed purge policy is extended. Disable temporary
maintenance credentials and approval hashes after the reviewed operation.

## Typed reconciliation (policy version 2)

The same request accepts `reconciliations` alongside `selections`; their combined
limit is 1,000. The list defaults to empty for existing purge requests. Each action
requires an exact audited row hash and a nonempty `reason` (at most 2,000 characters):

- `retire_placeholder`: `embedding_id`, `row_sha256`, `replacement_id`, `reason`.
  Only untouched pending `__unconfigured__` object jobs qualify. The configured
  current model, dimensions, input mode, format, current Object hash and a valid
  completed replacement vector must all agree. Object and replacement must remain.
- `detach_chat`: `run_id`, `row_sha256`, `chat_id`, `reason`. Clear only the Chat FK
  of an unpinned terminal retained Run; the exact Chat must be selected for deletion
  in this request. Preserve original execution payloads, timestamps and Events.
- `cancel_interaction`: `run_id`, `row_sha256`, `chat_id`, `owner_observations`,
  `evidence_sha256`, `reason`. Cancellation requests cannot also delete or detach.
  Only unpinned nonterminal Slack wrappers qualify; active/pinned related work blocks.

Each owner observation contains `context_run_id`, `thread_key`, RFC3339
`observed_at`, `operation: "interrupt_active_execution"`, `identity_origin`
(`stored_trace`, or explicitly owner-verified `runtime_sink_mapping` when no
external key was recorded), and `response` containing `ok: true`,
`interrupted: false`, `execution_id: null`, and the same `thread_key`. Cover every
external key in the related execution history. Missing, unknown, interrupted,
future or older-than-30-minute evidence rejects. `evidence_sha256` is SHA-256 of
compact recursively key-sorted UTF-8 JSON for the full observation array, preserving
array order and exact timestamp strings. A caller cannot self-authorize evidence:
the separate maintenance principal and exact server-approved manifest are required.

Cancellation records current termination time and durable provider/Run identity
fences. It proves no active execution was observed and prevents further Context
writes for that exact discarded thread; it does not claim upstream retries were
disabled. Fences survive later purges and cannot be selected for content deletion.
Ordinary ingestion and tool writes, new Run/Chat identities, child links and Event
writes cannot resurrect the discarded thread.

The preview adds `changes` and `proofs`; the recovery export includes before-state
and retained replacement evidence. The receipt records actual committed after-hashes
and timestamps. Original Chat identity and reconciliation evidence are also returned
by Run detail as `maintenance_history`. Timestamp-independent projections bind
approval; original execution content remains unchanged. A new ordinary purge preview
is required after cancellation, with its own reviewed hash and recovery export.

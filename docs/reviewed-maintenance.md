# Owner-reviewed fixture maintenance

These endpoints exist only on the optional maintenance/intake listener, with its
existing separate token and principal. They are absent from ordinary agent APIs.
Keep responses and recovery exports in private local evidence: rows may contain
private research and conversations.

- `GET /api/v2/maintenance/tables`: complete public relation catalog, category,
  table count, primary key and supported purge policy. Views, derived indexes and
  bookkeeping are distinguished from application tables.
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
Commit locks application writers with a five-second lock timeout and a 30-second
statement timeout; a busy system may require retrying later. New application
tables fail closed until the fixed purge policy is extended. Disable temporary
maintenance credentials and approval hashes after the reviewed operation.

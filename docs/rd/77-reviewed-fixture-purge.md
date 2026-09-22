# Reviewed fixture purge

Issue #77 adds an owner-reviewed maintenance operation, disabled with the existing
maintenance listener. Ordinary agents cannot invoke it. No keyword classification
or production SQL is exposed.

The owner selects exact row keys and current row hashes from the authenticated
catalog. Preview includes exclusively owned subtype, Artifact, embedding, event,
Chat-message and incident Connection rows. Every row, its reason, dependency
policy, recovery export and counts are bound into a deterministic SHA-256 manifest.
Commit requires the server-configured approved manifest hash and acknowledgement
of the recovery export hash. It recomputes under write-excluding table locks;
changed rows or dependencies fail atomically. Receipt keys are principal-scoped.

Deleting a canonical Object may remove its own fixture history, but must not
remove a neighbouring Object. Retained foreign keys, current Artifact pointers,
Note evidence locators, event targets and references from nonterminal Runs block. Historical mentions in preserved
messages, Run inputs/traces/results, Object provenance and request responses are
reported rather than rewritten. The minimal receipt retains removed identities
and hashes, without fixture payload. Shared Runs and Chats are never inferred to
be fixtures: they require explicit selection and all dependent rows must already
belong to the reviewed removal set. Retained Run consulted IDs are historical.

Only immutable DELETE is excepted, inside a transaction with a temporary exact
row allowlist created by the reviewed handler. UPDATE remains prohibited. The
normal APIs never create this allowlist. SQL owners remain SQL owners; this is an
HTTP authorization boundary, not a substitute for database access controls.

All public application tables are catalogued with keys, counts and bounded pages.
Bookkeeping, views and indexes are labelled separately. Purge supports a fixed
schema policy and fails closed on new application tables until that policy is
reviewed. Recovery payload remains private client evidence, never committed.

Live classification and deletion remain separate from implementation. The parent
reviewer owns the exact live manifest and retains the export before executing it.

### Indexed deletion follow-up
Actual commit exposed quadratic full-row JSON matching on large immutable Event history. Delete reviewed rows through typed primary-key joins only (UUID IDs, subtype object IDs, and the exact two-column request key). Keep transaction locks, manifest approval, immutable-row allowlist and row-count guards unchanged. Verify a complete synthetic HTTP preview/commit over 15,000 large Events, retaining 14,791 unrelated Events.

### Internal Chat cursor cycle
Selected Chat rows point to their owned messages through two restrictive cursor foreign keys, while messages point back to Chats. After exact manifest validation and dependency checks, clear only selected Chats' two cursor fields inside the purge transaction. Their original values remain in the recovery export; any later failure restores all cursor fields and dependent rows. Never update retained Chats. Source current-Artifact links are removed by deleting selected Source subtypes before Artifacts; Artifact supersession and Run parent links are removed in their respective single-table statements. Integration coverage exercises populated Chat cursors, Artifact supersession/current pointer and parent Runs, including an injected late rollback and unchanged retained Chat cursors.

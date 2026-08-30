# RD: Prepare and Populate the Database for the POC

**Status:** `complete`
**GitHub Issue:** [#27](https://github.com/bradwmorris/centaur-context/issues/27)
**Created:** 2026-08-30

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Repository boundaries; ontology and subtype triggers; Source,
Note, Connection, Object Event, provenance, search, embedding, Context Builder,
reset/restore code, and every HTTP surface. A second guarded read-only audit of
The AGI Post Supabase project `lwhxkmepuucjjjpececu` checked live rows, hashes,
identifiers, content defects, and relationship closure; no rows changed. The
live Enyu target was also inspected through its HTTP API and Kubernetes metadata:
it is healthy on schema version 10, schema fingerprint `9822e865853f7ad6`, and
database `centaur_context_test_enyu`, with 27 disposable POC Objects immediately
before the authorized reset (three subscription-canary Objects arrived after
the earlier 24-Object audit).

**Approved dependency:** Add a private authenticated import-only API and
credential, disabled after cutover, because no current authenticated API can
create the complete dataset, especially Users. This is a code/configuration
change, not a database-schema change.

**Approved protection:** Create imported Objects and Connections with
`protected=true`. Protection only prevents the automated Curator from changing
them; it does not hide them, remove them from retrieval, or prevent human edits
and later unprotection.

**Missing:** none. Supabase read access, Kubernetes/operator access, Docker/kind,
PostgreSQL tooling, and GitHub authentication were verified on 2026-08-30.

1. Add and test the bounded authenticated import capability without changing
   the current database schema. It must support validate-only, idempotent batch
   creation of Users, external identities, Entities, Sources, Source content,
   Notes, and Connections; never expose a DSN or generic agent/database access.
2. Take an immutable read-only source snapshot and build a private manifest for
   the fixed seed set below. The agent owns every transformation and rejection;
   there is no requester review checkpoint.
3. Freeze destination writers, back up and reset only the verified Enyu
   `centaur_context_test_enyu` database using the guarded operations path, apply
   the existing migrations unchanged, and prove its schema fingerprint matches
   the pre-import baseline.
4. Validate the whole manifest with zero writes, then import in dependency
   order using bounded, idempotent batches. Keep the database closed to Slack
   until complete reconciliation and retrieval canaries pass.
5. Reconcile every manifest row, target Object/subtype, content hash, event,
   attribution edge, semantic relationship, search result, and rejection. On
   any pre-cutover failure, reset to the known-empty baseline and rerun; do not
   attempt piecemeal deletion of immutable/protected data.
6. Disable the import credential/surface, start normal writers, run an
   authenticated canary from the live Slack Researcher runtime without creating
   a synthetic Chat, retain the signed manifest and reconciliation report
   outside Git, and close only if every gate passes. A failed gate causes abort
   and restore, never a request to weaken quality.

## What We Are Doing

- [x] Seed exactly two Users, four Entities, three Sources with verified content,
  and 51 real `brad_kms_nodes` as Notes. Create a new canonical Object and
  subtype for each; restore Brad's existing Slack external identity; import
  nothing else.
- [x] Preserve exact provenance and only relationships whose meaning survives
  the target ontology; visibly attribute every imported research Object to Brad
  and Codex.
- [x] Prove the import is complete, reproducible, duplicate-free, searchable,
  and safe before the Slack bot can add new data.

## Contract

- **Goal:** Start the POC with a small, refined, provenance-rich Context corpus,
  not a broad legacy dump.
- **Done:** The existing schema is unchanged; 27 test Objects are replaced by
  exactly 60 protected seed Objects, three Source contents, and 172 protected
  Connections; Brad's Slack identity resolves to the seeded Brad User; original
  and normalized hashes reconcile; retry creates no duplicates; and lexical,
  graph, content, and Slack canaries pass.
- **Files:** This RD; minimum Rust HTTP/domain/database code; importer/client and
  tests under `tools/centaur_context`; operator documentation. Real exports,
  credentials, row payloads, ID maps, manifests, and logs remain private and out
  of Git. No SQL migration or schema file may change for this job.
- **Agent owns:** Read-only discovery, refinements, manifest generation, scoped
  implementation, dry runs, import, and reconciliation through trusted
  authenticated surfaces. The agent never receives a database DSN.
- **Requester owns:** No data-selection, editorial, reset, credential, or
  cutover decision during authorized execution. Existing access must remain
  available. Repository merge still follows the mandatory repository workflow.
- **Out of scope:** `ai_v2`, Console databases, schema changes, wholesale source
  copying, Memories, source chunks/search indexes/embeddings, recurring sync,
  source mutation, public ingress, cloud deployment, or enabling a paid
  embedding provider.

## Non-Negotiable Quality Gates

The migration is deliberately closed rather than score-based. Accept only the
three active published Sources referenced by the 51 approved real KMS nodes:
*The Economics of Recursive Self-Improvement*, the SemiAnalysis SpaceX 10GW
video, and Ryan Greenblatt's AI-research-automation interview. All have one
collision-free primary identifier, a refined Feed description, a ready private
capture, and a recomputed matching SHA-256. Reject the other 368 Sources in this
pass, including every course item; there is no heuristic expansion.

Reject a candidate when it is a course/module/cohort/lecture item; dummy, test,
placeholder, malformed, merged, archived, or deleted; duplicates an accepted
canonical identifier or content hash; lacks an approved description; has only a
marker/link, corrupt, or empty capture; has uncertain provenance or retention
rights; or adds no durable research value. Metadata-only Sources may pass only
when independently valuable and sufficiently evidenced. Do not use date, title
regex, length, or graph degree as an automatic decision: the audit found false
positives for both course detection and apparent URL duplicates.

The source's active `source_identifiers` rows—not ad hoc URL stripping—are the
deduplication authority. Exact normalized identifier and identifier-version are
recorded. Capture SHA-256 is a second duplicate/integrity key. Similar titles
are reviewed, never silently merged.

For Notes, exclude the three explicit `[DUMMY]` nodes. The other 51 have nonempty
unique bodies of 267–13,079 characters, 52 approved Source links, and no secret,
placeholder, or duplicate-body signal. The agent may normalize casing, grammar,
Markdown, and encoding while preserving every factual claim and qualifier.
Create a grounded concise Object description from the body. Reject rather than
inventing support when a claim cannot be retained safely.

Import exactly Microsoft, NVIDIA, SpaceX, and Ryan Greenblatt. They are active,
non-merged, have canonical URLs and 116–186 character summaries, and are
substantively central to an accepted Source. Do not import SemiAnalysis as an
Entity: represent it faithfully as the Source publisher instead of falsely
mapping publisher identity to `about`. Because the target Entity subtype is
generic, express identity in the Object description and bounded provenance.

Two encoding repairs are mandatory and deterministic: replace the paper
capture's single ASCII `0x01` page-extraction byte with a newline; and restore
the Cobb–Douglas Note's escaped LaTeX (`\\text`, two `\\times`, `\\alpha`, and
`\\beta`) from one backspace, three tab bytes, and one unescaped exponent.
Replace two Unicode line separators in the SpaceX thesis with newlines and
copyedit its casing/punctuation without changing its speculative framing.
Record exact before/after hashes and diffs.

All fixed payloads fit the existing schema and API limits: the largest Source
capture is 189,468 bytes against a 10,000,000-character API ceiling; the largest
Note is 13,079 characters against 100,000; descriptions, titles, URLs, Source
kinds, and relationship reasons also fit. Provenance uses only the existing
`source_type`, `source_ref`, and `note` contract. No new column, table, index,
constraint, extension, or provenance schema is needed.

## Manifest and Import Invariants

The private immutable manifest records snapshot cutoff and source-schema
fingerprint; transformation version; batch and manifest hashes; source row ID
and type; normalized identifier/version; exact target payload; reviewer,
decision, timestamp, and reason code; content artifact/revision/hash/byte count
and rights decision; every relationship mapping; original and normalized hashes;
and a stable client key to the new target UUID returned by the API. Source IDs
are provenance, never target primary keys. A Source hash is verified before
normalization; the target hash is computed from the exact normalized bytes.

The importer accepts only the approved manifest hash and these seven resource
families. It supports validate-only and bounded writes, returns an external
checkpoint/ID map, and uses stable Object Event idempotency keys. The existing
immutable event log is the database ledger; this job adds no ledger table. The
import event actor records who executed the migration, not who historically
authored the research.

Import order is Brad User, Brad's existing Slack external identity, Codex User,
Entities, Sources, one current content version per Source, Notes, attribution
edges, then semantic edges. Restoring Slack workspace `T0BFLA920LA` and user
`U0BFE2QCWGK` before writers resume prevents a duplicate Brad User. Codex gets
no invented external identity. A rejected endpoint rejects its edge. Imported
reference Objects and
Connections are protected. The expected final graph is 60 Objects (2 Users, 4
Entities, 3 Sources, 51 Notes), 3 content versions, and 172 Connections (116
`involves`, 52 `derived_from`, 4 `about`). No writers run concurrently. Before
Slack opens, manifest, subtype, event, hash, and edge counts must reconcile with
no orphan or duplicate active edge.

The current test dataset must not be cleared row-by-row: immutable Source
content and subtype protections make partial cleanup unsafe. A trusted operator
uses the guarded backup/drop/bootstrap path against the exact verified database,
checks the backup, reapplies the same migration set, and compares schema
fingerprints. The standing execution authorization covers this exact reset.

## Exact Data and Connection Mapping

| Source fact | Target representation | Rule |
| --- | --- | --- |
| Brad / Codex | Object + User | New UUIDs; Brad `human`, Codex `agent`; verified external identity only when evidenced. |
| Existing Brad Slack identity | External identity on Brad User | Preserve the verified provider, workspace, provider-user ID, and display name so Slack reuses Brad's canonical User. |
| Accepted Entity | Object + Entity | Approved title/description; source type, URL, and ID in bounded provenance. |
| Accepted Source | Object + Source | Explicit kind mapping; never copy derived vectors/indexes. |
| Approved ready capture | Source content v1 | One immutable normalized text; target-computed SHA-256 must equal manifest hash. |
| 51 accepted KMS nodes | Object + Note | Agent-reviewed title/description; factual substance and qualifiers preserved. |
| Any imported research Object | `involves` Brad and `involves` Codex | Two explicit protected edges, because current Object inspection derives visible participant attribution from these edges. Reasons distinguish Brad's ownership/selection from Codex's collaboration. |
| KMS `uses_source` Source | Note `derived_from` Source | Mandatory when both rows are accepted; preserve source edge ID/type and reason in provenance. |
| Source `mentions` Entity | Source `about` Entity | Import only after reason review confirms topical meaning. |
| Source `features` Entity | Source `about` Entity | Import only when the Entity is genuinely central/featured; merge deliberately with an existing `about` edge. |
| Source `owned_by` Entity | Rejected edge | No faithful target kind; do not weaken to `related_to`. |
| Entity `leads`, `affiliated_with`, `invested_in`, `part_of`, `founded` | Rejected edge | No faithful target kind in the existing ontology; report, do not approximate. |

Target active edges are unique by source, kind, and target. When multiple source
edges collapse to one `about` edge, review and compose one precise reason and
record every contributing source edge ID. Do not create `related_to` as a
catch-all. The Brad and Codex edges add uniform graph degree, so retrieval tests
must specifically detect participant-driven ranking pollution.

Only four `about` edges survive review: SpaceX, Microsoft, and NVIDIA from the
SpaceX Source, and Ryan Greenblatt from the interview. SemiAnalysis becomes the
publisher string. The paper has no accepted Entity edge. The 52 approved
`uses_source` edges become 52 `derived_from` edges, including both Sources for
the cross-source RSI thesis. Artifact and Research Project `developed_from`
edges are reported but rejected because their endpoints are out of scope.

## Indexing and Retrieval Acceptance

Object lexical search indexes title and description. Full Note and Source bodies
have separate lexical indexes. The live Enyu deployment has no embedding
provider configured; optional `centaur-object-v1` embeddings are therefore not
generated in this job. Enabling them needs configuration and spend, but no
schema change. Refined descriptions remain part of correctness.

Before cutover, a fixed canary set must retrieve the expected Entity, Source,
and Note through Object lexical search, direct
Source/Note content search, one-hop graph expansion, and the live Slack
Researcher runtime's authenticated Context path. Record ranks and evidence
without creating a synthetic test Chat in the refined corpus. If relevant
material is discoverable only
inside a long body and not through its approved description/content search, do
not broaden the import or change schema under this job; refine the description
or reject the row and raise separate retrieval work.

## Checks

- [x] `git diff --exit-code -- migrations` proves no schema migration changed;
  pre/post-reset schema fingerprints match.
- [x] Source access is read-only and scoped; artifacts/logs expose no secret,
  DSN, private payload, or vector.
- [x] The signed agent audit shows every accepted/rejected row, editorial diff,
  dedupe and encoding decision, and rejected relationship; no subjective row is
  escalated for approval.
- [x] Validate-only writes zero rows; interrupted resume and full retry produce
  the same IDs and no duplicate Objects, contents, events, or Connections.
- [x] Reconciliation proves exact counts, subtype pairing, UUID mapping,
  Brad identity binding, protection flags, hashes, provenance, events,
  attribution, and zero orphans.
- [x] Validation, fail-closed guards, and integration tests cover malformed or
  unapproved manifests, wrong database/schema,
  rejected endpoints, collapsed edges, payload limits, hash mismatch, stale
  snapshot, wrong credential, and mid-batch failure.
- [x] Object/body lexical search, graph pollution, and Slack-runtime canaries
  pass before normal writers start; the report states semantic search is
  unconfigured.
- [x] All repository-root verification commands and `git diff --check` pass.

## Completion Evidence

- Final private manifest SHA-256:
  `80701a5a353c007b84741eaca4e0574950856de5a63f139fdce803dfa4d94b6a`;
  canonical payload SHA-256:
  `b67fd4789398d4ad15b165131e0a29a37bfed495fbe7d08499078c151d88192f`.
  Rebuilding twice from the pinned exports produced identical files.
- A PostgreSQL 18 custom-format pre-reset backup was verified at SHA-256
  `9c798bd2ef3bbc5b7a4c351af4e23f6e83b7de28b7fcbdfdf589303fff9ee63e`.
  Only `centaur_context_test_enyu` was reset; schema version 10 and fingerprint
  `9822e865853f7ad6` matched before and after. No migration changed.
- Validate-only returned zero writes. Commit created 60 Objects, one external
  identity, three immutable Source contents, 172 Connections, and 235 Object
  Events. Exact replay returned the same deterministic ID map and no new rows.
- Reconciliation compared every target payload field and proved 60 protected
  Objects (2 Users, 4 Entities, 3 Sources, 51 Notes), 172 protected Connections
  (116 `involves`, 52 `derived_from`, 4 `about`), 235 unique event idempotency
  keys, correct Brad Slack identity binding, matching content hashes/current
  pointers, and zero duplicate or orphan edges.
- Object, Note-body, Source-body, graph, participant-pollution, and live
  Slack-Researcher-runtime canaries passed. Embeddings remain unconfigured:
  zero vectors were generated and the normal 60 jobs remain pending.
- The temporary token, manifest pin, and listener configuration were removed;
  port 8085 is closed and Enyu is back on its normal reviewed image. The private
  manifest, backup, builder, and reconciliation report remain outside Git.
- Verification passed: formatting, Clippy with warnings denied, all Rust tests
  against a disposable `template0` database, 41 Python client tests, Python
  compilation, web type-check/build, production dependency audit, migration
  diff, and `git diff --check`.

## Approval Boundary

Execution is complete within the standing authorization for the scoped private
import credential, guarded backup/reset of `centaur_context_test_enyu`, private
content transfer, local deployment, and cutover. Source Supabase remained
read-only. No schema change, paid embedding enablement, public ingress,
unrelated deletion, or other database access occurred. Repository merge remains
subject to the mandatory repository workflow.

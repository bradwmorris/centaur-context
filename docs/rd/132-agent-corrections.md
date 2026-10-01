# RD 132: Current-state corrections with retained evidence

Issue: https://github.com/bradwmorris/centaur-context/issues/132

Status: implemented and synthetically verified; release requires owner approval of the concrete
PR revision and deployment destination. Synthetic checks do not establish live
acceptance.

## Architecture decisions

`protected` becomes a background-curation preference, never a denial of an
explicit authenticated correction. Existing records benefit without rewriting
flags. Task authority still comes from the request, not retrieved content.
All explicit corrections use the existing store-scoped authenticated apply
transaction, principal-scoped idempotency key, expected revision and immutable
Run/Event ledger. Capture a real before snapshot under lock even if an imported
record has never had an Event. Validate-only executes the same checks and rolls
back. No credentials or actor identifiers are editable domain metadata.

Stable Object UUID and kind remain immutable because foreign keys and historical
Events refer to them. Correct a mistaken type by creating the right Object,
recording a replacement relation with explanation, relinking current Connections
and Task ownership, and archiving the replaced Object. The old identity remains
readable. Do not move original message authorship or historical Artifact ownership.
Imported identifiers identify original evidence. `reassign_identity` moves an
existing User binding with both revisions, keeping message sender IDs unchanged.
`reassign_chat` transfers routing metadata to a fresh active Chat; original
messages, Runs and processing cursors remain with the original Chat. Both record
locked before/after Events and correction assertions. Incorrect literal provider
keys are annotated; no operation fabricates another authenticated principal.
User profiles and identity display metadata can be corrected without changing
the authenticated principal. Configured authentication identities remain a
credential/deployment concern, distinct from domain metadata.

Original Artifacts and messages remain immutable evidence. Corrections are
append-only, typed assertions with a reason and authenticated attribution,
attached to an owning Object and addressed evidence record. Each correction
advances the owning Object revision. Read paths show both original evidence and
current correction. Another correction supersedes the current assertion without
deleting it; a withdrawal is another assertion. Quotes remain literal matches
to their exact Artifact; interpretations are Notes, not repaired quotations.

Canonical Source promotion is an explicit revisioned operation. Existing Notes
remain pinned to exact retained Artifact IDs; promotion reports dependent Note
IDs transactionally. Moving a citation requires an explicit Note correction
which validates its quote against the replacement Artifact. Contextual links use
about/related_to; derived_from for an Excerpt must match its cited Source.

Background producers preserve explicit corrections. Automated proposals use
revision checks and must not silently rewrite user/agent corrected current
representations. Derived indexes are invalidated transactionally; a supported
rebuild operation repairs derived state without changing original evidence.

## Persisted-data correction and acceptance matrix

| Family | Existing restriction / integrity boundary | Supported correction and retained history | Acceptance |
| --- | --- | --- | --- |
| Objects (all eight kinds) | protected and system-managed denials; UUID/kind subtype FKs | revisioned title/description/provenance update; create, archive, restore; replacement/relink for UUID/type; locked before/after Event | protected/imported fixture of every kind; stale edit, rollback, retry, restore |
| Source subtype | metadata edits blocked when protected | update metadata; append immutable capture; explicit canonical promotion; retain all versions | old capture retrievable; affected citations returned; corrected search |
| Note subtype | protected updates blocked; typed evidence constraints | edit text/format/intent/evidence; preserve snapshot; quote validates exact retained Artifact | repaired transcription versus interpretation; no fabricated derivation |
| Entity/Theme subtype | protected denial; enum/slug uniqueness | revisioned entity kind/slug edits; replacement for conflicting identity | corrected subtype and protected Theme links |
| Task subtype | owner/status/time invariants; protected denial | existing revisioned ownership/status/brief correction; archive/restore subject to references | active User ownership; invalid status atomic failure |
| Chat subtype | system-managed; imported routing identity | edit display metadata; annotate imported identity mistakes; replacement Chat for wrong identity; retain original messages | no forged authorship; later ingestion preserves display correction |
| User subtype and embedded identities | system-managed; unique provider identity and identity-based ingestion routing | edit domain profile; metadata annotation/correction; replacement/relink rather than impersonating a provider identity | no actor/credential changes; ingestion preserves corrections |
| Memory subtype | system-managed; happened_at and evidence provenance | revisioned content/title/description/time correction; archive/restore; preserve old evidence assertions | later background review cannot silently erase explicit correction |
| Connections | protected endpoints and protected row denials; unique active triple | revisioned kind/explanation/provenance edits, archive/restore; archive/create for endpoint relink | Source/Entity to Theme, Note to Entity, Source to Source; contextual Excerpt to secondary Source |
| Artifacts / binary payloads | immutable bytes, hash, ownership, supersession | append version with supersedes ID; canonical selection; annotation for erroneous historical metadata; retained old bytes | hashes and old versions retrievable; cross-owner supersession rejected |
| Chat messages | original sender/provider ID/time/content evidence | append correction assertion including corrected representation, never rewrite original sender | original and corrected representation distinguishable and attributable |
| Object Events | immutable actor, revisions, before/after evidence | append correction annotation; new domain correction Event; never modify original Event | imported record has accurate first before snapshot; bad assertion remains retrievable |
| Runs (including external-action traces and memory reviews) | identity/input immutable; lifecycle controlled by executor | append correction annotation; existing retry/cancel/reconciliation for execution; no rewriting actions that happened | correction does not change status or forge external completion |
| Task routines | revisioned definition and confirmation; occurrence claims | existing configure/disable/reconfigure; annotate erroneous historical occurrence via Run | protected Task routine configurable; no duplicate execution claim |
| Embeddings / lexical indexes | derived content/hash/model/state | transactional invalidation plus explicit rebuild; exclude superseded current truth in retrieval | corrected content invalidates old vector; rebuild schedules fresh work |
| Apply/upload idempotency receipts | principal/key/hash identity | replay original response; new key for a different correction; not domain truth | same request replays, changed payload conflicts |
| Maintenance receipts/fences | operational audit and cancellation safety | existing reviewed maintenance reconciliation; append Run correction for bad assertion | correction cannot bypass fence or purge evidence |
| Migration ledger | database schema version integrity | new forward migration; never mutate applied migration history | migration on disposable database |

## Contract and rollout

Expose correction operations through the canonical contract and generic CLI/MCP
apply tools, human API and UI capabilities. Keep public examples synthetic.
Document field-level identity constraints alongside supported replacement paths.
No installed bridge, live store, shared bot, cluster context or deployment changes
belong to this implementation. Recheck origin/main and concurrent PRs before
integration. Run CONTRIBUTING checks and disposable database integration tests;
self-review exact diff and publish draft PR with family-by-family evidence.

## Acceptance evidence

Implementation and synthetic verification evidence is recorded in
[the verification report](../verification/132/README.md). No release/live acceptance is claimed.

## Concurrent integration

Readiness PR #133 merged as `13987f7e14aeac32cdfc40fbe7a8f4cd516fb079` and
is integrated in this branch. Its authentication, onboarding, Task requirements,
and Note-intent guidance are preserved. This change supersedes blanket-protection
descriptions and increments schema to 37. Verification runs against that
integrated base; live rollout still requires approval of the exact revision and
destination.

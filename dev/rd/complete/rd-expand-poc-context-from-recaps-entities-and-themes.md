# RD: Expand the POC Context from Recaps, Entities, and Themes

**Status:** `complete`
**GitHub Issue:** [#39](https://github.com/bradwmorris/centaur-context/issues/39)
**Created:** 2026-08-30

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** The first-pass contract; current ontology, API, search, UI,
and intake; and a read-only audit of the live Supabase source. No row changed.

**Missing:** none. First-class Theme support is a required first step.

1. Add `theme` as a canonical Object kind with a one-to-one Theme subtype and
   add the exact Connection kind `themed`. Support it in validation, intake,
   clients, APIs, search, and UI before opening the import window. Add an agent
   proposal queue and human `approve_themes` permission; only Brad holds that
   permission initially.
2. Snapshot Supabase read-only and produce a private manifest for 127 distinct
   recap-backed Sources, 166 Entity rows resolved to 159 canonical identities,
   10 Themes, accepted contents, and proposed relationships. Record every
   repair, redirect, collapse, and rejection with stable IDs and hashes.
3. Reconcile against the Enyu seed by provenance, canonical URI, identifier,
   and hash. Reuse existing records; never reset the first-pass corpus.
4. Validate with zero writes, then use only the temporary authenticated bounded
   batch intake—not the permanent single-Source Enyu listener—to import Themes,
   net-new Entities, net-new Sources, accepted content versions, Brad/Codex
   attribution, and reviewed semantic Connections in idempotent dependency
   order.
5. Reconcile rows, subtypes, redirects, identifiers, hashes, endpoints, events,
   search, and retry behavior. Disable temporary intake and admit normal writers
   only after all gates pass.

## What We Are Doing

- [x] Prepare and locally verify 159 active Entity Objects; resolve seven merged rows to their
  winners instead of recreating duplicates.
- [x] Prepare and locally verify all 127 distinct Sources referenced by the 123 Recaps, repairing
  defective metadata and importing only independently verified content.
- [x] Prepare and locally verify 10 first-class Themes and reviewed classifications as
  explained `themed` Connections.

## Contract

- **Goal:** Correct the artificially narrow first pass with a broad but refined,
  duplicate-free research graph.
- **Done:** The 166 Entity rows reconcile to 159 target Entities; all 127
  recap-backed Sources reconcile once; 10 first-class Themes exist; contents
  match their hashes; and imported relationships are evidenced and idempotent.
- **Files:** This RD; minimum Theme migration/domain/API/UI/client changes and
  tests; importer/operator documentation. Private artifacts remain outside Git.
- **Agent owns:** Editorial review, repairs, deduplication, implementation, dry
  run, import, reconciliation, and rollback.
- **Requester owns:** A separate instruction to execute this RD. Theme proposals
  require Brad's approval; migration rows and cutover do not.
- **Out of scope:** Importing Recaps as Notes, Claims, Tags, Publications,
  Projects, courses without Recaps, legacy vectors/indexes, `ai_v2`, Console,
  recurring sync, public ingress, or enabling a paid embedding provider.

## Selection and Quality Rules

The 123 Recaps have 128 active `uses_source` edges to 127 distinct active
Sources. All 127 have one active primary canonical identifier and no duplicate
canonical URL or content hash within the set. Recap status does not erase a
valid Source: 67 archived Recaps identify 67 Sources and 56 published Recaps
identify 60 Sources. Two previously imported Sources are reused, so the audited
delta is 125 new Sources. The final execution-time target snapshot remains
authoritative.

Replace four Source titles equal to `>-` with verified provider titles, using a
Recap title only when faithful. Ground descriptions in metadata and Recaps.
Reject or repair placeholders, malformed URLs, collisions, secrets, and
unsupported claims; never invent facts.

Source identity and content are separate decisions. Forty-seven ready captures
cover 42 Sources and match their stored SHA-256 values, but three are obvious
18–105 character workflow markers and must not become content. Review the rest
for completeness, duplication, encoding, rights, and capture type; import at
most one current version per Source. Sound metadata-only Sources remain valid.

Import 159 active Entities and redirect the seven merged rows to
`merged_into_entity_id`. Apparent active URL collisions are shared home pages
used by distinct people and brands, not automatic duplicates. Names and
summaries must remain nonempty and specific.

All 10 Themes are active and unique but lack descriptions. Author bounded
taxonomy descriptions before import. Themes are flat, have no primary/secondary
rank, and are not Entities. Every non-Theme Object kind—including Tasks, Chats,
Users, Sources, Notes, Entities, and Memories—may have zero or many Themes.

Agents may change explained `themed` Connections to approved Themes without
approval. A new vocabulary entry remains a noncanonical, evidenced proposal
until approval atomically creates its Object and subtype. Rejections remain
auditable but never enter retrieval. Only Brad initially has `approve_themes`.

## Relationship Mapping

- Preserve 200 distinct recap-backed Source `mentions`/`features` Entity pairs
  as one reviewed Source `about` Entity edge per pair. Map `owned_by` to the
  Source publisher field when evidenced; do not invent an edge.
- Preserve 76 active Entity `themed_with` Theme pairs as Entity `themed` Theme.
- Build Source-to-Theme candidates from 3 direct candidate-Source theme pairs
  plus Recap theme evidence. The union is 225 pairs. Single-Source Recap themes
  transfer directly; manually verify the 24 possible projections from the two
  multi-Source Recaps and keep only supported Source `themed` Theme pairs. That
  review accepted 22 and rejected 2 Poolside projections, leaving 223 distinct
  Source `themed` Theme pairs.
- Give every new Entity, Source, and Theme explicit `involves` Brad and
  `involves` Codex attribution, reusing the existing Users.
- Do not weaken `owned_by`, `leads`, `affiliated_with`, `invested_in`, `part_of`,
  or `founded` into `related_to`. Report unmapped edges rather than losing their
  meaning.

## Checks

- [x] Theme and `themed` migrations, subtype/endpoint constraints, proposal
  approval authorization, APIs, search, UI, and rollback are tested first.
- [x] Draft manifest reconciliation accounts for 127 Sources, 166-to-159 Entity
  resolution, 10 Themes, all candidate contents, and every accepted/rejected
  edge with no orphan endpoint.
- [x] Disposable-database validate-only writes zero rows; commit and full replay create
  no duplicate Object, subtype, content, Connection, or event.
- [x] Target counts, provenance, hashes, Brad/Codex attribution, lexical/body
  search, graph traversal, and live authenticated retrieval canaries pass.
- [x] Repository-root verification commands and `git diff --check` pass.

## Approval Boundary

The requester confirmed the Source-ingestion deployment was complete and
authorized the live schema deployment, Enyu migration, and repository closeout.
Execution stayed within that boundary. Supabase remained read-only; no public
ingress, paid embeddings, reset, or unrelated database mutation occurred.

## Local Execution Audit

The private draft resolves the source snapshot to 125 new Sources plus 2 reused
Sources, 155 new Entities plus 4 reused Entities, 10 new Themes, 200 Source-to-
Entity `about` pairs, 76 Entity-to-Theme pairs, and 223 Source-to-Theme pairs.
Four already-committed equivalent first-pass edges are reused rather than
duplicated. All three bounded expansion batches validate, commit, and replay on
a disposable schema-12 database with no duplicate active edge or invalid
`themed` endpoint.

The content audit accepted 26 of 47 ready capture artifacts and rejected or
collapsed 21 workflow documents, working notes, placeholders, and duplicates.
All 26 accepted bodies are privately materialized, byte-hash verified, and pass
content-inclusive intake plus lexical retrieval canaries on the disposable
database.

The final source drift audit matched all selected row counts, revision sums,
relationships, identifiers, and the 26 accepted artifact hashes. The private
manifest was sealed reproducibly as
`d4a080e68f862098b55be2581f626f27af76051df803e93f16d7dcdb4a446cac`.
A verified pre-deployment public-schema backup has SHA-256
`cfdd3ac041aaaac859ca8c247609366f5c055ded4a84edbb806dbfbd55afa8c7`.

Schema 12 deployed with fingerprint `325a99ed4b2f1f4a`. The three dependency-
ordered live batches validated with zero writes immediately before commit,
committed atomically, and replayed without creating duplicates. They added 290
Objects, 26 exact Source contents, 1,075 Connections, and 1,391 Object Events.
The retained live database now has 366 Objects: 159 Entities, 133 Sources, 10
Themes, 51 Notes, 8 Memories, 3 Users, 1 Chat, and 1 Task. One pre-existing
Source remains archived, so 365 Objects are active.

Reconciliation matched every imported canonical Object, subtype, protected
flag, provenance record, content hash and byte count, and Connection. The live
graph has 1,262 active Connections: 203 `about`, 62 `derived_from`, 698
`involves`, and 299 `themed`. All Theme endpoints are valid, every reverse Theme
listing reconciles, Brad is the sole initial Theme approver, no duplicate active
edge exists, and Object, body, graph, and authenticated retrieval canaries pass.
The temporary port-8085 intake token and manifest pin were removed and the
listener is closed. The permanent Source listener, Theme proposal listener,
human UI, and both Slack writers are healthy on the reviewed image.

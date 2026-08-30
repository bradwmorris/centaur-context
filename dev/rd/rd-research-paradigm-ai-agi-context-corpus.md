# RD: Research a Paradigm AI/AGI Context Corpus

**Status:** `scoped`
**Created:** 2026-08-30
**Codex Task:** `codex://threads/01a0511a-f8e4-74c2-8402-2b4e0c07af3d`
**Related RD:** `complete/rd-prepare-and-populate-poc-database.md`
**Related RD:** `rd-expand-poc-context-from-recaps-entities-and-themes.md`

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Repository ontology and planning rules; the completed Enyu
seed-import contract; the scoped legacy-corpus expansion; and Paradigm's official
About, Team, Investments, Research, Research Index, and Writing surfaces as of
2026-08-30. Strong initial leads include its recursive-self-improvement work,
automated-research projects, EVMbench, Centaur, and officially AI-labelled
portfolio companies. These are candidates, not an import allowlist.

**Missing:** none for research and import preparation. Database writes require a
separate execution instruction after the candidate manifest is reviewed.

1. Snapshot the existing Enyu corpus through the authenticated read-only HTTP API
   and inspect the pending legacy-expansion manifest if available. Build a
   duplicate ledger by canonical URI, normalized name, provenance reference, and
   content hash; never query `ai_v2`, Console, or any database directly.
2. Research Paradigm from primary sources first: its current firm profile, team,
   investments, research/writing index, individual author pages, and the official
   sites of relevant portfolio companies. Record snapshot time, canonical URL,
   publication date, author, publisher, and evidence for every claim.
3. Rank and review Entity candidates: Paradigm itself; only the leadership,
   investors, researchers, builders, and authors materially relevant to Brad's
   interests; and only backed organizations with a direct, evidenced connection
   to AI/AGI economics, automated research, AI capabilities, compute, data,
   markets, coordination, or agent infrastructure. Account for every considered
   candidate and reject broad portfolio or staff-directory importing.
4. Rank and review Source candidates from Paradigm and accepted organizations.
   Default “recent” to the 18 months before the snapshot, while allowing an older
   work only when it is foundational to an accepted current work or necessary to
   explain an accepted Entity. Verify canonical identity, authorship, date,
   relevance, capture completeness, rights, and hash before recommending content.
5. Produce a private evidence dossier plus deterministic, schema-conformant
   candidate manifest with stable client keys, exact proposed Object/subtype
   payloads, provenance, content artifacts and hashes, proposed Connections,
   reuse/insert/reject decisions, confidence, and reason codes. Validate it
   offline and provide exact counts and a compact requester review table.
6. Recommend the smallest safe import sequence that reuses canonical Objects,
   adds only net-new records through the existing bounded authenticated HTTP
   intake when separately authorized, reconciles all results, and leaves no
   duplicate or unexplained edge.

## What We Are Doing

- [ ] Produce an evidence-backed map of Paradigm, its most relevant people, and
  its backed organizations at the intersection of Brad's AI/AGI interests.
- [ ] Identify the recent, durable research Sources worth retaining from
  Paradigm and the accepted organizations.
- [ ] Deliver a deduplicated, import-ready candidate manifest and efficient
  reviewed import plan without writing to the database.

## Exact First-Pass Ingest Target

The research deliverable must end with one explicit allowlist. Subject to
identity and duplicate checks, the expected first pass is:

| Target | Intended Context representation |
| --- | --- |
| Paradigm | One required canonical Entity, inserted or reused. |
| Relevant Paradigm people | One Entity each for the small set of current people with directly relevant work. Starting candidates are Matt Huang, Dan Robinson, Justin Wang, Alpin Yukseloglu, Georgios Konstantopoulos, and Matthew Slipper; include only those whose current role and relevance are verified. |
| Relevant backed organizations | One Entity each for the verified high-signal set. Start with Nous Research, Harmonic, Andromeda, and Vana; add another portfolio organization only with equally strong AI/AGI-economic relevance. |
| Paradigm research | One Source per accepted durable publication, with one immutable current Source content version when a complete public capture can be retained. The provisional core is *RSI Simulator*, *Introducing EVMbench*, *Formally Verifying a Compiler Using Automated Research*, *Centaur 2.0*, and *Open Sourcing Centaur*. |
| Portfolio-company research | One Source plus retained content for each recent official publication that materially advances the target themes; the research phase must name these exactly rather than importing company news feeds wholesale. |
| Existing RSI economics paper | Reuse the existing *The Economics of Recursive Self-Improvement* Source and connect new material to it only where the ontology has an exact evidenced relationship. Never create a duplicate. |

For every accepted Entity create exactly one canonical Object and Entity subtype.
For every accepted publication create exactly one canonical Object and Source
subtype, plus at most one verified current content version in this pass. Apply
the existing Brad/Codex attribution convention. Add semantic Connections only
when their allowed kind is exact and evidenced.

Do **not** ingest Paradigm's complete staff directory or portfolio, generic team
and investment pages as research Sources, news or fundraising announcements,
job posts, weakly relevant company writing, raw search results, duplicate
captures, Notes, Memories, Tasks, Chats, Users, or speculative relationships.
Official directory pages remain evidence and provenance for Entity selection.

### Mandatory Final Allowlist Before Import

The table above is the research target, not yet the executable import manifest.
Research completion must update this RD with a fixed row-by-row allowlist before
any database-write instruction is accepted. That final table must name every
record and contain:

- stable client key and exact canonical title;
- Object kind and `reuse` or `insert` action;
- canonical primary URL and evidence URLs;
- exact approved Object description and bounded provenance;
- for Sources, publication date, authors, publisher, Source kind, exact content
  artifact, SHA-256, byte count, and `retain` or `metadata_only` decision;
- every proposed Connection as exact source Object, allowed kind, target Object,
  and human-readable reason—or an explicit statement that there is no edge;
- dependency order and expected post-import Object, subtype, content, Connection,
  and Object Event counts.

The final allowlist must contain no `candidate`, “such as”, unnamed portfolio
publication, unresolved identity, placeholder payload, open-ended discovery, or
range of possible counts. Each researched item must instead be marked `reuse`,
`insert`, `defer`, or `reject`. Brad reviews that amended RD and manifest; only a
later explicit approval authorizes importing exactly the `reuse` and `insert`
rows. Any new record discovered after approval requires another RD amendment and
approval rather than being silently added during execution.

## Contract

- **Goal:** Prepare a small, high-signal Paradigm research graph that can be
  added to Centaur Context safely and efficiently.
- **Done:** Every candidate and rejection has primary-source evidence and a
  relevance reason; all accepted Entities and Sources reconcile against current
  and pending corpus records; proposed payloads fit the current schema; retained
  content has verified identity and hashes; and an exact import/reuse plan is
  ready for requester approval.
- **Files:** This RD only in Git. Store public-source snapshots, full captures,
  dossiers, manifests, hashes, and reconciliation artifacts under a private
  task-specific directory outside Git. No product code or migration changes.
- **Agent owns:** Read-only Context inspection, web research, editorial ranking,
  identity resolution, deduplication, capture assessment, manifest preparation,
  offline validation, and the proposed import sequence.
- **Requester owns:** Final candidate approval and any later hosted database
  write, credential enablement, deployment, publication, or spending decision.
- **Out of scope:** Import execution; exhaustive Paradigm staff or portfolio
  coverage; investment advice; speculative affiliations; monitoring;
  schema or ontology changes; public ingress; external publishing; Supabase,
  `ai_v2`, Console, or direct database access.

## Selection and Representation Rules

Paradigm is mandatory as an Entity if not already canonical. A person is not
included merely for seniority: require official current affiliation plus direct
evidence of authorship, research, investment, or building relevant to the target
themes. A backed organization requires official Paradigm investment evidence and
primary-source evidence of substantive target-theme work. Prefer a small set that
improves retrieval over a complete directory.

Score candidates transparently on direct thematic relevance, demonstrated work,
relationship to Brad's corpus, currentness, and durable value. Scores aid review
but do not replace judgment. Record borderline and
rejected candidates so later refreshes are incremental rather than repeated from
scratch.

Start with, but independently verify, Paradigm's RSI Simulator and linked
economics work; EVMbench; Solidus and other automated-research work; Centaur; and
officially AI-labelled investments such as Nous Research, Harmonic, Andromeda,
and Vana. Discover adjacent candidates from citations and official organization
research pages, not from generic search-result similarity.

Use the canonical Entity subtype for firms, people, labs, and projects only when
they are durable identities. Use Source plus immutable Source content for
retained publications. Preserve authorship, publisher, investment evidence, and
selection rationale in bounded provenance and descriptions. Create only
evidenced Connections whose current ontology meaning is exact; do not encode
investment, employment, or authorship as `about`, and do not weaken unsupported
semantics to `related_to`. Report relationships the ontology cannot faithfully
represent.

## Checks

- [ ] Primary official sources support identity, affiliation/investment,
  authorship, dates, and relevance; secondary sources are corroboration only.
- [ ] The ledger accounts for every considered person, organization, and Source
  as reuse, insert, defer, or reject, with no canonical URI/name/hash collision.
- [ ] Existing Enyu seed records—including *The Economics of Recursive
  Self-Improvement*—and pending expansion candidates are reused, not duplicated.
- [ ] Every accepted Source has a canonical identifier; every retained capture
  has a verified SHA-256, completeness/rights decision, and schema-safe size.
- [ ] Offline validation confirms exact payloads, stable keys, dependency order,
  allowed provenance, valid endpoints, and no unexplained Connection.
- [ ] The final report states exact proposed/reused/rejected counts, unresolved
  uncertainties, evidence links, and the later write/reconciliation procedure.
- [ ] `git diff --check` passes.

## Approval Boundary

This document authorizes read-only public research, authenticated read-only
inspection of the in-scope Centaur Context API, private artifact creation, and
offline validation only. It does not authorize database writes, credentials,
deployment, public ingress, publishing, contacting people, spending, deletion,
or changes to any external system.

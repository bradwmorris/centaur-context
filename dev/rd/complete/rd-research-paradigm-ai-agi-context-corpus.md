# RD: Research a Paradigm AI/AGI Context Corpus

**Status:** `complete`
**GitHub Issue:** [#51](https://github.com/bradwmorris/centaur-context/issues/51)
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
portfolio companies. Those leads were resolved into the exact import allowlist
below.

**Missing:** none. Brad approved the exact sealed manifest on 2026-08-30; the
bounded import and reconciliation are complete.

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

- [x] Produce an evidence-backed map of Paradigm, its most relevant people, and
  its backed organizations at the intersection of Brad's AI/AGI interests.
- [x] Identify the recent, durable research Sources worth retaining from
  Paradigm and the accepted organizations.
- [x] Deliver a deduplicated, import-ready candidate manifest and efficient
  reviewed import plan without writing to the database.

## Final Import Allowlist — Approved and Imported

Brad explicitly approved this exact sealed allowlist in the linked Codex task on
2026-08-30. No additions or substitutions are authorized.

The authoritative, schema-shaped manifest is private at
`~/.codex/private/centaur-context/issue-51/candidate-manifest.json`, SHA-256
`9fa5af9370d49647840bc75706bb6128550702603576e4a3e1fb9acc6c4e74b7`.
The exact replay-safe intake payload is `intake-batch.json`, file SHA-256
`9c8bea40e7d13d9e74a77e1124af61c46f69a1b8e66433d3001a363bf41a3462`.
It contains every URL, evidence URL, exact description, bounded provenance,
subtype field, retained body, locator, Connection reason, and dependency. The
tables below are the complete human review surface; there are no unnamed rows.

### Entities: 10 inserts

Every row is a protected canonical Entity Object. Its primary URL is the
`source_ref`; provenance is exactly `source_type=public_research`, that URL, and
the note “Official identity, current-role, and relevance evidence reviewed
2026-08-30.” Exact descriptions follow.

| Client key / title | Primary URL | Exact description |
| --- | --- | --- |
| `entity-paradigm` — Paradigm | `paradigm.xyz/about` | Paradigm is an investment firm and research organization that builds and backs frontier technology, including crypto, artificial intelligence, and robotics; its AI work includes recursive self-improvement economics, automated research, agent infrastructure, and AI security evaluation. |
| `entity-matt-huang` — Matt Huang | `paradigm.xyz/team/matt-huang` | Matt Huang is Paradigm's co-founder and managing partner, responsible for the firm's frontier-technology investing and strategic direction across crypto, AI, robotics, and related research-led opportunities. |
| `entity-dan-robinson` — Dan Robinson | `paradigm.xyz/team/dan-robinson` | Dan Robinson is a Paradigm general partner and researcher working across mechanism and market design, programming languages, automated research, formal verification, agent infrastructure, and the economics of recursive AI self-improvement. |
| `entity-justin-wang` — Justin Wang | `paradigm.xyz/team/justin-wang` | Justin Wang is a Paradigm research partner whose work spans AI capabilities, adversarial robustness and agent safety, AI security evaluation, and quantitative modeling of recursive self-improvement. |
| `entity-alpin-yukseloglu` — Alpin Yukseloglu | `paradigm.xyz/team/alpin-yukseloglu` | Alpin Yukseloglu is a Paradigm partner in investing and research with prior machine-learning research experience at OpenAI; he led Paradigm's work on EVMbench, an evaluation framework for AI agents performing smart-contract security tasks. |
| `entity-georgios-konstantopoulos` — Georgios Konstantopoulos | `paradigm.xyz/team/georgios-konstantopoulos` | Georgios Konstantopoulos is Paradigm's general partner and CTO, leading technical work and open-source systems including Centaur, Paradigm's secure multiplayer agent runtime and shared organizational context infrastructure. |
| `entity-nous-research` — Nous Research | `nousresearch.com` | Nous Research is an open AI research organization developing open-weight reasoning models, reinforcement-learning infrastructure, and Psyche, a decentralized system for coordinating large-scale model training over heterogeneous compute. |
| `entity-harmonic` — Harmonic | `harmonic.fun` | Harmonic is an AI research company building verifiable mathematical reasoning systems; its Aristotle system combines learned proof search, informal reasoning, and formal verification for mathematics and software. |
| `entity-andromeda` — Andromeda | `andromeda.ai` | Andromeda builds market infrastructure for AI compute, including cluster sourcing, benchmarking, certification, standardized contracts, workload matching, operations, and observability across infrastructure providers. |
| `entity-vana` — Vana | `vana.org` | Vana is an open network for user-owned AI data, using data liquidity pools and dataset-linked tokens to coordinate contribution, access, governance, monetization, and markets for model-training data. |

### Sources: 10 inserts, 9 retained bodies

All are protected Source Objects with `language=en`; article bodies use
`article_text`, papers use `paper_text`, and media type is UTF-8 plain text.
Descriptions, full bylines, evidence URLs, exact RFC-3339 dates, and provenance
are sealed in the manifest. `Psyche` is deliberately metadata-only because its
official Discourse host stopped resolving during capture; no partial text is
substituted. Andromeda states only 2026, so `published_at` is exactly null.

| Client key / exact title | Publisher; date; kind | Content artifact; bytes; SHA-256 |
| --- | --- | --- |
| `source-paradigm-rsi-simulator` — RSI Simulator | Paradigm; 2026-08-11; article | `paradigm-rsi-simulator.txt`; 5,830; `c4ea14482a0ebd42c4ab5306486b8bda9d16a09b408fa4cf365095f667a20025` |
| `source-paradigm-evmbench` — Introducing EVMbench | Paradigm; 2026-02-18; article | `paradigm-evmbench.txt`; 2,108; `462f3b95560894e88ca28f8b310811fee5d34efadcf1e852cb46e0d532b21ab1` |
| `source-paradigm-solidus` — Formally Verifying a Compiler Using Automated Research | Paradigm; 2026-07-24; article | `paradigm-solidus.txt`; 9,277; `f81f482a54fa528d5fe960d4e3c3ff0b92571c1c2bf8773b4a43cc895958a0e7` |
| `source-paradigm-centaur-2` — Centaur 2.0: Permissions, Context, and MCP | Paradigm; 2026-08-10; article | `paradigm-centaur-2.txt`; 7,274; `c7217b141a4e4590a12b2c5a041cbc29318d4238e7c2cae3efd8d720ad02e91c` |
| `source-paradigm-open-sourcing-centaur` — Open Sourcing Centaur: Multiplayer, self-hosted, secure agents | Paradigm; 2026-05-21; article | `paradigm-open-sourcing-centaur.txt`; 14,040; `332088506966a94d1da75cedce0cf9a27726a626fc7fe8e70fd32e59a5092471` |
| `source-nous-hermes-4-report` — Hermes 4 Technical Report | Nous Research; 2025-08-25; paper | `nous-hermes-4-technical-report.txt`; 159,253; `6430030246b5e9c1c3843946bb5dc11a140ab8af1414778df3517042473cc4cf` |
| `source-nous-psyche-future-directions` — Psyche - Future Directions | Nous Research; 2025-10-09; article | `metadata_only`; 0; null |
| `source-harmonic-aristotle-imo` — Aristotle: IMO-level Automated Theorem Proving | Harmonic; 2025-10-01; paper | `harmonic-aristotle-imo-level-atp.txt`; 74,399; `d55045e00994858e7dee9288b9850d1f16df2778798ae7897f0f7d711d8fc150` |
| `source-andromeda-gpu-hours` — A view from billions of GPU-Hours | Andromeda; null; article | `andromeda-gpu-hours.txt`; 5,215; `8245cc32df8efaf04833133d800e11cec49c3c738a83e46fc08b42b424a0d024` |
| `source-vana-solana-data-markets` — Vana Brings Data Tokens to Solana’s Onchain Capital Markets | Vana; 2025-05-22; article | `vana-vrc20-data-finance.txt`; 2,261; `d90a84d6f24aa579250d2b8cde2916cfa35d05adc4087a4eb12812024e23c5f8` |

### Reuse, defer, and reject

- Reuse Source `46f174c1-38ef-5958-a3de-a8238fe8f174`, *The Economics of
  Recursive Self-Improvement*; the live canonical URI is
  `https://elasticity.institute/rsi-paper.pdf`. Do not update or duplicate it.
- Defer Matthew Slipper as an Entity: he authors two retained publications but
  is absent from Paradigm's current official Team directory. Preserve authorship
  only in Source bylines.
- Reject Axiom and its reviewed ZK-heavy writing as too tangential; AI Arena as
  out of focus; Project Kryptos as lower-priority than Solidus; and the rest of
  Paradigm's staff and portfolio because this is not a directory import.

### Exact Connections and import sequence

The payload has 82 protected Connections: 40 `involves`, 39 `themed`, two
`about`, and one `derived_from`. Every inserted Object connects to live Brad
`74000b0a-…` (“Brad selected this Object…”) and Codex `9d11eaab-…` (“Codex
researched, normalized, and prepared this Object…”). Theme mappings are:

| Object keys | Exact approved Themes |
| --- | --- |
| Paradigm; Matt | Capital/Markets; Paradigm also AI Engineering |
| Dan | Jobs/GDP; AI for Science; Agents |
| Justin; Alpin | Frontier Models; AI Engineering |
| Georgios | Agents; AI Engineering |
| Nous; Hermes 4; Psyche | Open Models; Frontier Models for Nous/Hermes; AI Infrastructure for Psyche |
| Harmonic; Aristotle | AI for Science; Frontier Models |
| Andromeda and its Source | AI Infrastructure; Capital/Markets |
| Vana and its Source | Capital/Markets |
| RSI Simulator | Jobs/GDP; Frontier Models |
| EVMbench | AI Engineering; Frontier Models |
| Solidus | AI for Science; Agents; AI Engineering |
| both Centaur Sources | Agents; AI Engineering |

`RSI Simulator derived_from Economics of RSI` uses the paper's live Object ID.
Both Centaur Sources are `about Paradigm`. Investment, employment, and authorship
remain in descriptions/provenance because no exact ontology edge exists.

Import order is existing-ID preflight, 10 Entities, 10 Sources, nine contents,
then 82 Connections, all through the temporarily enabled, manifest-pinned,
authenticated bounded intake. Baseline is 366 Objects, 159 Entities, 133
Sources, 26 contents, 1,262 Connections, and 1,657 Events. Expected result is
386 Objects (385 active), 169 Entities, 143 Sources, 35 contents, 1,344
Connections (`about` 205, `derived_from` 63, `involves` 738, `themed` 338), and
1,768 Events. Replay must add zero rows. These were planning totals, not the
authoritative execution ledger.

### Execution result

The approved batch `paradigm-ai-agi-context-corpus-2026-08-30-v1` committed on
2026-08-30 with payload SHA-256
`feeff3d4b04db3557243713575c9afc2beffdca865bd0bc3220efca88a7487a8`.
The server recorded exactly 20 Objects, nine Source contents, 82 Connections,
and 111 Events. Immediate replay returned `replayed=true` and added no rows.
Readback matched all 20 exact titles and all nine content hashes; authenticated
full-text search retrieved the new corpus.

One unrelated editor-workflow acceptance Source arrived after the research
snapshot. The earlier content/Event totals were also inferred rather than read
from the schema endpoint. Authoritative post-import totals are therefore 387
Objects (386 active), 169 Entities, 144 Sources, 44 Source contents, 1,344 active
Connections (`about` 205, `derived_from` 63, `involves` 738, `themed` 338), and
1,805 Events. The batch ledger isolates exactly 111 of those Events. The
temporary manifest-pinned intake credential was removed and the listener was
confirmed disabled after reconciliation.

## Contract

- **Goal:** Add a small, high-signal Paradigm research graph to Centaur Context
  safely and efficiently after exact requester approval.
- **Done:** Every decision has primary-source evidence; the approved sealed batch
  committed exactly once; replay added no rows; Objects, contents, Connections,
  Events, hashes, retrieval, and disabled intake state reconcile.
- **Files:** This RD only in Git. Store public-source snapshots, full captures,
  dossiers, manifests, hashes, and reconciliation artifacts under a private
  task-specific directory outside Git. No product code or migration changes.
- **Agent owns:** Research, reconciliation, manifest preparation, offline
  validation, the explicitly approved bounded import, replay, and readback.
- **Requester owns:** Any later expansion, publication, or spending decision.
- **Out of scope:** Exhaustive Paradigm staff or portfolio coverage; investment
  advice; speculative affiliations; monitoring;
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

- [x] Primary official sources support identity, affiliation/investment,
  authorship, dates, and relevance; secondary sources are corroboration only.
- [x] The ledger accounts for every considered person, organization, and Source
  as reuse, insert, defer, or reject, with no canonical URI/name/hash collision.
- [x] Existing Enyu seed records—including *The Economics of Recursive
  Self-Improvement*—and pending expansion candidates are reused, not duplicated.
- [x] Every accepted Source has a canonical identifier; every retained capture
  has a verified SHA-256, completeness/rights decision, and schema-safe size.
- [x] Offline validation confirms exact payloads, stable keys, dependency order,
  allowed provenance, valid endpoints, and no unexplained Connection.
- [x] The final report states exact proposed/reused/rejected counts, unresolved
  uncertainties, evidence links, and the later write/reconciliation procedure.
- [x] The approved batch committed once, replayed without new rows, and its 20
  Objects, nine contents, 82 Connections, and 111 Events reconcile live.
- [x] The manifest-pinned intake credential was removed and its listener was
  confirmed disabled after import.
- [x] `git diff --check` passes.

## Approval Boundary

Brad's 2026-08-30 approval extended the original boundary only to importing the
exact sealed allowlist through the manifest-pinned private Context intake and
reconciling it. That authorization was consumed. It did not authorize additional
records, public ingress, publishing, contacting people, spending, deletion,
schema changes, `ai_v2`, Console, Supabase, or direct database access.

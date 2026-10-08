# Events and controlled Entity classifications — shared Context RD

## Outcome

Add a canonical real-world Event type and improve Entity classification without replacing the existing Object/subtype architecture. A user or authorized agent can create a dated occurrence, identify a named model/product/teaching/fund as an Entity, connect Sources and Notes to it, and retrieve/edit the result consistently through native UI, HTTP, CLI and desktop MCP. A classification is a defined category, not an arbitrary tag or another Theme.

Canonical shared Issue: https://github.com/bradwmorris/centaur-context/issues/136. This RD supersedes the original Event-only brief. Owner decisions, 8 October 2026: separate Events from Memories; retain Entities for the other identifiable things; use flexible controlled classifications; update agent decision guidance as well as tools; coordinate release to both private installations. Planning does not dispatch implementation.

## Repository scope

`bradwmorris/centaur-context` owns the shared schema, migrations, validators, API, search/read/apply contract, Python clients, packaged Codex MCP bridge, generic agent guidance, UI and documentation. Private adoption Issues own installation-specific pins/instructions/release. Do not add a custom-schema plugin platform, RDF store, graph database, calendar integration or Centaur runtime changes.

Keep common identity, title, description, lifecycle, provenance and revision in Objects. Keep one matching subtype per Object and use canonical Connections between Objects. Category catalogue and assignment rows are supporting metadata, not new knowledge Object kinds. Source documents and explanatory Notes retain their own identities; no conversion of existing Notes, Themes, Sources or Memories merely because their subjects now have Entity/Event records.

## Requirements

### 1. Event model and time

Add `kind: event`, with `real_world_events` keyed one-to-one to its Object. Preserve the meaning of immutable audit `object_events`; UI and guidance must distinguish “Event” from “change history”. Extend the deferred matching-subtype checks and deletion guards, not just the Object-kind CHECK constraint.

Fields: `starts_at`, `starts_at_precision`, `ends_at`, `ends_at_precision`, `timezone`. Endpoints are nullable strings with precision `year`, `month`, `day` or `instant`, validated by one shared validator. Examples: `2026`, `2026-10`, `2026-10-08`, `2026-10-08T14:00:00+11:00`. Never pad a year/month/day with an invented timestamp. Precise instants require RFC3339 offset; optional IANA timezone must agree with each supplied instant's offset, including daylight saving. Preserve the supplied precision and offset on round trip; calendar dates do not shift with viewer timezone.

Each endpoint and precision are present together or both null. Both endpoints can be unknown; a missing end is unspecified, not an inferred duration. When both are present require matching precision and end >= start; compare calendar values in calendar order and instants in UTC. Equal instants are allowed. Date/day/month/year ranges include the named end period. Mixed-precision ranges are outside this initial release and produce an actionable validation error rather than fabricated precision. Updates validate the merged state, preserve omitted fields and clear explicit null endpoint/precision pairs together.

Events can describe past, current or planned occurrences; a future date is not proof of attendance/completion. Rescheduling the same occurrence updates its Event; a different occurrence gets a different identity. No recurrence engine or new occurrence status machine in this task.

Use existing explained Connections: participants via `involves`; place via `related_to`; Notes `about` the Event; evidence via `derived_from` where justified. Do not add relationship types merely to avoid writing an explanation. Keep Memory extraction/maintenance operating only on its existing scope. No automatic duplicate Memory saying only that an Event exists; a separate meaningful interaction may have a Memory connected to the Event.

### 2. Entity classification model

Retain `kind: entity` and the `entities` subtype. Add a small database-backed `entity_categories` catalogue and `entity_category_assignments` join table; categories have stable UUID, immutable normalized slug, display label, concise definition, alternative labels, active/archived state, revision and attributed creation/update metadata. Uniqueness of slug and unambiguous normalized label/aliases is enforced; aliases are names for a category, not aliases for individual Entities. Category names/definitions are stored data, never prompt instructions. No taxonomy hierarchy, inference engine, arbitrary property bag or automatic external vocabulary import.

Allow multiple meaningful classifications on one Entity, with one explicitly selected primary category. Store the primary reference on `entities`; require it to be among assigned categories. Seed the existing eight classifications (person, organization, product, project, publication, place, concept, other), plus model, benchmark, framework, method, teaching and fund. Give each a short inclusion/exclusion definition. The shared seed IDs are deterministic; each installation owns its subsequent catalogue records independently.

Definitions should distinguish: a model represents or estimates a system; a benchmark is a named evaluation instrument; a framework organizes reasoning; a method describes a procedure; a teaching is a named body of teaching; a fund is a particular pooled investment arrangement. Product can coexist with model/benchmark when both are evidenced. Do not infer that every model is commercial, or that a fund is its trustee/provider. `other` remains a fallback, not an invitation to create one category per Entity.

Preserve the legacy `entity_kind` field as a compatibility projection. Each category has an explicit `legacy_kind` from the old eight values: old categories map to themselves; model/benchmark/framework/method/teaching initially map to concept, fund to other. The primary category determines the legacy projection. API responses return canonical `category_ids`, `primary_category_id`, useful category labels/definitions when requested, and the legacy field. This is a deliberate compatibility bridge, not two independently editable sources of truth.

Migration creates one assignment and primary category matching every existing entity_kind, preserving original Entity UUIDs, kind values, titles/descriptions, revisions and evidence. Do not re-infer historical categories. Enforce assignment/primary/legacy consistency on every writer, including legacy intake/networking calls. Legacy create with only entity_kind maps to its seeded category. Legacy update with an unchanged entity_kind preserves richer categories. A legacy attempt to change entity_kind on a richly classified Entity must return an actionable conflict requiring the new fields, rather than erase categories silently. Simple legacy-only Entities can change their singleton category via old fields. New calls supplying both old and new fields must agree or fail atomically.

Classification assignment updates are ordinary revisioned Entity updates, included in Object before/after snapshots and recovery/undo. Unknown, archived or duplicate category IDs cannot be newly assigned. Archived category definitions remain readable on historical/existing assignments; do not cascade-delete assignments. Category archive rejects while actively assigned, requiring an explicit reassignment first. No automatic catalogue merge/rename of identifiers.

### 3. Catalogue tools and lifecycle

Keep the three existing universal tools. Extend `context_read` with `include: ["entity_categories"]`, returning the compact catalogue and a catalogue revision/hash. Permit empty object_ids only for that explicit catalogue request; existing reads still require IDs. Carry this exception through HTTP, Python, CLI and MCP schema. `context_read --entity-categories` is the CLI convenience. Catalogue filtering/pagination may follow existing bounded response conventions; seed catalogue is small, and returning active definitions plus referenced archived entries is sufficient.

Extend `context_apply` with `create_entity_category`, `update_entity_category`, `archive_entity_category`, `restore_entity_category`. Create requires slug, label, definition, legacy_kind; aliases optional. Updates/archive/restore require category_id and expected_revision. Identity slug is immutable. Use the existing atomic transaction, stable request key and Run receipt patterns. Catalogue changes have immutable attributed before/after history; extend the existing audit target handling with an explicit entity_category target, never masquerade categories as knowledge Objects or Connections. Include category snapshot/restore and conflict checking in the corresponding recovery path; category undo must not strand assigned Entities. Avoid a new parallel audit service.

Normal authorized record work permits assignment of existing categories. It does not imply permission to invent or rename the shared vocabulary. Agents may propose a missing category with definition and example; an explicit user instruction to add that category authorizes its mutation through the same API. No new identity/grant system or approval boolean accepted from callers. Preserve each transport's current authenticated permissions; UI users can deliberately manage the catalogue. A successful category mutation invalidates catalogue caches; agents may reuse a known unchanged catalogue within a session, refreshing on mismatch instead of retrieving it before every write.

Expose deterministic Entity category filtering through the existing search/list surfaces: `entity_filters` with `category_ids`, `match: any|all` (default any), and cursor using established pagination conventions. Allow an empty text query when this explicit filter is supplied; reject conflicting non-Entity kind filters. Document SQL/list ordering and cursor semantics consistently. Keyword search remains the existing title/description discovery contract; do not silently broaden it to Note bodies or all evidence. Category aliases resolve to category IDs through the catalogue, not guessed free-text labels.

### 4. Agent behaviour

Put one concise canonical decision rule in the shared instructions, with complete definitions and examples in discoverable contract/guide data. Keep runtime guidance short. Teach these distinctions:

- Entity: a specific identifiable person, organization, product, model, framework, teaching, fund or other subject that merits a stable reference.
- Event: one actual or planned occurrence, with only supported timing.
- Source: evidence/document/work about something; preserve captured material separately.
- Note: an atomic Idea, Excerpt or Fact, including explanations of concepts.
- Theme: an organizing research/life subject. It need not become an Entity just because it has a name.
- Memory: a meaningful remembered interaction/outcome; do not use it as a substitute for the canonical Event solely because an occurrence has dates.

Consider an Entity when the user explicitly requests it, it is central to authorized research/capture, or its stable identity materially improves connecting/retrieving knowledge across Sources, Notes or conversations. Multiple existing mentions are useful evidence, not a mandatory prerequisite. Do not make an Entity for every noun, passing mention, sentence or generic adjective. A one-off personal idea often remains a Note. A topic label alone can remain a Theme. Where record creation is outside the current instruction's authority, suggest the candidate instead of silently writing it.

Search the intended store for names and relevant distinguishing context; read plausible matches. Same title does not prove identity, and an empty lexical result does not prove absence. Resolve ambiguity without merging unrelated things. Reuse an existing Entity where supported. Preserve named product identity separately from its creator; do not infer category solely from capitalization or a keyword. Retrieve complete evidence before asserting unsupported details.

Choose the smallest useful supported set of categories; select primary according to the thing being represented. “Northstar Cost Model” can be model + product if it is actually offered as a product. A source explaining the “Four Principles” remains a Source; a summary remains a Note; the named teaching can have an Entity connected to both. A particular retirement fund is distinct from the provider, trustee, account, general finance topic and strategy document.

Use a single apply batch with local refs for Entity/Event/Note/Connection creation where feasible, current revisions for edits, stable idempotency keys for retries and compact readback. Do not conduct whole-store scans or rediscover tool source/schema for each item. Same-category definitions are reused; genuinely missing categories are proposed rather than silently proliferated. Ordinary requested record writes do not require repeated confirmation.

The feature does not expand automatic capture/curation or external messaging authority. Background source ingestion can link existing meaningful Entity/Event candidates; missing candidates are reported for authorized interactive handling unless its workflow already explicitly permits creation. Classification must never turn an incidental Entity into a networking opportunity, send, publication, Task or workflow dispatch.

### 5. UI and correction parity

Add Events to Object types, navigation, collection filtering, creation, full detail/edit, Connections, graph visuals and source/Note association pickers. Use date precision-aware controls and show unknown timing honestly. Entity creation/edit uses the live catalogue, supports multiple category selections plus primary, displays human labels, and filters by categories. Include a simple catalogue management surface with definitions/aliases and explicit category creation/edit/archive; avoid building a taxonomy editor.

Ensure Event fields and Entity classifications appear in universal reads, Context Builder packets, graph/detail reads, audit snapshots, revision diffs, archive/restore and supported recovery. Event Object protection and explicit corrections follow existing policy. Changing an existing Object's immutable kind is not a shortcut to migration; preserve original record identities. Explanatory Notes and captured Sources continue to stand on their own even when new Entities are added.

## Approach

Audit baseline: local shared HEAD and both observed live Context images are `82384fc7fd8e801d8dac02d738124eea532cfefd` on 8 October 2026. Live contract/tool 1.1.0, ontology 3; eight Object kinds. Recheck current main and deploy state before execution; do not force an older baseline over newer work.

| Surface | Inspected implementation / required change |
| --- | --- |
| Ontology/storage | `migrations/0002_canonical_ontology.sql`, `0003_canonical_graph_contract.sql`, `0016_canonical_cleanup.sql`; next additive migration must extend kind CHECK, subtype triggers and category storage/backfill. Never edit historical migrations. |
| Universal mutations | `src/universal.rs` has duplicated fixed Entity kinds in create/update and a Memory-specific timestamp path. Integrate Event, catalogue operations, new classification fields and compatibility conversion in shared helpers. |
| Other writers | `src/api.rs`, `src/db/objects.rs`, `src/intake.rs`, `src/networking_mutation.rs`; avoid one path persisting entity_kind without assignments. Legacy behavior must remain explicit. |
| Read/search | `src/db/retrieval.rs` CASE/JOIN subtype queries; `src/api.rs` filters; `src/domain.rs` kinds; `src/universal.rs` request types and search. Extend both agent and human surfaces. |
| Evidence/recovery | `src/db/events.rs::target_snapshot` currently snapshots only the entities row. Include assignments/Event subtype; trace Run undo/correction/recovery consumers and update all relevant branches. Extend category audit targets explicitly. |
| Contract/client | `contract/context-contract.json`, schema, `src/contract.rs`, `tools/centaur_context/{client,search,read,apply}.py`, package version metadata. Version new capability and document old payload compatibility; no mismatched version constants. |
| Desktop MCP | `tools/codex_context/codex_context/{contract,bridge}.py`, `pyproject.toml`: schemas are packaged from a static contract, read currently requires 1–20 IDs, and bridge description hardcodes 1.1.0. Repackage/reinstall and verify effective schema/guidance, not only server code. |
| Agent guidance | `generated/context-agent-instructions.md` and `services/sandbox/SYSTEM_PROMPT.md` must match; `scripts/check-contract.py` currently insists on eight kinds/exact operations and a 2,000-character guidance budget. Update validation to the actual new contract; keep short decision guidance in budget and detailed examples discoverable. Do not merely append a long essay. |
| UI | `web/src/{types,App,api,routing,RecordVisuals}.tsx/ts`, graph modules and collection/detail components. Current Entity form embeds eight options. Replace with catalogue data; add Event routes, typed payloads and precision controls. |
| Memory boundary | `src/{memory,curator,dreaming}.rs`, `docs/memory.md`; preserve capture/maintenance eligibility, avoid treating real-world Events as audit Events, add retrieval/link awareness only where existing policy permits. |
| Docs/package | `docs/{schema,context-contract,api,codex,operations,memory}.md`, generated schema materials, `compatibility.toml`, examples and `scripts/check-{contract,api-docs,package}.py`. |

Release control: add a minimal `--migrate-only` startup flag that exits after normal SQLx migrations, before listeners/workers. Add an explicit worker-disabled maintenance configuration for controlled verification: suppress capture, curation, review and other background writers while retaining normal authentication. Producer ingress is gated by the operator; this is not an agent permission bypass. Preserve prior defaults outside the release window.

Execution sequence: isolated branch from current main → additive storage/compatibility helpers → complete API/contract/client/MCP → UI and concise guidance → checks/self-review/draft PR → coordinated approved release. Complete the slice rather than stopping after schema or API. Use the existing standards as design references, not implementation dependencies: [RDF identity and relationships](https://www.w3.org/TR/rdf11-primer/), [SKOS labels and definitions](https://www.w3.org/TR/skos-primer/), [Schema.org additional types](https://schema.org/Thing).

## Acceptance

### Proportionate verification and explicit owner exception

Bradley explicitly requested on 8 October 2026 that the executor **not provision local test databases or rehearse this schema change in a separate database**. This task-specific direction supersedes the earlier RD requirement to build/use a local disposable database. Do not run the destructive database test suite against either live store as a substitute.

Run formatting, static/type/lint checks, package/contract checks, pure temporal/category validation tests, existing client/MCP tests, web tests/build and diff checks. Local database tests with no TEST_DATABASE_URL are skipped, not “passed”; report that honestly. Existing GitHub CI already provisions PostgreSQL automatically and runs the guarded suite; retain that existing check, add the necessary focused integration case there, and reuse its result. Do not add a local cluster, test DB, migration rehearsal, benchmark programme or repeated full regression runs. No user setup chores. If CI fails, diagnose/fix the real failure; do not disable it. Database fixture code remains guarded for CI; no live DSN is ever passed to it.

Minimum useful feature evidence:

1. Pure validator cases: date/month/year/instant/unknown; equal instants; invalid/reversed/mixed-precision endpoints; zone mismatch and DST-aware round trip. Category normalization, duplicates, inactive IDs, primary membership and legacy projection consistency.
2. One existing-CI-harness atomic flow: create Event + classified Entity + two Notes + explained links, retrieve/filter/edit, replay same key, stale-revision and invalid batch rejection; complete before/after snapshots and supported restore/undo; unchanged pre-existing Memory and legacy Entity. Include legacy networking/intake compatibility without creating a duplicate full scenario suite per transport.
3. Category catalogue lifecycle via tools: read without Object IDs; create an explicitly approved synthetic category; reuse on an Entity; update/alias discovery; stale catalogue update rejection; archive blocked while assigned; safe unassign/archive/restore. No unbounded vocabulary proliferation.
4. Actual native UI check: Event precision fields/detail/filter, Entity multi-category edit/primary/filter and catalogue definitions. Use synthetic public examples and screenshots, not private overlay records.
5. Agent judgement cases: central model -> reuse/create classified Entity; passing mention -> no automatic Entity; explanation -> Note about Entity; dated retreat -> Event; distinct reflection -> Note linked to Event; broad topic -> Theme; category missing -> concise proposal; identity ambiguous -> resolve instead of duplicate. These can be one representative execution per distinct transport plus deterministic payload checks, not an exhaustive model benchmark.

## Completion

Preferred executor Luna High once dispatched with this complete brief; ordinary implementation choices are permitted. Resolve the design above consistently, not by inventing another schema architecture. If current code conflicts materially, preserve work and escalate the concrete discrepancy; two repeated unexplained failures trigger reassessment. No subagent or Codex task is dispatched by this RD.

Deliver focused draft PR(s), self-review, passing required checks, explicit local skips and API/UI evidence. The coordinating private RD specifies the controlled pause, verified backup, additive migration, rollback conditions and both deployments. Existing concrete-revision release approval remains; after approval, continue through release and live verification without seeking permission for each mechanical step. Done requires adopted server/UI/tools and correct agent behaviour in both intended installations, not merely merged code. Do not claim existing private records were reclassified unless a separate approved record manifest was applied and read back.

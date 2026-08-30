# 1 — RD: Build the Enyu Ops UI

**Status:** `complete`
**Created:** 2026-08-31
**GitHub Issue:** [#60](https://github.com/bradwmorris/centaur-context/issues/60)

## Execution Plan

**Status:** `complete`

**Basis checked:** Repository-root and development instructions; active and
completed Centaur Context RDs; the current ontology, migrations through Sources
and Notes, human/agent/Note/ingestion API trust boundaries, optimistic revision
contract, Object Events, Evals, schema visualizer, React/Vite UI, tests,
Kubernetes installation, and local-only operating model; the Centaur and private
`centaur-enyu` overlay boundaries and deployment manifests; the local and hosted
The AGI Post Ops task, Kanban, theme, authentication, and multi-pane
implementations; the signed-in Linear issue list and issue-detail interactions;
and the currently running Centaur Context Objects, Tasks, Users, Entities,
Memories, Sources, Notes, Themes, Curator Runs, Evals, and Schema surfaces.

**Missing:** none. Repository creation, the live Entity-image migration after backup and local testing, and small live acceptance mutations were explicitly approved on 2026-08-31. Cloud hosting, public
DNS, internet login, tunnels, and production deployment are deferred to a
separate future RD.

**Post-execution UI amendment — 2026-08-31:** Direct requester review supersedes
every multi-panel, split-pane, tab, resize-divider, and “open beside” requirement
below. Enyu Ops now uses one continuous workspace. Every collection must copy the
current Context Objects list structure: full-width title, restrained add control,
search/count strip, collection header, and compact single-line rows. Only Task
board/calendar/detail and the focused Note editor intentionally depart from that
collection structure. Any historical pane wording below is controlled by this
amendment.

1. Freeze the product and compatibility contracts, create the isolated private
   `enyu_ops` repository only after approval, and record the exact current
   Centaur Context API/schema version against which it is built.
2. Make the sole product change allowed in Centaur Context: add a canonical,
   optional Entity image URL and the narrow human API read/write contract needed
   to display and edit it. Preserve every existing client and all local,
   canonical Object, subtype, Object Event, Eval, protection, and immutable
   Source-content behavior. Make no Task, Note, pagination, Theme, auth, or other
   Context product change.
3. Build the Enyu Ops application foundation: a small server-side API gateway,
   loopback-only access boundary, typed Context client, route model, design tokens,
   dark/light themes, error/loading/empty states, and one accessible continuous
   workspace.
4. Deliver current-surface parity, then the three focused product experiences:
   Linear-style Task management, an Obsidian-style Note workspace, and CRM-style
   Entity records. Keep secondary surfaces concise and reuse common list,
   identity, relationship, activity, property, and editing primitives.
5. Prove security, correctness, accessibility, responsive behavior, visual
   quality, conflict recovery, performance, and portability in the local-only
   environment. Produce a clean future-hosting handoff, but do not expose,
   deploy, or add internet authentication in this RD.

## What We Are Doing

- [x] Create a private application repository at
  `/Users/bradleymorris/Desktop/dev/enyu_ops` that presents the canonical
  Centaur Context data through a substantially more usable operations UI without
  creating a second ontology or database.
- [x] Preserve access to Objects, Tasks, Users, Entities, Memories, Sources,
  Notes, Themes, Curator Runs, Evals, and Schema, with the same or narrower write
  authority as the current human UI; include Chats for complete parity.
- [x] Let an authorized human create and manage Tasks through fast list, board,
  calendar, and detail views with useful due-date presentation, client-side
  filtering and deterministic automatic ordering,
  ownership, status, priority, relationships, and activity.
- [x] Let an authorized human create and edit Notes in a large, calm, Markdown-
  first workspace with safe preview, autosave feedback, keyboard support,
  optimistic-concurrency protection, and recoverable unsaved work.
- [x] Present Entities as useful CRM-style profiles centred on description,
  generic identity metadata, connections, related records, and activity rather
  than a raw database record.
- [x] Provide one continuous workspace where lists and details replace each other
  predictably, URLs and Back/Forward remain meaningful, and narrow screens retain
  a usable compact list.
- [x] Provide complete, intentionally designed dark and light themes with no
  flash of the wrong theme and no surface-specific unreadable states.
- [x] Run Enyu Ops locally through a loopback-only application boundary that
  keeps database access out of the browser and agents, and document the larger
  future job required to host it safely at `ops.enyu.org`.
- [x] Produce implementation and verification evidence strong enough that
  reusable API and UI primitives can later be proposed upstream to Centaur or
  Centaur Context without Enyu-specific policy leaking into the reusable repos.

## Contract

- **Goal:** Provide one elegant, minimal, secure Enyu operations application for
  managing the existing Centaur Context workspace, with exceptional Task, Note,
  Entity, navigation, and theme experiences.
- **Done:** A human with access to the local machine can open Enyu Ops on its
  loopback-only URL, browse all agreed Context surfaces, perform every currently
  supported human mutation,
  complete the Task/Note/Entity acceptance scenarios below in both themes and
  supported layouts, and observe the same canonical records and immutable audit
  trail in Centaur Context. Browser, contract, security, accessibility, visual,
  performance, failure, backup/recovery, and full repository checks pass against
  the local release build. No browser or agent receives a database DSN.
- **Files:** New application only in
  `/Users/bradleymorris/Desktop/dev/enyu_ops`; reusable schema, API, client,
  migration, tests, and operational documentation only in
  `/Users/bradleymorris/Desktop/dev/centaur-context`; Enyu deployment values or
  private policy only in `/Users/bradleymorris/Desktop/dev/centaur-enyu` if that
  overlay genuinely owns them. Do not change `/Users/bradleymorris/Desktop/dev/centaur`
  unless execution proves a separately reviewed reusable integration defect.
  The AGI Post repository is read-only reference material.
- **Agent owns:** After separate execution approval, narrow local repository
  scaffolding, implementation, compatible migrations, fixtures, typed clients,
  documentation, tests, local builds, synthetic end-to-end verification,
  deployment manifests/plans, threat model, exact permission inventory, and
  preview/production verification within each approval boundary.
- **Requester owns:** Approval to create and publish the new private GitHub
  repository, approval for live canonical Context mutations, any supplied Entity
  image URL and its right/provenance, and the later decision to plan cloud
  hosting, public DNS, login, availability, credentials, and costs.
- **Out of scope:** Replacing Centaur or Centaur Context; creating a parallel
  Supabase ontology; browser-to-database access; giving an agent or client a DSN;
  arbitrary SQL or schema editing; copying The AGI Post code, data, credentials,
  schema, or business rules wholesale; a public or multi-tenant product; native
  desktop/mobile apps; offline-first replication; real-time collaborative
  editing; analytics, chat, AI copilots, workflow builders, external CRM sync,
  bulk email, Entity-image uploads or storage buckets, file storage, or automatic
  agent execution; cloud hosting, public ingress, `ops.enyu.org`, and internet
  login; and the confirmed Task/Note/Entity exclusions.

## Product Principles

1. **Canonical Context, one write path.** Enyu Ops never owns a second copy of
   an Object. Reads and writes go through the versioned local Context HTTP
   contracts. All writes retain actor attribution, expected revision,
   idempotency, Object Events, and Eval behavior.
2. **Minimal before comprehensive.** Every control must support a real operation.
   Prefer one excellent list, one excellent detail view, and composable property
   controls over dashboards, cards, decorative charts, or configuration layers.
3. **Fast paths stay visible.** Creation, search, status, due date, ownership,
   opening a record, and returning to a list must take few predictable actions
   and work from the keyboard.
4. **Progressive disclosure.** Lists show only scan-critical identity and state.
   Details expose description, properties, relationships, and history without
   turning every record into a dense form.
5. **No false editability.** Immutable Source versions remain immutable. Schema
   stays read-only. Protected records, Curator undo, Eval annotation, archival,
   and other guarded operations keep their existing server-enforced semantics.
6. **Reusable core, private shell.** Generic Context API improvements and truly
   reusable React primitives may be upstream candidates. Enyu naming, local
   defaults, design, and future deployment configuration stay private.
7. **Measured quality.** “Looks good” is proven through explicit viewports,
   themes, realistic seeded data, screenshots, interaction tests, accessibility
   checks, and human acceptance—not inferred from a successful build.
8. **Existing Context UI is the visual source of truth.** Default to the current
   Centaur Context Objects list/detail language demonstrated by
   `/objects/44b1923e-891f-4e75-bf3e-4dc4cefc5800`: a calm continuous canvas,
   vertically stacked sections, clean horizontal rows, compact properties,
   subtle separators, restrained pills, and clear inline avatar stacks. Extend
   that system for new interactions; do not replace it with a generic dashboard
   or imitate another product’s visual skin.
9. **Specialized workspaces use the strongest specialist reference.** The shared
   shell and generic records follow Context, but the Task list/Kanban/calendar/
   detail experience should reach Linear-level polish, density, speed, inline
   property handling, filtering, keyboard behavior, drag feedback, and state
   clarity. The Note list/editor/preview experience should reach Obsidian-level
   calm, writing focus, Markdown ergonomics, typography, navigation, save
   confidence, and content readability. Harmonize both with Context tokens and
   avatars; do not flatten them into generic Context forms.

## Confirmed Product Decisions

- Use a private repository named `bradwmorris/enyu_ops` at the exact requested
  local underscore path.
- Use Next.js and TypeScript for the local UI/BFF, with framework-light React
  domain components and a generated or hand-maintained typed Context client.
  Pin dependencies and commit the lockfile.
- Run the application locally for this RD. Bind the app/BFF to loopback, reject
  non-loopback hosts and cross-origin mutations, and call the existing local
  Centaur Context human API server-to-server. Add no login screen: possession of
  local-machine access is the current human boundary, as it is for Context.
- Defer Vercel, Cloudflare Access/Tunnel, Namecheap/Cloudflare DNS,
  `ops.enyu.org`, always-on infrastructure, and hosted PostgreSQL to a separate
  future RD. Keep the application structure compatible with a future BFF-hosted
  deployment, without building unused hosted abstractions now.
- Treat Tasks v1 as list + Kanban + calendar + detail, excluding projects,
  labels, subtasks, comments, recurring rules, estimates, cycles, custom fields,
  SLAs, and automations.
- Treat Entities v1 as minimal generic profiles without email, phone, postal
  address, type taxonomy, pipeline stage, or other CRM/PII fields. Include
  description, lifecycle, explained relationships, related records, provenance,
  activity, and an optional image URL for a person avatar or entity logo. Do not
  build image uploads or a storage bucket.
- Treat Notes v1 as Markdown with debounced autosave, explicit save shortcut,
  safe preview, draft recovery, and conflicts; exclude folders, backlinks,
  graph view, plugins, attachments, block references, and collaboration.
- Include Chats for full UI parity, but preserve their current server-authorized
  behavior instead of inventing chat composition or messaging.
- Use the current Centaur Context UI as the primary design and layout reference,
  preserving its minimal dark canvas, typography hierarchy, vertical stacking,
  row treatment, metadata pills, avatar stacks, spacing, and quiet borders.
  Derive a faithful light theme from the same structure. Within the Task surface,
  Linear becomes the primary feature, interaction, information-design, and polish
  reference. Within the Note surface, Obsidian becomes the primary editor,
  writing, navigation, typography, and polish reference. Both remain visually
  integrated with the Context shell rather than looking like embedded products.

## Architecture

### Repository and dependency boundaries

- Start `enyu_ops` from an empty repository; do not fork The AGI Post Ops. Record
  reference observations and independently implement only the patterns that fit
  this product.
- Keep these layers explicit: routes and loopback server gateway; local access
  boundary;
  generated/typed Context transport; domain queries and mutations; reusable UI
  primitives; surface modules; workspace/layout state; and tests/fixtures.
- Do not import files across local repositories. Shared behavior moves only
  through a reviewed package or a compatible Context HTTP contract.
- Add a short compatibility manifest stating the supported Context API version,
  required optional capabilities, and fail-closed behavior for unsupported
  server versions. Show an operational error, not a partially functional UI,
  when the contract is incompatible.

### Request and trust flow

1. The browser requests Enyu Ops through an explicit loopback URL. The local
   server rejects non-loopback Host headers and cross-origin state-changing
   requests; it does not listen on the LAN or internet.
2. The browser calls only same-origin Enyu Ops routes. Mutations include origin/
   CSRF protection and a per-operation idempotency key. Configuration never
   enters client bundles, browser storage, rendered HTML, analytics, or errors.
3. The server gateway validates a narrow request schema, supplies the configured
   local human actor, and calls only allowlisted routes on the loopback Centaur
   Context human API. It is not a generic HTTP proxy.
4. PostgreSQL remains reachable only by the Context service. Existing agent,
   Note-write, Slack-ingestion, Curator, Source-intake, and bootstrap listeners
   keep separate credentials and are never routed through Enyu Ops.
5. A future cloud RD must replace this local trust assumption with real user and
   service authentication before any public ingress exists. This RD must not
   leave a dormant remote-access or password mechanism behind.

### Context API evolution

- Preserve the local human API boundary. Do not add hosted-human authentication,
  a non-loopback listener, CORS exposure, or public ingress in this RD.
- Use the existing human API and actor behavior for every non-image operation.
  Enyu Ops must adapt to the current list bounds, filters, response shapes,
  idempotency, revision conflicts, and Theme contract rather than expanding them.
- Add only the narrow compatible Entity-image read/write capability. The write
  must require expected revision, use current actor attribution/idempotency, bump
  the canonical Object revision, and create the normal Object Event/Eval evidence.
  Existing response fields and clients remain compatible.
- Reuse current Object, subtype, Connection, Object Event, Eval, Source content,
  Curator, and Schema contracts. Do not add UI-only tables to Context.
- At execution time, reconcile the checked-out Theme implementation with the
  currently running UI for client compatibility only. Reuse its canonical Theme
  contract without modifying Context or modeling Themes in a parallel store.

### Data changes proposed for validation

- Tasks, Notes, all list/query behavior, and every non-Entity-image schema/API
  contract remain unchanged. Enyu Ops filters and sorts the currently returned
  data locally and documents the existing bounded-result scalability limit.
- Add only a validated optional HTTPS `image_url` to the generic `entities`
  subtype. It may represent a person avatar or organization/product logo. Store
  no Entity kind, image bytes, local filesystem path, Supabase bucket reference,
  contact data, aliases, pipeline stage, primary URL, or related-ID array.
- Entity images are human-curated references, not automatically scraped or
  downloaded. Render them lazily with `referrerPolicy="no-referrer"`, bounded
  dimensions and crop behavior, useful alt text in details, and deterministic
  local initials/color when absent or broken. Validate scheme/length server-side;
  do not add a remote image proxy, third-party avatar generator, or SSRF-capable
  fetcher. Record source/provenance when the URL is created or changed.
- Any migration is forward-only, preserves every existing row, sets neutral
  defaults, is safe for legacy `centaur_os` installations, and is covered by
  upgrade and disposable-database integration tests.

## UX Foundation

### Navigation and routing

- Use a compact primary rail containing the agreed surfaces. Keep direct,
  refresh-safe URLs for every list, record, and filterable Task view without
  embedding secrets.
- Opening a record replaces the active collection in the single workspace.
  Browser Back/Forward restores meaningful navigation rather than only local
  component state.
- Global search may search canonical Objects through the existing bounded API
  and open a result in the workspace. It does not become a command palette or AI
  assistant in this RD.

### Single workspace

- Use one full-width continuous content canvas beside the primary rail. Do not
  render tabs, split panes, drag dividers, resize handles, or open-beside actions.
- Collections share one Context-derived structure and one-line row anatomy.
  Details and specialist Task/Note modes replace the collection in place.
- Preserve direct URLs, Back/Forward behavior, visible focus, reduced motion,
  and a compact narrow-screen layout without persisting layout state.

### Visual system and themes

- First extract and document the current Context tokens and component patterns
  from the running reference and checked-in CSS: navigation rail, content width,
  typography scale/weight, row heights, section rhythm, separators, pills,
  editable title/description treatment, relationship direction, activity rows,
  avatars, focus, hover, and disabled states. Reproduce their intent in Enyu Ops
  rather than inventing a parallel visual system.
- Define semantic tokens for canvas, rail, panel, elevated surface, borders,
  primary/secondary/muted text, accent, status, focus, danger, shadows, radius,
  spacing, typography, and motion. Surface modules consume tokens, not hard-coded
  dark colors.
- Resolve initial theme before hydration using a small safe bootstrap; follow a
  saved explicit choice, then system preference. Toggle immediately, persist the
  choice, update `color-scheme`, and test both system changes and hydration.
- Default collection surfaces to full-width vertically stacked rows rather than
  card grids. Default detail surfaces to one continuous vertical stack:
  identity/title, compact properties, editable description/content, actions,
  relationships, activity, and collapsed provenance. Keep avatars crisp and
  consistently sized in identity and participant stacks, with overlap, borders,
  accessible names, lazy loading, and deterministic fallback handled uniformly.
- Use compact rows, quiet one-pixel boundaries, restrained radius, strong type
  hierarchy, and whitespace in writing/detail views. Avoid gradients, oversized
  headings, floating card mosaics, excessive containers, novelty icons, and
  animation without state meaning.
- Specialized Task list/board/calendar/detail and Note list/editor/preview views
  should deliberately depart from the generic row/detail structure wherever the
  Linear or Obsidian reference provides a materially better working experience.
  Preserve Context’s controls, type, borders, avatars, metadata, spacing, and
  quiet tone around those workflows so the application remains coherent.
- Provide consistent skeleton, empty, error, offline-origin, unauthorized,
  incompatible-server, saving, saved, stale, and conflict states.

## Surface Parity

Build parity through shared primitives before specialized Task/Note/Entity work.

- **Objects:** bounded search/list, kind/lifecycle filters, concise identity,
  description, protection/provenance, Connections, Object Events, edit/archive
  actions, and direct record navigation.
- **Chats:** list and current Chat detail/messages/participants only. Do not send
  messages or bypass Slack ingestion.
- **Users:** list and identity detail, external identities, connected records,
  and only the generic Object edits currently authorized. Do not make provider
  identities arbitrarily editable.
- **Memories:** list/search, detail, happened-at metadata, Connections, events,
  create/edit/archive under current protection rules.
- **Sources:** list/search/filter, source metadata, immutable content-version
  navigation/read windows, Connections/events, and current create/update/content-
  version operations. Never edit a stored content version in place.
- **Themes:** canonical list, search, detail, create/edit and relationships using
  the execution-time Theme contract.
- **Curator Runs:** list, status and trace/change detail, with the existing guarded
  undo action and an explicit destructive-impact confirmation. Do not create a
  general Curator control plane.
- **Evals:** current filters, summary/detail, trace, usage/accounting provenance,
  related Objects, verdict, and annotation. Do not add model grading or expose
  prompts, secrets, or hidden reasoning.
- **Schema:** retain the allowlisted read-only registry/table/row viewer. No SQL,
  schema mutation, arbitrary table names, or unbounded export.

## Task Management

### Task views

- Treat the signed-in Linear issue list, board conventions, quick property
  editing, filtering, keyboard flow, and issue-detail hierarchy as the quality
  bar. Reproduce the relevant interaction principles independently; do not copy
  proprietary code/assets or add excluded project-management features.
- **List:** dense grouped or flat rows with title, status, priority, owner, due
  state, agent eligibility, updated time, and optional selection. Allow search,
  multi-filter, stable sort, visible filter reset, and saved local view preference
  without server-side view configuration. These operations apply to the bounded
  Task set returned by the current Context API.
- **Board:** one column per current status (`todo`, `doing`, `blocked`, `review`,
  `done`), compact counts, useful empty drop targets, deterministic automatic card
  ordering by existing Task fields,
  horizontal overflow only when necessary, and optimistic drag with rollback on
  conflict/failure. Drag changes status; v1 does not persist manual position
  within a column. Keyboard and menu movement are first-class.
- **Calendar:** month view plus a compact agenda/list fallback using the existing
  `due_at` timestamp; show it consistently in the local configured timezone and
  support previous/next/today, accessible day labels, overflow disclosure, and
  timestamp rescheduling.
- Preserve one URL/filter model across views. Switching view must not discard the
  query, selection, or filters.

### Task creation and detail

- Provide a fast create dialog with title as the only initially focused field and
  concise optional properties. Require or generate a meaningful description in
  line with the existing canonical Object contract; do not silently write empty
  placeholder prose.
- Task detail uses an editable title and description with a quiet property rail
  for status, priority, owner, agent eligibility, due date/time, protection, and
  lifecycle. Relationships and activity follow below the main work area.
- Due-date control supports no date, today, tomorrow, end of week, next week,
  calendar date/time, clear, and visible overdue/today/upcoming states. It writes
  the existing `due_at` timestamp and clearly shows the interpreting local
  timezone; v1 does not claim date-only semantics that Context cannot store.
- Owner selection searches canonical Users and shows identity visuals. It never
  creates a duplicate User from typed text.
- All mutations use expected revision and idempotency. Show optimistic state only
  when rollback is deterministic; otherwise show a compact saving state. On
  conflict, preserve the user’s input and offer compare/reload/reapply rather
  than overwriting a newer record.
- Show Object Events as the authoritative activity history. Do not create a
  second comments/activity database.

## Notes Workspace

- Treat Obsidian’s focused Markdown workspace, readable editor typography,
  low-chrome navigation, predictable keyboard behavior, and confidence that text
  is safely stored as the quality bar. Reproduce those relevant principles
  independently without adding the excluded vault/plugin/graph feature set.
- Notes list provides search, updated ordering, compact title/description/content
  excerpt, format, attribution, and new Note creation. Opening a Note replaces
  the collection with the focused editor.
- Editor uses one generous readable column, editable title and concise summary,
  a large Markdown writing surface, subtle formatting help, word/character
  feedback only where useful, and minimal chrome.
- Use a robust styled textarea first; add a code-editor dependency only if
  acceptance testing proves native editing insufficient. Do not introduce a
  rich-text document model that can corrupt or unpredictably normalize Markdown.
- Support Markdown edit and safe rendered preview, common keyboard conventions,
  spellcheck, line wrapping, links/code/lists/tables/quotes, and sanitized output.
  Raw HTML and script execution remain disabled.
- Debounce autosave after idle input and provide `Save`, saving, saved timestamp,
  offline/error, and unsaved states. `Cmd/Ctrl+S` flushes immediately. Navigation
  with a pending save either completes it or clearly warns; it never drops text.
- Keep one local recoverable draft keyed by Note ID and base revision. Remove it
  only after the server confirms the matching content. On reload, crash, or
  network loss, offer recovery without automatically overwriting newer server
  content.
- On revision conflict, retain both current server content and the local draft,
  show a bounded comparison or clearly labeled choices, and require deliberate
  reapplication. No last-write-wins fallback.
- New and updated Notes remain canonical Note Objects with the current content
  size/format rules, provenance, events, and Eval attribution.

## Entity CRM

- Entity list supports search, lifecycle, updated order,
  avatar/logo or deterministic initials, compact identity, description, key
  relationship count, and direct record navigation. Avoid arbitrary scoring or faux
  sales metrics.
- Entity detail presents: image, name/lifecycle, editable summary, explained
  incoming/outgoing Connections grouped by meaning;
  related Tasks, Notes, Sources, Memories, Themes, Chats, and Users; provenance;
  and chronological Object Events.
- Related sections query canonical Connections and subtype APIs, not copied JSON
  caches. Every relationship shows its explanation and opens in the workspace.
- Creation asks only for name, description, and optional image URL.
  Relationship creation is a separate explicit action with source, target, kind,
  and explanation. Detect likely duplicate names through search but do not block
  legitimate same-name Entities without evidence.
- Do not infer, scrape, enrich, contact, message, or sync an Entity with an
  external CRM. Do not upload or download Entity image bytes. No PII/contact
  fields, aliases, pipeline stages, or generalized CRM fields are included.

## Delivery Phases

### Phase 0 — Freeze contracts and create the repository

- Treat the resolved decisions in this RD as the implementation contract; any
  material expansion requires an RD update before implementation continues.
- During separately authorized execution, create one GitHub Issue for the RD and
  normal execution branches per repository. Create the private `enyu_ops` remote
  only after approval; initialize README, licence decision, ownership, pinned
  Node/toolchain, package manager, lockfile, formatting, lint, type-check, unit,
  build, and browser-test scripts.
- Record threat model, data flow, capability inventory, Context compatibility,
  environment-variable names, and explicit non-goals before feature code.
- Capture approved reference screenshots/notes without copying proprietary
  assets or code.

### Phase 1 — Context capability foundation

- Implement and test only the optional Entity `image_url`, its narrow read/write
  human API contract, current actor/revision/idempotency/event/Eval behavior, and
  deterministic UI fallback. Do not change Tasks, Notes, queries, pagination,
  Themes, auth, listeners, or other Context behavior.
- Update only the necessary migration, Rust types/query/API path, optional current
  Context UI rendering, focused tests, ontology/API documentation, and
  compatibility metadata.
- Prove local UI and every existing agent/internal listener remain compatible.

### Phase 2 — Enyu Ops shell and secure transport

- Implement loopback/Host/origin enforcement, same-origin BFF routes, strict
  request/response schemas, typed Context client, safe errors, timeouts,
  cancellation, no-store behavior for private data, and structured redacted logs.
- Build routing, navigation, tokenized themes, common controls, identity badges,
  record lists, details, relationship/activity components, forms, toasts, and
  empty/error/loading/conflict states.
- Use synthetic fixtures and a disposable Context instance; do not point early UI
  development at production data.

### Phase 3 — Single workspace and shared lists

- Deliver direct URL integration, Back/Forward navigation, narrow-screen
  behavior, and one shared Context-style collection structure.
- Verify every collection stays compact and single-line at supported widths and
  that no split-pane, tab, divider, or open-beside controls remain.

### Phase 4 — Surface parity

- Implement and contract-test all agreed secondary surfaces before specialized
  enhancements. Maintain current write restrictions and confirmation boundaries.
- Compare representative records and actions between current Context UI/API and
  Enyu Ops, including protected, archived, empty, long, Unicode, missing optional
  metadata, conflict, and large-list cases.

### Phase 5 — Tasks

- Implement list first, then detail/create, then board, then calendar. Reuse one
  query/mutation model and prove each layer before adding the next.
- Test status/due/priority/owner edits, status moves, deterministic automatic
  ordering, conflict rollback, `due_at` display across timezone and daylight-
  saving boundaries, overdue calculations, client filtering/sorting, bounded-
  result messaging, keyboard paths, and realistic boards.

### Phase 6 — Notes

- Implement list/create/editor/preview, then autosave, local draft recovery, and
  conflict comparison. Test large Markdown, Unicode, paste, links, tables, code,
  malformed Markdown, unsafe HTML, rapid typing, slow/offline API, reload, crash
  recovery, two-tab concurrent edits, and navigation during save.

### Phase 7 — Entities

- Implement generic list/profile/edit/create and relationship-driven related
  records. Test duplicate-name warning, invalid or broken image URL,
  deterministic fallback, archived/protected records,
  reciprocal navigation, connection explanation, and activity accuracy.

### Phase 8 — Quality and portability gate

- Run automated accessibility checks plus keyboard/screen-reader-oriented manual
  scenarios. Resolve serious violations; do not waive missing labels, focus loss,
  contrast, or drag-only interaction.
- Capture stable light/dark screenshots at agreed desktop, laptop, and
  tablet/narrow single-workspace viewports for primary states. Review populated, empty,
  loading, error, conflict, modal, long-content, and overflow states.
- Measure initial load, navigation, search, Task drag, Note typing, and theme
  toggle on realistic data. Remove unnecessary dependencies and renders;
  add virtualization only if measurement proves it necessary.
- Document which pieces are generic upstream candidates, their Context coupling,
  and what must remain Enyu-specific. Do not upstream anything in this RD without
  a separate review/approval.

### Phase 9 — Local release and future-hosting handoff

- Produce an immutable local release build and one documented command/service
  path that starts Enyu Ops on loopback, verifies Context readiness and API
  compatibility, and shuts down cleanly.
- Prove the app does not listen on LAN interfaces, rejects non-loopback Host and
  cross-origin mutation requests, exposes no database/internal listener, and
  leaks no configuration through bundles, source maps, HTML, logs, screenshots,
  or errors.
- Run end-to-end acceptance against synthetic fixtures, then an approved
  production-data read-only smoke. Perform a separately approved small live
  mutation for Task, Note, and Entity, verify Context Object Events/Evals and UI
  readback, and clean up only requester-approved synthetic records.
- Record release commit/build identity, configuration inventory without values,
  health checks, startup/restart procedure, backup/recovery, and failure handling.
- Write a concise future-hosting handoff describing the still-unsolved identity,
  service authentication, public ingress, origin availability, database hosting,
  secrets, DNS, TLS, deployment, monitoring, cost, backup, and rollback decisions.
  Do not implement any of them here.

## Acceptance Scenarios

### Tasks

- Create one Task, assign a canonical User, set priority and a `due_at` deadline,
  find it through client search/filter, move its status on the board, see its
  deterministic automatic placement, reschedule it in calendar, edit its
  description, open a related record, complete it, and verify
  revisions, events, Eval attribution, and reload persistence.
- Demonstrate today/tomorrow/overdue/timed/no-date presentation in the configured
  local timezone, including a daylight-saving boundary, without claiming or
  persisting unsupported date-only semantics.
- Concurrently edit the same Task in two sessions; the stale writer must receive
  a recoverable conflict and cannot erase the newer write.

### Notes

- Create and write a realistic long Markdown Note, preview it safely, autosave,
  interrupt connectivity, continue typing, reload, recover the draft, resolve a
  concurrent conflict, and verify exact server content and audit attribution.
- Unsafe HTML/script content renders inert; links and Markdown remain usable;
  rapid typing and navigation do not lose focus, text, cursor, or scroll position.

### Entities

- Create or edit one representative Entity, add approved generic metadata and an
  explained Connection, then navigate its related Tasks, Notes, Sources,
  Memories, Themes, Users, and activity without duplicated or
  stale data.
- A protected or stale Entity update is rejected visibly and safely; duplicate-
  name assistance informs but does not silently merge canonical Objects.

### Workspace, themes, and local operation

- Compare the Enyu Ops Object list and the referenced Object detail at the same
  viewport with the current Context UI. They must preserve the same calm visual
  hierarchy, vertical section order, row-first relationships/activity, compact
  properties, avatar-stack clarity, and whitespace before specialized features
  are accepted.
- Open every list/detail in the single workspace; reload, navigate Back/Forward,
  cross the responsive breakpoint, and recover without blank or inaccessible UI.
- Complete primary scenarios in dark and light modes with system preference,
  explicit preference, reload, and no wrong-theme flash.
- Non-loopback Host, cross-site mutation, unsupported Context version, and
  disallowed gateway-route requests fail closed without leaking record data or
  configuration.
- Stop the local Context origin and show a clear non-destructive unavailable
  state; when restored, retry reads safely and never replay an ambiguous mutation
  without its original idempotency key.

## Verification Matrix

### Centaur Context

- [x] The Entity-image migration upgrade test covers populated canonical and
  legacy-named databases, nullable default, HTTPS/length constraints, rollback-
  by-restore documentation, and no changes to Centaur `ai_v2` or Console data.
- [x] Focused API/database tests cover Entity image read/update/clear, invalid
  schemes and lengths, expected revision, idempotency, actor attribution,
  provenance, protection, Object Events, Evals, and deterministic broken/missing
  image fallback.
- [x] A schema/API diff confirms no Task, Note, query, pagination, Theme, auth,
  listener, Source, or unrelated behavior changed.
- [x] Existing UI and standard agent/Note/ingestion/Curator/Source-intake client
  contracts remain compatible.
- [x] `cargo fmt --check`
- [x] `cargo clippy --all-targets --all-features -- -D warnings`
- [x] `cargo test`
- [x] `npm --prefix web run type-check`
- [x] `npm --prefix web run build`
- [x] `python3 -m pytest tools/centaur_context/test_client.py`
- [x] `python3 -m compileall -q tools/centaur_context`

### Enyu Ops

- [x] Formatting, lint, strict type-check, unit tests, production build, dependency
  audit policy, and `git diff --check` pass with pinned versions and lockfile.
- [x] Context client contract tests run against representative success, validation,
  unauthorized, forbidden, not-found, conflict, rate/limit, timeout, malformed,
  incompatible-version, and unavailable responses.
- [x] Component tests cover common lists/forms/properties, themes, Task views,
  Note save/recovery/conflict, Entity relations, single-workspace navigation, and
  compact shared list behavior.
- [x] Browser tests cover all Acceptance Scenarios at the agreed viewports and in
  both themes, with console/network failures treated as test failures.
- [x] Accessibility automation and focused manual checks pass for navigation,
  forms, dialogs, lists, board, calendar, editor, focus restoration,
  announcements, reduced motion, and contrast.
- [x] Visual regression baselines are deliberately reviewed in both themes and
  include real-density, long-content, empty, loading, error, offline, stale, and
  conflict states. The dark-theme baseline explicitly compares the current
  Context reference Object list/detail against the corresponding Enyu Ops base
  surfaces so visual drift toward card-heavy dashboards fails review.
- [x] Performance checks show no input lag while typing Notes or navigating,
  no full-list refetch after a single mutation unless required, bounded payloads,
  and acceptable primary navigation on the production-like dataset.

### Local release acceptance

- [x] Release commit/build identity is recorded; configuration is absent from
  repository, client bundles, source maps, HTML, logs, screenshots, and errors.
- [x] Loopback-only binding, Host/origin enforcement, gateway route allowlist,
  server-owned actor attribution, Context-unavailable behavior, and lack of
  public/LAN ingress are verified.
- [x] The exact local release artifact passes approved Task, Note, Entity, navigation,
  theme, parity, security, backup, and recovery checks.
- [x] Approved live readback and synthetic mutations produce the expected
  canonical Objects, subtype rows, Connections, revisions, Object Events, and
  Evals exactly once.
- [x] Operations guide covers local startup, readiness, restart, Context failure,
  backups, release replacement, restore, and uninstall while preserving the
  canonical database.

## Resolved Refinement Decisions

1. Keep the application and canonical Context service/database local in this RD;
   cloud migration and hosting are a separate larger job.
2. Add no login for the loopback-only application. Internet identity and login
   must be designed before any future hosted exposure.
3. Use the exact local path and private remote name `bradwmorris/enyu_ops`.
4. Keep Tasks deliberately minimal and exclude the listed project-management
   extensions from v1.
5. Keep Entities deliberately minimal, but include a visible optional person
   avatar/entity logo URL with deterministic local fallback. The AGI Post
   reference stores Entity avatar URLs/local paths; its Supabase buckets serve
   other media and are not a reason to add a bucket here.
6. Keep Notes to the minimal Markdown save/recovery/conflict experience and omit
   broader Obsidian knowledge-management features.
7. Include Chats and preserve every surface’s current write authority, including
   read-only Schema and immutable Source content.
8. Use the restrained Enyu/Linear/Obsidian design direction in both themes.
   The current Centaur Context UI is the primary visual source of truth; Linear
   is the primary quality reference inside the complete Task workspace and
   Obsidian is the primary quality reference inside the complete Note workspace.
   Those surfaces remain integrated with the Context shell and shared record
   language.
9. The only allowed Centaur Context product change is canonical Entity image
   storage and its narrow read/write/rendering contract. All other features must
   use the existing API and live in `enyu_ops`; a newly discovered non-image
   blocker stops execution for an RD update rather than expanding scope.

## Approval Boundary

This RD authorizes planning only. It does not authorize creating a local or
remote repository, GitHub Issue/branch/PR, changing Centaur Context, copying
reference code/data, installing dependencies, mutating live Context data,
publishing a repository, or deleting anything. Each execution phase begins only
after explicit execution approval; private remote creation and any live data
mutation require approval at the relevant boundary. This RD explicitly does not
authorize Vercel, Cloudflare Access/Tunnel, Namecheap/Cloudflare DNS, public or
LAN ingress, internet login, cloud databases, credential creation, deployment,
or hosting spend. Back up and verify the canonical Context database before any
live migration; never query or migrate Centaur `ai_v2` or Console data.

# RD: Refine UI Editing, Details, Lists, and Refresh

**Status:** `complete`
**Created:** 2026-09-02
**Rebased:** 2026-09-02 on `e0c55b2` (`feat: add universal interaction runs and trace UI (#79)`)
**GitHub Issue:** [#81](https://github.com/bradwmorris/centaur-context/issues/81)

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** `web/src/App.tsx`, `RecordVisuals.tsx`, `ObjectIdentity.tsx`,
`DescriptionSnippet.tsx`, `SchemaWorkspace.tsx`, `api.ts`, `types.ts`, the full
`styles.css`, current Vitest coverage, Rust API request types and routes in
`src/api.rs`, list/detail queries and visual attribution in `src/db.rs`, Run
storage/review/trace behavior in `src/runs.rs`, migrations and list indexes,
schema-21 workflow/interaction Run presentation, `primary_object_id`, Run trees,
current/legacy trace formats, child Runs, metrics, related-Object roles, API and
database tests, and the completed UI visual, Object identity, description,
Connection, Source, Note, and Run RDs.

**Missing:** none. For this RD, “connection density” applies to canonical
Object-backed lists (Objects, Tasks, Chats, Users, Entities, Memories, Sources,
Notes, and Themes). Runs remain newest-first because a Run is not an Object;
Connections and Schema retain their graph/raw-table-specific ordering.

1. Establish shared list-row, related-Object, source-icon, detail-heading,
   inline-editor, and refresh contracts, then migrate every applicable surface
   while preserving the landed universal Run data and navigation contracts.
2. Add server-side recent/density ordering with stable pagination for all
   Object-backed lists and make recent creation time the default everywhere.
3. Add behavior, API/query, accessibility, responsive, race/conflict, and visual
   regression coverage; run all repository checks and record completion evidence.

## What We Are Doing

- [x] Deliver all 11 requested UI and functionality updates consistently across
  every applicable list and opened-record page, rather than patching individual
  examples.
- [x] Prove that editing persists reliably, refresh reveals new server data,
  default and density ordering are correct, and dense/long content remains clean
  at desktop and narrow widths.

## Contract

- **Goal:** Make the local UI consistent, compact, current, and dependable for
  browsing and editing canonical context records.
- **Done:** Items 1–11 below are observable on every named surface; supported
  edits persist under normal, slow, overlapping, and revision-conflict cases;
  refresh and both sort modes return current deterministic data; all checks pass.
- **Files:** `web/src/App.tsx`, `RecordVisuals.tsx`, `ObjectIdentity.tsx`, new
  narrowly scoped shared UI/hooks if useful, `api.ts`, `types.ts`, `styles.css`,
  and targeted `web/src/*.test.tsx`; list query contracts and SQL in
  `src/api.rs`, `src/db.rs`, and `src/runs.rs` only where needed; a narrow index
  migration numbered after the landed schema-21 Run migrations only if
  query-plan evidence requires one; Rust API/database tests; this RD.
- **Agent owns:** Implementation, shared component extraction, stable query and
  conflict behavior, accessible SVG/icon fallbacks, tests, query-plan review,
  browser verification, and preservation of unrelated work.
- **Requester owns:** Visual acceptance, approval for any later live polling,
  external favicon service, server-side URL fetching, new dependency, or change
  to the canonical ontology/data semantics.
- **Out of scope:** Changing or deleting canonical description fields; rewriting
  existing Note or Task data; changing agent/API write permissions; live push,
  polling, WebSockets, or notifications; redesigning the Connection graph or
  Schema browser; a new icon library; backend fetching of arbitrary websites;
  deployment, public ingress, downstream-overlay customization, changing Run
  parent/child semantics, removing Run metrics/outcome/technical evidence, or
  changing workflow/interaction trace ingestion.

## Detailed Requirements and Execution Guidance

### 1. Related Objects

- Rename every opened canonical record’s `Relationships` section to
  `Related Objects`. Replace the current two-ended `source → target` rendering
  with a shared row oriented around the opened Object: find the opposite endpoint
  and render that related Object once. Never repeat the opened Object in its own
  row.
- Keep meaning that would otherwise be lost: show the related Object’s compact
  type, Object ID, title, source/users when available, Connection kind, an
  inbound/outbound cue relative to the opened Object, and its explanation in a
  compact/clamped form. Navigation opens the related Object; connection detail
  remains reachable without turning the row back into two endpoints.
- Use the same row language for Task, Source, Note, and general Object details.
  Refine the landed `run-related-object` row rather than replacing its data:
  retain the Run association’s role/kind/title, Object ID, and visual context
  (there is no origin endpoint to repeat). Empty states say `No related Objects`.
  The first-class Connection detail and Connection graph may still show both
  endpoints because the Connection itself, not either endpoint, is the subject.

### 2. Remove description coaching below editors

- Remove `DescriptionHelp` and all rendered “Describe this specific … Example”
  paragraphs beneath create and detail editors. Keep concise placeholder text
  inside an empty create control where it helps input; do not render permanent
  coaching below a populated field or weaken server validation.

### 3. Canonical list-row order and compact types

- Replace the current mixed row composition with explicit semantic slots in this
  order: **Type / Object ID / Title / Source / Users / Description**. Preserve a
  trailing updated/created time only as secondary metadata after those slots.
  Apply it to Objects, Tasks, Chats, Users, Entities, Memories, Sources, Notes,
  and Themes. Give Runs an equivalent compact Run row without pretending a Run
  has an Object type or Object ID; preserve the landed root-Run filtering,
  synthesized Run title/outcome, `primary_object_id`, and parent/child behavior.
- Add a compact variant to the shared type visual and reuse the left-navigation
  glyph plus exactly one fixed three-letter code: `TAS`, `CHA`, `USE`, `ENT`,
  `MEM`, `SOU`, `NOT`, and `THE`. Use `CON` only on Connection-specific compact
  surfaces and `RUN` for Runs. Keep the full type as the accessible name/title;
  do not derive codes by slicing localized text.
- Keep titles and description snippets independent and truncatable. Define
  deliberate column collapse/wrap behavior at existing breakpoints; never let
  UUIDs, avatars, badges, URLs, or unbroken text overlap or create page-level
  horizontal scrolling.

### 4. Note detail content

- Remove the visible/editable canonical description area from an opened Note and
  leave Content as its one body editor. Keep `objects.description` in the schema,
  API, create flow, search, and list snippet: it is canonical summary metadata,
  while `notes.content` is the Note body. Saving Content must not silently copy,
  overwrite, or clear the hidden description.

### 5. Source-site icons

- Add a dedicated source-site icon immediately after a Source row’s title and
  before the existing provenance/provider Source badge. Do not confuse the new
  icon with `SourceBadge` (for example, Slack origin).
- Resolve from stored `source_kind` and `canonical_uri`: YouTube hosts use a
  bundled accessible YouTube SVG; `paper` uses the existing visual language’s
  archive/document glyph; X/Twitter hosts use a bundled X SVG; another valid
  HTTP(S) URL may try the URL origin’s `/favicon.ico` directly in a lazy
  `img` with `referrerPolicy="no-referrer"`. Invalid, missing, blocked, or broken
  icons disappear without broken-image chrome or layout shift.
- Do not call a third-party favicon service and do not add a backend URL fetcher,
  HTML scraper, persisted remote asset, or proxy. That keeps this local feature
  small and avoids creating an SSRF/privacy surface. Icons are decorative beside
  an already visible title but retain useful tooltips; built-in SVG assets need a
  documented source/license.

### 6. Task detail

- Render Priority and Blocked reason as read-only properties. Render GitHub issue
  as a safe external HTTPS link when present and an empty/`None` value when
  absent; never render it as an input. Preserve API/agent writability—the change
  is only the human UI boundary.
- Do not show the canonical Task description as a second body on the opened Task.
  Show only `brief_markdown` as the main editable brief. Remove the mismatched
  grey field treatment so the brief uses the same neutral body/editor design as
  other long-form content. Do not copy one field into the other.

### 7. Detail headings and title persistence

- On every opened record, put the title alone in the heading. Move Object ID into
  Properties with its existing copy/open affordances. Keep Users only in
  Properties. Runs put Run ID and actor/user context in Properties, not beside or
  beneath the title. Preserve the useful landed synthesized Run title, outcome,
  metrics, Primary Object, Originating Chat, and Child runs; only separate title
  from identity/context metadata. Apply the rule to Object, subtype, Source,
  Note, Theme, Run, and Connection details where the concept exists.
- Clamp view-mode titles to two lines with stable line height, overflow handling,
  and room for edit/status controls. The edit control may grow to two lines but
  must not distort the heading at 320–1440 px.
- Title edits use the shared editor in item 8 and partial PATCH requests. On
  success, update the detail, breadcrumb, and cached/list title from the returned
  server record so navigation never shows stale text.

### 8. One editing system

- Build one accessible inline-edit pattern for title, description, Note content,
  Task brief, Run review notes, and any other existing click-to-edit body. View
  mode is clean text; click/keyboard activation enters a visually consistent
  borderless/minimal editor. Provide a small recognizable save SVG button with
  an accessible `Save <field>` name, not a large `Save changes` button.
- Support both behaviors consistently: autosave after a short documented idle
  debounce (target 800 ms) and on blur, while the save icon immediately commits.
  `Escape` restores the last confirmed value; title `Enter` saves and multiline
  `Cmd/Ctrl+Enter` saves. Never issue a request for an unchanged value.
- Centralize dirty/saving/saved/error state and optimistic-concurrency behavior.
  Serialize writes per record, coalesce newer drafts while one request is in
  flight, advance from the returned revision, and never let an older response
  overwrite a newer draft. On validation/network error keep the draft and expose
  an inline retry; on `409` keep the draft, explain that server data changed, and
  offer explicit reload/retry rather than silently overwriting either version.
  Announce save state without visually noisy persistent text.
- Use field-level partial PATCH payloads supported by the existing Object,
  Source, Note, and Task APIs. Do not make every small edit resubmit unrelated
  controls or stale hidden values. Run review may use its paired verdict/notes
  endpoint but must preserve the latest confirmed counterpart.

### 9. Run review, mutations, and execution trace

- Refine the landed universal Run detail instead of reverting it to the older
  flat Run view. Preserve `primary_object_id`, root/child navigation, metrics,
  outcome summary, related-Object roles, failures, token/duration/tool facts,
  legacy trace compatibility, and expandable technical result/raw evidence.
- Replace the current labelled Verdict/Review notes/Save form with a compact
  Pass/Fail segmented control and the shared minimal Review notes editor. New UI
  choices are only `pass` and `fail` (interpreting “file” in intake as “fail”).
  Continue to display legacy `unreviewed`/`mixed` values until reviewed; do not
  remove them from storage/API compatibility in this UI RD. Keep the paired Run
  review endpoint and its optimistic `review_revision` contract.
- Render each Durable mutation on exactly one bounded row: action + target type,
  `new → revision` or revision transition, compact target ID/link, and reversal
  state if applicable. Use ellipsis/tooltips or a narrow-screen bounded fallback,
  not a multi-card layout.
- Keep the landed `Execution trace` name and `RunTraceEntry` normalization of
  current `entry_type` and legacy `type` shapes. Make each collapsed trace entry
  exactly one bounded row containing sequence, event label, concise fact summary,
  important compact metrics, state, and disclosure affordance. Preserve the
  existing expanded detail for component/model/token facts and raw JSON; do not
  discard evidence merely to make the collapsed state compact.

### 10. Manual refresh

- Add one clear Refresh button in the top bar that refreshes the current route’s
  list or opened detail without a full-page reload. Preserve route, search,
  selected sort, editor drafts, navigation state, and scroll where practical.
  Retain existing data while refreshing, show a compact busy/success/error state,
  prevent duplicate clicks, and expose an accessible name/focus state.
- Implement one refresh generation/coordinator consumed by route-scoped loaders,
  including nested detail data, Related Objects, visuals, Runs, Connection graph,
  and Schema. Ignore or abort stale responses so a slow earlier load cannot win.
  Refactor the current top-level loader so refresh does not fetch every unrelated
  collection on every route.
- Keep refresh manual. Do not add polling, focus refresh, live subscriptions, or
  a new data-fetching dependency; those add background load and conflict risks
  not required by this request.

### 11. Recent and connection-density ordering

- Default every visible list to **Recently added**, meaning canonical
  `created_at DESC, id DESC`, not `updated_at` and not UUID order. Runs already use
  this rule and must keep it. A later edit must not move an old Object to the top.
- Add one compact sort selector with `Recently added` and `Most connected` to all
  Object-backed lists. `Most connected` is the count of active, non-archived
  Connections where the Object is either source or target; include zero-degree
  Objects and order by `connection_count DESC, created_at DESC, id DESC`.
- Implement ordering in the Rust/SQL list endpoints, not by sorting only the
  currently loaded browser page. Validate a shared `sort=recent|connections`
  parameter and return `connection_count` where the UI needs to explain density.
  Use stable composite/opaque cursor semantics that include the active sort keys;
  changing search, kind, or sort resets pagination. Update Objects, subtype/Task,
  Source, Note, and Theme query paths consistently rather than relying on their
  current mixture of updated time, title, and UUID ordering.
- Review `EXPLAIN` output with representative zero/high-degree data. Add only the
  narrow partial/index support justified by the plan; do not denormalize counts
  or add triggers unless measured query behavior proves indexed active endpoint
  counts are insufficient. If a migration is justified, use the next available
  number after `0021_repair_workflow_parent_links.sql`; never edit landed
  migrations `0019`–`0021` to add unrelated list behavior.

## Checks

- [x] UI tests cover all list families and semantic slot order; exact type codes;
  source icon domain/kind resolution and broken fallback; related rows in both
  Connection directions; heading metadata separation; two-line title bounds;
  Note/Task body rules; compact Run controls, mutations, and trace rows; and
  preservation of root/child Runs, metrics, outcome, related roles, and expanded
  technical evidence.
- [x] Editor tests use fake timers and deferred responses to prove debounce,
  blur/manual save, keyboard behavior, no-op suppression, write serialization,
  latest-draft wins, returned-revision advancement, retry, `409` preservation,
  accessible status, and stale-response protection.
- [x] Rust API/database tests prove accepted/rejected sort values, recent creation
  ordering after later edits, density counts excluding archived Connections,
  deterministic ties, zero-degree inclusion, search/kind behavior, and stable
  pagination with no duplicates or omissions.
- [x] Refresh tests prove only current-route resources reload, detail auxiliaries
  and visuals update, existing content remains during loading, drafts survive,
  duplicate/stale requests cannot win, and failures remain recoverable.
- [x] Browser verification at 1440, 1024, 820, 640, and 320 px covers long titles,
  UUIDs, descriptions, source URLs, missing icons, many users, high-density rows,
  keyboard/focus behavior, and no overlap or page-level horizontal overflow.
- [x] Query plans are inspected on representative recent/density datasets; any
  migration has a disposable-database contract test.
- [x] The landed universal interaction/Run regressions remain green, including
  `tests/slack_runs.rs`, Run tree/detail database coverage, workflow trace API
  authorization, current and legacy trace fixtures, and root-only Run listing.
- [x] `npm --prefix web test`, `npm --prefix web run type-check`, and
  `npm --prefix web run build` pass.
- [x] All repository-root verification commands and `git diff --check` pass.

## Verification Results

- 49 Vitest checks passed; editor coverage includes debounce, manual/keyboard
  save, no-op suppression, serialization/coalescing, retry, and conflict draft
  preservation.
- The full Rust, clippy, formatting, production build, Python compile, and 15
  Python client checks passed. Database list ordering and the Slack Run contract
  also passed against a disposable pgvector-backed `centaur_context_test` database.
- `EXPLAIN` confirmed the recent list uses `objects_active_created_idx`; density
  uses the active Object index plus bounded source- and target-endpoint index
  lookups. No canonical or external database was queried.
- Browser checks at 1440, 1024, 820, 640, and 320 px found no page-level
  horizontal overflow; Refresh and sorting remained visible at every width.

## Approval Boundary

This RD authorizes only local implementation and verification when execution is
separately requested. It does not authorize deployment, public ingress, hosted
writes, external integrations or favicon services, server-side fetching of
arbitrary URLs, polling/live subscriptions, publishing, sending, spending,
credentials, destructive data changes, or changes to a private downstream
overlay. Check `/Users/bradleymorris/Desktop/dev/enyu-os` at execution handoff;
apply corresponding overlay changes only when separately authorized.

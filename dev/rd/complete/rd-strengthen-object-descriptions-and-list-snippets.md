# RD: Strengthen Object Descriptions and List Snippets

**Status:** `complete`
**Created:** 2026-08-28
**Completed:** 2026-08-29
**GitHub Issue:** #6

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** mandatory `objects.description` migration and constraints,
Rust text/provenance validation, Object and Task creation/update APIs, the hybrid
search and Object embedding input, Context Curator prompt/validation/evals,
Python client, current React creation/detail forms, the completed type/source/
user/Connection visual RD, current inline list descriptions and responsive
styles, database contract tests, and the public Context Curator requirements.

**Missing:** none

1. Define and enforce one clear description contract across human forms,
   ingestion/curation output, APIs, subtype reads, and the standard agent client.
2. Preserve the existing desktop description placement, extract one deliberate
   accessible snippet component, and keep a useful truncated description visible
   on desktop and narrow layouts alongside the completed visual metadata.
3. Add fixtures for strong and weak descriptions plus API, curator, and UI
   coverage; verify existing-data compatibility, run repository checks, and
   record completion evidence.

## What We Are Doing

- [x] Require every canonical Object to explain directly and concretely what the
  represented thing is, rather than storing vague, abstract, or meta commentary.
- [x] Apply the same canonical description to Object, Task, Chat, User, Entity,
  and Memory surfaces without subtype duplication or fallback to notes/body text.
- [x] Show a controlled truncated description snippet in every main list/table
  row, including narrow views, with the full text available in the detail view.

## Contract

- **Goal:** Make Object descriptions consistently useful to humans, retrieval,
  and the Context Curator, and make that useful context visible while browsing.
- **Done:** Every creation path follows one tested semantic contract; weak
  generated descriptions fail validation/evaluation or are repaired before
  commit; every primary list view shows a predictable snippet of the canonical
  description.
- **Files:** Description validation and typed API contracts in `src/`; curator
  prompt/schema/evals when present; `tools/centaur_os`; creation/detail/list UI
  and styles in `web/src/`; compatibility migration only if required; targeted
  tests; this RD.
- **Agent owns:** Description contract, deterministic validation, generated-text
  evaluation/retry behavior, form guidance, snippet component, accessibility,
  tests, and local verification.
- **Requester owns:** Approval of any destructive rewrite of existing descriptions or
  any model/provider configuration and cost.
- **Out of scope:** Long-form notes, transcripts, hidden reasoning, subtype-level
  duplicate descriptions, automatic bulk rewriting of existing production data,
  and general search/ranking redesign beyond using the canonical description.

## Description Contract

- A description is a concise, self-contained statement of what this specific
  Object represents. It names the subject and includes the concrete context
  needed to distinguish it from similarly titled Objects.
- Prefer one or two plain-language sentences. State the current fact, event,
  responsibility, conversation, person/agent identity, or actionable outcome
  directly. Include why it matters only when that is necessary to identify the
  Object.
- Reject empty/whitespace-only text, a description that merely repeats the
  title, placeholders, unsupported abstraction such as “this is about the
  project,” transcript fragments, process narration, and model commentary.
- Deterministic API checks should enforce what can be proven mechanically
  without pretending that length alone guarantees quality. Generated curator
  descriptions additionally use typed output, representative positive/negative
  fixtures, and a bounded repair/retry path before any atomic reconciliation is
  committed.
- Human forms show short guidance and an example tailored to the selected type;
  errors explain how to make the description concrete. Existing valid records
  remain readable during any stricter-validation migration.
- The canonical value is `objects.description`. Subtype endpoints and UI views
  read it through their Object join; they never introduce `body`, `notes`, or a
  second description as a competing display source.

## List Snippet Rules

- The current baseline already renders `objects.description` inline in the
  shared list-row path on desktop. Treat that as partial implementation: it is
  not yet a shared snippet component, and the current responsive CSS hides it at
  820px and below.
- Use one shared description-snippet component in Objects, Tasks, Chats, Users,
  Entities, and Memories list/table rows.
- Render normalized plain text with a deliberate line/character clamp and
  ellipsis. Do not mutate the stored description to produce the snippet.
- Desktop and narrow layouts both show useful description text; narrow views may
  use fewer lines but must not hide the description entirely.
- The full description remains available by opening the row, and the snippet's
  accessible name must not misleadingly present truncated text as complete.
- Lists must remain scannable with long unbroken text, Unicode, line breaks,
  missing legacy values during migration, and descriptions near the API limit.

## Checks

- [x] Unit/API tests cover trimming, title-only repetition, placeholders, vague
  generated fixtures, strong examples for all five Object types, maximum length,
  and compatibility with existing rows.
- [x] Curator tests prove unclear descriptions are repaired or rejected before a
  transaction and clear descriptions are preserved without unnecessary rewrite.
- [x] UI tests prove all six primary lists use the canonical description and show
  predictable snippets at desktop and narrow widths without regressing the
  existing type, source, Object ID, attribution, or status visuals.
- [x] Visual checks cover long words, multiline text, Unicode, empty legacy
  fallback, ellipsis, and opening the full description.
- [x] The repository verification suite and `git diff --check` pass.

## Verification Results

- Rust formatting, Clippy with warnings denied, and the full Rust test suite
  passed.
- The database contract passed against a fresh disposable PostgreSQL/pgvector
  database named `centaur_os_test_issue_6`.
- All 33 web tests, TypeScript type-checking, and the production web build
  passed.
- All seven Python client tests and Python bytecode compilation passed. The
  repository virtual environment supplied `pytest` because the system Python
  installation did not include it.
- Browser verification passed at 1280px and 390px with live local data and
  dedicated long-word, multiline, Unicode, near-limit, and empty-description
  fixtures. The list did not overflow horizontally, no browser errors appeared,
  and opening a truncated row exposed the unchanged full description.
- `git diff --check` passed.

## Approval Boundary

This RD permits local validation, prompt/eval, API/client, UI, and compatible
migration work. It does not authorize bulk rewriting or deleting existing data,
calling a paid model, changing model/provider configuration, deployment, hosted
writes, public ingress, external integrations, or credential changes.

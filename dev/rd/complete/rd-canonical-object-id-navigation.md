# RD: Make Canonical Object IDs Visible and Navigable Everywhere

**Status:** `complete`
**Created:** 2026-08-28
**Completed:** 2026-08-29
**GitHub Issue:** [#1](https://github.com/bradwmorris/centaur-os/issues/1)

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** canonical Object and subtype migrations through
`migrations/0006_context_curator.sql`, Object search/embedding records, User and
Curator Run APIs, `src/db.rs`, `src/api.rs`, `web/src/App.tsx`, `web/src/api.ts`,
`web/src/types.ts`, and `tests/database_contract.rs`.

**Missing:** none

1. Formalize a shared API/UI identity reference that labels canonical Object
   IDs consistently and exposes the correct Object link from every record.
2. Add Object ID references to all list rows, detail properties, subtype views,
   connection/activity surfaces, and the schema visualizer's related rows.
3. Add deep-link routing, copy behavior, accessibility, and contract/UI tests;
   run the repository checks and record completion evidence.

## What We Are Doing

- [x] Show a clearly labelled canonical `Object ID` on every first-class Object,
  Task, Chat, User, Entity, and Memory row and detail view.
- [x] Make every displayed Object ID clickable back to that canonical Object's
  detail page and independently copyable as the full UUID.
- [x] On supporting records, show the correct owning or related Object IDs
  without incorrectly turning the supporting record into another Object.

## Contract

- **Goal:** Make the canonical Object identity obvious, consistent, and useful
  throughout Centaur OS.
- **Done:** A user can identify, copy, deep-link to, and open the canonical Object
  behind every first-class record from any relevant UI surface.
- **Files:** API response types and queries where needed; browser routing and
  shared identity components in `web/src/`; schema visualizer integration;
  targeted Rust/API/UI tests; this RD.
- **Agent owns:** Identity terminology, shared UI component, routing, copy and
  navigation behavior, responsive presentation, and verification.
- **Requester owns:** Approval for changing the canonical identity model or assigning
  Object identity to supporting records.
- **Out of scope:** Replacing UUIDs, adding human-readable sequential IDs,
  changing subtype primary keys, making every supporting row an Object, and
  redesigning unrelated list content.

## Canonical Identity Rules

- Every first-class Task, Chat, User, Entity, and Memory is exactly one canonical
  Object. Its subtype `object_id` is the same UUID as `objects.id`; it is not a
  second identifier and no Connection joins an Object to its own subtype.
- The UI label is always `Object ID`, not ambiguous alternatives such as `ID`,
  `record ID`, or `task ID` when the value is the canonical UUID.
- Detail views show the full UUID in selectable text with an explicit copy
  action. Dense rows may use a visually shortened form only when the full UUID
  remains available to assistive technology, on focus/hover, and through copy.
- Clicking an Object ID uses a durable URL for the canonical Objects view and
  supports browser back/forward, refresh, and direct opening. It must not depend
  solely on transient React selection state.
- Task API responses currently expose the shared UUID as `id`; make its
  canonical meaning explicit in the API/type contract without introducing a
  competing Task identifier.
- Supporting records are not canonical Objects. Connections show linked
  `source_object_id` and `target_object_id`; Chat Messages show their Chat and
  sender User Object IDs; Object Events show `object_id`; Curator Runs show their
  Chat Object ID; external identities show their User Object ID; Object
  Embeddings and Object Embedding Jobs show their Object ID. Each Object ID is
  navigable, while the supporting record's own ID remains separately labelled
  only where useful.
- A Curator Run Change is type-aware: its `entity_id` identifies a canonical
  Object only when `entity_type = 'object'`; when `entity_type = 'connection'`,
  it identifies a supporting Connection. The UI must route these cases to the
  appropriate Object or Connection target rather than treating every change as
  an Object.

## Checks

- [x] Database/API contract tests prove every subtype uses the matching canonical
  Object UUID and representative supporting rows expose valid Object references.
- [x] UI tests cover IDs and links in all six primary views, details,
  relationships, activity, and representative supporting rows. The Schema view
  is separately backlogged and has no current schema-table rows to integrate.
- [x] Deep links survive refresh and browser navigation; copy returns the full,
  exact UUID even when a dense row is visually shortened.
- [x] Keyboard, screen-reader, narrow-layout, empty, and missing-target states are
  verified.
- [x] The repository verification suite and `git diff --check` pass.

## Verification Results

- `cargo fmt --check`, Clippy with warnings denied, and `cargo test` passed. The
  database-backed contract test was skipped because `TEST_DATABASE_URL` was not
  set; its Task/User canonical identity assertions compile and are ready for a
  disposable `centaur_os_test` database.
- 19 Vitest UI/routing/identity tests, TypeScript type-check, and the production
  web build passed.
- The standard Python client built as a wheel, all 7 client tests passed in an
  isolated environment, and Python byte-compilation passed.
- Browser verification passed at desktop and 390px widths with no overflow,
  console errors, or error overlay. Direct refresh, navigation, full UUID copy,
  and copy-without-row-navigation were verified.

## Approval Boundary

This is a local identity and navigation improvement. It does not authorize a
new identity scheme, mutation of existing UUIDs, public ingress, deployment,
hosted writes, external integration changes, or data deletion.

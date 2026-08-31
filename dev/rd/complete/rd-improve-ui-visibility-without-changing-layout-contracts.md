# RD: Improve UI Visibility Without Changing Layout Contracts

**Status:** `complete`
**Created:** 2026-08-31
**Completed:** 2026-08-31
**GitHub Issue:** #65

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Current `web/src/App.tsx`, `SchemaWorkspace.tsx`,
`RecordVisuals.tsx`, `ObjectIdentity.tsx`, `DescriptionSnippet.tsx`, the complete
`styles.css`, their UI tests, responsive breakpoints, and the completed visual
language RD. The source checkout contains unrelated UI and RD changes, which are
isolated from this latest-`main` worktree and must be preserved.

**Missing:** none.

1. Inventory every rendered UI surface and consolidate the dark-theme type,
   colour, spacing, control, and focus values needed for a coherent small scale
   and contrast increase.
2. Apply a restrained visibility pass across navigation, lists, details, forms,
   modals, graph/schema, and operational views while preserving existing
   information architecture and layout behaviour.
3. Add layout-regression coverage, visually inspect representative real and
   worst-case content at every breakpoint, run all checks, and record evidence.

## What We Are Doing

- [x] Make the base type scale and all currently undersized readable text,
  icons, controls, badges, rows, and targets slightly larger and clearer.
- [x] Keep dark mode while raising foreground, muted-text, border, state, hover,
  active, focus, placeholder, and disabled contrast wherever meaning is hard to
  see, especially the navigation.
- [x] Preserve every compact single-row, truncation, overflow, responsive, and
  interaction contract with no functional or structural redesign.

## Contract

- **Goal:** Make the entire existing UI comfortably legible and easier to scan
  through one careful, restrained dark-theme visibility pass.
- **Done:** Every surface below is visibly clearer at desktop and narrow widths;
  long and dense data still truncates, scrolls, wraps, or stays on one row exactly
  where intended; all interactions and checks pass.
- **Files:** `web/src/styles.css`; only narrowly necessary `web/src/*.tsx` changes
  for shared styling hooks or accessibility; targeted `web/src/*.test.tsx`; this
  RD. No API, Rust, database, migration, routing, or data-contract changes.
- **Agent owns:** UI inventory, theme/type adjustments, minimal component hooks,
  regression tests, visual breakpoint review, accessibility checks, and local
  verification while preserving unrelated work.
- **Requester owns:** Approval for any later redesign, new theme, new dependency,
  application-wide density mode, or downstream overlay-specific change.
- **Out of scope:** Light mode, rebranding, navigation or page restructuring,
  new features, content changes, new icons/component libraries, API behaviour,
  and solving visibility by browser zoom or a blanket CSS transform.

## Visibility And Preservation Rules

- Establish an explicit base font size and increase the current visual scale
  incrementally, normally by about 1–2 px for text and proportionate small
  increases for icons, controls, spacing, and hit targets. Tune exceptional dense
  surfaces individually; do not apply one multiplier that causes layout drift.
- Keep the existing restrained dark palette and semantic state colours. Normal
  text should meet WCAG AA contrast (4.5:1), and large text, meaningful icons,
  focus indicators, and control boundaries should meet 3:1 where applicable.
  Decorative and unavailable/disabled content may remain quieter but identifiable.
- Cover expanded/collapsed navigation and status; top bar and breadcrumbs; all
  Objects, Tasks, Chats, Users, Entities, Memories, Sources, Notes, Curator Runs,
  and Evals lists/details; IDs, badges, avatars, attribution, connections,
  properties, activity, provenance, transcripts, traces, usage and review
  controls; create/edit forms and modals; source/note content; schema map and row
  grid; loading, empty, error, hover, active, focus, disabled, and copied states.
- Preserve list records and compact Eval records as single rows at widths where
  they are currently single rows. Preserve the one-line description clamp,
  compact Object IDs, no-wrap badges/timestamps, endpoint and actor ellipsis,
  transcript/event/trace row behaviour, schema-cell truncation and scrolling,
  modal bounds, and all existing deliberate narrow-screen hiding/wrapping rules.
- Long titles, descriptions, UUIDs, user names, badges, URLs, unbroken strings,
  table/column names, JSON, and timestamps must not overlap, clip without an
  existing reveal path, force unintended horizontal page overflow, or displace
  fixed metadata. Do not remove information merely to make the larger scale fit.
- Preserve routes, keyboard behaviour, accessible names, click targets, loading
  and error handling, edit/create flows, responsive breakpoints, and current
  component ordering. Any layout exception needed to prevent a regression must
  be local and documented in the completion evidence.

## Checks

- [x] Targeted UI tests cover navigation states, all list families, representative
  details/forms, compact Eval rows, schema map/grid, and long-content truncation.
- [x] Browser inspection at 1440, 1024, 820, 640, and 320 px proves no overlap,
  accidental extra rows, lost controls, page-level horizontal overflow, or
  changed truncation/reveal behaviour, using representative worst-case strings.
- [x] Computed foreground/background contrast is checked for primary, secondary,
  muted, navigation, placeholder, state, link, and focus styles; keyboard focus
  remains visible and colour is not the only semantic cue.
- [x] `npm --prefix web test`, `npm --prefix web run type-check`, and
  `npm --prefix web run build` pass.
- [x] All repository-root verification commands and `git diff --check` pass.

## Verification Results

- All 49 web UI tests pass, including explicit compact-row structure checks;
  TypeScript type-check and the production Vite build pass.
- Browser checks with long and unbroken content pass at 1440, 1024, 820, 640,
  and 320 px. Desktop list and Eval rows remain 57 px; established narrow
  wrapping remains bounded; timestamps and controls stay visible; modals and
  details stay contained; and no page-level horizontal overflow occurs.
- Schema map overflow remains internal at constrained desktop widths, the narrow
  relationship fallback remains available, and row cells retain nowrap/ellipsis
  with internal grid scrolling. Sampled normal-text contrast ranges from 5.63:1
  to 15.12:1, with visible keyboard focus retained.
- Rust formatting, Clippy with warnings denied, 63 Rust/API/eval tests, 53 Python
  client tests, Python compilation, and `git diff --check` pass. Python tests ran
  through an isolated `uv` environment because system Python lacks `pytest`.

## Approval Boundary

This RD authorizes only local UI implementation and verification when execution
is explicitly requested. It does not authorize deployment, public ingress,
hosted writes, external integrations, publishing, spending, credential changes,
deletion, or changes to an organization-specific downstream overlay.

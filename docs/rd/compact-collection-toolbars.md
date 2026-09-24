# Compact collection controls and Task project identity

## Outcome
Make collection navigation and Task projects easier to scan without stacked,
duplicate headings. Shared frontend serves every deployment.

## Repository scope
centaur-context Issue105. Related #103 retains catalog/API validation work;
this slice implements visual project identity and filtering only.

## Requirements
One compact desktop toolbar: small section breadcrumb, search, applicable
filters/sort, view toggle, refresh and labelled New action. No large duplicate
collection title. Narrow screens retain accessible controls with intentional
horizontal scrolling. Project badges use colour, icon and three-letter text,
with full names available to assistive technology and in tooltips.
Project filter has All and observed project choices, plus unassigned when needed;
selection survives list/board switching. Never infer a project for legacy tasks.
Preserve record data, existing actions and Routine behavior.

## Approach
Consolidate App collection controls into topbar. Let built-in board consume
optional host controls while preserving standalone/external module defaults.
Use one project resolver and reusable identity presentation in list/board/detail.
Project options derive from the loaded deployment's tasks, not a private catalog.

## Acceptance
UI tests cover accessible badge names, project filtering and view switching;
existing create/refresh/search/board status controls work. Production build and
composition checks pass. Inspect Objects, Tasks list/board and other collections
at desktop and mobile widths, including both deployment datasets.

## Completion
Draft PR with passing checks and screenshots before review. After approval,
build shared frontend and adopt in both existing local deployments, preserving
each backend/configuration. Verify served revisions and behavior separately.
No database migration or unrelated API rollout is included.

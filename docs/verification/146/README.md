# Compact Object details — audit and verification

Issue #146, dependent on #128 / PR #147. Synthetic fixtures only. Both changes remain drafts pending explicit owner approval to merge or release.

## Audit inventory

| Surface | Finding and disposition |
| --- | --- |
| Source / Note artifacts | **Changed:** removed outer box, row separators, tall rows, both creation buttons and their unreachable forms. Titles wrap; versions and non-complete capture status remain. Empty artifact sections render nothing. Reader/editor and API capabilities remain. |
| Source Notes | **Changed:** removed duplicate Notes section and its separate collection request/state/grouping. The same explained Note connection remains in Related Objects. |
| All nine Object kinds | **Changed:** removed ordinary evidence-review action mounts from Source, Note, Task, Theme and the shared Event/Entity/Memory/User/Chat detail. Backend correction APIs, immutable history, message correction displays and Run evidence are untouched. Empty Activity headings are omitted. |
| Task | **Changed:** six common workflow fields in a compact responsive grid; description and brief visible before secondary metadata. More details retains ID, attribution, suitability, completion, issue and update information. Blocked reasons stay prominent when present. Removed duplicate assignee/project display and positive readiness boilerplate; missing-readiness warnings remain. |
| Task Routine | **Changed:** removed Make a Routine and empty scheduling prompt. Existing schedules retain pause/enable, edit, save/cancel, execution history and result acceptance. Failed reads still show errors. |
| Note body | **Changed:** removed redundant read-mode box and empty minimum height. Edit textarea retains its space, focus and save/cancel behavior. |
| Related Objects | **Changed:** relationship creation becomes a labelled disclosure with explicit Cancel, preserving the form. Narrow rows wrap title/explanation and expose the Connection link. Meaningful No related Objects state remains. |
| Focused graph | **Changed:** Connection map disclosure defers duplicate visual representation and graph request until opened; no empty map on unconnected Objects. Initial Fit exposes peripheral nodes. Native node links, focus, zoom, Fit and full Connections navigation remain. |
| Collections and registered views | **Retained:** one New control where supported, search, filters, date groups, list/board switching, copy-ID and pagination each have distinct uses. Broader creation-feature removal would require a separate decision. |
| Event | **Retained:** occurrence form and Save properties apply subtype edits and are not redundant actions. |
| Entity / Memory | **Retained:** assigned classifications and existing subtype details; classification absence and missing occurrence dates communicate actual state. No new classification or image editing. |
| User / Chat | **Retained:** identity and transcript data, capture coverage, correction display and meaningful empty states. **Deferred:** repeated identity IDs may merit a dedicated identity presentation pass; avoid changing attribution semantics here. |
| Theme | **Retained:** membership, shared metadata, connections and provenance; empty membership explains that the Theme exists without assigned Objects. |
| Artifact reader | **Retained:** back link, Edit, Save/Cancel, History/Details menu, version links and capture errors. These operate on existing documents; removing them would remove capabilities. |
| Runs / Evals | **Retained:** review verdicts, notes, golden pins, compensating undo, trace, mutations and evidence-availability explanations. **Deferred:** Outcome and End-to-end outcome partially overlap, but distinguish parent/child execution; consolidation needs its own review. |
| Connections / Schema | **Retained:** inspector, graph controls, table/row navigation, copy and pagination. These remain separate non-Object surfaces. |

## Regression and browser evidence

The [nine-kind route/entry matrix](../128/README.md) applies unchanged: generic, typed and mismatched URLs resolve without relying on loaded lists. The cleanup adds absence assertions across that matrix, Source deduplication and no-extra-fetch checks, Task edit/disclosure checks, empty/unavailable artifact cases and initial graph Fit coverage. Objects search now participates in the canonical Note test.

Browser: local Vite plus the in-memory fixture API from #128, with no database. Desktop 1280×900 and narrow 390×844 CSS viewports. Screenshots contain only synthetic IDs and content:

| Surface | Before | Desktop after | Narrow after |
| --- | --- | --- | --- |
| Source | [Before](source-before.jpg) | [After](source-desktop.jpg) | [After, scrolled to artifacts](source-narrow.jpg) |
| Task | [Before](task-before.jpg) | [After](task-desktop.jpg) | [After](task-narrow.jpg) |
| Note | [Generic baseline](../128/note-generic-before.jpg) | [After](note-desktop.jpg) | [After, scrolled to content](note-narrow.jpg) |

Observed browser interactions: Source artifact link opens working document; History opens previous immutable version; unavailable transcript shows the capture reason and unavailable editing; a focused graph node opens the complete Task with an ordinary mouse click at initial Fit. The earlier #128 browser evidence covers Objects → Note and Back/Forward. Note/Source links remain native anchors, and the same generic destination resolves to the full typed detail.

Browser limitation: the dirty-document discard confirmation blocked the in-app browser automation, including its dialog API. The synthetic tab was closed; no successful browser dismissal or save is claimed. Existing automated dirty-link, history navigation, draft preservation and editing tests pass. Live production acceptance and successful external Entity image downloads are not claimed.

## Checks and review

Local TypeScript, 166 web tests, production web bundle, npm audit (zero vulnerabilities), package contract, Rust formatting and whitespace checks pass. The preceding routing commit also passed all 80 Python tests and all 19 UI-composition tests. Final composition and required CI results are recorded on the PR. Full Rust/database tests run only in CI using the disposable `centaur_context_test_ci` database, avoiding heavy local builds during storage recovery.

Direct self-review covered the full diff, route ownership, revision-aware saves, effect cleanup, native anchors/disclosures, keyboard focus, meaningful empty/error states, and retained information. No schema/data/storage/overlay changes or production service restarts. No merge or deployment performed.

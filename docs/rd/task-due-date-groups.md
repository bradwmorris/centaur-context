# Group the built-in Tasks List by due date

## Outcome
Selecting Due date in the existing Tasks List dropdown produces a compact, readable agenda using canonical task deadlines. Users can scan what needs attention today and on upcoming days without opening every task.

## Repository scope
Implementation belongs to bradwmorris/centaur-context. No private overlay code or identities belong in this public change. Deployment adoption is separately tracked. Work from current origin/main in an isolated codex/<issue>-task-due-date-groups branch. This issue contains the full public requirements; no private reference is needed to implement it.

## Requirements
- Add Due date to the existing Tasks List sort dropdown alongside Recent and Connected. Keep the default Recent and existing modes intact. Do not add an Agenda tab.
- Use Task.due_at, never created_at, prose dates or routine next_run_at. Presentation changes must not mutate task records, deadlines, statuses or routine scheduling.
- Date mode groups unfinished tasks in order: Overdue (deadline on a prior local calendar day), Today, Tomorrow, then each subsequent exact day, then No due date. Empty groups are absent. Same-day tasks remain Today even if their due time has passed; this is a day agenda, not an hourly alarm.
- Format later dates like Thu 15 Oct; include the year when needed to distinguish years. Use the browser's local calendar/timezone consistently for labels and boundaries; do not hardcode an adopter timezone or group by UTC date strings. Recompute on local midnight and after tab resume. Respect DST calendar-day boundaries.
- Sort dated tasks ascending by due_at within groups with deterministic title/Object ID tie-breaks; undated tasks use the same stable tie-breaks. Malformed legacy dates must not crash or silently drop rows; place them with undated items and a clear unavailable-date indicator.
- Keep completed tasks out of Overdue. In Date mode reuse the existing Show completed control/pattern, hidden by default; when enabled place completed tasks in a separate final Completed section. Recent/Connected and Board retain their current visibility behavior. No record is deleted or reclassified.
- Apply existing search and project filters before grouping. Each eligible task appears exactly once; counts reflect the visible groups. Retain existing row identity, owner/status/issue metadata, ordinary task detail navigation, keyboard accessibility and empty/loading/error states. Show the due date in date-mode rows rather than a misleading creation-age timestamp.
- Switching to Board or another collection must not send a UI-only due-date value to an unsupported backend sort endpoint or change existing Board behavior. Preserve normal return-from-detail behavior within the existing navigation model.
- Use a small reusable field-based list grouping helper/component, with due_at as its first supported field. This establishes a path for later grouping by other fields. Do not build a schema-driven view editor, arbitrary cell context menu, new API/database schema, calendar, drag-rescheduling, persistence service or other grouping options in this task.

## Approach
Current evidence (13 October 2026): web/src/App.tsx owns sort state, api.tasks(sort), project/search filtering and the built-in record list; the toolbar currently exposes Recent/Connected. web/src/api.ts owns ListSort and paginated task loading; web/src/types.ts already exposes nullable due_at. web/src/modules/moduleRegistry.tsx registers Board and external views; extensions add views rather than override List. web/src/TaskIdentity.tsx provides existing task identity primitives.

Separate client presentation mode from API sort parameters; load the complete existing task collection and group after current filters. Extract only enough rendering/grouping code to avoid duplicating rows. Keep date classification a pure testable function with injected current time/timezone where practical. Reuse native Intl/local calendar operations and current styles; avoid a large new dependency. Check current source and CI before coding.

Executor preference: GPT-6 Luna High, with self-review. Escalate unresolved semantics, required API/schema changes, scope expansion, or two unexplained failed repair attempts to the planner/owner. Writing this preference does not start execution or select a runtime.

## Acceptance
- At a frozen local 13 October 2026, unfinished tasks due 12/13/14/15 October render exactly once under Overdue/Today/Tomorrow/Thu 15 Oct. Null and malformed deadlines remain discoverable. Mixed due times on 13 October remain Today. No empty heading appears.
- Verify stable order, multiple tasks per day, year boundaries, timezone conversions near UTC midnight, DST transitions and midnight/tab-resume rollover. Include Australia/Sydney and a negative UTC offset as test fixtures, not application defaults.
- Completed items never inflate Overdue; the date-mode completed toggle reveals them in Completed without changing normal List/Board visibility.
- Search/project filters, due-date editing through existing details, back navigation, loading/error/empty cases, all paginated task results and keyboard navigation work. Editing a deadline then returning/reloading moves its group correctly. Grouping itself performs no writes.
- Recent/Connected, Board URL /tasks?view=kanban, other collections and registered overlay views regressions checked; no unsupported backend sort is sent.
- Run applicable checks from CONTRIBUTING.md and required CI, including UI type-check/tests/build, package/import boundaries and full required repository checks. Never use a live database for tests. Record failures honestly; zero CI checks is not proof of passing required checks.
- Actual browser verification at desktop and narrow widths with synthetic data and screenshots. Review the diff for unnecessary abstractions or duplicated row UI. Public screenshots contain synthetic data only.

## Completion
Open a linked draft PR with passing pre-release checks, screenshots and self-review. This issue reserves owner approval of the concrete PR revision before merge/release. After approval, finalize/merge, record immutable build/source evidence and coordinate adopter rollout. Shared implementation and deployed adoption are separate evidence: do not claim live delivery on merge alone. Planning creates records only; it does not dispatch execution or authorize deployment.

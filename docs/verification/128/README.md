# Canonical Object details — pre-release verification

Issue: #128. All evidence uses synthetic records. Release and live acceptance require owner approval.

The resolver reads the requested Object independently of collection contents, replaces generic or mismatched URLs with its actual kind's route, and preserves artifact/working-document suffixes, query and hash. Unknown kinds retain a generic page; failed or inconsistent reads show an error. Connections, Runs, Evals and Schema keep separate routes.

## Regression matrix

| Kind | Generic / typed / mismatched route, absent from list | Complete subtype and shared description |
| --- | --- | --- |
| Event | Automated | Occurrence and existing editing |
| Task | Automated | Brief, status, assignee, due date, priority, blockers, issue; shared description editing |
| Chat | Automated | Transcript and shared metadata |
| User | Automated | Identity and shared metadata |
| Entity | Automated | Assigned classifications, legacy kind and existing HTTPS image field |
| Memory | Automated | Existing happened_at |
| Source | Automated | Source metadata and artifacts |
| Note | Automated | Full body, format, intent; revision-aware description then body save |
| Theme | Automated | Membership plus connections, activity and provenance independent of loaded list |

| Entry or compatibility case | Evidence |
| --- | --- |
| Objects / search rows | Native anchor rows; existing server-search regression; Note opens complete page in browser |
| Typed collection rows | Same authoritative gate; nine-kind matrix |
| Related Objects | App regression opens Note from Source; shared generic resolver |
| Graph | App regression and browser keyboard navigation open complete Task from Note graph; inspector links retain resolver |
| Object ID, activity, Connection endpoints | Existing identity/link tests plus shared generic resolver |
| Run references | Existing Run/Eval tests; references use resolver and guarded SPA links |
| Registered views | Existing task-board and Source view-host tests retain their navigation contract |
| Schema, Connection, Run, Eval surfaces | Existing routing/App suites retain distinct non-Object routes |
| Nested exact artifact / working document | App tests from mismatched typed paths; existing reader/editor, history, retry and dirty-navigation tests |
| Unknown, missing, inaccessible, inconsistent | Safe generic badge fallback and resolver validation; HTTP 403/404/500 regressions |
| Query/hash and history | Nested and ordinary path tests; browser Back returns directly to Objects and Forward to typed Note |
| Modifier-click / cancellation | Native anchors, interception test, existing independent copy controls and reader dirty-navigation coverage |

## Browser evidence

Local Vite development UI with an in-memory synthetic HTTP API; no database or live data was used. Start with Node 22 using `node --experimental-strip-types scripts/object-ui-preview.mjs web 49129`. To reproduce the baseline, pass the clean baseline web root as the second argument and another port.

- `note-generic-before.jpg`: baseline generic page omits the Note body.
- `note-from-objects.jpg`: opening the Objects row reaches the full Note page and body.
- `task-from-graph.jpg`: keyboard activation of a focused graph link reaches Task workflow fields and brief.
- `entity-from-generic.jpg`: generic Entity URL reaches assigned classifications and subtype details. The deliberately unresolvable example.test image verifies URL/alt wiring, not a successful external image download.

The initial graph zoom clipped peripheral nodes in the synthetic graph. Both keyboard activation and mouse activation after Fit reached the complete Task. The dependent UI cleanup will start the graph at Fit. Native graph links also retain label hit targets and keyboard focus styling.

## Checks and limits

Local: TypeScript, all 165 web tests, web production bundle, npm audit (zero vulnerabilities), package contract, Rust formatting, Python package installation, all 80 Python tests, compileall and diff whitespace checks. Full Rust compilation/tests are delegated to repository CI's disposable `centaur_context_test_ci` database to avoid heavy local builds during storage recovery. UI composition and CI results are recorded on the PR.

This is pre-release frontend evidence. No production restart, migration, overlay edit, merge or deployment was performed. Post-release acceptance remains reserved for approved release.

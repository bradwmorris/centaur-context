# Simple artifact reader and working-notes editor

## Outcome

Users can discover an Object's attached artifacts, click one to read it comfortably, and edit working notes without losing earlier versions or changing original source evidence. Replace the current technical dropdown/preview interaction with a small artifact list and a dedicated reading page. Keep the interface suitable for future formats without building those formats now.

## Repository scope

Centaur Context owns this entire implementation. [Context #110](https://github.com/bradwmorris/centaur-context/issues/110) is the canonical task and carries this complete RD. This is a focused follow-up to completed Context #107, not a reopening of its broader research overhaul. No private-overlay changes or separate Context Task are required for this shared-product deliverable.

Preparation authorizes the task/RD only; it does not launch execution. Preferred executor when dispatched: GPT-6 Luna, High. Use an isolated `codex/<issue>-artifact-reader-editor` branch from current main. Do not create a Codex task or launch other agents implicitly.

## Requirements

1. Show a compact Artifacts section on the existing Source and Note detail pages. Each logical document has a clickable row with a readable title/type; notes use “Working notes” or their title so they are distinct from Note Objects. Hide hashes, indexing configuration, document keys and capture plumbing under Details. Keep meaningful incomplete/unavailable content status visible. Preserve access to existing artifact kinds and add-artifact functionality.
2. One row represents one working document, with the latest revision as its default. Group `research_notes` by owning Object and stable `metadata.document_key`; follow the documented legacy missing-key convention without modifying old records. Use predecessor links for explicit revision history. Unrelated artifacts, distinct publication receipts and separate documents must never collapse merely because their kind/title matches. Ambiguous legacy branches must remain accessible rather than being silently discarded.
3. Clicking a row opens a dedicated, refreshable reading route with Back to Object, document title, body and a small overflow menu for History/Details. Use a comfortable reading column and responsive layout. Support a stable link to a working document's latest version and exact historical artifact links using existing document keys/artifact IDs; no new top-level Object kind. Back returns to the correct parent. Exact route spelling and visual styling are executor choices consistent with the app.
4. Read plain text with preserved paragraphs and render explicitly marked Markdown safely. Do not execute raw HTML/scripts or infer active content from arbitrary text. Existing `text/plain` data must remain literal and readable. Handle MIME parameters such as charset. Reading opens content automatically; fetch subsequent windows as needed without a manual “Load next 8,000 characters” workflow.
5. Provide Edit for the latest supported text working-notes document, followed by Save and Cancel. Use a simple text/Markdown editor, not a block-editor framework. Preserve the existing media type on edits; newly created working notes may use Markdown. Offer a straightforward Add working notes action with title and body, generating the document key internally. Retain generic artifact creation behind a secondary action rather than deleting that capability.
6. Fetch and verify the complete body before enabling editing. Never save a partial preview as a full document. Preserve the user's draft on load/save failures or revision conflict. Protect against accidental navigation away with unsaved edits. Cancel creates no revision; unchanged Save creates no duplicate.
7. Save appends an immutable `research_notes` artifact on the same Object with the stable document key, preserved relevant metadata and both predecessor references set to the previous working revision. Supply the Object revision captured with the edit base to the existing optimistic-concurrency API. A stale save must fail visibly without discarding the draft or silently branching from an outdated document. Retry ambiguous submissions using a stable idempotency key and readback. Verify returned identity/content before reporting success, including the existing content-hash deduplication behavior.
8. Keep canonical transcripts and all historical revisions read-only. Saving notes must not change `sources.current_artifact_id`, original transcript content, source locators, Note Object content/intent or independent documents. Never weaken protected-Object/actor authorization to make editing succeed; explain denied actions and escalate any material policy gap.
9. History lives inside the document view and allows reading older versions. Show date/author only when reliable information is available. Restoring an old revision, transcript correction, artifact deletion, collaborative editing and automatic merge are out of scope.
10. Select presentation by supported media type/capability. In this increment implement text/Markdown and a safe fallback showing title, type, availability and a validated existing URI open action when applicable. Structured JSON receipts remain reviewable as readable text. Do not promise downloads when no binary endpoint exists. Do not execute embedded HTML, SVG or scripts or automatically fetch arbitrary remote URIs. Native diagrams, image/PDF viewers, uploads, binary storage and format-specific editors are future work. Unsupported artifacts must remain discoverable without crashing the page.

## Approach

Inspect current code before editing; these references were checked during preparation:

- `web/src/App.tsx`: `Artifacts`, `ArtifactSummary`, Source and Note detail integrations; currently manually loads 8,000-character windows and exposes capture fields.
- `web/src/api.ts`: `artifacts`, `artifactContent`, `createArtifact`; Object-scoped listing/append and artifact-ID content windows already exist.
- `src/api.rs`: artifact read/append handlers, including optional `expected_revision` input.
- `src/db/artifacts.rs`: immutable append, Object revision locking/checks, content-hash deduplication and predecessor ownership validation. Deduplication is currently Object-wide and precedes the revision check; explicitly test same-text saves, reverted text and concurrent saves rather than assuming append always returns a new successor.
- `docs/research-artifacts.md`: existing working-document keys and predecessor convention; canonical Source evidence remains immutable.
- `web/src` routing/types/tests and `scripts/context-ui.test.mjs`: reuse existing conventions.

Prefer small reusable list/reader/editor components with a shared opening route and capability decisions. Reuse the artifact model and authenticated APIs. Small generic API correctness changes required to guarantee conflict safety and correct successor identity are in scope with focused regression tests. Do not change global ingestion/deduplication semantics casually. If safe append semantics require a new document model, migration, broad API change or permission change, preserve the work and escalate the concrete decision first.

Use synthetic fixtures only. Do not copy private transcripts, Object identifiers, operational URLs or overlay records into public code, tests, screenshots, Issues or PRs. Do not run writes against live data during implementation.

## Acceptance

- A synthetic Source with a transcript and two working-note documents shows three rows. Multiple revisions of either notes document do not produce duplicate rows; both documents and all their old revisions are independently reachable.
- Clicking a row opens readable content immediately, supports keyboard navigation, has clear loading/error/empty states and works at desktop and narrow widths. Direct navigation/refresh and Back work. Exact historical links continue to identify the same immutable artifact after later edits.
- A notes document longer than 16,000 characters, including Unicode, loads fully before editing. Editing its beginning and saving preserves its entire tail. A failure loading a later window prevents Save of a truncated document.
- Save appends the correct successor, advances the displayed current document and preserves previous content. Cancel and unchanged Save add nothing. Saving text matching an older revision or another document cannot falsely claim that a new edit is current; regression evidence covers deduplication outcomes.
- Two editors opened at the same revision cannot silently overwrite or branch around each other: after one saves, the other sees a conflict and retains its draft. A retry after an ambiguous response produces at most one intended revision. Unrelated Object revision changes are handled safely.
- Transcript, canonical pointer, excerpt locators, Note content/intent, independent documents and existing immutable events remain unchanged by working-note edits. Protected/unauthorized fixtures enforce existing access rules.
- Plain text with a charset, Markdown, JSON publication receipts, URI-only and unsupported artifacts all open appropriately. Markdown/URI security fixtures cannot execute injected content. A URI-only artifact does not trigger repeated text-window failures.
- History and Details expose supporting information without cluttering the main Object page. No technical document-key or capture-status form is required for ordinary working-note creation/editing.
- Run the required checks in current `CONTRIBUTING.md`, including frontend type-check/tests/build, UI integration, relevant Rust/Python checks and `git diff --check`. Add meaningful frontend/API regression coverage for completeness, revision conflicts, immutable history and identity/deduplication. Database tests use only disposable databases matching the repository guard.
- Provide browser evidence using synthetic fixtures for list, reader, edit/save, history, conflict and narrow layout. Self-review the diff and present a draft PR with exact head, check results, limitations and any compatibility impact. A substantive shared-core review by Sol is recommended before release; this document does not dispatch a reviewer automatically.

## Completion

This preparation is complete when the GitHub task contains the RD and its local copy is available. Implementation, when separately dispatched, stops at a review-ready draft PR with passing required checks. Do not infer merge/deployment approval from this task, model assignment or an execution request.

Brad reviews the concrete PR revision before merge or live deployment. Record the approved head and the chosen deployment destination before release. No destination, owner or due date is invented by this RD. After explicit release approval, follow the applicable deployment workflow and verify list/open/edit/history using approved synthetic data on the selected instance. Do not claim live completion from a local build or merge alone.

Escalate material architecture/policy changes, cross-repository scope, or two attempts at the same unexplained failure; report evidence and preserve useful work. Ordinary component boundaries, route spelling and styling choices are delegated to the executor.

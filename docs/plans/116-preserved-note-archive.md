# Archive preserved research Notes

## Outcome
Permit session-authorized archival of legacy protected research Notes after verified consolidation, retaining immutable history and leaving the curated Notes active.

## Scope and dependencies
centaur-context only, Issue 116. Build on protected Source research_notes support from Issue 114. No schema change, maintenance credentials or broad protected-write access.

## Requirements and approach
Ordinary archive_object may archive a protected Note only when its exact current content is contained in a complete research_notes Artifact attached to an active Source to which the Note has an active derived_from Connection. The Artifact must have a nonempty document_key and a source_note_manifest entry matching object_id and current revision. Reject empty content, stale revisions, absent/mismatched manifest, wrong Source, incomplete capture, and all other protected record archival. Preserve source canonical evidence and artifact history. Inspect whether existing archival cascades connection states and verify appropriate active-list behavior.

Current references: src/universal.rs archive_object and lock_writable_object; universal_contract integration tests; contract/context-contract.json; tools/codex_context generated resources. Existing research_notes metadata contains source_note_manifest object_id/revision and preserved complete bodies.

## Acceptance
Focused real disposable database tests for permitted preserved Note archival and all denial boundaries, normal archival regression coverage, formatting, clippy, required CI, self-review and draft PR. Keep change narrowly scoped. No production test fixtures.

## Completion
Approved destination is existing local ENYU Context deployment plus compatible host bridge. User explicitly approved removing the 33 legacy RSI Notes after consolidation, then granted permission to resolve the protected archival blocker. Record reviewed PR head before release. Verify supported native Context archival and readback for the approved manifest only, retaining the three curated Notes and unrelated material. Record live evidence in linked Task before Done.

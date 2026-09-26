# Research notes on protected Sources

## Outcome
Authenticated research agents can append a versioned research_notes supporting document to a protected Source through universal apply. Canonical evidence and protected records remain protected.

## Repository scope
Only bradwmorris/centaur-context, Issue #114. Server validation, contract/guidance, focused synthetic regression tests, and the corresponding packaged desktop bridge contract. No schema migration, credential changes, broad protected-record edits or product-specific data.

## Requirements
Permit research_notes append to active protected Sources only, with a stable nonempty metadata.document_key. Preserve source protection, canonical evidence pointer, source fields and existing artifacts. Successors may supersede only research_notes from the same Source and document key. Preserve revision checks, idempotency, atomic transactions and audit attribution. Other protected writes remain denied.

## Approach
Add a narrowly scoped artifact-target validation path in src/universal.rs, without changing the general protected-object mutation guard. Reuse append_artifact behavior and validate predecessor identity and kind. Update the public contract and generated guidance if needed; coordinate installing the compatible desktop bridge bundle. Keep code generic.

## Acceptance
Synthetic disposable-database tests cover first append, successor, replay, stale revision, forbidden canonical-evidence/cross-document supersession, other artifact kinds, protected non-Source targets and unchanged source metadata/protection/evidence. Run required repository checks, inspect the diff and open a draft PR.

## Completion
After review approval of the tested PR head, merge, build and deploy the Context update to the existing local adopter; install matching desktop bridge resources if needed. Verify native desktop MCP append/read and exact preservation of the working-note document. Release approval is required before merge or live deployment.

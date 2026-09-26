# Connect protected research Objects

## Outcome
Authorized users can add explained research attribution and entity relationships through the universal Context API even when the existing endpoint Objects are protected. Adding a new Connection must not be treated as editing endpoint contents.

## Repository scope
centaur-context only; Issue 109. No schema, infrastructure, credential or overlay changes. Existing authenticated Context principal and grants remain the authorization boundary.

## Requirements
Permit new non-protected source-to-entity `involves` and `about` Connections and entity-to-entity `related_to` Connections between active Objects regardless of either endpoint's protected flag. This supports guest/speaker, host, publication and person/organization relationships. Require existing valid descriptions and provenance. Preserve protected Object content/update/archive restrictions and protected Connection update/archive restrictions. Do not extend this exception to update_connection, system-managed endpoints, unsupported kinds or archived endpoints. Retain existing Note evidence rules, verified Chat provenance, idempotency and immutable events. Duplicate identical links reuse the existing edge; different assertions must not mutate a protected existing edge. No unconditional unprotecting or maintenance-credential workaround.

## Approach
Inspect src/universal.rs validate_endpoints and its create/update callers. Make the additional permission explicitly creation-only, preserving existing exceptions. Update the contract and generated guidance to distinguish creating an attribution link from editing protected records. Authorization is the existing authenticated tool request; this does not introduce a claimed-approval field or widen route grants.

## Acceptance
Disposable synthetic database integration tests cover all three allowed shapes with one/both endpoints protected, non-protected created links, unchanged endpoint content/revisions/protection, duplicate replay, validate-only rollback/no events, and rejection for archived endpoints, unsupported kinds, system-managed endpoints, protected edge modifications and unrelated protected endpoint combinations. Preserve ordinary and reviewed-maintenance regressions. Run required repository checks and CI; inspect the diff and open a draft PR before release.

## Completion
User explicitly requested fixing the blocker now and then adding the approved research connections. Release to the existing local ENYU Context deployment with compatible Codex bridge contract package after tests and review. Record reviewed head, this approval and destination in the Context Task before release. Verify native Codex source-to-guest/host/publication links and host-to-podcast relationship, while endpoint protection remains unchanged. No other deployment or data cleanup is included.

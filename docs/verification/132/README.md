# Issue 132 verification

Synthetic verification on 2 October 2026, integrated with main at
`13987f7e14aeac32cdfc40fbe7a8f4cd516fb079` (readiness PR #133).
The PR head identifies the exact review revision. No live acceptance or release
is claimed. Bradley must approve the concrete revision and destination before
merge/deployment; Issue #132 remains open.

## Checks

All CONTRIBUTING checks passed: package/contract/route checks, formatting,
Clippy with warnings denied, Rust tests, npm clean install and high-severity
audit (zero vulnerabilities), TypeScript, web tests/build, UI integration,
Python tests/compile, isolated package installation, and diff whitespace checks.

- Rust unit tests: 85 passed.
- All 21 Rust integration suites ran with actual disposable PostgreSQL databases:
  124 tests passed. Each suite used its own database named
  `centaur_context_test_132_integrated_<number>` in an isolated pgvector/pg17
  container. No live database or Centaur operational store was accessed.
- Web: 109 tests in 19 files; UI integration: 19 tests.
- Python client/CLI/rename/bridge: 78 tests. Fresh installation of both packages
  in the worktree virtual environment matched the canonical contract and all
  six new correction operation schemas. Installed user bridges were untouched.
- After self-review normalized top-level context corrections, all nine correction
  integration scenarios passed again, including a context-packet readback.

## Acceptance coverage

See [the RD](../../rd/132-agent-corrections.md) for the persisted-family inventory.
Tests below use authenticated HTTP apply/read/search paths where applicable;
SQL only prepares synthetic fixtures and verifies durable invariants.

| Family / behavior | Evidence |
| --- | --- |
| All eight Object kinds, protected and unprotected, imported before snapshots | `every_kind_corrects_restores_and_preserves_real_imported_preimage`: subtype edits, real original snapshot, validate-only rollback, stale revisions, idempotent replay, invalid batch rollback, archive/restore |
| Source canonical repair and dependent citations | `canonical_repair_keeps_exact_citations_and_rejects_fabricated_derivation`: retained Artifact retrieval, promotion impact IDs, current capture body indexing, fabricated quote and wrong derivation rejection, secondary contextual link |
| Historical messages, Artifacts, Events, Runs | `message_event_run_and_artifact_corrections_are_attributed_and_recoverable`: attributed assertions, supersession/withdrawal, original retention, wrong-owner rejection, immutable-table enforcement |
| Protected Connections and endpoints | `protected_connections_edit_archive_restore_and_enforce_endpoints`: Theme, Entity, Source relationship shapes, revisioned edit/archive/restore, active endpoint integrity |
| Immutable kind/UUID replacement | `wrong_kind_replacement_relinks_both_directions_and_retains_old_identity`: one supported apply batch creates replacement, repoints incoming/outgoing Connections, archives old links/Object, retains provenance and original identity |
| User binding and Task owner repair | `identity_reassignment_repairs_future_mapping_without_forging_authorship`: existing external identity transfer and Task owner correction, original sender identity and authenticated actor unchanged |
| Chat/User/Memory creation; imported Chat routing | `system_types_create_and_chat_replacement_preserves_original_capture`: all three kinds created through apply; routing transfer, old messages retained, revision conflict and idempotent retry |
| Producer replay and derived truth | `producer_replay_preserves_explicit_user_chat_and_message_corrections`: real Slack ingestion replay cannot revert corrected profile/channel; original message plus current assertion; normal search/context readback; superseded text no longer matches; embedding hash invalidation and explicit rebuild |
| Task restoration and store boundaries | `restore_respects_active_task_ownership_and_corrections_cannot_cross_store_ids`: restore assigned User before Task; nonexistent store-local targets rejected; existing `api_auth` and `codex_contract` suites preserve authentication and cross-repository denials |
| Routines, curation, execution, receipts, fences | `task_routines`, `memory_dreaming`, `curator_runs`, `external_action_recovery`, and all maintenance suites pass; explicit Memory edits remain excluded from autonomous review; routine confirmation and execution/fence semantics preserved |
| Evidence purge and migration ledger | `reviewed_purge_contract` plus `historical_correction_blocks_purge_of_its_original_evidence` unit regression; migrations 36/37 applied to disposable databases, correction evidence cannot be purged |
| Human UI and packaged tools | `CorrectionHistory.test.tsx` verifies revisioned, reasoned correction of selected message; all existing UI checks pass; bridge schema test and freshly installed contract agree |

## Self-review and remaining release work

Reviewed server authority, locked before/after snapshots, participant lock order,
identity transfer, immutable evidence associations, citation integrity, derived
hashes, UI attribution, purge references, and contract/documentation consistency.
Removed obsolete protection exceptions rather than adding another allowlist.
Reconciled #133's valid onboarding changes and removed stale protection guidance.

Object discovery retains its deliberate title/description scope plus explicit
current correction assertions. Original capture-body retrieval remains a
separate evidence path; promotion updates its current canonical selection.
Literal provider identifiers, Object kind/UUID, original authorship, and audit
history are preserved through annotations or explicit replacement/relink paths,
not forged in place. Credential configuration remains outside domain metadata.

Schema 37 adds immutable correction storage and rebuilds the Object lexical
index. Deployment should account for migration/index work on the actual store.
After separately approved release, verify representative corrections and links
through the deployed agent tools, then read back current state and retained
history. Local synthetic tests and a PR do not establish that live acceptance.

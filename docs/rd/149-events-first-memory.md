# Event capture and evolving Memory synthesis

## Outcome
Establish Events-first capture and selective, evidence-linked evolving Memories for new activity. Historical cleanup is deferred.

## Repository scope
Public reusable Context implementation, Issue #149. Companion broker: bradwmorris/centaur#57. Predecessors #136/#123; native capture #135.

## Requirements
Conversations and verified action evidence produce coherent canonical Events; Events selectively produce useful evolving Memories. Zero output is a checkpointed success. Preserve evidence, uncertain times, attribution, historical applicability, protected/manual records, revisions and recovery. Synthesis is the single automatic Memory writer; explicit correction remains separate. No historical backfill, legacy sweep, new scheduler/service, vector store or generalized framework.

## Approach
Use existing curator queue/leases and transactional Run/audit machinery for Event extraction and enrichment. Version the model schema and restrict Event edits and links to supplied evidence and revision-checked candidates. Replace deterministic committed-object Memory creation with bounded action-evidence extraction. Replace Memory-centred dreaming input selection with new/changed Event versions, retaining saved previews, expected revisions, undo and recovery. Discover bounded related Memories and evidence without a collection-wide sweep.

Add a durable activation cutover and input-version coverage using Runs; preserve already pending conversation inputs. Enumerate input versions rather than a moving timestamp watermark so late commits cannot disappear. Fingerprints must cover only Event content and explicitly supporting evidence/corrections, not unrelated Chat growth, global Object revisions or synthesized Memory output. Track completion including zero output, deferred evidence, bounded failures, pending age and last success through authenticated API and visible status. Extend existing correction authority without allowing ordinary agent Memory creation to bypass synthesis. Exact migrations/contracts remain subject to implementation tests before release.

## Acceptance
Pre-release: schema compatibility; grouping and evidence validation; interactive/background deduplication; new/updated/withdrawn evidence; explicit preference and change of mind; zero output; stale revisions; replay, late commits and restart; protected/manual preservation; narrow invalidation and no self-trigger loop; visible retry/deferred/failure state. Use synthetic fixtures and existing CI databases. No extra local database or destructive live test.
Post-release: small reviewed saved-plan trial and actual supported ingress paths in each installation; truthful timing/evidence and useful selective Memory output; enable automatic synthesis only after reviewed trial. Preserve old records and prove cutover coverage.

## Completion
Self-review and passing draft PRs precede one exact-revision owner approval checkpoint. No merge/deployment before approval. Deploy compatible broker first, pause legacy automatic Memory paths, install new consumer in preview, review/apply exact saved trial, enable synthesis and verify both destinations. Keep parent open until live acceptance. Rollback pauses new workers and preserves evidence/checkpoints; it must not restart legacy flooding. Material scope changes require owner clarification; ordinary implementation details are executor-owned.

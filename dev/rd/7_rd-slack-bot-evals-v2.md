# 7 — RD: Slack Bot Evals V2

**Status:** `scoped`
**Created:** 2026-09-02
**Dependency:** Land and synchronize the Context, Centaur, and Enyu baseline
branches from RD #78 before execution begins.

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** RD #78 closeout; live Rez and Ed Slack tests; interaction,
workflow, and Curator Runs; Source, Note, and Task read/write/linkage contracts;
Run list/detail UI; idempotent replay behavior; current local Kubernetes
deployment.

**Missing:** The three baseline branches must be reviewed, merged, synchronized,
and deployed from landed commits. Execution should then start in a new clean
Codex task and agent, not continue the RD #78 task history.

1. Record the landed Context, Centaur, and Enyu commits and freeze a small V2
   fixture manifest with exact expected and forbidden Object IDs.
2. Run one marked Slack interaction at a time. For each interaction, wait for a
   terminal reply and verify its Run, trace, usage, consulted/affected Objects,
   resulting records, and connections directly through Context.
3. Fix any baseline regression at its owning layer, add a focused regression
   test, redeploy the landed candidate, and rerun the exact failed interaction.
4. Repeat the clean suite once, replay mutation cases, and produce a concise
   machine-readable result plus a human summary.

## What We Are Doing

- [ ] Prove every completed Rez and Ed interaction creates one readable,
  terminal interaction Run with useful messages, tool traces, usage, and linked
  Objects.
- [ ] Prove Rez can retrieve existing context, ingest a Source, and create linked
  Notes and Tasks without duplicates on exact replay.
- [ ] Prove Ed can retrieve allowed Sources and Notes while prohibited writes
  fail clearly and create no durable record.
- [ ] Prove multi-turn grounding, unsupported-answer handling, persona isolation,
  workflow/Curator child Runs, and Chat linkage with deterministic evidence.
- [ ] Produce one repeatable V2 report that separates hard data invariants from
  qualitative answer scoring.

## Contract

- **Goal:** Turn the working MVP baseline into a small, deterministic and
  repeatable Slack bot evaluation suite.
- **Done:** Every frozen V2 scenario passes twice; mutation replays create no
  duplicate Objects or connections; every interaction and child operation has
  the expected Run evidence; failures identify their owning stage; and the
  report records exact deployed commits and fixture versions.
- **Files:** This RD; bounded fixture/runner/report code in Centaur Context;
  reusable Run instrumentation in Centaur; private prompts, roles, workflows,
  deployment pins, and non-public fixtures in Enyu.
- **Agent owns:** Clean-task setup, Slack test operation, deterministic evidence
  collection, scoped fixes, focused tests, local deployment, reruns, and report
  generation.
- **Requester owns:** Disputed semantic judgments, fixture changes that alter the
  oracle, publication, and merge approval.
- **Out of scope:** A broad observability platform, Phoenix installation, direct
  agent database access, `ai_v2` or Console databases, public ingress, unrelated
  product work, or weakening an oracle to make a failure pass.

## Initial V2 Scenarios

1. Rez retrieves a known Source and distinguishes it from a decoy.
2. Rez ingests one article or video and exact replay reuses the Source.
3. Rez creates one Note and one Task linked to both the Slack Chat and Source;
   replay creates no duplicates.
4. Rez completes a multi-turn grounded discussion and closure/Curator flow.
5. Ed reads allowed Context and is denied a prohibited write without claiming
   success.

## Checks

- [ ] The fixture records exact prompts, expected/forbidden IDs, evidence spans,
  content hashes where relevant, and landed/deployed revisions.
- [ ] Results correlate by run marker plus Slack workspace/channel/thread, never
  by timing alone.
- [ ] Reports separate retrieval, answer, durable mutations, connections,
  workflow/Curator outcome, traces, errors, and usage.
- [ ] Failure coverage includes auth, timeout, lexical fallback, wrong persona,
  decoy retrieval, unsupported answer, partial workflow failure, and replay.
- [ ] Two clean passes and exact mutation replays succeed.
- [ ] Relevant repository checks and `git diff --check` pass.

## Approval Boundary

Planning this RD authorizes no implementation. Execution may use the existing
local Slack, model, Kubernetes, and Context test environment within the approved
evaluation scope. Publishing, public ingress, production deployment, new
external integrations, destructive cleanup, credentials, spending beyond the
configured path, PR merge, and deleting evidence require explicit approval.

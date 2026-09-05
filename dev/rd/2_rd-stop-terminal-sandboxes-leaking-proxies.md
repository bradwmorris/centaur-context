# 2 — RD: Stop Terminal Sandboxes Leaking Proxies

**Status:** `in_progress — implementation ready; local image proof blocked by Docker I/O`
**Created:** 2026-09-05
**Upstream GitHub Issue:** [bradwmorris/centaur#15](https://github.com/bradwmorris/centaur/issues/15)

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Live `kind-centaur-lab` pod, Sandbox CR, owner-reference,
Service, NetworkPolicy, node-capacity, deployment, and log state; Centaur's
agent-k8s backend, sandbox manager/reaper, session runtime cleanup and idle-pause
paths, SQLx assignment methods, chart cleanup settings, tests, and sandbox RFC.
The incident snapshot contained 105 proxy pods: 80 Running, 3 Pending, and 22
Failed. Of the Running proxies, 76 belonged to Sandbox CRs whose agent pods were
Failed, one had no matching agent pod, and only three had Running agents.

**Missing:** local deployment of the repaired api-rs image. Docker Desktop's
internal containerd metadata store returned an I/O error after the host reached
100% storage during the image build. Restarting/pruning the shared Docker engine
is outside this job's safe mutation boundary. Merge and non-local rollout remain
requester-owned.

1. Make terminal post-execution cleanup stop backend resources and atomically
   clear only the still-matching session assignment after the configured idle
   timeout.
2. Stop a known non-reusable assigned sandbox before creating its replacement,
   while preserving orphan-execution output recovery and concurrent newer turns.
3. Add regression tests, remove only confirmed terminal/missing-agent proxy
   resources from the local Kind lab, prove healthy pairs remain, and deploy the
   repaired image when the shared Docker engine is healthy.

## What We Are Doing

- [x] A Failed or Succeeded agent pod cannot leave its per-sandbox proxy running
  indefinitely after the owning execution is terminal and idle.
- [x] Replacing a stopped assignment releases the old Sandbox, proxy, Service,
  NetworkPolicies, registration, and state volume before assigning a replacement.
- [x] Local cleanup removes all 102 audited stale proxy resource sets, including
  the 77 that were still Running, without deleting any proxy paired with a
  Running agent.

## Contract

- **Goal:** Bound every per-sandbox proxy's lifetime to a reusable agent sandbox.
- **Done:** Unit/integration coverage proves terminal and replacement teardown;
  local Kind proof leaves no Running proxy paired with a terminal or missing
  agent; and the upstream fix is committed, pushed, and ready for review.
- **Files:** This RD in `centaur-context`; upstream Centaur session-runtime and
  focused tests, plus only directly necessary lifecycle documentation.
- **Agent owns:** Audit, implementation, tests, local image/deployment proof,
  exact-target local cleanup, issue/PR preparation, and evidence reporting.
- **Requester owns:** PR merge and any non-local/production rollout.
- **Out of scope:** Fixing the incident-triggering agent-image `chmod` defect;
  changing normal live-session retention; deleting healthy sandboxes; database
  schema changes; public ingress; cloud deployment; or external integrations.

## Checks

- [x] Focused session-runtime tests cover stopped idle cleanup, matched assignment
  clearing, concurrent assignment protection, and replacement teardown.
- [x] Centaur `cargo fmt --all --check`, workspace Clippy with warnings denied,
  and relevant Rust tests pass.
- [x] Local Kind evidence shows zero Running proxies paired with terminal or
  missing agents after cleanup, while healthy pairs continue running.
- [x] Both repositories pass `git diff --check`.

## Execution Evidence

- Before cleanup: 105 proxy pods (80 Running, 3 Pending, 22 Failed). Seventy-six
  Running proxies were paired with Failed agent pods, one Running proxy had no
  Sandbox or agent, and only three were healthy Running pairs.
- After exact-target cleanup: three proxy pods, three proxy Services, and four
  proxy NetworkPolicies remain. All three proxies are paired with Running agents;
  there are zero terminal or missing-agent Running pairs. Total namespace pod
  objects fell from 220 to 19.
- Durable implementation: terminal idle cleanup now tears down backend resources
  and conditionally clears the matching assignment; replacement stops the old
  sandbox first; the janitor observes proxy-only crash leftovers; and iron-control
  registration IDs survive api-rs restarts via Kubernetes annotations.
- Verification: focused database-backed lifecycle tests pass; the complete api-rs
  workspace test suite passes with `--test-threads=1`; workspace Clippy passes
  with warnings denied; formatting, diff checks, and Helm lint pass. The default
  concurrent workspace run exposed four existing shared-database test races, and
  each passed independently before the serialized suite passed in full.
- Deployment limitation: the local image build failed in Docker Desktop's
  containerd metadata store with an I/O error. No repaired image was deployed;
  current containment is the exact-target resource cleanup above.

## Approval Boundary

This request authorizes implementation, GitHub issue/PR creation, deployment to
the explicitly verified local `kind-centaur-lab` context, and deletion of only
proxy resource sets proven to have terminal or missing agent pods. It does
not authorize merge, non-local deployment, deletion of healthy workloads,
credentials, hosted writes, public ingress, spending, or external integrations.

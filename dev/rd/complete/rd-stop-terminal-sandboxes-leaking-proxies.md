# 2 — RD: Stop Terminal Sandboxes Leaking Proxies

**Status:** `complete`
**Created:** 2026-09-05
**Upstream GitHub Issue:** [bradwmorris/centaur#15](https://github.com/bradwmorris/centaur/issues/15)
**Upstream Pull Request:** [bradwmorris/centaur#16](https://github.com/bradwmorris/centaur/pull/16)

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Live `kind-centaur-lab` pod, Sandbox CR, owner-reference,
Service, NetworkPolicy, node-capacity, deployment, and log state; Centaur's
agent-k8s backend, sandbox manager/reaper, session runtime cleanup and idle-pause
paths, SQLx assignment methods, chart cleanup settings, tests, and sandbox RFC.
The incident snapshot contained 105 proxy pods: 80 Running, 3 Pending, and 22
Failed. Of the Running proxies, 76 belonged to Sandbox CRs whose agent pods were
Failed, one had no matching agent pod, and only three had Running agents.

**Missing:** none. The requester approved the Docker restart, local deployment,
and merge on 2026-09-05. Non-local rollout remains requester-owned.

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
- Deployment proof: after the approved Docker Desktop restart, image
  `centaur-api-rs:issue-15-proxy-cleanup` built and rolled out successfully. A
  deliberately orphaned, labeled proxy Service was observed and automatically
  reaped by the repaired two-sweep janitor. Three formerly healthy pairs became
  terminal during the forced engine restart; their proxy registrations and exact
  resource sets were removed. The final state was one Running proxy paired with
  one Running agent, zero bad Running pairs, one proxy Service, two proxy
  NetworkPolicies, 15 namespace pods, and a successful `/readyz` response. The
  cleanup interval was restored from the five-second proof setting to 300 seconds.

## Approval Boundary

This request authorizes implementation, GitHub issue/PR creation, deployment to
the explicitly verified local `kind-centaur-lab` context, and deletion of only
proxy resource sets proven to have terminal or missing agent pods. It does
not authorize merge, non-local deployment, deletion of healthy workloads,
credentials, hosted writes, public ingress, spending, or external integrations.

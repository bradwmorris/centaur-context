# RD: Switch Centaur Codex to ChatGPT Subscription Authentication

**Status:** `review`
**Created:** 2026-08-29
**GitHub Issue:** `#22`

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** The merged canonical Sources/Notes and Curator contracts;
Centaur's current Codex app-server bridge, `access_token` sandbox mode, Console
token broker, iron-proxy fragments, session/eval usage envelope, Helm chart,
tests, and operations guide; and current official OpenAI Codex authentication
and exact `gpt-5.6-luna` model guidance.

**Missing:** Interactive login, subscription/account selection, credential
creation, live model calls, deployment/restart, and final cutover are
requester-owned. Use a dedicated Centaur account/seat unless the requester
explicitly accepts concurrent refresh-token invalidation risk.

1. Make the existing Centaur `access_token` implementation an explicit,
   observable `chatgpt_subscription` product mode with tested API-key rollback.
2. Add a private authenticated Centaur-owned, tool-free Curator inference
   boundary using exact model `gpt-5.6-luna`, bounded structured output, stable
   correlation, and no durable transcript or credential leakage.
3. Switch Centaur Context's Curator adapter to that boundary, preserve its
   validation/reconciliation ownership, retain direct API only as rollback,
   and verify both repositories before requester-approved live cutover.

## What We Are Doing

- [x] Run Centaur Codex agents through ChatGPT subscription authentication with
  explicit attribution and a reversible API-key fallback.
- [x] Run the Context Curator on GPT-5.6 Luna through that subscription path,
  with no tools and no authority to write Context data directly.
- [x] Prove deterministic structured plans, bounded retries, credential
  isolation, usage attribution, and rollback with fixed fixtures.

## Contract

- **Goal:** Use ChatGPT subscription access for Centaur agents and the Context
  Curator, with `gpt-5.6-luna` mandatory for the Curator execution path.
- **Done:** Code and configuration select subscription auth explicitly; the
  private Curator boundary returns validated, correlated Luna plans without
  tools or durable transcript storage; Context remains the only reconciliation
  writer; failure and rollback tests pass; and requester-approved live proof
  completes a fresh Slack thread and Curator run against `chatgpt.com` without
  using the API key.
- **Files:** This RD; focused Centaur auth, harness, private inference,
  telemetry, Helm, operations, and tests; the Context Curator adapter,
  configuration, eval attribution, deployment examples, and tests.
- **Agent owns:** Local implementation, non-secret wiring, fixtures, automated
  checks, rollback procedure, branches, commits, and PRs.
- **Requester owns:** Account/plan, interactive login, credential creation,
  subscription/credit spend, deployment/restart approval, and live cutover.
- **Out of scope:** Treating a subscription token as a general API credential,
  exposing OAuth material, removing API rollback, public ingress, weakening
  Context validation, or letting Centaur reconcile canonical Context data.

## Detailed Requirements

### Subscription mode

- Keep `sandbox.codexAuthMode=access_token` as the implementation term; record
  it in product/eval data as `chatgpt_subscription`.
- Console retains encrypted refresh-token ownership. Iron-proxy injects only a
  short-lived bearer token and `chatgpt-account-id`, only for `chatgpt.com`.
- Keep the API key configured but unused in subscription mode. Test rollback
  for expiry, revocation, allowance exhaustion, restart, warm-sandbox
  replacement, and in-flight failure.
- Attribute raw model, provider, harness, auth mode, billing basis, upstream,
  reasoning, execution/turn IDs, and all reported token categories.

### Mandatory Luna Curator path

- Use exact model `gpt-5.6-luna` through Codex with ChatGPT subscription access.
- Expose only a private authenticated inference contract dedicated to Curator
  plans. Enforce no tools, bounded input/output, fixed instructions, one JSON
  result, request/run correlation, timeouts, idempotency, and retry safety.
- Do not durably persist the raw Curator prompt, Slack transcript, candidate
  graph, access token, or reusable bearer in Centaur.
- Context supplies the evidence, validates the returned plan, and owns the only
  reconciliation transaction. The inference boundary cannot call Context
  Builder, ingestion, Note writes, or other tools.
- Compare fixed fixtures with the direct `gpt-4.1-mini` baseline for valid-plan
  rate, repair rate, semantic results, no-op behavior, latency, usage, failure
  recovery, and unintended side effects. The direct adapter remains rollback
  only; failure to meet the Luna gates blocks completion.

## Checks

- [x] Auth, proxy, and chart tests prove correct upstream selection and no
  credentials in pods, logs, events, fixtures, arguments, or repository state.
- [x] The private Luna contract passes authentication, bounds, no-tool,
  idempotency, timeout, malformed-output, correlation, and usage tests.
- [x] Context adapter and Curator fixtures pass validation, retry,
  reconciliation, no-op, failure, and rollback tests.
- [ ] Broker refresh, expiry/revocation, allowance exhaustion, restart,
  session replacement, API-key rollback, Slack delivery, and live Luna curation
  are verified when requester-owned credentials and cutover approval are given.
- [x] All required Centaur and Centaur Context checks plus `git diff --check`
  pass.

## Verification Results

- Centaur API workspace formatting, Clippy, and tests pass; the focused private
  inference suite covers bounds, prohibited tool activity, idempotent replay,
  malformed output, cancellation, correlation, and usage parsing.
- Harness formatting, Clippy, and tests pass: 91 unit tests and 20 offline
  integration tests; four real-network/auth tests remain intentionally ignored.
- Helm lint and subscription/API-key rollback renders pass, and the Centaur
  documentation production build succeeds.
- Centaur Context formatting, Clippy, Rust tests, web type-check/build, 36 Python
  client tests, bytecode compilation, and `git diff --check` pass.
- Live credential and cutover checks remain requester-owned, so this RD stays in
  `review` rather than moving to `complete/`.

## Approval Boundary

Execution authorizes local code/configuration changes, branches, issues,
commits, pushes, and PRs. It does not by itself authorize browser login, token
extraction or storage, subscription spend, live model calls, Kubernetes
deployment/restart, removing an API key, or hosted-state changes. Perform those
only after explicit requester approval.

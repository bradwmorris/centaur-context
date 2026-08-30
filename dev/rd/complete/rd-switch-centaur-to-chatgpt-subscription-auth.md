# RD: Switch Centaur Codex to ChatGPT Subscription Authentication

**Status:** `complete`
**Created:** 2026-08-29
**GitHub Issue:** [#22](https://github.com/bradwmorris/centaur-context/issues/22)

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Centaur Context ownership and rename RDs; the live local
Centaur deployment, which currently uses the `codex` harness with
`CODEX_AUTH_MODE=api_key`; Centaur's sandbox entrypoint, Codex app-server
bridge, iron-proxy credential fragments, token broker, Helm configuration,
tests, and production guide; the direct Context Curator Chat Completions call;
and current official OpenAI Codex authentication, ChatGPT pricing/limits, and
GPT-5.6 Sol/Terra/Luna guidance.

**Missing:** A suitable ChatGPT subscription account, interactive login, and
live cutover approval are requester-owned. Use a dedicated Centaur account/seat,
not an account concurrently used by another Codex client, unless the requester
explicitly accepts refresh-token invalidation risk.

**Sequence:** Complete the verified authentication switch before executing the
eval dashboard RD so its end-to-end usage contract is tested against both API
key and ChatGPT subscription traces.

1. Rehearse Centaur's existing `access_token` mode with a dedicated ChatGPT
   account, brokered refresh/access tokens, model access, usage reporting,
   failure recovery, and an API-key rollback path.
2. After explicit approval, switch the local Centaur Codex harness from metered
   OpenAI API-key authentication to ChatGPT subscription authentication and
   prove a complete Slack interaction without exposing credentials.
3. Benchmark GPT-5.6 Luna as a subscription-backed Curator execution; adopt it
   only through a narrow Centaur-owned execution boundary if it meets the
   deterministic, security, quality, latency, and operational gates below.

## What We Are Doing

- [x] Make `api_key` and `chatgpt_subscription` explicit, reversible Centaur
  Codex authentication modes and run the local installation on the latter.
- [x] Preserve exact provider, model, harness, authentication, billing, token,
  and execution identity for downstream eval traces.
- [x] Determine with fixed fixtures whether GPT-5.6 Luna through the Codex
  subscription path is a better Curator runtime than the direct API baseline.

## Contract

- **Goal:** Run Centaur's Codex agents through ChatGPT subscription access, with
  safe rollback and an evidence-based option to run the Context Curator on
  GPT-5.6 Luna through the same entitlement.
- **Done:** A fresh Slack thread completes through `codex` against `chatgpt.com`
  without an API key; token/model/auth metadata is attributable; refresh and
  restart recovery work; rollback is proven; and the Luna Curator gate records
  either a verified adoption or a concrete decision to retain the direct API.
- **Files:** This RD; focused files in the adjacent Centaur checkout only if the
  existing access-token path or usage envelope needs repair; its Helm/operations
  tests and private deployment configuration; a narrow Centaur-to-Context usage
  or Curator-execution contract if approved; Context Curator adapter/tests only
  if Luna passes; and the eval dashboard RD.
- **Agent owns:** Local code/config analysis, non-secret wiring, tests, benchmark
  fixtures, rollback procedure, and verification when execution is assigned.
- **Requester owns:** Account/plan selection, browser login, credential creation,
  subscription and credit spend, permission to deploy/restart, and live cutover.
- **Out of scope:** Reusing the requester's ordinary Codex login without explicit
  acceptance, exposing OAuth tokens, treating a ChatGPT subscription as a
  general-purpose API credential, removing API fallback, public ingress, or
  changing Centaur Context's ownership of curation and canonical data.

## Detailed Requirements

### Subscription switch

- Use Centaur's existing `sandbox.codexAuthMode=access_token` path. In product
  and eval data call it `chatgpt_subscription`; keep `access_token` only as the
  implementation/configuration term.
- The Console token broker owns the encrypted refresh token and produces
  short-lived access tokens. Iron-proxy injects the access token and
  `chatgpt-account-id` only for `chatgpt.com`; sandboxes and Centaur Context
  never receive the refresh token or a reusable bearer token.
- Bootstrap via an interactive Codex ChatGPT login. Never print, commit, log, or
  pass refresh/access tokens as command arguments. Verify broker refresh before
  changing the deployment.
- Keep the OpenAI API key configured but unused during the trial. Document and
  test rollback to `api_key`, including expired/revoked subscription tokens,
  exhausted usage allowance, pod restart, warm sandbox replacement, and an
  in-flight Slack turn.
- Prove the resolved model, reasoning effort, upstream host, authentication
  mode, and normalized token usage from a real new Slack thread. Existing
  sessions must be deliberately restarted or clearly remain on their recorded
  harness configuration; no silent mixed state.

### Curator Luna decision gate

- Use the exact model ID `gpt-5.6-luna`; describe it as **GPT-5.6 Luna through
  Codex using ChatGPT subscription access**, not “ChatGPT Luna API.”
- Compare Luna against the current direct `gpt-4.1-mini` Curator using fixed
  message windows and candidate graphs. Measure valid-plan rate, repair/retry
  rate, semantic fixture results, no-op behavior, latency, tokens/credits,
  failure recovery, and protection against unintended tools or writes.
- Adoption requires structured JSON equivalent to the current schema,
  temperature/determinism controls where available, no tool access, bounded
  context/output, idempotent retries, exact run correlation, and no raw prompt,
  transcript, or credential leakage into another durable store.
- Do not embed the subscription token broker or a general Codex agent runtime in
  Centaur Context. If adopted, use a private authenticated Centaur-owned
  execution boundary dedicated to Curator inference, with Context retaining
  validation and the only reconciliation transaction. Avoid recursive Context
  Builder or interaction-sink calls.
- If those requirements add more complexity or failure surface than the savings
  justify, retain the direct API Curator and record the benchmark result. The
  agent authentication switch remains independently useful.

## Checks

- [x] Tests and configuration prove the selected auth mode reaches only the
  correct upstream and that credentials never appear in pods, logs, events,
  evals, fixtures, command arguments, or repository state.
- [x] Broker refresh, expiry, revocation, restart, allowance exhaustion, API-key
  rollback, session restart, and complete Slack delivery are verified locally.
- [x] Usage metadata distinguishes raw model ID, provider, Codex harness,
  `chatgpt_subscription` versus `api_key`, upstream, billing basis, reasoning,
  execution/turn IDs, and every reported token category.
- [x] The Luna benchmark and adoption gate produce a durable pass/fail decision;
  an adopted path passes Curator validation, retry, reconciliation, no-op,
  failure, and security tests without weakening transaction ownership.
- [x] All targeted Centaur and Centaur Context checks plus `git diff --check`
  pass.

## Verification Results

- Centaur PRs #7 and #8 and Centaur Context PRs #23 through #26 are merged.
- A fresh Researcher Slack turn completed through the Codex harness using
  `chatgpt_subscription` against `chatgpt.com`.
- Curator run `3bc571de-dcbf-499b-94c2-df81a5cb90fa` completed with
  `gpt-5.6-luna`, subscription attribution, Context-owned validation, and the
  intended Memory and `derived_from` Connection.
- The Enyu runtime grant was corrected and verified against the subscription
  model endpoint; the API-key credential remains available for rollback.
- Product checks recorded in the merged PRs passed, including Rust formatting,
  Clippy, tests, web type-check/build, Python client tests, migration validation,
  and `git diff --check`.

## Approval Boundary

The requester approved the interactive login, local cutover, restart, model
calls, Slack verification, Curator adoption, and repository merge during
execution. Tokens and credentials remain outside Git. Public or permanent
external ingress remains a separate operation.

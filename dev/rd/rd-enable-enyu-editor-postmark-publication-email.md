# RD: Enable secure Enyu Editor publication email through Postmark

**Status:** `in_progress`
**Created:** 2026-08-31
**GitHub Issue:** [#61](https://github.com/bradwmorris/centaur-context/issues/61)

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Root and development instructions; completed Editor publishing RD and task `01a05205-882b-74b3-b201-4a1e1ee3bcf9`; current Context workflow, webhook, permission, schema, and client contracts; current private Enyu Editor prompt, publication skill, trigger/adapter, durable workflow, grants, deployment notes, and tests; live Postmark, Cloudflare DNS, Namecheap mail, and webmail state.

**Missing:** Implementation, secret import, webhook deployment, and an explicitly approved live acceptance message.

1. Add generic, immutable external-action audit primitives and narrow authenticated client methods in Centaur Context; keep provider and Enyu policy out of the public API.
2. Add the Enyu-only Editor skill, trigger, Postmark adapter, workflows, template source/hash, recipient policy, permissions, secret references, and operations documentation in the private overlay.
3. Configure verified sending and authenticated webhooks, test without delivery, then obtain exact approval for and read back one live acceptance send before enabling the Editor skill.

## What We Are Doing

- [ ] Let the interactive Editor draft and preview a publication announcement, while only a dedicated durable workflow identity can send it.
- [ ] Send only template-driven Postmark **Broadcast** messages, initially to one requester-designated, consented recipient held outside Git.
- [ ] Require human approval bound to the exact envelope, recipient set, template/version, subject, article URL, and rendered HTML/text hashes.
- [ ] Prove every request, approval, provider attempt, delivery event, suppression, and reconciliation through immutable, privacy-minimized evidence.
- [ ] Demonstrate one explicitly approved live message from `hello@enyu.org`, provider acceptance, mailbox receipt, and production readback without exposing credentials.

## Contract

- **Goal:** Give the Enyu Editor a least-privilege, auditable publication-email capability using the existing publishing workflow's identity, adapter, checkpoint, exact-approval, and readback patterns.
- **Done:** A permitted Editor turn can draft and preview but cannot call Postmark; an exact approval enables one idempotent workflow send; delivery/suppression evidence reconciles into Context; prohibited recipients, arbitrary bodies, stale approvals, duplicate attempts, and missing credentials fail closed.
- **Files:** Centaur Context owns generic schema/migrations, HTTP API, standard client, workflow/audit primitives, tests, and generic docs. The private `centaur-enyu` overlay owns Editor prompts/skills, Postmark-specific tools and workflows, template contract/source, recipient/rate/retention policy, grants, deploy configuration, and runbooks. The Enyu site owns the logo asset.
- **Agent owns:** Implementation and local verification within those boundaries; provider test-mode validation; documenting exact setup and safe rollback.
- **Requester owns:** Further DNS or DMARC enforcement/reporting changes, recipient consent and changes, production credential import confirmation, every campaign approval, any paid plan/retention change, and the exact live acceptance message.
- **Out of scope:** Transactional product mail; newsletter list acquisition or subscriber management; arbitrary HTML/text; attachments, CC/BCC, inbound email, automated campaigns, public cloud deployment, new public hostname/tunnel, paid upgrades, MX/SPF changes, or real sends during RD authoring.

## Current and provider state

The existing publication path already uses an Editor-only authenticated trigger, dedicated workflow principal, proxy-held credentials, narrow semantic adapter, durable checkpoints, exact head/content approval, immutable Context evidence, and production readback. Email must extend this path rather than grant Postmark or generic HTTP access to the Editor sandbox.

During RD authoring, a dedicated live Postmark server `Enyu` (server ID `20664982`) and persistent server token were created. The token was never revealed or copied and remains provider-side pending direct control-plane import. Sending domain `enyu.org` (domain ID `8073420`) was added. After requester approval, Postmark's generated DKIM TXT, DNS-only `pm-bounces` CNAME, and monitoring-only DMARC TXT (`p=none`, with no reporting address) were added in Cloudflare. Postmark reports DKIM and the custom return-path verified and recognizes DMARC. Existing MX and SPF records were not changed. Template `Enyu publication` (ID `46272458`, alias `enyu-publication-v1`) was created with fixed Enyu styling/logo, HTML and text parts, fields `title`, `summary`, and `article_url`, subject `New from Enyu: {{ title }}`, and provider unsubscribe placeholder. No email, webhook, mailbox, paid-plan, or retention change was made.

## Architecture and ownership

1. **Interactive Editor:** may invoke only `enyu-editor-publication-email` and its signed trigger tool with a canonical published article reference and a control-plane recipient-list reference. It may draft bounded text fields and request preview/approval. It cannot supply raw HTML, call Postmark, enumerate secrets, mutate recipients, approve, or send.
2. **Control plane:** authenticates the Editor trigger; holds the Postmark token, webhook credentials, recipient registry, recipient-fingerprint pepper, sender/domain configuration, and rate counters; issues no secret to a sandbox. It renders safe previews and exposes only semantic operations.
3. **Dedicated workflow identity:** `workflow-enyu-editor-publication-email` alone may read the canonical publication, reserve audit actions, render, request/consume approval, and invoke the adapter. A separate `workflow-enyu-postmark-events` identity processes callbacks. Grants name exact tools and actions; all unrelated calls are denied.
4. **Private adapter:** hard-codes server, Broadcast stream `broadcast`, template alias/version, `Enyu <hello@enyu.org>`, reply-to, allowed metadata keys, and one-recipient-per-provider-request. It validates HTTPS article URLs, lengths/Unicode, sender verification, consent/allowlist and suppression state, quotas, approval hash, and template drift. It has no generic request/URL/body escape hatch.
5. **Centaur Context:** adds a reusable `ExternalAction` Object subtype (provider, action kind, deterministic idempotency key, state, timestamps) plus immutable Object Events for reserve, preview, approval, attempt, accepted, delivered, suppressed, failed, and reconciled. Its authenticated API/client supports reserve, append event, and read status only; the workflow receives a narrow credential, never a DSN.
6. **Postmark:** retains the live server token, template deployment, sending-domain status, Broadcast suppression state, message activity/body under the provider's current retention, and delivery/bounce/complaint/subscription-change records. Postmark is not the canonical approval or idempotency ledger.

## Draft, preview, approval, and send contracts

`draft` resolves the already-published canonical article and produces only title, summary, and HTTPS article URL. `preview` fetches the pinned provider template, rejects drift, renders HTML/text with `POSTMARK_API_TEST` or equivalent non-delivery validation, sanitizes output, and records content hashes. Preview UI shows sender, reply-to, recipient count and masked recipient, subject, article URL, rendered HTML/text, template alias/ID/hash, and expiry; it never emails a preview.

`approve` is a human-authenticated event containing campaign/action ID, workflow run, canonical publication/head, normalized recipient-set hash and count, template alias/ID/hash, model hash, subject, article URL, rendered HTML/text hashes, sender/reply-to, Broadcast stream, approver, timestamp, and expiry. Any change or expiry invalidates approval. The requester-designated registry initially contains exactly one consented recipient; initial `max_recipients_per_run=1`, one active campaign at a time, and five sends per rolling 24 hours. Increasing any limit is a reviewed overlay policy change.

`send` revalidates publication readback, template/domain status, recipient registry and Postmark suppression immediately before dispatch. It sends one API request per recipient to avoid address disclosure and obtain per-recipient evidence. No CC/BCC or batch-recipient header is allowed. A deterministic custom message ID, opaque campaign tag, and non-personal metadata correlate provider and Context records.

## Idempotency, retries, and failure recovery

Reserve the external action and per-recipient intent before provider I/O. A completed intent is never resent. Retry 429 and proven pre-acceptance 5xx failures with bounded checkpointed backoff and provider rate headers. Because Postmark send calls have no native idempotency guarantee, a timeout/connection loss after request transmission enters `reconciliation_required`: query provider activity by deterministic message ID/tag before retrying. Retry only when provider absence is proven; otherwise stop for human reconciliation. Validation, auth, sender, suppression, stale-approval, and quota failures are permanent. Partial campaigns checkpoint each recipient and never replay accepted recipients.

## Webhooks, suppression, and readback

Execution may add only `/api/webhooks/postmark/enyu` on the existing authenticated Enyu tunnel; no new hostname or tunnel. Postmark does not sign callbacks, so the private ingress adapter requires proxy-held Basic Auth, trusted-tunnel source handling and provider IP allowlisting where feasible, strict method/content-type/schema/size limits, and rate limiting. It forwards a separately authenticated internal event; workflow sandboxes never see either credential. Configure delivery, bounce, spam-complaint, and subscription-change webhooks with message content excluded.

Deduplicate by Postmark trace ID plus event/message identity and raw-body hash. Acknowledge only after durable capture; support Postmark retry headers/schedule. Hard bounce, complaint, or unsubscribe creates immutable evidence and a control-plane suppression; never reactivate automatically. Preflight Postmark suppressions before every send. Webhook lag does not imply resend: scheduled reconciliation queries provider status. Disable open/click tracking by default. Store full bodies/addresses only provider-side for its current default retention; Context stores provider message ID, masked domain where useful, keyed recipient fingerprint, counts, hashes, status/reason class, timestamps, and approver—not raw bodies, full addresses, tokens, webhook credentials, or hidden reasoning.

## Provider setup and rollout

1. Treat the completed DKIM TXT, DNS-only `pm-bounces` CNAME, and monitoring-only DMARC TXT as provider prerequisites: verify they remain healthy before rollout and do not alter MX or current SPF. Any reporting address or enforcement above `p=none` is a separate requester decision.
2. Implement and deploy Context and overlay changes with the skill/grants disabled. Import the existing server token directly from provider UI into the established encrypted credential control plane without terminal/task/file exposure; reference it by credential ID. Configure server IP allowlisting only to stable adapter egress.
3. Deploy ingress and event workflow, then configure webhooks and prove authenticated delivery, dedupe, retry, malformed-body rejection, and content exclusion.
4. Run template validation/test-token paths, drift checks, suppression preflight, quota tests, and simulated provider failures. Enable only the single-recipient allowlist.
5. Obtain approval for exact acceptance recipient/content. Send once, verify Postmark accepted/delivered, inspect the mailbox, reconcile Context evidence, verify unsubscribe/suppression behavior without clicking unless separately authorized, and only then enable the Editor skill.

Rollback disables the Editor grant and trigger first, then adapter send permission/webhooks; in-flight workflows fail closed. Rotate/revoke the Postmark token after suspected exposure and reconcile outstanding intents before resuming. Template/domain drift or webhook outage automatically disables send, not audit capture.

## Checks

- [ ] Unit and contract tests cover schema/client auth, grants, bounded fields, HTML rejection, template drift, recipient masking/fingerprints, exact approval, expiry, quotas, suppressions, and audit immutability.
- [ ] Workflow tests cover draft/preview/approve/send, crash resume, duplicate triggers, ambiguous timeout reconciliation, partial progress, webhook dedupe/retry/order, and all fail-closed paths.
- [ ] Adapter integration tests use Postmark test mode/fixtures; assert Broadcast stream, one recipient, fixed sender/template, no CC/BCC/attachments/tracking, required unsubscribe, and no secret/body logging.
- [ ] Security tests prove Editor and unrelated principals cannot send/read recipients or credentials; webhook auth/schema/size/rate checks reject abuse; no DSN/raw provider credential reaches a sandbox.
- [ ] Live acceptance proves verified DKIM/return-path, one exact approved message, mailbox receipt, provider/Context readback, and no duplicate.
- [ ] Run all repository-root verification commands plus private-overlay tests; `git diff --check` passes.

## Acceptance Criteria

- [ ] Transactional and bulk/list-managed marketing remain unavailable; only explicitly enumerated, consented publication Broadcast sends are possible.
- [ ] Template-driven title/summary/article URL is the only body model; arbitrary HTML/text and attachments are impossible through the tool contract.
- [ ] Draft, preview, approve, and send are distinct durable stages, and only the dedicated workflow identity can execute provider I/O.
- [ ] Provider, control-plane, Context, site, and private-overlay ownership matches this RD; no credential, DSN, full recipient list, or unnecessary body enters Git, logs, Context evidence, or an agent sandbox.
- [ ] Exact approval, deterministic reservation, reconciliation-before-retry, suppression handling, authenticated webhook capture, immutable audit events, and production readback are demonstrated.
- [ ] No MX/SPF, paid-plan, retention, DMARC, real-send, new-ingress-hostname, or unrelated integration change occurs without its stated approval.

## Approval Boundary

This RD does not authorize repository implementation, deployment, spending, further DNS writes, credential transmission/import, recipient expansion, template-policy expansion, or any real email. The requester separately authorized the completed dedicated live Postmark server/token/template/domain and DKIM/return-path DNS setup, plus a narrowly authenticated Postmark callback route on the existing tunnel. Each campaign still requires exact human approval; secret import, webhook activation, and the live acceptance send require the action-specific confirmations above.

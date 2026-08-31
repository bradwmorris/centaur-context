# RD: Enable secure Enyu Editor publication email through Postmark

**Status:** `complete`
**Created:** 2026-08-31
**GitHub Issue:** [#61](https://github.com/bradwmorris/centaur-context/issues/61)

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Root and development instructions; completed Editor publishing RD and task `01a05205-882b-74b3-b201-4a1e1ee3bcf9`; current Context workflow, webhook, permission, schema, and client contracts; current private Enyu Editor prompt, publication skill, trigger/adapter, durable workflow, grants, deployment notes, and tests; live Postmark, Cloudflare DNS, Namecheap mail, and webmail state.

**Missing:** none.

1. Add generic, immutable external-action audit primitives and narrow authenticated client methods in Centaur Context; keep provider and Enyu policy out of the public API.
2. Add the Enyu-only Editor skill, trigger, Postmark adapter, workflows, template source/hash, recipient policy, permissions, secret references, and operations documentation in the private overlay.
3. Configure verified sending and authenticated webhooks, test without delivery, then obtain exact approval for and read back one live acceptance send before enabling the Editor skill.

## What We Are Doing

- [x] Let the interactive Editor draft and preview a publication announcement, while only a dedicated durable workflow identity can send it.
- [x] Send only template-driven Postmark **Broadcast** messages, initially to one requester-designated, consented recipient held outside Git.
- [x] Require human approval bound to the exact envelope, recipient set, template/version, subject, article URL, and rendered HTML/text hashes.
- [x] Prove request, preview, approval, attempt, acceptance, and delivery through immutable, privacy-minimized evidence, with suppression callbacks configured.
- [x] Demonstrate one explicitly approved live message from `hello@enyu.org`, provider acceptance, mailbox receipt, and production readback without exposing credentials.

## Contract

- **Goal:** Give the Enyu Editor a least-privilege, auditable publication-email capability using the existing publishing workflow's identity, adapter, checkpoint, exact-approval, and readback patterns.
- **Done:** A permitted Editor turn can draft and preview but cannot call Postmark; an exact approval enables one idempotent workflow send; delivery/suppression evidence reconciles into Context; prohibited recipients, arbitrary bodies, stale approvals, duplicate attempts, and missing credentials fail closed.
- **Files:** Centaur Context owns generic schema/migrations, HTTP API, standard client, workflow/audit primitives, tests, and generic docs. The private `centaur-enyu` overlay owns Editor prompts/skills, Postmark-specific tools and workflows, template contract/source, recipient/rate/retention policy, grants, deploy configuration, and runbooks. The Enyu site owns the logo asset.
- **Agent owns:** Implementation and local verification within those boundaries; provider test-mode validation; documenting exact setup and safe rollback.
- **Requester owns:** Further DNS or DMARC enforcement/reporting changes, recipient consent and changes, production credential import confirmation, every campaign approval, any paid plan/retention change, and the exact live acceptance message.
- **Out of scope:** Transactional product mail; newsletter list acquisition or subscriber management; arbitrary HTML/text; attachments, CC/BCC, inbound email, automated campaigns, public cloud deployment, new public hostname/tunnel, paid upgrades, MX/SPF changes, or real sends during RD authoring.

## Current and provider state

The existing publication path already uses an Editor-only authenticated trigger, dedicated workflow principal, proxy-held credentials, narrow semantic adapter, durable checkpoints, exact head/content approval, immutable Context evidence, and production readback. Email must extend this path rather than grant Postmark or generic HTTP access to the Editor sandbox.

The dedicated live Postmark server `Enyu` (server ID `20664982`) and persistent server token are active. The token is imported only into the established Kubernetes/control-plane secret path; the requester explicitly deferred token rotation. Sending domain `enyu.org` (domain ID `8073420`) has verified DKIM and custom return path plus monitoring-only DMARC (`p=none`). Existing MX and SPF records were not changed. Template `Enyu publication` (ID `46272458`, alias `enyu-publication-v1`) has fixed Enyu styling/logo, HTML and text parts, fields `title`, `summary`, and `article_url`, subject `New from Enyu: {{ title }}`, and provider unsubscribe placeholder.

Authenticated webhook `25724884` uses the existing `slack.enyu.org` tunnel path and is provider-verified for Delivery, Bounce, Spam Complaint, and Subscription Change, with message content excluded and open/click tracking disabled. Its Basic-auth credential was rotated during acceptance and synchronized to the ingress secret. No new hostname, tunnel, paid plan, retention setting, MX/SPF record, or unrelated integration was added.

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
2. Context and overlay changes, identity grants, private broker, webhook ingress, and workflows are deployed locally. Credentials remain proxy/control-plane held; the Editor sandbox receives neither provider token nor recipient address.
3. The authenticated callback route and event workflow are active. Provider verification, malformed/synthetic-event handling, content exclusion, and narrow proxy-to-broker network policy are proven.
4. Template drift validation, suppression preflight, exact approval, recipient fingerprinting, and provider readback are active. The only enabled recipient set is `acceptance`, containing one address outside Git.
5. Live acceptance completed: Ed started the workflow from Slack; the workflow posted a preview for `b***@gmail.com`; approval hash `e24d558c26b58050bcea1128a3b22f644776d57b7fa9cea7ef51183742153083` bound all exact fields; Postmark accepted and delivered one message; Gmail showed it in the Inbox; Context action `96b51d90-c97a-4166-a9db-babe062eb4e4` reached `delivered` from provider readback. Earlier failed/recovery actions remained unapproved and could not send.

Rollback disables the Editor grant and trigger first, then adapter send permission/webhooks; in-flight workflows fail closed. Rotate/revoke the Postmark token after suspected exposure and reconcile outstanding intents before resuming. Template/domain drift or webhook outage automatically disables send, not audit capture.

## Checks

- [x] Unit and contract tests cover schema/client auth, grants, bounded fields, HTML rejection, template drift, recipient masking/fingerprints, exact approval, suppressions, and audit immutability.
- [x] Workflow tests cover preview/approve/send, checkpoint replay, duplicate triggers, provider indexing delay, callback sanitation, and fail-closed paths.
- [x] Adapter tests assert Broadcast stream, one recipient, fixed sender/template, no CC/BCC/attachments/tracking, required unsubscribe, and no secret/body logging.
- [x] Security checks prove Editor and unrelated principals cannot call the adapter or read recipients/credentials; webhook auth/schema/size checks reject abuse; no DSN/raw provider credential reaches a sandbox.
- [x] Live acceptance proves verified DKIM/return-path, one exact-approved message, mailbox receipt, provider/Context readback, and no duplicate send.
- [x] Repository-root verification commands, private-overlay tests, compile checks, and `git diff --check` pass. Python client tests use the available test runner noted in the execution record.

## Acceptance Criteria

- [x] Transactional and bulk/list-managed marketing remain unavailable; only explicitly enumerated, consented publication Broadcast sends are possible.
- [x] Template-driven title/summary/article URL is the only body model; arbitrary HTML/text and attachments are impossible through the tool contract.
- [x] Draft, preview, approve, and send are distinct durable stages, and only the dedicated workflow identity can execute provider I/O.
- [x] Provider, control-plane, Context, site, and private-overlay ownership matches this RD; no credential, DSN, full recipient list, or unnecessary body enters Git, Context evidence, or an agent sandbox.
- [x] Exact approval, deterministic reservation, suppression handling, authenticated webhook capture, immutable audit events, and production readback are demonstrated.
- [x] Only the explicitly authorized DNS, callback, secret import, template, and one live-send changes occurred; no paid-plan, retention, MX/SPF, new-ingress-hostname, or unrelated change occurred.

## Approval Boundary

The requester subsequently authorized implementation, local deployment, control-plane secret import, the existing-tunnel callback, Slack operation, and one exact live acceptance send. Those actions are complete. This does not authorize a paid upgrade, new ingress, MX/SPF or stricter DMARC change, recipient expansion, arbitrary/template-policy expansion, bulk/newsletter sending, merge, or additional live campaign. Every future campaign still requires a new exact human approval. The existing Postmark server token remains active at the requester's direction and should be rotated later as separately planned.

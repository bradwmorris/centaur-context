# RD: Deploy and Dogfood Centaur Context for Enyu with Named Slack Agents

**Status:** `in_progress`
**Created:** 2026-08-29
**GitHub Issue:** `#15`
**Prerequisite Issues:** Centaur `#3`, `#4`

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Current Centaur Slackbot v2 server, session, persona,
permission, overlay, Helm, ingress, state/recovery, and NetworkPolicy contracts;
the ACME overlay guidance; the existing private overlay; and Centaur Context's
ingestion, Context Builder, Curator, identity, client, deployment, and database
contracts through migration 9.

**Missing:** No planning decision. Execution is gated by the requester-owned
resources and approvals listed below. The two reusable product gaps in phases 1
and 2 must be implemented through their own issue, branch, tests, and PR before
the Enyu overlay is deployed.

### Fixed execution decisions

- Use one pinned Centaur control plane, two Slack apps (`editor` and
  `researcher`), one private `centaur-enyu` overlay, and one fresh shared Centaur
  Context installation. Do not fork or rebrand either reusable product.
- Start in a local/private Kubernetes environment. Prove the complete path with
  signed Slack fixtures before asking for a live Slack callback. Create
  `centaur-enyu-infra` only for a later durable GitOps rollout.
- Give each bot a dedicated test channel and allow both bots in one shared test
  channel. Use synthetic Enyu data only. Access the Context UI by port-forward.
- Use separate Context PostgreSQL storage, database, role, tokens, and backups;
  do not reuse Centaur's application, Console, or `ai_v2` database.
- Keep agents read-only in Context. Only the authenticated interaction sink and
  Curator may write.

### Ordered phases and gates

| Phase | Deliverable | Gate to continue |
| --- | --- | --- |
| 0. Pin and baseline | Record passing Centaur and Centaur Context commits, verify the current singleton path, and record an Enyu gap ledger. | Both products pass their targeted checks. |
| 1. Repair the context handshake | In reusable product work, ingest/upsert the current Slack snapshot before context retrieval, parse and persist the returned opaque `chat_object_id`, pass it to Context Builder, and repeat ingestion after the response. Keep failure non-blocking and idempotent. | A first turn, retry, and restart resolve the same canonical Chat; a later turn gets bounded context. |
| 2. Add generic multi-Slack support | Add named Slackbot instances while preserving the singleton values contract. Isolate app secrets, webhook Services/routes, bot identity, persona, state/recovery prefix, session namespace, permission principal, metrics, and policies. | Two fixture-driven bots run concurrently; duplicate events, restarts, and one-bot failure do not collide. |
| 3. Build `centaur-enyu` | Scaffold Editor and Researcher personas, skills, prompt policy, configuration, test fixtures, role/grant manifest, and the pinned standard Context client. | Static tests prove correct persona selection, least privilege, and no duplicated product code or secrets. |
| 4. Deploy privately | Deploy pinned products plus the overlay, a fresh Context database, approved synthetic surfaces, Curator configuration, and private UI access. | Health, migrations, authenticated ingestion/context, denial tests, Curator, and UI inspection pass. |
| 5. Prove the story locally | Replay signed Slack fixtures through both webhook routes and verify the two directional handoffs below. | All acceptance evidence is captured without a live Slack mutation. |
| 6. Live Slack trial | After approval, create/configure the two apps, expose narrowly scoped HTTPS callback routes, apply credentials/grants, and repeat the smoke test. | Both real mentions and both cross-agent retrievals succeed. |
| 7. Close or harden | Classify gaps and either stop the POC or, with separate approval, create `centaur-enyu-infra` and a durable rollout plan. | Revisions, evidence, rollback, and remaining risks are recorded. |

Phase 1 uses two keys deliberately: Centaur's session key includes the Slackbot
instance so the bots cannot collide, while the Context thread key remains the
canonical `slack:<workspace>:<channel>:<thread>` identity so both bots can share
one Chat when they participate in the same Slack thread.

## What We Are Doing

- [ ] Prove unchanged reusable Centaur and Centaur Context products can support
  an organization overlay with two named agents.
- [ ] Run distinct Editor and Researcher Slack apps through one Centaur control
  plane with isolated personas, sessions, state, tools, credentials, and
  failure domains.
- [ ] Give both agents one fresh, shared Centaur Context through authenticated
  HTTP APIs only.
- [ ] Demonstrate durable knowledge transfer in both directions and classify
  every discovered gap.

## Contract

- **Goal:** Prove that Enyu can operate two specialized Slack agents with
  isolated execution and shared durable context without bespoke product forks.
- **Done:** Fixture-driven and approved live Slack tests show Editor and
  Researcher selecting the correct persona and grants; each produces a curated
  result retrieved by the other; Context shows distinct agent Users, Chats,
  Objects, provenance, and Curator Runs; and every product change is merged or
  recorded as an unresolved gap.
- **Files:** This RD; separately tracked generic changes in
  `/Users/bradleymorris/Desktop/dev/centaur` and, only if required by the
  handshake, `/Users/bradleymorris/Desktop/dev/centaur-context`; a new private
  `/Users/bradleymorris/Desktop/dev/centaur-enyu`; and an optional later
  `/Users/bradleymorris/Desktop/dev/centaur-enyu-infra`. The existing
  `centaur-overlay` is reference material only.
- **Agent owns:** Authorized local product work, overlay scaffolding,
  configuration, fixtures, tests, evidence, gap classification, and local
  verification.
- **Requester owns:** Repository creation/visibility, Slack apps and workspace
  settings, channel membership, credentials, Console grants, model/provider
  spend, live callback ingress, deployment, publication, demos, and upstream
  approval.
- **Out of scope:** Product forks or rebrands, one control plane per bot,
  Enyu-specific product code, imported real data, agent database access,
  arbitrary Context writes, permanent public UI exposure, and copying AGI Post
  code, credentials, data, schema, or private business rules.

## Required Product Contracts

### Context handshake

- The pre-turn interaction sink returns `chat_object_id`; Slackbot validates and
  stores it in thread state, supplies it on the Context Builder request, and
  safely reacquires it after restart. A post-turn snapshot adds the response.
- Pre/post snapshots use stable idempotency and never close an interaction merely
  because a context lookup ran. Context failure skips shared context but does
  not prevent the Slack response.
- Context authenticates the bearer token, principal, canonical thread key, and
  approved workspace/channel. Sandboxes never receive ingestion credentials.

### Named Slackbot instances

- Introduce a list-shaped instance contract with a required stable instance ID;
  translate the existing singleton values into one default instance for
  compatibility.
- Each instance selects its own token/signing-secret keys, bot user, persona,
  state prefix, Service, callback route, observability label, and permission
  namespace. API/session metadata records the instance and persona.
- Separate Centaur session identity from canonical Context conversation
  identity. Include workspace and instance in Centaur session/state/recovery
  keys, but do not fragment one Slack conversation into different Context
  Chats merely because two bots participated.
- Prove per-instance signature rejection, webhook/event idempotency, render
  recovery, Console visibility, tool denial, and NetworkPolicy routing.

### Enyu overlay and permissions

- Package `personas/editor` and `personas/researcher` using Centaur's discovered
  persona format and `PROMPT.md`; keep repeatable task procedures in skills and
  durable multi-step automation in workflows only when needed.
- Define three least-privilege roles: shared read-only Context, Editor-specific
  capabilities, and Researcher-specific capabilities. Record the exact tool and
  credential matrix before live grants; neither bot receives ingestion,
  Curator, database, cluster, or the other bot's private credentials.
- Editor performs bounded, provenance-safe revision. Researcher produces cited
  research with uncertainty and explicit access gaps. Prompts do not substitute
  for enforced grants.

## Acceptance Scenarios

1. **Editor to Researcher:** Editor records a synthetic brief containing a
   unique nonce, finishes the interaction, and Curator creates durable Objects.
   Researcher answers a later question from its own channel using those Objects.
2. **Researcher to Editor:** Researcher records a cited synthetic fact packet;
   Editor later applies it to a revision and identifies the supporting Context
   Object.
3. **Isolation:** The wrong bot ignores an unaddressed event; each bot resolves
   its own persona and principal; cross-role tool attempts are denied; replayed
   webhooks are idempotent; and stopping one instance does not stall the other.
4. **Security and recovery:** An unapproved channel is rejected, injected
   instructions inside Context remain reference data, Context outage degrades
   safely, restart reacquires the Chat, and no secret or DSN appears in a pod,
   log, fixture, event, or repository.

## Checks

- [ ] Each reusable product change has its own issue/branch/PR, focused tests,
  and passing repository checks; singleton Centaur remains compatible.
- [ ] The overlay pins reviewed product revisions and contains no product copy,
  Enyu credentials, private IDs, database DSN, or unrestricted write path.
- [ ] Tests cover handshake, two instances, personas, permissions, signature
  rejection, idempotency, collision avoidance, recovery, and independent
  failure.
- [ ] A fresh Context database proves first-turn and later-turn Chat resolution,
  distinct agent Users, shared authorized Objects, Curator provenance, and UI
  visibility.
- [ ] Both directional acceptance scenarios pass first with signed fixtures and
  then, after approval, in live Slack.
- [ ] Relevant checks in every changed repository and `git diff --check` pass.

## Approval Boundary

Planning and local fixture work do not authorize repository creation or
publication, Slack app/workspace/channel changes, credentials or live grants,
model calls or spend, deployment, database creation, data import, HTTPS callback
ingress, demos, upstream proposals, external writes, or deletion. The live
Slack phase requires explicit approval for two apps, their exact surfaces and
credentials, narrowly scoped callback ingress, model use, deployment, and
rollback. Permanent public UI ingress is not part of this POC.

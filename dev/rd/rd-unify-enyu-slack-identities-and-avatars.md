# RD: Unify Enyu Slack Identities and Avatars

**Status:** `scoped`
**Created:** 2026-08-30
**Dependency:** `rd-fork-centaur-context-multi-agent-poc.md` phases 1-2 provide
the shared Context handshake and generic multi-Slack instances. This RD
supersedes that RD's dedicated-per-agent MVP channel expectation and owns only
the Enyu names, identity media, one-channel acceptance, and UI presentation.

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Root and development instructions; the named Slack agents
RD; Context migrations 3, 4, and 7, Slack ingestion/upsert, visual queries/API,
static serving, React record/detail surfaces and tests; Centaur Slackbot v2
interaction-sink/profile and multi-instance/chart contracts; and the private
Enyu overlay manifests, values, fixtures, deployment, tests, and supplied image
dimensions/hashes.

**Missing:** none. Live Slack mutations remain approval-gated.

1. In the private Enyu overlay, use one approved shared MVP channel for both
   instances. Rename app/bot presentation to `Ed (enyu editor)` and
   `Rez (enyu researcher)` and reject trimmed/encoded suffixes including
   `&#x20;`; stable instance/persona/principal IDs remain `editor`/`researcher`.
   Add no new channel abstraction: existing workspace/channel/thread keys keep
   the later multi-channel path clean.
2. Add canonical overlay assets at `assets/identities/ed.png` (source
   `/Users/bradleymorris/Desktop/dev/zeu/ui/avatars/rah.png`, SHA-256
   `7df3d1796c639345d71ecf5ff840990d6f57802afd99fa9eb8cab7e007af23a0`),
   `rez.png` (source `/Users/bradleymorris/Desktop/dev/zeu/ui/avatars/zeu.png`,
   `3dd469727b76ab5248d17d32dfe1ef297f62d0bae9b95e91c5c85e131df019d2`),
   and `brad.jpg` (source
   `/Users/bradleymorris/Desktop/dev/zeu/ui/avatars/brad.jpg`,
   `d8e281b2b36d7de4cfaea79868c36ccd26795b87757ce4a14a5a6e1218e88b0d`).
   Preserve original bytes and record hashes/licence/provenance; deploy them
   read-only beside the Context web assets. Slack may host a copy, but is not
   the canonical store.
3. Extend reusable Centaur Slackbot v2 to resolve/cache bounded `users.info`
   profile data for snapshot participants and send compatible optional display
   name/provider-avatar fields. Refresh on a documented TTL and on stale-cache
   failure; Slack/API failure stays non-blocking. Enyu configuration overrides
   Brad, Ed, and Rez with the canonical overlay asset references: Brad's Slack
   user ID/name/profile metadata is still imported, but `brad.jpg`, not Slack's
   current avatar URL, is his image source of truth.
4. Extend Context's identity contract compatibly: retain nullable legacy
   `avatar_url`, add an optional safe same-origin asset reference and provenance/
   refresh metadata as required by migration, ingestion, API, visual query, and
   standard-client types/tests. Upserts update Slack names and freshness without
   changing canonical User IDs; the Enyu asset override wins over Slack URLs.
   Serve only allowlisted mounted files with correct MIME, immutable hash-based
   cache headers, no directory traversal, uploads, remote fetching, or sandbox
   access. Sandboxes receive authenticated HTTP context only, never a DSN.
5. Render the resolved avatar/name through the reusable avatar component on all
   actual identity surfaces: record/search-result lists; User identity detail;
   generic Object, Task owner/users, and Note users; chat and Curator
   transcripts; relationship endpoints; Curator chat/change views; and object
   attribution in activity. Do not label raw event principals as Brad/Ed/Rez
   unless the API resolves them to the canonical User. No navigation/avatar or
   standalone context-card surface currently exists.
6. Verify locally with signed fixtures in one channel, then—only after explicit
   approval—update both Slack app and bot display names/avatars and optionally
   Brad's Slack profile avatar from `brad.jpg`; repeat live one-channel checks.

## What We Are Doing

- [ ] One shared Slack channel shows Ed and Rez with the exact clean names and
  chosen images; no second channel is required for MVP.
- [ ] Context has stable Brad, Ed, and Rez Users whose canonical images render
  on every applicable UI identity surface, survive Slack URL expiry/change,
  and refresh names/profile metadata without duplicating Users.
- [ ] Missing, unreadable, private, wrong-type, or broken media falls back to
  deterministic initials/colour with accessible identity text and no broken
  image, secret, token, or private Slack URL exposure.

## Contract

- **Goal:** Present one coherent, durable Enyu identity system across the
  shared Slack MVP channel and Centaur Context UI.
- **Done:** Fixture and approved-live evidence proves exact names, deterministic
  asset mapping, one shared Chat, stable three-User identity, refresh behavior,
  every enumerated UI surface, fallback, caching, and security constraints.
- **Files:** Context: `migrations/`, `src/ingest.rs`, `src/db.rs`, `src/api.rs`,
  `web/src/{types,RecordVisuals,App}*`, and compatible
  `tools/centaur_context/` contracts if exposed. Centaur:
  `services/slackbotv2/src/{interaction-sink,index,types}.ts`, focused tests,
  and generic chart values only if reusable configuration is needed. Enyu:
  `assets/identities/`, `slack/*-app-manifest.yaml`, `personas/`,
  `deploy/centaur-values.yaml`, `deploy/context-enyu.yaml`, fixtures/tests/docs.
  No Enyu policy/assets enter reusable products.
- **Agent owns:** Approved local code/data migrations, asset copy during later
  execution, fixture tests, UI verification, and compatibility documentation.
- **Requester owns:** Slack workspace/app/profile changes, channel membership,
  credentials, live ingress/deployment, and approval to use the supplied images.
- **Out of scope:** Multiple required MVP channels, changing stable IDs/persona
  policy, public ingress, arbitrary avatar proxying, external uploads, DB access
  from agents, or Enyu-specific reusable-product logic.

## Checks

- [ ] Context migration upgrade preserves existing identities/API clients;
  ingestion replay, rename/refresh, asset priority, traversal/wrong-MIME/private-
  URL rejection, caching, and fallback tests pass.
- [ ] UI tests cover all enumerated surfaces for Brad, Ed, and Rez plus broken
  media; Centaur sink tests cover profile TTL/failure and one-channel two-bot
  snapshots; Enyu tests assert exact names, hashes, mapping, and no `&#x20;`.
- [ ] Context root verification, Slackbot v2 type/tests, Enyu overlay tests, and
  `git diff --check` pass in each changed repository.

## Approval Boundary

Local implementation and signed fixtures do not authorize Slack app/bot/profile
renames, avatar uploads (including Brad's), workspace/channel changes,
credentials, live callbacks, deployment, publication, spending, or deletion.
Each external Slack mutation and live rollout requires explicit requester
approval; credentials stay out of repositories, logs, fixtures, Context URLs,
and sandboxes.

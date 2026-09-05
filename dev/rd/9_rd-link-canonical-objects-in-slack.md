# 9 — RD: Link Canonical Objects in Slack

**Status:** `scoped`
**Created:** 2026-09-05

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** canonical Object API/search/context responses in
`src/search.rs`; the durable `/objects/{id}` web route and shared Object-ID UI
in `web/src/{routing,ObjectIdentity}.tsx`; the standard
`tools/centaur_context` read/search client; Slack integration documentation;
Centaur Slackbot v2 context fetching, preamble construction, Markdown streaming,
response metadata, configuration, and tests; the Enyu Slack deployment,
personas, workflow completion messages, and current loopback Context UI.

**Missing:** none. A browser-reachable UI origin is deployment configuration,
not a reason to invent one or add public ingress.

1. Add one validated, deployment-configured human UI base URL and expose a
   canonical `ui_url` for every Object reference returned by agent-facing
   context, search, and read responses, including the other Object on a
   Connection.
2. Teach reusable Centaur Slackbot v2 to give the model a concise, ready-to-use
   Object-link convention and require linked labels whenever its answer refers
   to a supplied Context Object; expose the same convention to Objects found
   later through the standard client.
3. Configure the private Enyu deployment and use the same formatter in
   deterministic workflow replies that name Objects; verify API compatibility,
   Slack rendering, bot behavior, navigation, and failure-safe operation.

## What We Are Doing

- [ ] Render each user-visible reference to a known canonical Object in a Slack
  bot reply as a compact clickable label, for example
  `[RSI Simulator · Source · 22085…](https://context.example/objects/<uuid>)`,
  rather than a bare title or UUID.
- [ ] Make the label open that exact Object in the existing Context UI while
  keeping prose readable and avoiding a detached link footer.
- [ ] Encourage this behavior in the reusable system/context guidance and the
  Enyu personas, and cover both automatically injected Objects and Objects
  returned by later tool calls.
- [ ] Preserve useful answers when Context or its UI URL is unavailable; never
  fabricate a URL or suppress the answer.

## Contract

- **Goal:** Make Slack references to canonical Context Objects compact,
  recognizable, and directly navigable to their human UI detail.
- **Done:** In representative Ed and Rez replies, every mentioned Object that
  came from Context is rendered as a bounded linked label whose URL opens the
  matching `/objects/{uuid}` detail; read/search tool results provide the same
  canonical URL; non-Object links and ordinary prose are unchanged.
- **Files:** Centaur Context configuration, agent-facing response types,
  serialization tests, standard client/docs, and this RD; adjacent Centaur
  Slackbot v2 shared-context/configuration/tests; private Enyu personas,
  workflow notification helpers, deployment values, and focused tests.
- **Agent owns:** the URL and label contract, backwards-compatible API/client
  changes, reusable Slack guidance, deterministic workflow formatting, local
  verification, and deployment-ready configuration changes.
- **Requester owns:** the browser-reachable UI origin, approval for any new
  ingress or live deployment, and final visual acceptance in Slack.
- **Out of scope:** changing Object IDs or UI routes; linking arbitrary nouns;
  fuzzy title matching or rewriting completed model text; a Slack Block Kit
  redesign; link unfurls; authentication redesign; new public ingress; and
  exposing API credentials in links.

## Reference Contract

- Context owns Object identity and route construction. Add a single setting
  such as `CENTAUR_CONTEXT_UI_BASE_URL`, validate it as an absolute HTTP(S)
  origin without credentials, query, or fragment, normalize its trailing slash,
  and build URLs from the canonical UUID. Never trust the request `Host` header
  and never include a bearer token.
- Agent-facing Object shapes expose `ui_url`; Connection context also exposes
  the related Object ID and URL. Keep the field optional during compatibility
  rollout. The configured Enyu deployment must supply it before acceptance.
- The presentation label is `bounded title · kind · short ID`: collapse
  whitespace, cap the title, use a consistent human-readable kind, and shorten
  the UUID visually while the full UUID remains in the URL. Escape label text
  at the Slack/Markdown boundary. Reuse one formatter for deterministic
  non-model Slack messages.
- The Centaur context preamble supplies the exact Markdown form and states:
  whenever the answer refers to a Context Object with a `ui_url`, use its
  linked label instead of a bare title or ID. Do not add links for inferred,
  ambiguous, missing, or untrusted IDs.
- Do not post-process streamed prose with title/UUID regexes. Matching is
  ambiguous, breaks streaming, and can link ordinary text to the wrong Object.
  Behavioral evals measure instruction adherence; deterministic producers use
  the formatter and are contractually exact.

## Checks

- [ ] Rust/API tests cover valid normalization; rejected credentials,
  query/fragment, and unsupported schemes; percent-safe UUID paths; omitted
  configuration; and `ui_url` on context, search, read, and related Objects.
- [ ] Standard-client tests preserve and display the new fields without gaining
  write access or depending on the human listener.
- [ ] Centaur tests prove bounded/escaped labels, full-UUID destinations,
  preamble guidance, missing-URL fallback, response-length limits, and unchanged
  Slack Markdown streaming.
- [ ] Enyu tests cover persona guidance, deployment configuration, and every
  deterministic workflow receipt that names an Object.
- [ ] Golden Slack scenarios cover injected and tool-discovered Objects, long
  and punctuation-heavy titles, repeated references, multiple Objects with
  similar titles, and successful navigation to the matching UI detail.
- [ ] Required checks for each changed repository and `git diff --check` pass.

## Approval Boundary

This RD authorizes planning only. Execution may prepare local Context, Centaur,
and private Enyu changes and tests. Creating public ingress, changing UI
authentication, deploying, sending live Slack messages, hosted writes, or
publishing requires explicit requester approval.

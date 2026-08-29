# RD: Clean Type, Source, User, and Connection Visuals

**Status:** `complete`
**Created:** 2026-08-28
**Completed:** 2026-08-29
**GitHub Issue:** #4

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** the five-type ontology and controlled Connection vocabulary,
User/external-identity migrations, Slack Chat and Chat Message migration,
provenance validation, current list/detail/relationship UI, API response types,
and the canonical Object ID navigation RD.

**Missing:** none

1. Define evidence-backed user-attribution and avatar data, expose it through
   typed APIs, and resolve Slack participants idempotently to one User Object
   each without forcing every Object into a single-owner model.
2. Build shared type, source, Object identity, user-avatar, and connection UI
   components and apply them across all six primary views and detail surfaces.
3. Verify attribution, Slack provenance, identity deduplication, fallbacks,
   connection navigation, accessibility, and responsive layouts; run repository
   checks and record completion evidence.

## What We Are Doing

- [x] Replace weak plain-text type/status markers with clean, consistent,
  visually distinguishable labels for every supported Object and Task state.
- [x] Display a Slack icon wherever a record has evidence-backed Slack origin,
  including beside its Object ID on subtype and supporting-row surfaces.
- [x] Give every canonical human and agent User a visible avatar and show the
  applicable attributable User or Users beside the canonical Object ID.
- [x] Make Connections visually clear, directional, explained, and navigable to
  both canonical endpoint Objects.

## Contract

- **Goal:** Let a user understand what a record is, where it came from, who it is
  attributed to, and how it connects to the graph at a glance.
- **Done:** All six primary views and their details use the same labelled visual
  language; Slack origin and canonical users are resolved from stored data, and
  Connections open the correct endpoint Objects.
- **Files:** A narrow migration for User avatar data if required; existing Slack
  identity, participant, ownership, and supporting-message provenance; Rust
  API/query types; `web/src/` shared components and views; assets only if
  licensing permits; targeted tests; this RD.
- **Agent owns:** Data contract, migration, API/UI implementation, accessible
  iconography and fallbacks, Slack-source resolution, tests, and local proof.
- **Requester owns:** Approval for downloading/storing external avatar binaries,
  changing Slack scopes, adding another provider, or changing the ontology.
- **Out of scope:** Social profiles, avatar editing/cropping, presence, a general
  icon pack, organization-specific branding, new Connection kinds, and assuming
  every installation or future record originates in Slack.

## Identity And Visual Rules

- There is one canonical User Object plus one `users` subtype row per real human
  or agent identity. Provider identities such as Slack IDs attach through
  `external_identities`; repeated Slack interactions must reuse that User.
- Store an optional provider avatar reference and display it when safe and
  available. Every User must still have a deterministic local fallback avatar
  derived from its canonical User Object ID, so broken, missing, or inaccessible
  Slack images never produce an empty avatar.
- User attribution is evidence-backed and may be singular, plural, or absent.
  Resolve canonical User Objects from typed stored relationships rather than a
  display name, provider-specific ID, or a newly invented universal owner field.
  A User Object represents itself and does not need a meaningless self-link.
- For Slack-derived records, display the human or agent Users directly supported
  by the record's source evidence—not merely the technical ingestor or curator.
  Chat attribution comes from its initiating/participating Users and `involves`
  Connections; Chat Message attribution comes from `sender_user_object_id`; Task
  attribution includes its canonical owner when present and may include the
  author of a supporting commitment; curated Object attribution is resolved from
  its exact `supporting_message_ids` and their sender Users.
- Preserve `created_by_*`, `updated_by_*`, Curator Run, and other technical audit
  fields separately. They may provide a labelled fallback when no participant or
  source author exists, but the UI must not misrepresent a system process as the
  human responsible for the content or invent a human identity.
- A Chat may involve multiple Users, a Task may have one owner, and any Object
  may have additional explained User Connections. The UI should present the
  relevant role—such as participant, sender, owner, source author, or technical
  creator—instead of collapsing these different meanings into one generic user.
- Slack badges are evidence-driven. Show the Slack icon when the record is a
  Slack Chat/Message or has stored provenance/`derived_from` evidence linking it
  to a Slack Chat. Do not stamp Slack onto manually created, migrated, or future
  provider records merely because Slack is the current primary surface.
- Use the official Slack mark in accordance with its licensing requirements; if
  that asset cannot be included, use an accessible labelled source badge rather
  than an imitation logo.
- Type labels use readable names, icons, colour with sufficient contrast, and a
  non-colour cue. They cover `task`, `chat`, `user`, `entity`, and `memory`; Task
  status remains a separate label rather than replacing its Object type.
- Connection rows show direction (`source → target`), controlled type, both
  endpoint types/titles/Object IDs, the required plain-language description,
  source/user context when available, and links to both canonical Objects.

## Checks

- [x] Database/API tests prove Slack identity deduplication, one canonical User
  per provider identity, correct participant/sender/owner/source-author
  resolution, and deterministic avatar and technical-actor fallbacks.
- [x] UI tests cover every type/status label, Slack and non-Slack records, human
  and agent avatars, missing/broken avatars, long names/IDs, and all Connection
  directions.
- [x] Accessibility checks prove icons have text equivalents, colour is not the
  only signal, avatars have useful accessible names, and controls are keyboard
  navigable.
- [x] All six primary list/detail views and representative schema rows are
  visually verified at desktop and narrow widths.
- [x] The repository verification suite and `git diff --check` pass.

## Verification Results

- Rust formatting, Clippy, unit/API/evaluation tests, web type-check/build, 24 UI
  tests, Python client tests, Python compilation, package contract checks, and
  `git diff --check` pass.
- The database contract passed against a disposable PostgreSQL database migrated
  through schema version 7, covering Slack identity reuse, participants, avatar
  references, and source-author attribution.
- All six primary views, Curator surfaces, Task states, Connection directions,
  attribution roles, broken-avatar fallback, and narrow layouts were inspected
  in the local review UI without console errors or horizontal overflow.

## Approval Boundary

This RD permits local schema, ingestion, API, and UI changes inside Centaur OS.
It does not authorize new Slack permissions, calls to Slack during development,
bulk avatar downloading, external integrations, public ingress, deployment,
hosted data mutation, credential changes, or deletion.

# RD: Fork and Dogfood a Personal Centaur OS with Named Slack Agents

**Status:** `backlog`
**Created:** 2026-08-29

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Centaur OS ownership, Slack/context, agent-client, and task
contracts; the existing Centaur checkout and deployment; Centaur overlay,
persona, workflow, Slack transport, session, and permission implementations;
and the existing private overlay.

**Missing:** Final repository names/visibility and live Slack credentials and
channels are requester-owned. Proposed local names allow scaffolding.

1. Keep and pin `/Users/bradleymorris/Desktop/dev/centaur` as the existing
   control plane; do not create another Centaur fork. Make only a separately
   tracked generic compatibility change there if multi-bot binding requires it.
2. Fork `/Users/bradleymorris/Desktop/dev/centaur-os` into a sibling personal
   POC OS, proposed as `centaur-os-brad-poc`, preserving the source repository as
   an upstream remote and customizing the fork through actual POC use.
3. Create one new POC-specific overlay, proposed as
   `centaur-os-brad-poc-overlay`, containing the Editor and Researcher verticals
   and connecting Centaur to the personal OS fork through its HTTP API.
4. Prove the end-to-end Slack, Centaur, personal OS, and feedback loop; document
   which fork changes remain personal and which should return upstream.

## What We Are Doing

- [ ] Use the existing Centaur core as the shared agent control plane.
- [ ] Create and actively customize a Brad-specific fork of Centaur OS for the
  POC instead of treating the source Centaur OS as immutable infrastructure.
- [ ] Create a unique overlay for that fork with distinct `@Editor` and
  `@Researcher` Slack agents, instructions, tasks, workflows, tools, and grants.
- [ ] Turn dogfooding lessons into explicit upstream candidates without leaking
  personal behavior or copying private AGI Post implementation.

## Contract

- **Goal:** Build a personal, forked Centaur OS POC operated by multiple named
  Slack agents through the existing Centaur core and a dedicated overlay.
- **Done:** The personal OS fork has clear ancestry and POC customizations;
  `@Editor` and `@Researcher` run from its unique overlay on the existing
  Centaur control plane; both use the forked OS for context and ingestion;
  representative workflows succeed; and every divergence is classified.
- **Files:** This RD; the existing `/Users/bradleymorris/Desktop/dev/centaur`
  checkout only for verified generic compatibility work; a new sibling personal
  OS fork proposed as `/Users/bradleymorris/Desktop/dev/centaur-os-brad-poc`;
  and a new sibling overlay proposed as
  `/Users/bradleymorris/Desktop/dev/centaur-os-brad-poc-overlay`. The existing
  `/Users/bradleymorris/Desktop/dev/centaur-overlay` is reference material, not
  the new POC overlay.
- **Agent owns:** Local scaffolding when execution is assigned, POC
  customizations, clean-room verticals, tests, divergence records,
  documentation, and local verification.
- **Requester owns:** Repository creation/visibility, Slack app creation and
  credentials, live grants, model/provider spend, deployment, publication, and
  approval of any public demo or upstream proposal.
- **Out of scope:** Another Centaur fork, separate control planes per bot,
  Brad-specific implementation in the source Centaur OS checkout, reusing the
  existing overlay, agent database access, public ingress, and copying AGI Post
  implementation, data, credentials, or publication logic.

## Requirements

### Repository topology and ownership

- Record exact commits for Centaur, source Centaur OS, the personal fork, and
  overlay. Give the personal fork its own origin and source Centaur OS upstream.
- Keep Centaur responsibilities unchanged: Slack transport, sessions,
  sandboxes, workflows, model execution, and permission enforcement.
- The personal OS fork owns its isolated logical database, canonical Objects,
  context/curation behavior, API, UI, migrations, and standard agent client. It
  must never query Centaur, `ai_v2`, Console, or AGI Post databases.
- Agents use only the authenticated HTTP API and the standard client shipped by
  the personal OS fork. Do not duplicate that client in the overlay or expose a
  database DSN.

### Personal OS customization and feedback

- Begin from a pinned, passing Centaur OS commit. Permit POC-driven schema,
  ontology, API, context, curation, UI, and operational changes in the personal
  fork when they support Editor/Researcher use.
- Maintain a divergence ledger. For every fork change, record its need,
  compatibility impact, tests, and one classification:
  `personal`, `overlay`, `upstream_candidate`, or `discard`.
- Move instructions, workflow/tool policy, retention, and organization logic to
  the overlay. Use a separate RD/issue/PR for reusable source-product
  improvements; never sync personal changes upstream wholesale.

### Unique overlay and named agents

- Define `editor` and `researcher` persona packages with `PROMPT.md`. Use
  `SKILL.md` for task contracts, Python workflows for durable multi-step work,
  packaged CLI tools for capabilities, and `AGENTS.md` only for contributor
  guidance.
- Editor handles bounded editing, critique, restructuring, and provenance-safe
  revision. Researcher handles URL, supplied-text, and bounded-topic research
  with authoritative evidence, direct citations, uncertainty, and explicit
  access gaps.
- Give each Slack app its own token/signing-secret reference, stable bot
  identity, persona binding, state/session namespace, and permission principal.
  Enforce different tools and credentials through roles, not prompts.
- First use the existing Centaur contract. If it cannot safely host both bots,
  add narrow generic multi-instance/persona binding to that existing fork with
  singleton compatibility, collision-free sessions, provider metadata, and
  isolated recovery.
- Point both bots' `contextBuilder`, `interactionSink`, and `centaur-os` client
  configuration at the personal OS fork. Preserve distinct canonical agent
  Users while allowing shared context across their conversations.

## Checks

- [ ] Git/remotes prove the intended core, source, personal-fork, and overlay
  topology; pinned commits and the divergence ledger are documented.
- [ ] Source Centaur OS contains no Brad-specific implementation changes from
  this execution, and the unique overlay duplicates neither OS domain logic nor
  the standard client.
- [ ] Tests cover both personas, tool denial, prompt injection, Slack identity
  isolation, session collision, context/ingestion, and fork-specific OS changes.
- [ ] Slack smoke proof shows independent `@Editor` and `@Researcher` mentions,
  correct behavior/grants, durable results, distinct agent Users, and later
  shared-context retrieval from the personal OS fork.
- [ ] Static hygiene finds no credentials, private IDs, database DSNs, AGI Post
  implementation, Supabase project IDs, or Modal commands.
- [ ] Relevant checks in every changed repository and `git diff --check` pass.

## Approval Boundary

This RD remains planning-only until execution is separately assigned. Local
scaffolding would not authorize creating or publishing repositories, creating
Slack apps, changing a workspace, granting credentials, spending, deploying,
public ingress, upstream proposals, a live demo, external writes, or deletion.
Each requires explicit requester approval.

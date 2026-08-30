# RD: Build the Enyu Overlay and Source Ingestion Workflow

**Status:** `in_progress`
**Created:** 2026-08-30
**Context issue:** [#29](https://github.com/bradwmorris/centaur-context/issues/29)
**Enyu issues:** [#1](https://github.com/bradwmorris/centaur-enyu/issues/1), [#3](https://github.com/bradwmorris/centaur-enyu/issues/3)
**Related RD:** `rd-fork-centaur-context-multi-agent-poc.md` (follow-on, not superseded)

## Goal

Extend the existing private `centaur-enyu` organizational overlay with one
Researcher-owned durable workflow that turns one explicitly supplied URL, text,
or text file into one protected, connected, searchable canonical Context Source.

## Ownership and boundaries

- Enyu owns its Researcher prompt, trigger tool, ingestion skill, Workflow v2
  definition, permissions, deployment wiring, and business rules.
- Context owns the authenticated permanent Source-intake HTTP contract and the
  standard `centaur-context` client methods.
- Centaur owns workflow execution, bearer-authenticated webhook dispatch,
  workflow principals, agent turns, sandboxes, and secret mediation.
- No Modal, direct SQL, agent database DSN, public Source, scheduled discovery,
  arbitrary Context write, schema migration, or automatic link ingestion.
- The conversation sink/Curator continues to record Slack context. It does not
  create Sources. The workflow alone creates the Source and links the originating
  Chat when its canonical Object ID is supplied, avoiding duplicate ownership.

## Required implementation

- [x] Preserve the distinct Editor and Researcher personas and subscription-backed
  model grant in the existing `centaur-enyu` overlay.
- [x] Enable conventional `workflows/` and `.agents/skills/` overlay discovery.
- [x] Add the narrow `enyu-source-ingest start` trigger tool and ingestion skill.
- [x] Add a durable Workflow v2 definition with a dedicated workflow principal,
  Researcher judgment turn, validation, atomic commit, readiness polling, and
  originating-thread completion/failure notice.
- [x] Deny the Editor the trigger and Source-intake credential; give the Researcher
  only the trigger; give the permanent Context write credential only to
  `workflow-enyu-source-ingestion`.
- [x] Add a private Context listener on port 8086 with its own bearer token and an
  exact principal check. The temporary bootstrap listener on port 8085 remains a
  separate migration-only contract.
- [x] Add `source_intake_validate`, `source_intake_commit`, and
  `source_intake_status` to the standard tool and CLI.
- [x] Normalize canonical URLs, hash exact content, reject ambiguous `related_to`
  edges, reject URI/content identity collisions, use deterministic identities,
  write one atomic transaction, and replay the same request safely.
- [x] Preserve private/protected records, narrow provenance, immutable events,
  explained connections, and optional originating Chat linkage.

## Verification

- [x] Overlay contract tests cover discovery, role separation, subscription grants,
  workflow metadata, trigger authentication, Editor denial, and exact Context
  manifest shape.
- [x] Context auth tests prove token isolation and exact workflow-principal access.
- [x] A disposable PostgreSQL contract test proves validate has zero writes, commit
  creates the Source/content/events atomically, retry replays, changed payloads
  conflict, duplicate URI/content identities conflict, and lexical readiness is
  reported.
- [x] Python client tests prove the permanent Source-intake credential never falls
  back to read, Note-write, or temporary migration credentials.
- [ ] Required repository-wide checks, PR review, and merge are complete.

## Approval boundary

This implementation does not deploy, create live credentials, invoke a paid model,
change Slack configuration, or write to the hosted Context database. Those live
actions remain separately approval-gated.

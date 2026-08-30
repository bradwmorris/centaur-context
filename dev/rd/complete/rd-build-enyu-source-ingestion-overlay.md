# RD: Build the Enyu Overlay and Source Ingestion Workflow

**Status:** `complete`
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
- [x] Required repository-wide checks, PR review, and merge are complete.

## Completion

- Context PR #31 merged as `66eb8aa621c3851e49197f87ac926a0f88098bec`.
- Enyu PR #4 merged as `e392efb3134d6d64325dc9d9b03ea0468f11171c`.
- The shared migration checkpoint `c5bed94` remains exact shared ancestry for
  migration closeout PR #32.
- Live deployment was subsequently approved and completed at Enyu Helm revision
  40, with durable Slack ingress at `https://slack.enyu.org` and two Cloudflare
  tunnel replicas.
- Centaur PR #9 merged as `7fcc2d032641898e04df05d59072ddf65fafa41f`,
  enabling the YouTube caption host used by the standard extraction tool.
- Enyu PR #12 merged as `9e3a0d7ebbd714456b85984731c7b1d64b7223c0`;
  stable deployment repin PR #13 merged as
  `d6e83a4d6e92838d6d9f1d5ea6d47df32ad35dc6`, followed by merged-main
  Context repin PR #14 as `21dee22afd7459b33837c805c2bf02a06bdd1f80`.
- Context PR #41 merged as `c076b496b52d7d82619482123b18f0bb3f5a5a91`,
  reconciling concurrent Curator and Source-ingestion ownership.
- Context PR #42 merged as `24c7ac81dba91946ff0d0f1a0688585e552f4ebb`;
  the live tool and service image now pin merged Context main rather than a
  pre-merge branch build.
- Live YouTube acceptance passed in workflow run
  `01a051cf-4c22-7b01-abc2-4d9f2fd18a44`: Source
  `7ac65959-201a-5539-87dc-219f9ce5277a` contains a transcript with 869 cues
  and 29,352 bytes. The earlier metadata-only Source was archived and its
  canonical URI cleared.
- The permanent port 8086 adapter remains distinct from the removed temporary
  migration listener on port 8085.

## Approval boundary

The original implementation PR did not perform live actions. Deployment,
credential creation, Slack configuration, model invocation, and hosted Context
writes were subsequently performed only after the requester explicitly approved
them. Any future live mutation remains separately approval-gated.

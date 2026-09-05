# 2 — RD: Create an Issue (and PR?) and Reach Out — Upstream Centaur System Message

**Status:** `scoped`
**Created:** 2026-09-05

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** Upstream `paradigmxyz/centaur` main at
`55bf331fdb4aebd22bc7dc330321b6bcb104c764`; the 303-line, approximately
5,500-word `services/sandbox/SYSTEM_PROMPT.md`; its Docker image and entrypoint
path; `compose_system_prompt.py` and tests; Helm's `overlay.systemPrompt` escape
hatch; repo-cache prompt overlays; persona discovery and composition; overlay
documentation; prompt history; and related upstream issues #1511 and #1562.

**Missing:** Upstream maintainer agreement on the public configuration contract
before implementation. This does not block preparing and submitting the issue.

1. Write an upstream issue that demonstrates the unconditional prompt cost,
   explains why append-only overlays cannot remove or safely contradict base
   guidance, and proposes the smallest compatible module-selection contract.
2. After Brad approves publication, submit the issue and wait for maintainer
   direction. Do not begin a broad refactor merely because the issue exists.
3. If the direction is accepted and implementation is explicitly authorized,
   create an isolated Centaur branch from current upstream `main`, split the
   prompt into reviewed fragments, implement deterministic composition and
   deployment selection, and update all affected chart and overlay contracts.
4. Prove compatibility and reduction using unit fixtures plus real local
   sandboxes for representative capability, platform, harness, persona, and
   overlay combinations; then prepare an upstream PR linked to the issue.

## What We Are Doing

- [ ] Give every sandbox only the universal and currently applicable runtime
  instructions instead of unconditionally injecting the entire stock prompt.
- [ ] Let deployments customize prompt composition without forking Centaur or
  carrying contradictory append-only instructions.
- [ ] Preserve mandatory safety and runtime invariants, deterministic ordering,
  persona behavior, and the stock deployment's effective behavior during
  migration.
- [ ] Submit the design upstream before investing in the implementation.

## Contract

- **Goal:** Establish an upstream-supported modular sandbox prompt whose
  effective contents are selected from explicit runtime and deployment facts.
- **Done:** Paradigm has an evidence-backed issue; after accepted direction and
  separate implementation authorization, an upstream PR demonstrates that two
  materially different sandbox configurations receive different, inspectable
  prompts while mandatory core instructions and stock compatibility tests pass.
- **Files:** This RD in `centaur-context`; proposed implementation only in the
  upstream Centaur sandbox prompt/composer, focused tests, Helm values/schema,
  and overlay/configuration documentation. Private organizational guidance
  remains in `/Users/bradleymorris/Desktop/dev/centaur-overlay`.
- **Agent owns:** Measurement, issue and PR drafts, modular design,
  implementation after authorization, tests, local verification, and concise
  migration documentation.
- **Requester owns:** Approval to publish the upstream issue, authorization to
  implement and open the PR, organization policy choices, deployment, and merge
  approval.
- **Out of scope:** Moving organization-specific instructions into Centaur;
  weakening tool, credential, data, upload, or chat-destination safeguards;
  model-specific prompt optimization; dynamic per-turn retrieval of arbitrary
  instructions; public ingress; cloud deployment; and changes to Centaur
  Context's schema or databases.

## Design View

```text
effective AGENTS.md
├── required core                 always included, not overlay-disableable
├── runtime fragments             selected by harness/platform/capabilities
├── installed-feature fragments  selected by actual tools and services
├── deployment fragments          ordered overlay modules or inline escape hatch
└── persona fragment              selected persona, with documented precedence
```

Each fragment needs a stable ID, documented purpose, deterministic order, and
an applicability predicate derived from authoritative configuration already
available when the sandbox is created. The composer must expose enough
provenance for an operator to see which fragment IDs produced the effective
prompt without printing secrets or the whole environment.

The first release must default to a compatibility profile that selects every
current stock fragment. A deployment may then select an upstream-supported
lean profile or explicitly include/exclude optional fragment IDs. Required
security and delivery fragments must fail closed and cannot be disabled through
an overlay. Unknown IDs, missing required fragments, contradictory selections,
and stale warm-sandbox composition must produce clear failures or safe
invalidation rather than silently changing behavior.

## Checks

- [ ] Record total bytes, words, and fragment IDs for the current stock prompt
  and each representative composed prompt.
- [ ] Composer tests cover deterministic order, required-core enforcement,
  unknown IDs, optional selection, overlay precedence, persona placement,
  observability-disabled guidance, reruns, and missing inputs.
- [ ] Chart rendering tests cover defaults, lean selection, explicit fragment
  configuration, and invalid values.
- [ ] Existing stock prompt assertions are migrated to fragment-level or
  effective-prompt assertions without reducing their safety coverage.
- [ ] A real local sandbox proves the effective prompt matches its actual
  platform, harness, permissions, installed tools, persona, and overlays.
- [ ] A fresh unused warm sandbox receives new prompt composition after relevant
  configuration or overlay prompt changes; existing sessions retain their
  documented behavior.
- [ ] Centaur's focused sandbox tests, Helm lint, relevant service checks, and
  `git diff --check` pass.
- [ ] This repository's `git diff --check` passes for the RD.

## Approval Boundary

Creating this RD does not authorize publishing an issue, creating an upstream
branch, opening a PR, merging, deploying, changing a private overlay, or
recycling live sandboxes. Each external publication and implementation phase
requires Brad's explicit approval. No credentials, production data, hosted
writes, public ingress, spending, or destructive operation are authorized.

# RD: Make Eval Runs Explainable

**Status:** `complete`
**Created:** 2026-09-05
**Issue:** [#91](https://github.com/bradwmorris/centaur-context/issues/91)

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** The consolidated `runs` schema and human Run API; current
Run/Eval detail UI; Slack interaction sink, Context builder, prompt assembly,
usage collector, and tool trace collector in Centaur; the live Ed Run
`3ce4d7ca-1eef-46ef-b65d-eeef43449e70`; and the completed Evals RDs.

**Missing:** none. OpenAI-owned hidden instructions and exact provider token
allocation by prompt component are unavailable by contract and must be labelled
as such rather than inferred.

1. Extend the existing Centaur-to-Context Run evidence contract so future Runs
   retain bounded snapshots of the application-controlled input, complete
   Context retrieval packet, sanitized tool activity, and visible response.
2. Extend the existing Run detail API and UI to present that evidence in a
   small number of readable, expandable sections without creating another eval
   record or execution path.
3. Verify exact-versus-estimated labelling, immutable historical evidence,
   redaction, truncation, compatibility with older Runs, and the full local
   Slack-to-Run flow.

## What We Are Doing

- [x] Make an individual Eval Run answer: what did the user ask, what did the
  agent receive, what Context was retrieved and why, what tools ran, what did
  the user see, and what changed?
- [x] Explain token usage with exact provider categories and clearly labelled
  estimates for captured input components.
- [x] Preserve evidence as it existed during execution instead of rebuilding a
  historical Run from current Objects, prompts, or configuration.

## Design View

```text
existing Run
├── input   → exact application-controlled input snapshot
├── trace   → retrieval packet + model usage + sanitized tool evidence
└── result  → exact visible response + provider message reference

Eval detail
├── Conversation
├── What the agent received
├── Retrieved Context
├── Model usage
├── Tool activity
└── Durable changes
```

Keep `runs` as the only canonical execution/eval-attempt store. Do not add an
eval table, prompt ledger, or reconstructed audit system. Store new bounded
evidence in the existing `input`, `trace`, and `result` JSON. Existing root/child
Run relationships remain unchanged.

### Evidence captured for future Runs

1. **Conversation:** exact triggering Slack message, exact visible assistant
   response, provider message IDs, timestamps, and conversation reference.
2. **Application-controlled input:** ordered components actually supplied by
   Centaur: workspace/runtime instructions, selected persona instructions,
   requester/session context, preloaded Context, Slack thread context, current
   message, and available tool catalogue. Each component records its source,
   SHA-256, character count, truncation state, exact bounded text where
   available, and an explicitly estimated token count.
   This specifically includes the effective sandbox instructions derived from
   Centaur's `services/sandbox/SYSTEM_PROMPT.md`. Capture the exact composed
   workspace `AGENTS.md` seen by the harness immediately before the execution,
   not the current contents of the source file when somebody later opens the
   Run. Identify the base sandbox prompt, deployment overlays, and persona
   contribution separately as well as showing the final composed text.
3. **Context retrieval:** query, time, duration, retrieval mode, packet budget,
   truncation/omission counts, and the exact ordered Objects sent to the agent.
   Each Object includes ID, revision, type, title, injected description and
   connections, score, rationale, and attached evidence excerpt when present.
4. **Tool activity:** sanitized command/tool name and arguments, start/end,
   status, duration, exit/error class, and bounded result or error text. Secret
   values and credentials are redacted before persistence; truncation is
   explicit.
5. **Model usage:** retain exact provider-reported input, cache creation, cache
   read, output, reasoning, and total tokens per reported model attempt. Derive
   uncached input and non-reasoning output where categories permit it. Never
   claim that estimated component sizes reconcile exactly to provider totals.
6. **Versions:** model, reasoning effort, harness, persona ID and prompt hash,
   application instruction hash, tool-catalogue hash, and available deployment
   or source revision.

The UI must distinguish preloaded Context from Objects read later through tools,
participants/originating Chat, and changed Objects. A related-Object list is not
a substitute for retrieval evidence.

The Eval detail page uses one vertical flow: properties first, metrics below,
then plain disclosure rows for captured evidence. Closed rows show only their
title. Metadata and content appear after expansion. Do not render cards, tinted
containers, or placeholder rows for provider-hidden instructions and
provider-managed tool definitions.

### Prompt boundary

Display the exact instructions and content controlled by this application under
**What the agent received**. OpenAI/provider-owned hidden system instructions
are not exposed to Centaur and must appear as `Unavailable — provider-owned`,
not as an empty value or reconstructed guess. Never capture hidden
chain-of-thought. Reasoning is represented only by provider token counts.

For clarity, Centaur's sandbox `SYSTEM_PROMPT.md`, its composed workspace
`AGENTS.md`, deployment prompt overlays, and Rez/Ed persona instructions are
application-controlled and therefore visible. They must not be classified as
provider-owned or hidden. The page shows their exact execution-time snapshot,
source path/type, hash, character count, and estimated token contribution.

### Historical Runs

Older Runs keep their current data. The UI uses what was actually persisted and
shows `Not captured for this Run` for absent prompt, retrieval, response, or tool
evidence. It must not rerun retrieval against today's corpus or read today's
prompt files and present them as historical fact.

## Contract

- **Goal:** Make every new Eval Run independently understandable from its
  trusted human detail page.
- **Done:** A fresh Rez or Ed Slack Run shows the exact application-controlled
  input, ordered retrieval evidence and reasons, exact visible response,
  sanitized tool inputs/results, exact provider usage categories, and clear
  unavailable/estimated labels; an old Run renders honestly without fabricated
  evidence.
- **Files:** A bounded extension to existing Run ingestion/types/API/tests and
  `web/src/` in this repository; Slack context, prompt, usage, trace, interaction
  sink, runtime/harness event contract, and focused tests in the adjacent
  Centaur repository; this RD.
- **Agent owns:** Implementation, redaction and size limits, compatibility,
  focused cross-service tests, local deployment, one real Slack verification,
  and repository verification when execution is separately approved.
- **Requester owns:** Semantic review of captured eval evidence, approval of
  external Slack test messages, merge, and any later retention policy.
- **Out of scope:** A new eval store, automatic grading, raw chain-of-thought,
  credentials, OpenAI-hidden instructions, exact token attribution unavailable
  from the provider, public ingress, or a general observability platform.

## Checks

- [x] Contract tests prove bounded snapshots are correlated idempotently to the
  correct Run and cannot overwrite earlier execution evidence.
- [x] Retrieval tests preserve packet order, revisions, rationale, evidence,
  injected text, budget, and truncation state.
- [x] Prompt/tool tests prove secrets are redacted, oversized values are visibly
  truncated, and provider-owned or unavailable fields are labelled honestly.
- [x] A sandbox fixture proves that the exact execution-time instructions
  composed from `services/sandbox/SYSTEM_PROMPT.md`, overlays, and persona are
  visible on the Run even if those source files change afterward.
- [x] UI tests cover new evidence, old Runs with missing evidence, exact versus
  estimated token labels, failures, and accessible expandable sections.
- [x] A real local Slack Run can be understood from the Eval detail page without
  opening logs, querying the database, or inspecting a sandbox.
- [x] Repository-root verification for each changed repository passes, except
  for the existing Slackbot single-process timing/isolation failures described
  below; the same unrelated failures reproduce on unchanged `main`.
- [x] `git diff --check` passes.

## Verification

- Context: `cargo fmt --check`, Clippy with warnings denied, all Cargo tests,
  web type-check/build, 58 web tests, 21 Python client tests, compileall, and
  `git diff --check` pass.
- Centaur: Slackbot type-check and all tests for the changed evidence modules
  pass; harness formatting and all 113 non-network tests pass (four real-provider
  tests ignored); all 38 sandbox tests, Helm lint, and `git diff --check` pass.
- The broad `pnpm --filter slackbotv2 test` command remains nondeterministic in
  unrelated renderer/model-selection timing tests. The same failures reproduce
  on unchanged Centaur `main`; no failing assertion touches this change's one-line
  `index.ts` packet handoff or its evidence collectors.
- Real failed replay retained as Run
  `dd1c08b0-29ec-47d6-ae7d-927eea667c8e`; it records the local Kubernetes pod
  limit failure and remains unpinned.
- Real successful replay retained and pinned as Run
  `515bfee4-528f-46d1-9302-610e5b8c067a`. Browser verification confirms the
  request/response, provider usage, exact application prompt, ordered retrieval
  packet and reasons, exact injected Context, tool catalogue, tool calls,
  omissions, timestamps, pass verdict, and review annotation are visible.
- No `enyu-os` change is required: its overlay only configures the existing
  interaction sink; the evidence contract and capture belong to Centaur and
  Centaur Context.

## Approval Boundary

Execution is approved for implementation, local verification, one exact local
Slack proof message, issue/branch/commit/push, and pull requests. Merge, hosted
writes, public ingress, deletion, and credential changes remain separately
controlled. Captured evidence stays on the existing trusted human surface and
is never exposed through agent credentials.

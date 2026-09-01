# 4 — RD: Run Slack Bot Golden Scenarios and Evals

**Status:** `in_progress`
**Created:** 2026-08-30
**Issue:** [#78](https://github.com/bradwmorris/centaur-context/issues/78)
**Dependencies:** Active priority 3 and the completed canonical-data,
Editor-publishing, and Paradigm-corpus RDs. User-facing bots are `Rez` (Enyu
researcher) and `Ed` (Enyu editor).

## Execution Plan

**Status:** `still needs work`

**Basis checked:** Live Slack interactions; authenticated Context read/write
paths; Slack interaction ingestion; Chat-aware Context Builder; Source-intake
workflow; Curator reconciliation; Run list/detail UI; tool and usage traces;
Enyu personas and role grants; deployed Kubernetes workloads.

**Missing:** The minimum product baseline now works. The remaining work is the
controlled golden-scenario run, deterministic evidence bundle, automated
scoring/reporting, repeat pass, and landing the three feature branches.

1. Freeze the private golden manifest with expected and decoy Object IDs,
   content hashes, exact prompts, and deployed revisions.
2. Run one marked Slack interaction at a time. Do not begin the next interaction
   until its reply, interaction Run, object read-back, and any expected workflow
   or Curator Run are terminal.
3. Complete R1–R3, E1–E2, and X1; collect authenticated evidence and score hard
   invariants independently from answer quality.
4. Repeat the clean suite and exact replays, document failures without weakening
   oracles, run repository checks, review, and land the feature branches.

## What We Are Doing

- [x] Establish a working baseline where every completed bot interaction creates
  a Chat-backed Run with messages, trace/tool calls, usage, and terminal status.
- [x] Prove Rez can ingest and replay the selected YouTube Source without a
  duplicate, and can create narrowly authorized Notes and Tasks.
- [x] Prove Ed can read existing Sources and Rez-created Notes while direct Note
  creation remains denied.
- [x] Prove finished conversation evidence can be curated into a Memory with an
  exact `derived_from` connection to its Chat.
- [ ] Complete the full golden matrix twice, including the article, multi-turn
  grounding, isolation, replay, decoys, and deterministic eval report.
- [ ] Land and synchronize the Context, Centaur, and Enyu changes.

## Contract

- **Goal:** Validate the complete human→Slack→bot/workflow→Context→Curator→Run
  loop against known answers.
- **Done:** Every required scenario passes its hard invariants and reviewed
  rubric twice; exact replay creates no duplicates; each failure is attributable
  to a stage; the evidence bundle identifies deployed and fixture versions; and
  the changes are landed on all canonical repositories.
- **Run identity:** One Slack thread maps to one shared Chat. Every user→bot turn
  that finishes maps to its own Run. Workflows and Curator operations have their
  own linked Runs. A multi-turn Chat therefore has multiple interaction Runs,
  not one aggregate interaction Run.
- **Files:** This RD; safe fixture/runner/report tooling in Context; reusable
  Slackbot instrumentation in Centaur; private Enyu manifests, prompts, roles,
  and deployment contracts. Private source text and credentials stay outside
  Git.
- **Agent owns:** Slack UI operation under Brad's standing test authorization,
  fixture proposal, authenticated evidence collection, scoring, replay,
  redaction, implementation fixes, and defect reports.
- **Requester owns:** Disputed semantic judgments, final source selection changes,
  publication approval, and merge approval.
- **Out of scope:** Direct SQL from agents, `ai_v2` or Console database access,
  public ingress, unrelated product changes, publication, or broadening Ed's
  write permissions.

## Implemented Baseline — 2026-09-01

- Run list/detail UI now presents interaction type, useful titles, Brad/bot
  identities, linked Chat/Objects, expandable traces, tool calls, usage, token
  counts, errors, and results.
- Slack interaction ingestion creates or updates the correct Chat and one Run for
  every completed bot turn. The current Chat Object ID is explicitly supplied to
  the agent so it does not infer an older Chat.
- Context CLI commands are named in Run traces. Read results are recorded as
  consulted Objects; only mutating commands populate affected Objects.
- Rez has narrow `create-note` and `create-task` access. It no longer suggests an
  unrelated Todoist integration for Context writes.
- Both personas can read Context through the brokered tool proxy. The read-only
  Iron Control rule includes Source, Object, and Note GET paths. Write roles
  remain separate and least-privilege.
- Replaying Source ingestion recognizes the existing reverse Chat→Source
  connection and does not violate the connection uniqueness constraint.
- The Context Curator subscription credential was synchronized with the Centaur
  inference credential. The required credential equality and read paths are
  recorded in Enyu's deployment contract.
- Live deployment is Centaur Helm revision 97. API, Context, Ed, Rez, and repo
  cache were all `1/1 Ready` at the checkpoint.

### Acceptance evidence

| Capability | Evidence | Result |
| --- | --- | --- |
| R1 article ingest | Source `220859be-ef33-5ab4-8f88-0affb9637498`; workflow Run `01a05beb-759c-7dad-ae77-7852ff9313e7`; intake child Run `344dbaa6-1181-536e-a0ab-b6067a178310`; Chat `a55be2d6-55b4-432e-aaab-0847ebe4444a` | Passed once; 26,447-byte complete article artifact, lexical/semantic readiness, four evidenced connections, and Slack completion confirmed. |
| R2 video ingest/replay | Source `fc63ac35-8340-56db-a5e0-9064f28cb046`; workflow Run `01a05ba3-474a-78c3-93b3-c699a936b7dc`; interaction Run `c8bf2b70-a497-41ec-905a-ce238f1e926f` | Passed once; replay reused one Source. |
| Rez Note/Task write | Note `cdaa149c-a9e7-48a2-afda-787a4154c6e8`; Task `4abd8559-8cce-4c8e-8431-213e86381dca`; Run `cfaf6a40-55dc-4688-9e2a-aaf8be75a846` | Passed; exact fields and both affected IDs recorded. |
| E1 Ed Source read | Run `8b9e5a33-e41e-41c1-b9c4-72c15679b69b` | Passed. |
| Cross-agent Note read | Run `ada732df-3ce0-432c-b9b3-6bd84d263302` | Passed; exact Note returned, consulted ID recorded, no affected IDs. |
| E2 Ed write denial | Run `4612234e-b58e-45f5-8af6-81dcb3ffe0df` | Passed; no prohibited Note exists. |
| Curator closure | Curator Run `9890d8e6-83ae-4a15-a60e-a3be535a8d52`; Memory `e13d475c-5fd9-4d37-9fa7-baceb5d1ba32` | Passed; Memory and `derived_from` Chat connection committed with model usage. |

Failed diagnostic interactions remain visible as test history. They exposed the
missing Note-read paths and stale Curator inference token. Both causes are fixed;
the later evidence above is the clean acceptance result.

The first live R1 attempt, workflow Run
`01a05bdd-9e91-7f59-84c8-7245e1774e95`, opened and read the Dwarkesh article
but rejected it because the prompt incorrectly required byte-identical raw HTML
instead of a complete browser-rendered article body. Its final error also hid the
agent's useful incomplete-capture reason behind `captured Source content is
empty`. Enyu commit `f86c8203c0f130ec3459ec7ad1565eae73eca5f5`
accepts a complete rendered article body as canonical readable content and checks
capture outcome before the empty-content guard. The focused and full Enyu suites
pass, the revision was deployed, and the exact Slack request then passed on the
first workflow attempt with the R1 evidence above.

### Additional implementation evidence

- **R2 fixture variance:** the selected URL resolved to its canonical watch URL
  and reused the existing Source, Ryan Greenblatt, Hugging Face, and Agents
  Objects; the same-subject video decoy was absent. The Source reached lexical
  and semantic readiness. Exact replay did not change Source revision, artifact
  count, or connection count. The frozen fixture describes 51,501 bytes, 1,141
  lines, and 8,959 words; the captured caption artifact has 42,664 bytes, 1,137
  lines, and 7,813 words. Brad accepted this caption-track variance as
  non-blocking and both hashes remain in the private bundle. The replay response
  still labels the already completed reused Run as `queued`, while terminal Run
  read-back is correctly `completed`.
- **Workflow trace MVP:** Source ingestion owns one top-level workflow Run and
  links its atomic Context intake as a child Run. The workflow API accepts
  privacy-minimized OpenTelemetry-shaped model/tool entries, token usage,
  durations, failures, and terminal outcome without exposing a database
  credential. Schema 21 repaired the historical Ryan Greenblatt linkage and
  labels unavailable pre-instrumentation telemetry instead of inventing spans.
  Browser verification confirmed the parent/child relationship, seven related
  Objects, Brad/Rez attribution, responsive layout, and no console errors.
- **Context Builder Theme repair:** a real Rez Note/Task request exposed an HTTP
  500 when a retrieved Theme decoded a null subtype. Context now returns an
  explicit Theme subtype and safely omits unknown null subtypes. The
  disposable-database regression test and exact Context-request replay pass; the
  repaired request returned six relevant Objects.
- **Narrow writes:** the standard agent API remains read-only. A separate
  authenticated listener permits Rez to create only Notes and Tasks, using a
  stable replay key. Ed and all broader Context mutations remain denied.
  Authorization, validation, client, database-creation, and exact-replay tests
  pass.

## Golden Scenario Matrix

| ID | Slack script and fixture shape | Hard oracle | State |
| --- | --- | --- | --- |
| R1 article | Ask Rez to ingest an article containing one exact Entity, one paraphrased related Entity, and one same-name decoy. | One canonical Source/content; expected IDs connected; decoy absent; no duplicates. | Baseline passed once; frozen decoy fixture and exact replay remain. |
| R2 video | Ask Rez to ingest a YouTube URL overlapping R1 and a pre-existing Theme/Entity. | Canonical watch URL, transcript/hash, correct IDs, terminal readiness, exact replay reuse. | Baseline passed once; golden fixture and second pass remain. |
| R3 discuss/close | In one Rez thread ask a grounded fact, connection question, unsupported question, then `done`. | Grounded answers; calibrated uncertainty; one shared Chat; one interaction Run per bot turn; one Curator Run and primary Memory with exact Chat linkage. | Not run. |
| E1 retrieve | In a fresh Ed thread ask for a fact only in an existing Source/Note body with lexical decoy and paraphrase. | Expected evidence in packet/answer; retrieval ranks and consulted IDs recorded; decoy unsupported. | Source and cross-agent Note baseline passed; decoy fixture remains. |
| E2 record/deny | Ask Ed to create a Note, then close. | Ed does not claim or create a durable Note; interaction and closure Runs still complete. | Baseline passed once. |
| X1 isolation/replay | Ask Ed to ingest R1/R2 and resend one event. | Ed denied; Rez allowed; duplicate delivery/replay creates no duplicate messages, records, changes, or usage. | Not run as full golden case. |

Each fixture includes expected IDs, forbidden IDs, evidence spans, allowed
connection directions, retrieval slice, and corpus hash. Identity, content,
linkage, permissions, idempotency, and Run completeness are binary gates.
Answer support, completeness, concision, and uncertainty use a reviewed 0–2
rubric. Fluency never compensates for a failed hard gate.

## Checks

- [x] Context client tests: 15 passed; Python compilation passed.
- [x] Slack Run trace tests: 5 passed; Slackbot type-check passed.
- [x] Enyu overlay/deployment contract and workflow tests: 57 passed.
- [x] `git diff --check` passed in all changed repositories.
- [ ] Runner correlates evidence by run marker plus Slack workspace/channel/thread,
  not timing, and emits JSON plus a short report.
- [ ] Reports separate retrieval rank, Context packet, answer, durable changes,
  workflow/Curator result, Run trace, and usage.
- [ ] Failure tests cover stale/missing embeddings, lexical fallback, timeout,
  retry, wrong persona, decoys, unsupported answers, and partial failure.
- [ ] Two clean runs and exact replays pass; full repository-root checks pass.

## Branch and Landing State

- Context implementation checkpoint before this RD update:
  `codex/78-slack-bot-golden-evals` at
  `1be88975e7496d8f3d676e6e33bd6703b0602458`.
- Centaur: `codex/78-universal-slack-runs` at
  `5ac746896e61a18474216dc30bffde82fd230fc1`.
- Enyu: `codex/78-slack-bot-golden-evals` at
  `daedbae851a798c08cee1711cb84173543e76f71`.
- All three branches are pushed. No PR is open and none of the changes is
  landed on `origin/main`.

## Approval Boundary

Brad has authorized continued local Slack/model/database testing and in-scope
fixes for this baseline. Agents still never receive a database DSN. Publishing,
public ingress, unrelated external integrations, destructive cleanup, production
deployment, spending outside the configured subscription path, PR merge, and
deleting evidence require separate explicit approval.

# 8 — RD: Build Progressive Architecture Diagrams and Documentation

**Status:** `backlog`
**Created:** 2026-09-03

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** The live [Centaur architecture page](https://centaur.run/architecture),
current docs and artwork, the Enyu overlay and ingestion implementation, sandbox
image, api-rs webhook handling, Helm configuration, and control-plane schema.

**Missing:** Final editorial approval of the six diagrams before publishing.

1. Establish one reusable visual system and progressive layout.
2. Build six diagrams, from a product-neutral agent harness anatomy through
   Centaur core and Enyu source ingestion.
3. Add concise accessible explanations to the Centaur Context architecture
   docs.
4. Render at desktop and mobile widths, run docs checks, and obtain Brad's
   approval before publishing or deploying.

## What We Are Doing

- [ ] Reveal the architecture in six stages instead of one dense diagram.
- [ ] Start with a product-neutral intuition pump showing how an agent harness
  combines a model, skills, and tools on a user-chosen machine.
- [ ] Emphasize that Centaur runs in user-owned infrastructure and the selected
  agent harness runs inside its isolated sandbox.
- [ ] Distinguish Centaur core, the optional Centaur Context app extension, and
  the private Enyu overlay.
- [ ] Finish with the complete source-ingestion example.

## Diagram Contract

### Product-neutral conceptual model

`context → model → tool call → harness executes → result added to context ↺`

When the model returns no tool call, the harness returns the final response and
ends the turn.

The harness is the orchestration and execution software around the model, not
the model itself. It assembles model context, makes approved tool definitions
available, executes requested tool calls, appends results to context, and
continues until the model produces a response. Skills are optional instructions
loaded into that context, not a separate execution engine. The harness process
may run on the user's device or a VM; the model may run locally or be reached
through an API.

### Shared Centaur conceptual model

`Slack/API request → user-owned Centaur runtime → sandbox agent (one selected harness) → approved tools or durable workflows → response`

Treat Postgres/state and controlled provider egress as supporting relationships,
not sequential steps. Make the user-owned runtime the dominant container. Slack,
API callers, model providers, and external services remain outside it. Postgres
stays in the user's infrastructure but outside individual sandboxes.

### Progressive series

0. **Agent harness anatomy.** Before introducing Centaur, show only the common
   product-neutral loop shared by harnesses such as Codex, Claude Code, and
   Hermes: request enters context; context is sent to the model; a tool call is
   checked and executed by the harness; the tool result is appended to context;
   the loop repeats; or a final response ends the turn. Keep skills as one item
   inside context, not a peer box. Annotate, without adding another topology,
   that the harness process can run on a device or VM and the model can be local
   or reached through an API. Exclude all other features, including Centaur,
   sandboxes, Postgres, Context, Enyu, Slack, persistence, memory, subagents,
   hooks, provider plumbing, and deployment detail.
1. **Centaur core.** Show Slack/API input, the user-owned Centaur runtime,
   control plane, durable state, one temporary agent sandbox, Centaur adapter,
   selected harness, approved tools/workflows, controlled egress, and response.
   Exclude Context, Enyu, Rez, Brad, and source ingestion. Put Claude Code,
   Codex, Hermes, and Pi inside the sandbox as one-at-a-time harness choices.
2. **Add Centaur Context.** Preserve diagram 1 and add Context as an optional app
   extension in the user-owned environment. Show its agent context/knowledge
   relationship and its own durable application data. Do not conflate that data
   with Centaur's operational session and workflow state.
3. **Add the Enyu overlay.** Preserve diagram 2 and add Enyu as the private
   overlay configuring personas, tools, workflows, permissions, and
   deployment-specific behavior. It is not a separate runtime or third-party
   service.
4. **Ordinary Rez Slack turn.** Apply diagram 3 to `Hey, what's up?`: Slack →
   `cloudflared` → Rez Slackbot → `api-rs`; the control plane persists state and
   creates or reuses the temporary Rez sandbox; one harness runs and its events
   become the Slack response. Show relevant Context/Enyu participation but no
   ingestion trigger, workflow run, or workflow sandbox.
5. **Rez source-ingestion turn.** Extend diagram 4 for Brad's `@Rez, can you add
   this conversation as a source?` and YouTube URL. The harness runs
   `enyu-source-ingest start …`. Its authenticated
   `POST /api/webhooks/enyu-source-ingestion`, routed through the sandbox's
   paired iron-proxy, triggers the workflow; the adapter does not. `api-rs`
   starts a separate temporary workflow sandbox containing the Python workflow
   host and `enyu_source_ingestion.py`. Checkpoints remain in Postgres, the
   canonical Source and Connections go to Context, and completion goes to Slack.

Diagram 5 may omit the child analysis-agent sandbox for legibility, but its
caption must identify the view as simplified.

### Visual direction

- Reuse visual grammar, boundaries, positions, and names so each frame clearly
  shows what was added.
- Use the Centaur palette: near-black, dark-gray panels, `#00E100` highlights,
  muted secondary text, thin borders, and generous spacing.
- Keep one dominant left-to-right story with minimal labels.
- Label the main container `Your infrastructure` and Centaur as its user-owned
  runtime. Keep all harness choices and the active harness loop inside the agent
  sandbox.
- Put Slack requests in speech bubbles. Use the supplied Brad and Rez avatars
  and the official Slack mark in example diagrams.
- Keep agent/workflow sandboxes visibly temporary, iron-proxy as a separate
  paired pod, and model providers outside the user-owned runtime.
- Preserve editable SVGs with `<title>`/`<desc>`, approved semantically named
  assets, ordered filenames, and verified PNG fallbacks.

## Reference Material

- Docs: `/Users/bradleymorris/Desktop/dev/centaur-context/docs/architecture.md`
- Architecture artwork: `/Users/bradleymorris/Desktop/dev/centaur-context/docs/assets/`
- Existing Centaur artwork and Slack mark are references only:
  `/Users/bradleymorris/Desktop/dev/centaur/docs/public/brand/`
- Style reference: `/Users/bradleymorris/Desktop/architecture.svg`
- Rough map: `/Users/bradleymorris/Desktop/map.png`
- Detailed draft and supporting icons: `/Users/bradleymorris/Desktop/temp for Centaur BIP/`
- Rez avatar: `/Users/bradleymorris/Desktop/T0BFLA920LA-U0BTPD1CYCC-7db727af2693-192.png`
- Enyu tool, workflow, and Helm values: `/Users/bradleymorris/Desktop/dev/centaur-enyu/`
- Sandbox and harness: `/Users/bradleymorris/Desktop/dev/centaur/services/sandbox/Dockerfile` and `/Users/bradleymorris/Desktop/dev/centaur/crates/harness-server/`
- Webhook route and schema: `/Users/bradleymorris/Desktop/dev/centaur/services/api-rs/crates/centaur-api-server/src/routes.rs` and `/Users/bradleymorris/Desktop/dev/centaur/services/api-rs/crates/centaur-session-sqlx/migrations/0001_session_control_plane.sql`

## Contract

- **Goal:** Publish an accurate progressive series that first teaches agent
  harness anatomy, then the user-owned Centaur runtime and its extensions.
- **Done:** Six reviewed SVGs and PNGs are in Centaur's docs; captions explain
  harness behavior, placement, ownership, state, extension, overlay, trigger,
  and response boundaries; the docs render without clipping or unreadable
  labels.
- **Files:** This RD, Centaur Context `docs/architecture.md`,
  `docs/assets/architecture-*`, and the README documentation link. The
  canonical Centaur repository is reference-only and must remain unchanged.
  Do not publish private Enyu assets or details without Brad's approval.
- **Agent owns:** Diagrams, verification, accessible copy, rendering, and checks.
- **Requester owns:** Likeness use, editorial approval, publication, deployment,
  and Enyu disclosure.
- **Out of scope:** Runtime/workflow changes, new ingress, full analysis-pod
  detail, deployment, and site redesign.

## Checks

- [ ] Verify every boundary and arrow against the implementation.
- [ ] Confirm the sequence is coherent and diagrams 0 and 1 each stand alone.
- [ ] Confirm diagram 0 distinguishes skills (instructions) from tools
  (actions), and does not imply a connected model is physically on the host.
- [ ] Confirm harnesses are named correctly, inside the sandbox, and clearly
  alternative choices.
- [ ] Distinguish Centaur operational state from Context application data.
- [ ] Inspect every SVG and PNG at desktop and narrow docs widths.
- [ ] Confirm useful SVG `<title>`/`<desc>` and page alt text or captions.
- [ ] Run discovered Centaur docs type-check/build commands.
- [ ] `git diff --check` passes in every changed repository.

## Approval Boundary

Local drafts are authorized. Publication, deployment, public likeness use,
private Enyu disclosure, and hosted writes require Brad's explicit approval.

# 1 — RD: Finish the Centaur Documentation and Build-in-Public Script, Including Diagrams

**Status:** `backlog`
**Created:** 2026-09-03

## Execution Plan

**Status:** `complete and ready`

**Basis checked:** The Centaur Context README and docs; the first two
architecture SVG/PNG pairs; the web app's routes, navigation, responsive shell,
and tests; Centaur's architecture docs; and the implemented product boundaries.

**Missing:** Brad's final first-person story, intended public audience,
publication destination, disclosure choices, and editorial approval. These are
collaboration inputs, not decisions for an agent to infer.

1. Work with Brad to capture the original problem, why Centaur was chosen, what
   was built, the unique Context extension, lessons, and next steps.
2. Turn it into a first-person script and companion article. Verify technical
   claims while separating shipped behavior, experiments, and future ideas.
3. Build only the diagrams needed to explain the story, progressing from a
   general agent harness to Centaur, Centaur Context, the private overlay, and a
   representative end-to-end interaction. Reuse or revise the existing first
   two diagrams rather than restarting automatically.
4. Derive durable Context documentation from the approved material. Update the
   README and architecture docs, then add a documentation/About UI surface.
5. Review the script, article, diagrams, README, repository docs, and UI together
   for one consistent explanation. Publish only after Brad's explicit approval.

## What We Are Doing

- [ ] Produce a Brad-led build-in-public script explaining the project and its
  reasoning, not merely a technical walkthrough.
- [ ] Produce a companion article from the same approved narrative and evidence,
  adapted for reading rather than copied mechanically from the script.
- [ ] Explain Centaur and Context without confusing Centaur's operational state,
  Context's knowledge model, and private-overlay behavior.
- [ ] Use a concise progressive diagram set to make those boundaries and one
  real end-to-end flow understandable.
- [ ] Make the stable Centaur Context documentation discoverable from the
  repository README and directly viewable in the product UI.

## Content Contract

### Build-in-public script and article

The public narrative is the primary source for the work. It should cover:

1. the user problem and Brad's motivation;
2. the agent-harness model and Centaur's role;
3. Centaur's sandboxes, control plane, permissions, tools, workflows, and
   user-owned infrastructure at an appropriate level;
4. why a separate shared-context layer was needed;
5. Centaur Context's canonical Objects, subtype records, Connections, Events,
   Sources, Artifacts, search, Runs, Curator, and human UI;
6. how a private overlay adds organization-specific personas and workflows
   without changing the reusable base products;
7. one representative Slack-to-agent-to-Context flow;
8. lessons, limitations, open questions, and next steps.

The two outputs may share a factual outline and diagrams, but each must work in
its medium. The script needs spoken pacing, demonstrations, and visual cues; the
article needs headings, captions, links, and standalone context. Neither may
disclose unapproved private information.

### Diagrams

Use the smallest sequence that tells the approved story. Expected subjects are:

- agent harness anatomy;
- Centaur core in user-owned infrastructure;
- Centaur plus the optional Centaur Context application;
- the private-overlay boundary; and
- one ordinary or source-ingestion flow that shows the system working.

Keep one visual language across editable SVGs and PNG fallbacks. Each needs
accurate boundaries, a caption and alt text, readable wide/narrow rendering,
and a simplified-view note when detail is omitted.

### Centaur Context documentation surfaces

- The README gives a concise orientation, clear relationship to Centaur, visual
  overview, documentation links, and a useful start path.
- Repository docs hold the durable technical explanation and diagrams. Reuse
  public-story material only when it is accurate and maintainable.
- The web UI exposes a documentation or About route from normal navigation,
  without requiring another server, and works when no data records exist.
- README, docs, and UI share source material or an explicit synchronization
  convention so the three surfaces do not quietly diverge.

## Contract

- **Goal:** Help Brad tell the complete build-in-public story, then turn its
  stable technical explanation into excellent Centaur Context documentation
  available in Git and in the product itself.
- **Done:** Brad approves a publication-ready script, companion article, and
  supporting diagrams; the README and repository docs accurately explain
  Centaur Context; and the running UI provides a tested, navigable documentation
  surface that renders the approved content and diagrams clearly.
- **Files:** This RD; an approved location for public narrative drafts;
  `README.md`; `docs/architecture.md` and related documentation; `docs/assets/`;
  focused web routing, documentation-view, styling, and test files. The upstream
  Centaur repository and private overlay are reference-only unless separately
  authorized.
- **Agent owns:** Interview prompts, research, outlines, drafting help,
  fact-checking, diagrams, documentation/UI implementation, accessibility,
  tests, and revisions.
- **Requester owns:** The story, opinions, voice, demonstrations, disclosure
  choices, likeness use, final editorial decisions, recording, publication,
  promotion, and decisions about what happens next.
- **Out of scope:** Inventing Brad's personal account; publishing without review;
  exposing private Enyu details; changing runtime behavior, ontology, deployment,
  ingress, or authentication; rebuilding Centaur's public documentation site; or
  turning the Context UI into a general-purpose CMS.

## Checks

- [ ] Architectural claims and diagram boundaries match current code or
  authoritative Centaur documentation.
- [ ] Brad approves the narrative outline before final prose and approves the
  complete script, article, and diagrams before publication.
- [ ] Script and article are recognizably the same story while fitting their
  respective spoken and written formats.
- [ ] README, repository docs, and UI documentation agree on terminology,
  ownership boundaries, capabilities, limitations, and setup links.
- [ ] All SVGs have useful `<title>`/`<desc>`, all rendered uses have meaningful
  alt text or captions, and SVG/PNG output is inspected at wide and narrow sizes.
- [ ] UI tests cover the documentation route, navigation, empty-state
  availability, and internal links; browser review finds no clipping, unreadable
  diagrams, broken links, or console errors.
- [ ] Repository-root verification commands and `git diff --check` pass.

## Approval Boundary

Local research, outlines, drafts, diagrams, and implementation are authorized
only when this RD is executed. Publication, recording or likeness use, disclosure
of private overlay details, changes to upstream Centaur, deployment, public
ingress, hosted writes, spending, credentials, and deletion require Brad's
explicit approval.

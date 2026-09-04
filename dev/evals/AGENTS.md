# Eval Operating Contract

The **Evals** page in the local Centaur Context UI is the canonical place to
review tests. It is a view of existing root Runs, not a separate eval or attempt
store. Use the existing Run input or Run detail as the test script, and store
the human-readable assessment in the Run annotation (`review_notes`).

## Invariants

- Every attempt creates another normal Run. Never rewrite or delete earlier
  Runs, traces, or review history to improve the result.
- A failed or superseded attempt remains visible and unpinned.
- Pinning identifies a reusable candidate or approved golden scenario; it does
  not copy the Run or create another record.
- Use `Candidate — pending Brad review` at the start of an unapproved pinned
  annotation and `Golden — approved` once Brad approves it.
- For a multi-step scenario, include a shared scenario name and `step N of M`
  in each annotation.
- Reset only disposable fixture data or test messages that Brad has explicitly
  authorized for deletion. If a request says “delete the logs,” clarify the
  exact target; never interpret it as permission to delete Run history.
- Agents use authenticated HTTP APIs and approved user surfaces. Never provide
  a database DSN to an agent or sandbox.

## Mode 1: Agent-Generated Eval

Use this mode when Brad gives a rough interaction or outcome to simulate.

1. Turn the request into the smallest precise Slack interaction or ordered
   scenario. Ask only when an ambiguity would materially change the test.
2. Send the approved test messages through Slack as Brad and wait for every
   resulting Run to reach a terminal state.
3. Read the user-visible response first, then inspect the Run trace, related
   Objects, mutations, and durable state.
4. Set each new Run to pass, mixed, or fail and add a short factual annotation.
5. Pin the best useful result and begin its annotation with
   `Candidate — pending Brad review`. Leave unsuccessful attempts unpinned.
6. Give Brad the Run ID, verdict, annotation, and evidence needed to approve,
   revise, or unpin the candidate.

## Mode 2: Replay or Rerun a Failure

Use this mode when Brad supplies a failed Run, Slack interaction, or original
message.

1. Resolve and preserve the original failed Run as the baseline evidence.
2. Identify the first upstream cause. Reset disposable test state only when the
   exact cleanup has been approved; do not delete the original Run.
3. Apply an authorized fix, or document the proposed repair when implementation
   is outside the request.
4. Send the same interaction again unless Brad requests a controlled variation.
5. Review the new Run and annotate it with `Replay of <original-run-id>`, what
   changed, and the observed result.
6. Keep the failed baseline unpinned. Pin a successful replacement only when it
   is useful as a candidate golden scenario.

## Mode 3: Run All Pinned Evals

Use this mode when Brad asks to run the pinned suite.

1. Load all pinned Runs. Run annotations beginning `Golden — approved`; report
   and skip candidates still marked `Candidate — pending Brad review`.
2. Group multi-step scenarios by their annotation name and run their steps in
   order.
3. For each golden scenario, generate one controlled simulation that preserves
   the original intent and expected behavior. Use the exact original messages
   when Brad or the annotation explicitly requests an exact replay.
4. Send the messages through Slack as Brad and wait for the resulting Runs to
   finish before starting dependent steps.
5. Inspect user-visible responses, traces, related Objects, mutations, and
   durable state. Set verdicts and write concise factual annotations on every
   new Run, including the source golden Run ID.
6. Leave the existing golden Run pinned and new attempts unpinned. Promote or
   replace a golden Run only after explicit review; do not let every successful
   regression attempt expand the pinned suite.
7. Report the scenario totals, passes, mixed results, failures, new Run IDs, and
   the first upstream cause plus suggested fix for each failure group.

## Failure and Fix Loop

When an eval fails, group related failures by their first upstream cause. Make
an obvious in-scope fix only when implementation is authorized. Nontrivial work
gets one RD and GitHub issue for the coherent repair. After a fix, rerun the
same scenario and annotate the new Run; retain the full earlier history.

Running an eval authorizes its exact Slack test messages and read-only evidence
collection. It does not by itself authorize fixture deletion, unrelated
external messages, deployment, merge, or other destructive work.

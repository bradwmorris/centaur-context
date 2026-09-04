# Running Evals

The **Evals** page in the local Centaur Context UI is the canonical place to
review tests. It is a second view of the existing root Runs, not a separate eval
or attempt store.

## Normal Loop

1. Open **Evals** and choose a pinned golden Run whose input should be repeated.
2. For a multi-step flow, follow the order written in annotations, for example
   `Sarah Guo — step 1 of 5`.
3. Send the exact test input through the approved user surface and wait for its
   normal Run to reach a terminal state.
4. Read the user-visible response, then inspect its trace, related Objects,
   mutations, and durable state.
5. Set the new Run to pass, mixed, or fail and write one short factual annotation
   describing what actually happened.
6. Pin only a stable, useful example. Keep failed and superseded attempts
   visible and unpinned.

Every retry creates another normal Run. Do not rewrite or delete earlier Runs to
improve the history. Group a multi-step scenario using a shared annotation name
and explicit step number; do not create a separate scenario record.

When a failure needs code changes, identify the first upstream root cause. Make
an obvious in-scope fix when authorized; otherwise create one RD and issue for
the coherent repair. Rerun the same input after the fix and annotate the new Run.

Running an eval authorizes only its exact approved test messages and read-only
evidence collection. Fixture deletion, unrelated external messages, deployment,
and merge retain their normal approval requirements. Never provide a database
DSN to an agent or sandbox.

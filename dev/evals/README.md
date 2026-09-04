# Eval Catalog

This folder is the canonical starting point for Centaur Context evaluation work.

- `evals.csv` contains concrete eval cases that have been attempted.
- This README contains candidate golden scenarios that have not necessarily been
  exercised as complete evals.
- Runtime Runs, traces, and detailed evidence remain in Centaur Context or the
  relevant RD; the CSV is a catalog, not a second execution log.

Before testing, find or add the case in `evals.csv` and work from its exact input
and expected result. Add a case immediately before its first real attempt, not
when it is only an idea. After changing an eval, update `updated_at` and use the
new definition for subsequent attempts.

## Table Contract

Keep `evals.csv` as UTF-8 CSV with this fixed header:

```text
id,suite,name,input,expected_result,pinned,status,updated_at
```

- `id`: Stable lowercase kebab-case identity. Never reuse an ID for a different
  test.
- `suite`: Shared lowercase kebab-case identity for related cases. Leave blank
  only for a genuinely standalone eval.
- `name`: Short human-readable description of the behavior under test.
- `input`: Exact user input. Represent an intentional line break as the two
  characters `\n` so each eval remains one CSV row.
- `expected_result`: Concise observable pass condition. Include identity,
  mutation, linkage, permission, or duplication requirements when relevant.
- `pinned`: `true` only for stable, high-value regressions that should be run
  regularly; otherwise `false`.
- `status`: `active`, `paused`, or `retired`. A failed attempt does not retire an
  eval.
- `updated_at`: Date of the latest definition change in `YYYY-MM-DD` form.

One row represents one independently testable case. Use `suite` to preserve the
order and shared context of a multi-step flow. Do not store credentials, private
source text, database connection strings, transient Object IDs, or verbose run
evidence in the CSV.

Pin an eval after it has proved repeatable and valuable as a regression. Pinned
evals use the same schema and workflow as every other eval; pinning is only a
commitment to exercise them regularly.

Existing files under the repository-root `evals/` directory may remain as test
fixtures consumed by code. They are not the canonical catalog. Any future eval
that is actively tested should first have a row here.

## Golden Scenario Candidates

These are the broader golden scenarios identified during the first Slack eval
round. Keep them here until each becomes a concrete attempted case in
`evals.csv`.

| ID | Scenario | Pass condition |
| --- | --- | --- |
| R1 | Rez ingests an article containing one exact Entity, one paraphrased related Entity, and one same-name decoy. | One canonical Source and complete content are stored; expected Objects are connected; the decoy is absent; replay creates no duplicate. |
| R2 | Rez ingests a video overlapping an existing Theme and Entity. | The canonical video URL, transcript and hash, expected Object links, terminal readiness, and exact replay reuse are correct. |
| R3 | In one Rez thread, ask a grounded fact, a connection question, an unsupported question, and then close with `done`. | Answers are grounded or appropriately uncertain; one Chat contains the thread; every turn has a Run; Curator creates one primary Memory linked to the Chat. |
| E1 | In a fresh Ed thread, ask for a fact found only in existing Source or Note content, with a lexical decoy and a paraphrase. | Retrieval records the expected evidence and consulted IDs; the answer is supported; the decoy is not treated as evidence. |
| E2 | Ask Ed to create a Note and then close the conversation. | Ed neither creates nor claims to create the prohibited Note, while interaction and closure Runs still complete. |
| X1 | Ask Ed to ingest the R1 or R2 fixture and resend one Slack event. | Ed is denied, Rez remains allowed, and replay creates no duplicate message, Object, mutation, or usage record. |

The attempted and pinned Sarah Guo RSI flow is represented by the
`slack-rsi-flow` rows in `evals.csv`.

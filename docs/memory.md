# Event memory and background curation

Memory records meaningful activity in one explicit sentence. Chat capture remains
append-only and human-grounded; a request does not prove completion. It skips
routine status traffic and explicit synthetic tests. Original research belongs
in Notes and Sources.

`MEMORY_CAPTURE_ENABLED=true` enables deterministic capture of newly committed
Task/Source/Note creations. It uses Object Events, not assistant success claims.
The first enablement records a durable starting point; restarts do not reset it.
Each pass examines at most 25 events, using the event ID as its replay key. No
model calls are made. Imports and explicit test fixtures are skipped. Exact
writer identities are used when available; otherwise wording identifies a user
or agent without guessing the initiating human. The actual principal stays in
provenance. Memory writes themselves are never recaptured.

`MEMORY_DREAM_MODE=preview|apply|off` controls background maintenance. Default is
`off`; a configured Curator model transport is required when enabled. Start with
`preview`, inspect the recorded `memory_dream` Runs, then use `apply` after
verification. `MEMORY_DREAM_INTERVAL_SECONDS` defaults to 3600 (minimum 60).
There is no new deployment or persistent agent session. The subscription
transport requires the compatible maintenance schema in the inference broker.

One pass considers at most 25 changed generated Memories and 25 exact-event
neighbors, with bounded evidence and graph reads. The serialized evidence input
is capped at 24,000 bytes, shrinking the batch before inference. A single record
that cannot fit is recorded as unchanged, with a reason, so it cannot starve
later work. The model has one attempt per wake and a 60-second HTTP timeout.
Subscription inference caps maintenance input at 28,000 bytes and output at
8,000 bytes, with a 60-second model timeout. These are byte/time limits, not a
claim of an exact tokenizer or reasoning-token cap. Existing usage accounting
records reported tokens. A quiet/unchanged batch uses zero model calls, including
an unchanged preview.

Review checkpoints reuse the existing Run ledger and exact Object revisions.
Late commits are not lost behind a timestamp cursor. The worker's own final
revisions are checkpointed, avoiding self-trigger loops. An advisory lease
prevents overlapping passes and releases on process loss. Failed passes retry at
the next scheduled wake; no immediate inference retry loop is used.

Maintenance permits only generated, unprotected Memories and Connections with a
Memory endpoint. Human/interactive-agent edits and `memory_locked` provenance are excluded.
It cannot edit Notes, Sources, Tasks, users or immutable evidence. Model proposals
are validated and written atomically against the observed revisions. Preview runs
exercise the same checks in a rolled-back transaction.

Merges require identical supporting event/message identities and event time.
A surviving Memory retains evidence, while retired IDs retain a `merged_into`
reference and full journal history. Optional relationships cannot silently vanish
in a merge: they must already exist on the survivor, or be reviewed separately.
No age-based expiry or automatic physical purge occurs. Retired rows disappear
from ordinary active retrieval. Ambiguous records stay unchanged.

The existing owner `POST /api/v2/runs/{id}/undo` can reverse a completed
`memory_capture` or `memory_dream` Run. It checks current revisions and reverses
the whole run atomically; a newer edit causes a conflict rather than an overwrite.
Undo does not erase the journal. Capture's processed event key survives undo, so
it cannot recreate the same retired event on its next tick. Pause maintenance
with `MEMORY_DREAM_MODE=off` before investigating quality regressions.

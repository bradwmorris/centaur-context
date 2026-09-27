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
verification. Review wakes after completed Runs commit (including later capture
Runs inserted already completed), and after ordinary Object/Connection changes.
There is no idle review timer. `MEMORY_DREAM_INTERVAL_SECONDS` remains accepted
for deployment compatibility but no longer schedules review. Startup and listener
reconnection scan durable pending versions before waiting. This worker runs inside
Context independently of capture, Slack and Routine Tasks.

One pass considers at most 25 changed generated Memories and 25 exact-event
neighbors, with bounded supporting messages, original Events, metadata search and
graph reads. Nearby earlier Memories are read-only connection candidates. Input
is capped at 24,000 bytes by reducing the batch; a single incomplete/oversized
Memory is deferred so other rows can proceed. The reviewer requests exactly
`gpt-6-luna` with `high` effort. Its HTTP timeout uses
`CURATOR_MODEL_TIMEOUT_SECONDS` (default 210 seconds); measure actual latency before
release. Subscription receipts must match the requested model and effort as well
as the existing request/provider/harness/authentication/billing contract. A broker
with the previous maintenance schema is incompatible: deploy a reviewed producer/
consumer pair before enabling this worker. Never silently fall back.

Review checkpoints reuse Runs with a fingerprint of Memory revision, relevant
connections, supporting Object revisions and origin-Chat message progress. The
worker checkpoints its own final versions. Preview checkpoints are distinct from
apply; an unchanged preview incurs no further inference. An advisory lease
prevents overlapping workers. Transient failures retry after 30/60 seconds, at most
three attempts for the same input. Missing evidence and invalid plans defer that
input until it changes. Authentication/schema/attribution failures pause model
calls for the same configuration. After repairing the broker, an operator can
advance `CURATOR_PROMPT_VERSION` to the reviewed release version and restart;
that explicitly starts a new configuration attempt without erasing failed Runs.
Inspect `result.paused`, `result.deferred`, `result.retry_at` and the Run error.
No-work passes make zero model calls.

Maintenance permits only generated, unprotected Memories and Connections with a
Memory endpoint. Human/interactive-agent edits and `memory_locked` provenance are excluded.
It cannot edit Notes, Sources, Tasks, users or immutable evidence. Model proposals
are validated and written atomically against the observed revisions. Preview runs
exercise the same checks in a rolled-back transaction.

Merges require identical supporting event/message identities and event time.
The plan must supply the survivor’s final title and description; surviving links
are transferred and deduplicated atomically. Retired IDs retain a `merged_into`
reference and full journal history. A protected/manually edited link blocks the
whole merge. A rewrite may also repair connections in the same transaction.
Connection repair can change an existing generated description, or disconnect an
optional relation and reconnect it with a corrected type. Required derivation
links cannot be removed. Distinct events can use `related_to` when context
establishes continuation; shared topics alone are insufficient.
No age-based expiry or automatic physical purge occurs. Retired rows disappear
from ordinary active retrieval. Ambiguous records stay unchanged.

The existing owner `POST /api/v2/runs/{id}/undo` can reverse a completed
`memory_capture` or `memory_dream` Run. It checks current revisions and reverses
the whole run atomically; a newer edit causes a conflict rather than an overwrite.
Undo does not erase the journal. Capture's processed event key survives undo, so
it cannot recreate the same retired event on its next tick. Pause maintenance
with `MEMORY_DREAM_MODE=off` before investigating quality regressions.

## Apply an exact reviewed preview

Keep `MEMORY_DREAM_MODE=preview` for initial cleanup. Switching to autonomous
`apply` runs new model proposals; it is not an exact application of saved wording.
For an approved initial batch, use the existing authenticated maintenance listener:

```json
{"preview_run_id":"<saved-memory-dream-preview-UUID>","validate_only":true}
```

Send this to `POST /api/v2/maintenance/memory-review` with the existing maintenance
bearer token, allowed principal and thread headers. Validation returns the saved
plan, exact Memory IDs/revisions and `approval_sha256`. It validates by rollback
and changes no Memory. The hash binds the originating preview, policy, compact
snapshot and exact plan. Original message/Note bodies are not duplicated.

After the owner approves that specific result, an operator adds the returned hash
to `MAINTENANCE_APPROVED_REQUEST_SHA256` using the existing deployment process.
Send the same preview ID with `validate_only:false`. The server applies only that
saved plan; it makes no model call and selects no new rows. Changed Memory,
connection, supporting-context or candidate revisions reject the whole batch.
The application creates a deterministic child Run: repeated successful requests
return that Run, concurrent requests either replay or receive a retryable conflict,
and its normal Memory undo restores the changes. An interrupted/failed attempt
with no committed Events can retry the same saved plan after full revalidation;
the Run retains the prior error and retry count. A reversed child requires a
fresh preview: approval cannot redo an undone plan. Old previews without a
saved approval snapshot also require a fresh preview. No caller-supplied approval
flag can bypass the server's configured exact-hash list.

Only enable autonomous `apply` after separate approval of ongoing automatic
review. Remove consumed hashes when they are no longer needed. This endpoint
uses the existing maintenance authority; ordinary agent listeners do not expose it.

## Event review policy v3

The `event-review-v3` checkpoint reconsiders eligible legacy revisions once.
Successful unchanged reviews make no further model call. Missing, oversized or
incomplete evidence is recorded as `deferred`, never `reviewed`, until the supporting version changes. Inspect those Run reasons before any
claim of complete cleanup. A graph exceeding the bounded input is also deferred.
Manual edits, protection and `memory_locked` still exclude a Memory.

Task review/completion transitions require a new result reference in the committed
brief and link the actual Task in the same transaction as the Memory. The event
records submission or recorded completion, not inferred merge, deployment or
acceptance. Later committed result references can supply evidence for an unchanged
Task status. Unsupported Task events remain deferred receipts until a later
supported update; an old immutable event is never rewritten into success.

New Git receipts are technical Run evidence only. A legacy Git Memory is eligible
only when its original completed capture Run identifies that exact Memory, actor,
provenance and proof digest. The adapter supplies the verified observation's narrow
meaning to maintenance; actor allowlisting alone cannot enable edits. Retirement
retains receipts and supports the existing Run undo.

## Paired broker configuration and failures

For the Luna6 producer, explicitly set `CURATOR_MODEL=gpt-6-luna` on Context.
Startup accepts Luna6 and explicit legacy Luna5.6 configurations, but never maps
one name to the other. Every successful subscription receipt must match the
requested model and effort. Ordinary extraction/summary calls remain Low; the
Memory reviewer independently requires Luna6 High. A legacy configured model
cannot be paired with a producer that serves only Luna6.

Non-success subscription responses may provide the bounded
`curator_inference_failed` envelope. Context verifies its request/model/effort
identity, retains only allowlisted diagnostics in `Run.result.inference_failure`,
and uses its explicit retryability. Authentication, quota and unsupported-model
failures therefore pause even if the broker returns HTTP503; transient failures
still use the same three-attempt budget. Unknown, malformed or oversized error
responses retain only the HTTP status and bounded legacy retry behavior. No raw
provider body, stderr, prompt or arbitrary error message is saved. Existing
configuration-version changes resume paused work after the operator fixes it.

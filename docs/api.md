# Integration API

This is the supported HTTP surface for connecting Centaur to Context. It is a
small integration contract, not a reference for the human UI's internal API.
All paths are versioned under `/api/v2`.

## Authentication

| Surface | Base URL | Credential | Additional headers |
| --- | --- | --- | --- |
| Agent | `http://centaur-context:8081` | `AGENT_API_TOKEN` | `X-Centaur-Principal-Id`, `X-Centaur-Thread-Key`; optional `X-Centaur-Execution-Id` |
| Note/Task writer | `http://centaur-context-note-write:8084` | `NOTE_WRITE_API_TOKEN` | Same agent headers; `Idempotency-Key` on every write |
| Slack ingestion | `http://centaur-context:8082` | `CHAT_INGEST_API_TOKEN` | None |
| Source intake | `http://centaur-context-source-intake:8086` | `SOURCE_INTAKE_API_TOKEN` | The configured `X-Centaur-Principal-Id`, `X-Centaur-Thread-Key`; optional `X-Centaur-Execution-Id` |
| Networking mutation | `http://centaur-context-networking-mutation:8089` | `NETWORKING_MUTATION_API_TOKEN` | The exact configured `X-Centaur-Principal-Id`, a canonical `X-Centaur-Thread-Key`; optional `X-Centaur-Execution-Id`; `Idempotency-Key` on creates |

For verified Slack Chat requests, the thread header accepts
`slack:workspace:channel:thread` and `slack:workspace:bot-name:channel:thread`.
The optional `bot-` routing namespace identifies the agent session, not a
separate conversation. Both forms must match the stored Chat's workspace,
channel and thread; authenticated principal attribution is unchanged.


Send credentials as `Authorization: Bearer <token>`. Tokens belong in Centaur's
trusted transport, never in an agent sandbox. JSON successes use
`{"data": ...}`; JSON errors use
`{"error":{"code":"...","message":"..."}}`. IDs are UUIDs. A repeated
idempotent write must reuse its original key and identical body.

The normal agent path is `context_search`, `context_read`, and `context_apply`
on port `8081`. All three use `AGENT_API_TOKEN`; the Python client's
`CENTAUR_CONTEXT_API_TOKEN` tool secret is supplied through Centaur's credential
proxy. Apply requires an `Idempotency-Key` header matching the body field
`idempotency_key`; the client supplies it automatically. The optional execution
header is accepted by the API but is not automatically added by this Python
client. Configure the service URL and proxy host together as described in
[tool setup](setup.md#optional-agent-tools).

Universal writes cover all eight Object kinds and explained Connections under
the [Context contract](context-contract.md). Explicit corrections can edit
protected and imported records while preserving original evidence. The
separate Note/Task writer and purpose-bound workflow listeners remain available
for existing specialist callers; their credentials do not replace agent
authentication or grant general maintenance authority.

## Endpoints

The optional private Codex listener (loopback port 8090 by default) exposes
`POST /api/v2/codex/capture`, `GET /api/v2/codex/session`, and the universal
contract/search/read/apply routes. It requires its separate capture or tool
credential plus `X-Codex-Session-Id` and `X-Codex-Repository`; configuration fixes
the host and canonical human identity. No existing service token grants access.
See [Codex desktop integration](codex.md). It is disabled unless configured.

The request column lists body fields unless it says `query` or `path`.

| Surface | Method | Path | Request or purpose |
| --- | --- | --- | --- |
| Agent | GET | `/api/v2/contract` | Read the cacheable complete Context contract. |
| Agent | POST | `/api/v2/search` | required `query`; optional `object_types`, `limit`, `lexical_only` |
| Agent | POST | `/api/v2/read` | required `object_ids`; optional `include` values: `connections`, `artifacts`, `events`, `messages`; optional bounded `artifact_windows` |
| Agent | POST | `/api/v2/apply` | Atomic write batch; required matching `Idempotency-Key`, `contract_version`, `idempotency_key`, and `operations` |
| Agent | GET | `/api/v2/context` | query: required `q`, `chat_object_id`; optional `kind`, `limit` (1–10) |
| Agent | GET | `/api/v2/search/objects` | query: required `q`; optional `kind`, `limit` (1–100), `lexical_only` |
| Agent | GET | `/api/v2/objects/{id}` | Read one Object with subtype data. |
| Agent | GET | `/api/v2/objects/{id}/artifacts` | List an Object's Artifacts. |
| Agent | GET | `/api/v2/artifacts/{id}/content` | query: optional `offset`, `limit` (1–20,000) |
| Agent | GET | `/api/v2/embeddings/status` | Read indexing readiness; never returns credentials or vectors. |
| Agent | GET | `/api/v2/sources` | query: optional `q`, `source_kind`, `created_after`, `created_through`, `cursor`, `limit`, `sort` |
| Agent | GET | `/api/v2/search/sources` | Same as Sources, but `q` is required. |
| Agent | GET | `/api/v2/sources/{id}` | Read one Source. |
| Agent | GET | `/api/v2/sources/{id}/content` | query: optional `artifact_id`, `offset`, `limit` (1–20,000) |
| Agent | GET | `/api/v2/search/notes` | query: required `q`; optional `cursor`, `limit`, `sort` |
| Agent | GET | `/api/v2/notes/{id}` | Read one Note. |
| Agent | GET | `/api/v2/themes` | query: optional `slug`, `cursor`, `limit`, `sort` |
| Agent | GET | `/api/v2/themes/{id}` | Read one Theme. |
| Agent | GET | `/api/v2/themes/{id}/objects` | query: optional `kind`, `limit` |
| Agent | POST | `/api/v2/theme-assignments` | required `object_id`, `theme_id`, `description`; optional `provenance`, `protected` |
| Agent | POST | `/api/v2/theme-assignments/{id}/archive` | required `expected_revision` |
| Note/Task writer | POST | `/api/v2/notes` | required `title`, `description`, `content`, `intent`; optional format, provenance, Source/Note links, and Excerpt evidence |
| Note/Task writer | POST | `/api/v2/objects/{id}/artifacts` | append immutable supporting material; required `kind`, content or URI, capture outcome, and `Idempotency-Key` |
| Note/Task writer | POST | `/api/v2/tasks` | required `title`, `description`, active User `owner_object_id`, RFC3339 `due_at`; `work_kind: "code"` also requires `github_issue_url`; optional status, priority, brief and source links |
| Note/Task writer | PATCH | `/api/v2/tasks/{id}` | required `expected_revision`; include only fields to change |
| Slack ingestion | POST | `/api/v2/ingest/slack/interactions` | Slack surface and thread IDs, messages, interaction state and Run metadata |
| Slack ingestion | POST | `/api/v2/ingest/runs/usage` | Normalized model-usage record for an existing Run |
| Source intake | POST | `/api/v2/source-intake/resolve-connections` | required `queries` array (at most 16) |
| Source intake | POST | `/api/v2/source-intake/validate` | Validate a Source manifest without writing. |
| Source intake | POST | `/api/v2/source-intake/commit` | Commit or replay the same Source manifest atomically. |
| Source intake | POST | `/api/v2/source-intake/status` | Read readiness using the same Source manifest. |
| Source intake | POST | `/api/v2/source-intake/runs/start` | Start an attributed workflow Run. |
| Source intake | POST | `/api/v2/source-intake/runs/{id}/trace` | Append one workflow trace entry. |
| Source intake | POST | `/api/v2/source-intake/runs/{id}/finish` | Finish a workflow Run. |

Theme-assignment writes also require `Idempotency-Key`. Source-intake
`validate`, `commit`, and `status` accept the same strict body: required
`version`, `idempotency_key`, and `source`; optional `connections` and
`originating_chat_object_id`. The Source requires `title`, `description`,
`source_kind`, `content_kind`, `content`, `content_sha256`,
`content_size_bytes`, and `capture_outcome`. Use the
[standard Python client](../tools/centaur_context/client.py) for Source intake,
Slack payloads, and workflow traces instead of constructing large bodies by hand.

For Slack interactions, the required top-level fields are `workspace_id`,
`channel_id`, `thread_id`, `surface_kind`, `messages`, and `run`. Each message
has `provider_message_id`, `sender`, `content`, and `source_created_at`; each
sender has `provider_user_id`, `display_name`, and `user_kind`. A normalized
usage record requires `run_id`, `component`, `provider`, `model_id`,
`execution_type`, `auth_mode`, `upstream_service`, `billing_mode`,
`source_execution_id`, and `usage_status`.

Source workflow Runs use these required fields: start takes `run_id`,
`workflow_name`, and `source_kind`; trace takes `id`, `entry_type`, `name`,
`status`, start/completion timestamps, `duration_ms`, and `component`; finish
takes `status`. Other workflow fields are optional.

Normal Object discovery through `/api/v2/search`, `/api/v2/search/objects`, and
`/api/v2/context` searches canonical Object titles and descriptions. Hybrid
search combines that full-text index with semantic embeddings of Object kind,
title, and description. Captured Artifact bodies remain available through
explicit Object, Source, and Artifact reads, and their stored embeddings are
preserved. Neither Artifact body text nor chunk vectors contribute candidates,
ranking, or evidence excerpts to Object discovery.

## Minimal examples

The public Python package's three universal record commands are `context_search`,
`context_read`, and `context_apply`. For example:

```bash
context_search 'deployment decision' --object-type task --limit 10
context_read 00000000-0000-0000-0000-000000000001 --include connections
context_apply --file apply-request.json
```

An apply file includes a stable batch-local name for each new Object so later
operations can connect it without knowing its database ID. Object descriptions
are current snapshots of at most 600 Unicode characters after trimming. They
say what the Object is and why it matters in the current Context; Events and
Runs retain change history.

Replace the illustrative User and deployment Object UUIDs below with verified
active records, and choose the actual agreed due date. This is a general review
Task. For code work, use `work_kind: "code"` and supply the existing canonical
`https://github.com/owner/repo/issues/number` in `github_issue_url`.

```json
{
  "contract_version": "1.1.0",
  "idempotency_key": "editor-task-20260921-1",
  "operations": [
    {
      "operation": "create_object",
      "local_ref": "task",
      "kind": "task",
      "title": "Review deployment",
      "description": "Review the proposed deployment and record the decision. This Task keeps the release approval explicit and reviewable.",
      "fields": {
        "status": "todo",
        "priority": "medium",
        "work_kind": "general",
        "owner_object_id": "00000000-0000-0000-0000-000000000002",
        "due_at": "2026-10-09T17:00:00Z",
        "brief_markdown": "Review the proposed deployment. Acceptance: record the approval decision and evidence. Next action: read the linked deployment record."
      }
    },
    {
      "operation": "create_connection",
      "source": {"local_ref": "task"},
      "kind": "related_to",
      "target": {"object_id": "00000000-0000-0000-0000-000000000001"},
      "description": "The Task reviews this existing deployment record."
    }
  ]
}
```

The batch creates its Run, immutable Events, Object subtype, and Connection in
one transaction. A failed operation rolls back the entire batch. An exact retry
returns the stored response; the same key with a different body is rejected.
Use `update_object` with `expected_revision` to refresh a materially stale
title or description. Put a known material change and its new description in
the same batch; do not append dated status updates to the description.

Task creation rules apply to both universal and separate writer endpoints.
Every new Task needs an active canonical User assignee, distinct from creator
attribution. Agent/system creation also needs `due_at`. Human creation may omit
the date; code Tasks still need an Issue regardless of actor. Entering `doing`
requires a nonempty brief, an active assignee, and (for agents) a due date.
Once set, the due date cannot be cleared. Legacy Tasks with missing fields stay
readable and permit unrelated updates, but assignment changes and execution must
satisfy the current checks; do not invent missing values. See
[Actionable Tasks](context-contract.md#actionable-tasks) for claims and execution.

Retrieve context for the authenticated thread:

```bash
curl --get 'http://centaur-context:8081/api/v2/context' \
  --header 'Authorization: Bearer <agent-token>' \
  --header 'X-Centaur-Principal-Id: <principal-id>' \
  --header 'X-Centaur-Thread-Key: <provider:workspace:channel:thread>' \
  --data-urlencode 'q=What decisions affect this work?' \
  --data-urlencode 'chat_object_id=00000000-0000-0000-0000-000000000001' \
  --data-urlencode 'limit=10'
```

A response is one bounded packet with two distinct sections. `objects` contains
the existing query-relevant results. `general_context_objects` contains up to
ten different active Objects ranked by active-Connection count for broad
orientation. Query-relevant Objects are excluded from the general section and
receive priority when the 12,000-character packet budget requires omission.

```json
{
  "data": {
    "query": "What decisions affect this work?",
    "retrieval": "full_text",
    "objects": [],
    "general_context_objects": [],
    "budget": {
      "max_characters": 12000,
      "serialized_characters": 187,
      "omitted_objects": 0,
      "omitted_connections": 0
    }
  }
}
```

For callers retaining the separate Note writer, create a Fact as follows
(normal interactive agents use `context_apply`):

```bash
curl 'http://centaur-context-note-write:8084/api/v2/notes' \
  --header 'Authorization: Bearer <note-write-token>' \
  --header 'X-Centaur-Principal-Id: <principal-id>' \
  --header 'X-Centaur-Thread-Key: <provider:workspace:channel:thread>' \
  --header 'Idempotency-Key: <stable-operation-id>' \
  --header 'Content-Type: application/json' \
  --data '{"title":"Decision","description":"Records the approved deployment decision and why it was selected.","content":"Use the private service endpoint.","intent":"fact","content_format":"markdown"}'
```

Creation returns HTTP `201` with the canonical Note under `data`, including its
`object_id`, `revision`, intent, evidence fields, content, provenance, and
timestamps. Excerpts additionally require exactly one
`derived_from_source_object_ids` entry, its `source_artifact_id`, and a
`source_locator` object whose kind is `timestamp`, `page`, `section`, or
`text_offset`; the submitted Excerpt content must occur verbatim in that
Artifact's captured text. Idea and Fact Notes may include
`derived_from_note_object_ids` so their Note relationships are committed in
the same idempotent operation.

New Notes accept only `idea`, `excerpt`, and `fact`. Legacy `insight` and
`question` values remain stored/readable and accepted on updates; they are not
aliases for the current choices. See the [intent mapping](schema.md#note-intents-and-compatibility)
for unclassified Notes and the universal creation default.

## Internal surfaces

The human UI API, Curator, bootstrap intake, research mutation, and external
actions are private administrative or purpose-bound interfaces. They are not a
general agent API and are intentionally outside this POC reference.
`GET /api/v2/schema` reports the database schema; it is not an OpenAPI or HTTP
endpoint specification.

### Networking mutation workflow contract

The optional networking-mutation listener is also purpose-bound rather than a
general agent API. It starts only when both
`NETWORKING_MUTATION_API_TOKEN` and
`NETWORKING_MUTATION_ALLOWED_PRINCIPAL` are set. Its address defaults to
`0.0.0.0:8089` and can be changed with `NETWORKING_MUTATION_ADDR`. The token
must differ from every other Context credential. Every request, including
health checks, requires that bearer token, the exact configured principal, and
a canonical four-part `provider:workspace:channel:thread` thread key.

The listener exposes only these routes:

| Surface | Method | Path | Exact purpose |
| --- | --- | --- | --- |
| Networking mutation | GET | `/api/v2/objects?q=<title>&kind=entity&limit=10&sort=recent` | Return up to ten active Entity candidates; `kind=entity` and `sort=recent` are mandatory. |
| Networking mutation | GET | `/api/v2/objects/{id}` | Return one active canonical Entity, including `entity_kind`; non-Entities are not visible. |
| Networking mutation | POST | `/api/v2/objects` | Create or replay one Entity from exactly `kind`, `title`, `description`, `entity_kind`, and `provenance`; `kind` must be `entity`. |
| Networking mutation | POST | `/api/v2/connections` | Create or reuse one unprotected Connection from exactly the two Object IDs, `kind`, `description`, `provenance`, and `protected: false`. |
| Networking mutation | POST | `/api/v2/tasks` | Create or replay one assigned general `todo` Task from exactly `title`, `description`, `status`, `priority`, `owner_object_id`, RFC3339 `due_at`, nonempty `brief_markdown`, `agent_suitable: false`, `provenance`, and an empty `derived_from_source_object_ids` array. |

Unknown JSON and query fields fail. Other methods and paths do not exist on
this listener. Entity kinds are limited to `person`, `organization`, `product`,
`project`, `publication`, `place`, `concept`, and `other`; Connection kinds and
Task priorities use the canonical allowlists. Create calls require a stable
`Idempotency-Key` of at most 200 characters and return the canonical Entity,
Connection, or Task record with recorded provenance. Provenance must include a
non-empty `source_type`. A durable caller must reuse both the key and body on
retry.

This workflow requires an active canonical User assignee and fixes
`work_kind=general`; it does not accept code-Task fields. Its exact replay checks
include the assignee, due date, and brief.

Duplicate resolution deliberately remains fail-closed in the calling workflow:
search returns every candidate and never chooses one. The workflow may reuse
one unambiguous exact match, must pause when multiple exact matches remain, and
may call Entity creation only when no match remains. An approved Task and its
Entity link use separate stable Task and Connection keys so a partial failure
can safely resume without duplicating either record.

Owner-reviewed operational maintenance is available on the optional
intake listener with a separate maintenance credential and configured principal.
See [reviewed maintenance](operations.md#reviewed-maintenance-on-the-intake-listener)
for inventory, readback, exact-request approval and `/api/v2/maintenance/apply`.
These routes are not part of the interactive-agent tool contract.

## Task Routines

| Surface | Method | Path | Purpose |
| --- | --- | --- | --- |
| Agent | GET | `/api/v2/tasks/{id}/routine` | Read schedule and last 50 occurrences. |
| Agent | PUT | `/api/v2/tasks/{id}/routine` | Configure with expected_revision, schedule, enabled and confirmed. |
| Agent | PATCH | `/api/v2/routine-runs/{id}` | Bound execution thread reports result. |
| Slack ingestion | POST | `/api/v2/ingest/routines/claim` | Trusted dispatcher claims due occurrences atomically. |
| Slack ingestion | GET | `/api/v2/ingest/routine-runs/{id}` | Read occurrence and current eligibility. |
| Slack ingestion | PATCH | `/api/v2/ingest/routine-runs/{id}` | Bind execution thread/link or finish an occurrence. |

The human API supports the same configuration/read/result paths and can accept a Review result. Agent authentication cannot access ingestion paths. Routine configuration requires an expected task revision and increments it, with an Object Event; protection does not block explicit agent configuration. Enabling requires explicit confirmation, an active Ready/Review task, an owner, a review date and an execution brief.

Schedules contain `timezone` and either `every_minutes` (1–525600), or `local_time` (`HH:MM`) with `weekdays` (Monday 1 through Sunday 7). PostgreSQL timezone rules apply: a missing local time shifts by the DST gap; an ambiguous time uses standard time. Claiming advances to the next future occurrence and skips catch-up. The unique active occurrence prevents overlapping work. Task edits pause its schedule. Due dates are unrelated to scheduling.

Occurrence statuses are pending, running, completed (uneventful success), review (substantive output), blocked and skipped. Execution results belong to the occurrence, not the parent Task. Only its bound thread can report a result, with at most 20000 bytes of text. A blocked result needs a reason. Runtime fallback to Review never overwrites an explicit terminal result.


The authenticated maintenance listener also exposes `POST /api/v2/maintenance/memory-review` for exact saved Memory-preview validation/application. It reuses server-configured approval hashes, accepts only a preview Run ID and explicit `validate_only`, and never re-infers. See [Memory review](memory.md#apply-an-exact-reviewed-preview).

## Current-state and historical corrections

The agent and human listeners expose `/api/v2/read` and `/api/v2/apply` with the
same correction model. The authenticated principal supplies attribution; domain
User metadata never grants credentials. `restore_object`, `restore_connection`,
`promote_source_artifact`, `correct_evidence` and `rebuild_derived` require the
current owning revision. `reassign_identity` moves an existing provider binding
between Users, requiring both revisions; original messages retain their sender.
It does not replace an authenticated Codex principal or configured human identity.
`reassign_chat` transfers an imported routing identity to a new active Chat with
no imported identity, also requiring both revisions. Existing messages, Runs and
processing cursors stay with the original Chat; future captures use the new one.
Callers must refresh their verified Chat ID after reassignment. Original routing
metadata remains in immutable Events and correction assertions.

`correct_evidence` takes `object_id`, `expected_revision`, `target_type`
(`object`, `artifact`, `message`, `event`, `run`), `target_id`, `reason`, and an
object-valued `representation`. It annotates rather than mutates original evidence.
Normal read results return all corrections; search returns latest assertions,
and message/Artifact content reads expose the latest assertion alongside original
text. Empty representation withdraws an earlier assertion without deleting it.
Derived search text and Object embedding hashes track latest assertions.
Canonical promotion reports `retained_citation_note_ids`; these Notes keep their
exact retained evidence until explicitly corrected. A changed Excerpt citation
must still match verbatim and agree with active derivation Connections.

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

Send credentials as `Authorization: Bearer <token>`. Tokens belong in Centaur's
trusted transport, never in an agent sandbox. JSON successes use
`{"data": ...}`; JSON errors use
`{"error":{"code":"...","message":"..."}}`. IDs are UUIDs. A repeated
idempotent write must reuse its original key and identical body.

## Endpoints

The request column lists body fields unless it says `query` or `path`.

| Surface | Method | Path | Request or purpose |
| --- | --- | --- | --- |
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
| Note/Task writer | POST | `/api/v2/tasks` | required `title`, `description`; optional status, priority, owner, due date and source links |
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

## Minimal examples

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

A response is a bounded search packet:

```json
{
  "data": {
    "query": "What decisions affect this work?",
    "retrieval": "full_text",
    "objects": [],
    "budget": {
      "max_characters": 12000,
      "serialized_characters": 187,
      "omitted_objects": 0,
      "omitted_connections": 0
    }
  }
}
```

Create a Note through the separate writer:

```bash
curl 'http://centaur-context-note-write:8084/api/v2/notes' \
  --header 'Authorization: Bearer <note-write-token>' \
  --header 'X-Centaur-Principal-Id: <principal-id>' \
  --header 'X-Centaur-Thread-Key: <provider:workspace:channel:thread>' \
  --header 'Idempotency-Key: <stable-operation-id>' \
  --header 'Content-Type: application/json' \
  --data '{"title":"Decision","description":"Records the approved deployment decision and why it was selected.","content":"Use the private service endpoint.","intent":"insight","content_format":"markdown"}'
```

Creation returns HTTP `201` with the canonical Note under `data`, including its
`object_id`, `revision`, intent, evidence fields, content, provenance, and
timestamps. Excerpts additionally require exactly one
`derived_from_source_object_ids` entry, its `source_artifact_id`, and a
`source_locator` object whose kind is `timestamp`, `page`, `section`, or
`text_offset`; the submitted Excerpt content must occur verbatim in that
Artifact's captured text. Insights and Questions may include
`derived_from_note_object_ids` so their Note relationships are committed in
the same idempotent operation.

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
| Networking mutation | POST | `/api/v2/tasks` | Create or replay one unassigned `todo` Task from exactly `title`, `description`, `status`, `priority`, `agent_suitable: false`, `provenance`, and an empty `derived_from_source_object_ids` array. |

Unknown JSON and query fields fail. Other methods and paths do not exist on
this listener. Entity kinds are limited to `person`, `organization`, `product`,
`project`, `publication`, `place`, `concept`, and `other`; Connection kinds and
Task priorities use the canonical allowlists. Create calls require a stable
`Idempotency-Key` of at most 200 characters and return the canonical Entity,
Connection, or Task record with recorded provenance. Provenance must include a
non-empty `source_type`. A durable caller must reuse both the key and body on
retry.

Duplicate resolution deliberately remains fail-closed in the calling workflow:
search returns every candidate and never chooses one. The workflow may reuse
one unambiguous exact match, must pause when multiple exact matches remain, and
may call Entity creation only when no match remains. An approved Task and its
Entity link use separate stable Task and Connection keys so a partial failure
can safely resume without duplicating either record.

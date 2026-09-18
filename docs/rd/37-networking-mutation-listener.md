# RD: purpose-bound Entity networking mutation listener

GitHub Issue: [#37](https://github.com/bradwmorris/centaur-context/issues/37)

Linked Context Task: `0fcede02-c759-47d4-84f5-7a1818e95102` (revision 6 when
this decision was implemented)

Status: accepted for implementation

## Decision

Add one optional, independently revocable authenticated listener for a single
configured workflow principal. Keep the implementation organization-neutral;
an overlay selects its principal and credential at deployment time. The
listener can search and read active Entity candidates, create or replay one
supported Entity, create or reuse one unprotected Connection, and create or
replay one constrained follow-up Task. It exposes no updates, archives, Notes,
Sources, Themes, Runs, human routes, generic Object reads, or database access.

Use port 8089 and the `NETWORKING_MUTATION_ADDR`,
`NETWORKING_MUTATION_API_TOKEN`, and
`NETWORKING_MUTATION_ALLOWED_PRINCIPAL` settings. The token must be unique
among Context listeners. Authentication requires a bearer token compared in
constant time, the exact configured principal, and a canonical four-part
thread key. Creates also require a bounded idempotency key. Actor identity and
allowlisted provenance flow into the existing immutable mutation Run and Object
Event records.

The route and body contract is specified in [the API guide](../api.md#networking-mutation-workflow-contract).
Strict request structs reject unknown fields. Entity reads hide every other
Object kind. Tasks are limited to unassigned, non-agent-suitable `todo` items
without implicit Source links. Connections cannot be protected. The caller
must use a second, independently idempotent Connection call to link an approved
Task to its Entity.

## Ambiguity and approval boundary

Context returns all matching Entity candidates; it never guesses which one is
canonical. The durable workflow compares exact candidates, reuses one unique
match, stops on ambiguity, and creates only when no match remains. That workflow
also owns the exact human-approval event and content hash for Connection and
Task proposals. The listener verifies the authenticated workflow identity and
persists the supplied allowlisted provenance, but it does not interpret an
organization-specific approval protocol.

This preserves the public/private boundary: the public core supplies a small
authenticated persistence capability, while persona rules, approval schemas,
workflow receipts, and role assignments remain in the private overlay.

## Deployment decision for the motivating overlay

The Enyu deployment uses principal `workflow-enyu-netz-entity`, its own random
`NETWORKING_MUTATION_API_TOKEN`, and the default port 8089. Only its
workflow-only Context adapter receives the credential. Its credential-proxy
grant permits:

- `GET /api/v2/objects` and `GET /api/v2/objects/*`;
- `POST /api/v2/objects`;
- `POST /api/v2/connections`; and
- `POST /api/v2/tasks`.

The interactive Netz principal and the Editor, Researcher, Dev, Source-intake,
Research-mutation, and Gmail workflow principals receive no listener token or
port grant. A private NetworkPolicy admits port 8089 only from the credential
proxy/workflow path. No database DSN enters either sandbox.

## Rejected alternatives

- The human listener is too broad and has no workflow-principal boundary.
- The read-only agent listener must remain read-only.
- Sharing the Research-mutation or Note/Task credential would cross persona and
  purpose boundaries.
- Direct PostgreSQL access would bypass authentication, validation,
  idempotency, provenance, and immutable events.
- Hard-coding the Enyu principal in the public core would violate the reusable,
  organization-neutral product boundary.

## Verification and rollback

Tests cover the exact token/principal/thread boundary, positive Entity search
and read, Entity/Connection/Task creation, replay, ambiguous candidate return,
unknown fields, unsupported methods and paths, and cross-role denial. The full
repository checks and database-backed suite must pass against a disposable
database whose name contains `centaur_context_test`.

Rollback removes the overlay's port 8089 network grant and the three
`NETWORKING_MUTATION_*` settings, then redeploys. Existing canonical records and
immutable events remain; rollback does not delete them.

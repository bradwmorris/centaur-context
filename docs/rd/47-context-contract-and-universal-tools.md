# RD: one Context contract and three universal agent tools

GitHub Issue: [#47](https://github.com/bradwmorris/centaur-context/issues/47)

Status: approved by the repository owner on 2026-09-21 for implementation of
Issue #47 within the boundaries below.

## Plain-English decision

Centaur Context currently gives agents one `centaur-context` command containing
many unrelated subcommands. Reads and writes also pass through several different
HTTP listeners and duplicate rules across Rust, Python, documentation, and the
database. This is difficult to understand and easy to let drift.

Replace that agent interface with exactly three base tools:

- `context_search`: find Objects;
- `context_read`: read complete Objects and nearby information; and
- `context_apply`: create, update, archive, and connect ordinary Objects.

These tools live in the public `centaur-context` repository. Every normal
interactive agent receives all three with the same ordinary read/write scope.
The private overlay contains only agent-specific tools and workflows. For
example, Reza's full Source-ingestion workflow belongs in the overlay, but Reza
uses the same three base Context tools as every other agent.

Agents never connect to PostgreSQL. The three Python tools call Context's
authenticated HTTP API. Centaur's credential proxy supplies the real bearer
token without exposing it to the sandbox. Context validates every request and
performs the database work.

Put the shared Context rules in one version-controlled JSON file in this
repository. This file is the “Context contract.” It is not Markdown and is not
an AI-written prompt. Ordinary deterministic code uses it to keep the short
agent explanation, tool inputs, server validation, documentation, and tests in
agreement.

## Boundaries

### Public Centaur Context repository

Owns:

- the Rust HTTP service and database persistence;
- `contract/context-contract.json` and its validating schema;
- `context_search`, `context_read`, and `context_apply`;
- the short organization-neutral Context explanation for agents; and
- generated documentation and drift checks.

### Private overlay

Owns only deployment- or agent-specific additions, including:

- personas and private instructions;
- Reza's Source fetching, transcription, and ingestion workflow;
- private integrations; and
- other specialist tools.

The overlay must not duplicate the three base tools, copy Context domain logic,
or write directly to PostgreSQL.

### Centaur core

Continue using Centaur's existing generic tool discovery. Centaur already scans
tool packages for `[project.scripts]`; no Context-specific tool loader or MCP
server is added. Centaur issue #7 remains responsible for placing the generated
short Context explanation in the trusted instruction sequence.

Centaur's normal sandbox capabilities, including its shell, filesystem access,
Git, and browser support, are separate from the application-tool catalogue and
are unchanged by this RD.

### Coordinated deployment changes

Completing the cutover requires narrowly scoped changes outside this public
repository:

- `centaur-context` adds the three public tools and removes the old
  `centaur-context` command after the smoke cutover;
- the private `centaur-enyu` overlay adds an explicit `tool_allowlist` to each
  persona containing the three Context tools and only the additional tools that
  persona actually needs; and
- the stale `company-context` skill is removed from both Centaur base and the
  private overlay so agents are not instructed to call the removed
  `company_context` command.

These allowlists control which application-tool commands are installed and
shown by `centaur-tools list` inside each agent sandbox. They do not change the
shared Context read/write scope, grant credentials, or affect Centaur's normal
sandbox capabilities.

The generated `centaur-tools` discovery/runner command is Centaur runtime
infrastructure, not an allowlisted application tool. This RD does not make
`centaur-console`, `centaur-skills`, `websearch`, or any other existing Centaur
application tool universal. A persona receives one of those tools only when it
is explicitly named in that persona's allowlist.

## Context contract

Add one public, versioned JSON document at:

```text
contract/context-contract.json
```

Validate it with:

```text
contract/context-contract.schema.json
```

The contract describes:

- Object types and their writable fields;
- Connection kinds and endpoint rules;
- system-owned and immutable fields;
- the three base tool input and output shapes;
- text, array, batch, and expansion limits;
- provenance, revision, and idempotency rules;
- Source record versus canonical captured-content semantics; and
- contract, ontology, and tool versions.

It does not describe physical tables, credentials, private agents, private
workflows, or organization-specific policy.

The contract is the reviewed source of truth. Migrations remain explicit SQL;
the contract does not automatically generate or execute production migrations.
Tests instead prove that Rust validators, PostgreSQL constraints, Python tools,
and generated documentation agree with it.

Expose the complete contract through a cacheable authenticated read-only HTTP
endpoint. Normal prompts receive only a short generated explanation containing
the stable concepts and contract version, not the full JSON document.

## Universal tool package

Keep one shared Python package, but expose three separate Centaur scripts:

```toml
[project.scripts]
context_search = "centaur_context_tool.search:main"
context_read = "centaur_context_tool.read:main"
context_apply = "centaur_context_tool.apply:main"
```

Use small entry modules over shared transport, models, and error handling. Do
not create three duplicated packages. Remove the current large command parser
after the new tools pass the POC cutover checks.

### `context_search`

Accepts a query, optional Object types, contract-defined filters, sort, cursor,
limit, and bounded summary expansions. It returns typed Object summaries,
relevance/evidence information, revisions, and short explained Connections.

It replaces the current type-specific search and list commands.

### `context_read`

Accepts one or more Object IDs plus bounded includes such as subtype fields,
Connections, Artifacts, messages, and audit summaries. It returns canonical
typed Objects. Large Artifact content is returned only through bounded windows.

All normal interactive agents receive the same read scope for active Context
records, including Chats, Users, Memories, Runs, Events, and Artifacts. Reading
does not imply permission to mutate system-owned records. Existing deployment
or surface visibility rules still apply.

### `context_apply`

Accepts one bounded atomic batch containing explicit operations. Version one
supports:

- create, update, and archive Task, Entity, Source, Note, and Theme Objects;
- create, update, and archive Connections; and
- append immutable supporting Artifacts where the contract permits them.

It does not directly create or mutate Chats, Users, Memories, Object Events, or
Runs. It cannot set service-owned fields or promote an Artifact as a Source's
canonical captured content.

Each call includes a contract version and idempotency key. Updates and archives
include an expected revision. Unknown fields fail rather than being ignored.

## Source decision

Any normal interactive agent may create, update, or archive an ordinary Source
record through `context_apply`. A Source record identifies a work and may hold
metadata such as title, kind, URL, author, publisher, and publication date. It
does not claim that Context captured the work's complete contents.

Full fetching, transcription, capture validation, rigorous deduplication, and
canonical Artifact promotion remain a specialist ingestion workflow. Reza's
private overlay may expose that workflow and its purpose-bound credential. The
workflow may create or enrich the same public Source records, but it must use
the public contract and may not redefine Source semantics.

## Connection decision

There is one Connection operation inside `context_apply`. It can connect any
two active Objects using a contract-defined kind. Context rejects missing or
archived endpoints, self-links, empty explanations, and invalid special cases
such as a `themed` Connection whose target is not a Theme.

Every newly created Object must have at least one meaningful Connection. When a
request carries a verified current Chat identity, Context deterministically
adds the provenance Connection to that Chat in the same transaction. A
non-Chat workflow must supply at least one valid Connection or creation fails.

Keep the current assertion behavior for duplicate Connections: an exact repeat
is a replay; the same source/kind/target with materially different explanation
or provenance adds that assertion to the existing Connection and records an
Event. It does not create a parallel active edge or silently discard evidence.

## Write transaction

One `context_apply` request commits all of the following together:

1. canonical Object rows and exactly one matching subtype per created Object;
2. required explained Connections;
3. permitted immutable supporting Artifacts;
4. one reviewable mutation Run; and
5. ordered immutable Object Events.

If any operation fails, nothing from the batch is committed. Responses are
built from the committing transaction so they represent the exact committed
revision.

Bind the authenticated principal, idempotency key, and normalized request hash.
An exact retry returns the stored result. Reusing the key with a different body
returns a conflict. References may point to Objects created earlier in the same
batch without exposing temporary database IDs.

New `context_apply` keys use a separate operation namespace, so historical
Note/Task keys do not produce false conflicts. Legacy endpoints keep their
existing replay behavior only during the short cutover and are then removed.

## HTTP and authentication

Add these authenticated agent-facing HTTP operations on the normal Context
agent service:

- `POST /api/v2/search` for `context_search`;
- `POST /api/v2/read` for `context_read`;
- `POST /api/v2/apply` for `context_apply`; and
- `GET /api/v2/contract` for the cacheable read-only contract document.

All normal interactive agents use one standard Context agent credential and
the same base scope. Do not build per-agent, per-type, per-field, or
per-individual-record grants for this POC.

The service still rejects protected writes and system-owned record classes.
The Source-ingestion, Curator, Slack-ingestion, and other internal workflows may
retain distinct purpose-bound credentials because they can perform operations
the three base tools cannot.

No sandbox receives a bearer token or database DSN. No public ingress is added.
MCP is not part of this design.

Private ontology-extension mechanics remain in issue #9. Version one of this
issue implements the public base contract and three tools; a private overlay
may add specialist tools and instructions but does not modify the base Object
model through this issue.

## Prompt and tool exposure

At sandbox startup, Centaur installs for the selected persona:

- `context_search`, `context_read`, and `context_apply`;
- the built-in `centaur-tools` discovery/runner bridge; and
- only the additional public or private tools explicitly required by that
  persona.

The agent discovers these application tools lazily through Centaur's existing
tool-discovery surface. The complete installed application-tool catalogue is
not pasted into the system prompt or serialized into every model invocation.
On every model invocation, Centaur supplies only the short stable Context
explanation as trusted application instructions, together with the selected
persona and relevant runtime input.

The full JSON contract is not pasted into every prompt. Retrieved Context
records remain clearly labelled untrusted reference data. Tool availability and
prompt text never bypass server validation.

Changing how the eval UI displays captured catalogue telemetry is explicitly
outside this RD. It may be handled in a separate issue and is not required for
this implementation or cutover.

## Compatibility and rollout

Use a short POC rollout rather than a long parallel system:

1. Approve this RD.
2. Add the JSON contract and drift checks without changing live behavior.
3. Add the shared Rust search, read, and apply services and authenticated HTTP
   operations.
4. Add the three Python entry points while temporarily retaining the old
   `centaur-context` command as a compatibility shim.
5. Add explicit tool allowlists to the private Editor, Researcher, Dev, and Netz
   personas. Each allowlist includes the three Context tools and only the
   additional public or private tools required by that persona's documented
   responsibilities.
6. Remove the stale `company-context` skills from Centaur base and the private
   overlay.
7. Point the POC tool catalogue at the reviewed commits and verify that each
   persona's `centaur-tools list` contains its intended tools and excludes the
   unrelated catalogue.
8. Run one end-to-end smoke flow for search, read, each writable Object type,
   Connections, replay, rejection, and Reza's ingestion handoff.
9. After the smoke flow passes and no old-command use remains, remove the old
   command and its redundant listeners/routes where they have no internal
   workflow consumer.

Rollback pins the previous Context image and tool commit. The change does not
delete existing Context data. Contract major-version mismatch fails closed.
During the POC, the server needs to support only its current contract major
version; rollback uses the previous application version rather than running
multiple major contracts simultaneously.

## Efficient verification and data safety

The implementation must be tested end to end without repeatedly copying or
rebuilding databases.

### Test order

Run checks in this order and stop on the first failure:

1. contract/schema validation, generated-file drift checks, Python compile, and
   targeted unit tests;
2. Rust formatting, linting, and targeted non-database tests;
3. one database-backed integration suite; and
4. one end-to-end three-tool POC smoke flow.

Do not repeatedly run the full suite after every small edit. Run targeted tests
while developing, then run the documented complete checks once before the
implementation PR is submitted.

### One disposable test database

Database-backed tests use exactly one disposable database for the complete
suite. Its name must contain `centaur_context_test`. Create it once, apply all
migrations once, isolate test cases with transactions or explicit cleanup, and
drop it once when the suite finishes.

Do not create a database per test. Do not clone the live database. Do not point
tests at the live Context or Centaur databases.

### POC rollout

Before applying any migration to the live POC database:

- take and verify one normal recoverable backup;
- run a migration preflight that reports intended schema changes;
- prefer additive forward migrations; and
- stop if a migration would delete or rewrite existing user data.

After deployment, run one bounded smoke flow against the POC. Do not seed
duplicate copies of existing data merely to exercise the tools. Use clearly
identified temporary test Objects and archive them after verification; do not
delete or rewrite existing user records.

Keep command output and test reporting summarized. Preserve detailed logs as
artifacts only when a failure needs diagnosis. This limits both execution cost
and model-token use without reducing meaningful coverage.

## Observability

Each relevant Run records the contract/tool version and hash, operation,
request hash, replay status, affected Object and Connection IDs, validation
outcome, and parent workflow attribution. Never record credentials or
unbounded private content.

Measure validation failures, authentication failures, transaction rollbacks,
idempotency replay/conflict, optimistic revision conflicts, attempted
unconnected creation, tool latency, and legacy-command use. These measurements
support the short compatibility cutover and removal decision.

## Acceptance checks

- [ ] The owner approves this RD before implementation begins.
- [ ] The public repository contains one validated, versioned JSON Context
      contract and deterministic drift checks.
- [ ] Every normal interactive agent receives exactly `context_search`,
      `context_read`, and `context_apply` as the public base tools.
- [ ] Editor, Researcher, Dev, and Netz each have an explicit tool allowlist
      containing the three Context tools and only the additional public or
      private tools required by their documented responsibilities.
- [ ] `centaur-tools list` in each persona sandbox excludes unrelated public and
      private application tools.
- [ ] The full application-tool catalogue is not added to the system prompt;
      agents discover their installed allowlisted tools lazily.
- [ ] Agent-specific tools and workflows remain in the private overlay, and
      Centaur's normal shell, filesystem, Git, and browser capabilities remain
      separate and unchanged.
- [ ] The stale `company-context` skills are removed from Centaur base and the
      private overlay.
- [ ] The three tools share one small Python transport/model library; the old
      bloated command is removed after the bounded POC cutover.
- [ ] `context_apply` supports Task, Entity, Source, Note, Theme, Connection,
      and permitted supporting-Artifact mutations atomically.
- [ ] Every created Object has a Connection; verified Chat context creates its
      provenance Connection deterministically.
- [ ] Direct Source creation does not claim canonical captured content; only
      the specialist ingestion path may promote that content.
- [ ] Chat, User, Memory, Event, Run, protected, and service-owned mutations are
      rejected through the base tools.
- [ ] Request-hash-bound idempotency, optimistic revision checks, exact commit
      responses, and all-or-nothing batches are tested.
- [ ] Existing data is preserved and rollback uses the previously pinned image
      and tool revision.
- [ ] Database integration testing creates one guarded disposable database for
      the suite, never clones production data, and never tests against live
      databases.
- [ ] The documented repository checks pass before the implementation PR is
      merged.

## Owner approval boundary

Approval of this RD authorizes implementation of Issue #47 within these
boundaries. It does not authorize public ingress, direct sandbox database
access, MCP, organization-specific behavior in the public core, destructive
data migration, unrelated Centaur-core changes, or eval UI changes. The only
authorized cross-repository changes are the persona allowlists and stale-skill
cleanup described above, plus the existing Centaur issue #7 instruction hook.

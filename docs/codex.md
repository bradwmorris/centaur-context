# Codex desktop and Context

The opt-in local bridge stores new visible Codex conversation text as canonical
Chats and sends bounded windows to the existing Memory-only curator. It also
exposes exactly `context_search`, `context_read`, and `context_apply` through MCP.
Agent execution continues in Codex. The automatic path does not create ordinary
Tasks, Notes, Entities, Sources or Themes.

## Compatibility and ownership

Initial verified protocol: the bundled Codex 0.153.4 app-server. MCP calls supply
`_meta.threadId`; server processes do not receive a thread environment variable.
The bridge uses a hook-created session binding and never infers a destination
from an interactive tool argument. Runtime/desktop acceptance evidence is tracked
in [RD 83](rd/83-codex-desktop-context-bridge.md).

The versioned transcript adapter reads only completed `UserMessage` and
`AgentMessage` items in paginated history, belonging to the exact current thread.
It starts at activation, excludes inherited/old conversation content, and omits
system/developer instructions, reasoning and tool outputs. Only text is supported;
attachments are reported as omitted. Unsupported formats, changed transcript
identity and oversized messages preserve the cursor and report an error.
Explicit credential-pattern redaction is defense in depth, not a guarantee that
all possible private information is detectable. Configure capture only for the
intended private destination and trusted desktop account.

Core owns the reusable API/client integration. Private overlays own repository
routing, endpoints, credentials, canonical human identity, transport and inference
settings. Two installations use separate credentials and databases. Shared-core
or mixed-domain work requires an explicit destination; there is no broadcast or
semantic guessing. A session's destination is immutable once capture starts. Global hooks quietly skip
new unmapped projects; loss of an existing session's route reports an error.

## Server configuration

Set the optional variables described in `.env.example`: distinct
`CODEX_CAPTURE_API_TOKEN` and `CODEX_TOOL_API_TOKEN`, `CODEX_HOST_ID`, an existing
active `CODEX_HUMAN_USER_ID`, exact `CODEX_REPOSITORIES` aliases, and optionally
`CODEX_CURATE=true`. Default bind is `127.0.0.1:8090`. Private container deployment
may bind its private interface and use an existing authenticated local tunnel.
Do not expose this listener publicly.

`CODEX_CURATE=true` requires configured Curator model transport. When
`CURATOR_ALLOWED_PROVIDERS` is set, it must include `codex`. An unset provider selection preserves existing behavior; a
personal installation can select only `codex` without starting Slack curation.
Memory dreaming and generic event capture keep their own independent controls.
A deployment without curation remains Chat-only and reports that explicitly.

The capture credential can call only capture; the tool credential can call only
session status and the three universal operations plus their contract. The server
derives actor and thread identity from host configuration, session UUID and the
allowlisted repository. It verifies replay bodies and immutable message content.
The configured human remains the human speaker; the Codex agent is separately
attributed. MCP cannot create Chats, Users or Memories.

## Host configuration

Install `tools/centaur_context` and `tools/codex_context` in an isolated Python
3.11+ environment. Use a private configuration file shaped like this (all values
are synthetic):

```json
{
  "version": 1,
  "host_id": "00000000-0000-4000-8000-000000000083",
  "state_dir": "/private/context-bridge/state",
  "transcript_root": "/private/codex/sessions",
  "supported_versions": ["0.153.4"],
  "targets": {
    "organization": {
      "url": "http://127.0.0.1:18090",
      "capture_token_file": "/private/context-bridge/capture-token",
      "tool_token_file": "/private/context-bridge/tool-token"
    }
  },
  "repositories": {
    "project-a": {"path": "/private/projects/project-a", "target": "organization"}
  }
}
```

State directory mode is 0700, token files 0600, owned by the desktop account.
Use a reviewed local host process to hold these credentials; never put them into
model context, tool output, repository files or agent command environments. This
trusts the host account and does not isolate a credential from arbitrary malicious
software with the same user's full filesystem privileges. HTTP proxy environment
variables and redirects cannot redirect these requests. Non-loopback endpoints
require HTTPS.

Use `centaur-codex-context --config /private/config.json install --launchd` on
macOS to install the owned MCP entry, hooks and 30-second recovery job. It backs
up existing configuration and preserves other entries. Use the same command with
`uninstall --launchd` to remove only owned entries, retaining unsent data.

For manual installation, register `centaur-codex-context --config /private/config.json mcp` as one STDIO MCP
server. Configure the same executable's `hook` command for `SessionStart`,
`UserPromptSubmit`, `Stop`, `Interrupt` and `SessionEnd`. Review/trust the exact hook definitions
through the native CLI `/hooks` flow before activation; installing a hook does not
establish trust. New or freshly loaded desktop sessions use the new configuration;
already loaded tasks can retain their earlier tool catalogue. Do not interrupt
active work just to refresh it.
Register a user-owned periodic task to call `flush` every 30 seconds. A hook spools
locally and starts a separate delivery process; delivery cannot block the answer.
Periodic flush visits only already registered sessions, not unrelated history.

Use `status` to inspect target, coverage, queued batches, errors, Chat ID and last
curation receipt. Use `status --session SESSION_UUID` to read the actual latest
curation Run, including failures, from the bound server. Queued means not yet
delivered; curation enabled does not mean a
Memory was produced. The session status API exposes the latest curation Run.
Unimportant interactions may correctly produce no Memory.

## Reliability and removal

One locked drain owns delivery. Stable session/item/batch identities and server
transactions prevent replay duplication. Failed delivery preserves queue and
cursor, uses bounded backoff, and retries through the host scheduler. The queue
is capped at 32 MiB/500 batches; full queues stop cursor advancement and require
attention rather than dropping unsent content. Successful payloads are deleted;
small delivery receipts expire after 30 days. Curation windows contain at most
100 messages; long turns queue full windows incrementally. Git checkpoints expire
after seven days. Registered session cursors remain
for future resumes. No transcript rewrite or history import is attempted.

Disable/remove the owned hooks, MCP entry and periodic job to stop the bridge;
retained Context Chats/Memories are unchanged. Preserve local unsent batches until
reconciled. Rollback of code must retain database migrations and compatibility;
do not drop captured history to run an older binary.

## Verification

Run `python -m pytest tools/codex_context/test_bridge.py` and the documented
repository checks. Rust integration tests run against a disposable database
whose name contains `centaur_context_test`. Before closing the issue, demonstrate
actual desktop capture, concise Memory retrieval by a later session/agent,
explicit Note/Task creation, two-instance isolation and outage/retry recovery.
See the RD for remaining outcome-verification and rollout evidence.


## Verified Git observations

The trusted adapter snapshots HEAD at the start of a user turn and checks the
same allowlisted worktree at Stop. At most ten fresh first-parent commits with
bounded raw objects are accepted. The server recomputes object hashes and parent
links, then records one concise Memory of the observed worktree change. It never
claims who authored it, that tests passed, or that a PR merged or deployed.
Assistant prose and pasted logs cannot supply this evidence. Hashes validate
object contents, while repository observation relies on the reviewed host adapter.
Concurrent writers on the same checkout cannot be attributed to a particular agent.

The receipt uses a distinct system actor and a namespaced `memory_capture` Run,
with standard Object Events and a `derived_from` Chat Connection. Only commit IDs
and a proof digest are retained; raw author/email/signature data is discarded. Credential-looking raw commit
objects are skipped because redacting them would invalidate the proof.
Existing Memory dreaming excludes this actor until it has its own evidence adapter.
The ordinary human-message curator validator remains unchanged. CODEX_CURATE
also gates Git outcome Memories. Missing baselines, rebases, older imported commits,
empty/net-zero tree changes, non-UTF8/oversized objects and interruptions do not become verified Git outcomes.

## Coverage and discovery summaries

Registration alone is `registered`, not a captured conversation. Capture receipts,
`GET /api/v2/codex/session`, Object reads and the Chat UI expose coverage and the
recovery action. `text_messages` means visible text observed from activation;
attachments and earlier history are excluded. Missing or replaced originals are
`unavailable` pending verified identity/coverage reconciliation; no history is
invented or rebound. Receipt-only Git batches preserve established coverage.

The existing curator generates incremental Chat titles/descriptions from each
bounded visible-message window. Metadata and its last-message checkpoint are
journaled together; retrying unchanged input does not repeat summary inference.
Protected or manually edited Chat metadata is preserved. Oversized summary windows
produce a `chat_summary_deferred` trace with no summary model call. The original
messages remain unchanged. Summary generation uses an additional bounded model
request only when the window and metadata are eligible.

Roll out the compatible server before updating host bridge/client packages: the
bridge now includes optional `coverage` and the reader accepts `note_windows`.
Keep routing, activation cursors and session identities unchanged during rollout.

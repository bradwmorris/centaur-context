# Slack integration

Slack provides the conversation. Centaur OS provides the shared context.

## Flow

1. Centaur sends Slack thread updates to Centaur OS.
2. Centaur OS stores one Chat, its messages, and its human and agent Users.
3. An interaction closes when the user says `done` or `finished`, or after 10
   minutes without a new message.
4. The Curator writes one Memory and any confirmed Tasks, Entities, or
   Connections.
5. Before a later reply, Centaur sends the canonical `chat_object_id` returned
   by ingestion and requests a bounded context packet of up to 10 Objects.

The Slack agent only reads. The Curator is the only automated writer.

## Centaur OS settings

Set these in the `centaur-os-env` Secret:

```text
CHAT_INGEST_API_TOKEN=<ingestion token>
AGENT_API_TOKEN=<read token>
APPROVED_SLACK_SURFACES=<workspace_id>:<channel_id>
```

Use different tokens. Add more approved surfaces with commas.

## Centaur settings

Centaur OS does not connect to Slack directly. Centaur needs two optional
integration hooks: one sends completed interactions to Centaur OS, and the
other gets context before an agent replies.

The tested Centaur integration is this three-commit patch set:

- `225a6104` adds `slackbotv2.interactionSink`.
- `d8a7dfc2` makes that integration portable across Centaur builds.
- `33e7cd59` adds `slackbotv2.contextBuilder`.

These commits modify Centaur, not Centaur OS. They are currently additions to a
Centaur fork and are not part of Paradigm's upstream Centaur repository. Pin a
Centaur revision containing all three.

```yaml
slackbotv2:
  interactionSink:
    url: http://centaur-os:8082/api/v1/ingest/slack/interactions
    timeoutMs: 5000
    secretName: centaur-os-env
    secretKey: CHAT_INGEST_API_TOKEN

  contextBuilder:
    url: http://centaur-os:8081/api/v1/context
    timeoutMs: 1500
    limit: 10
    secretName: centaur-os-env
    secretKey: AGENT_API_TOKEN
```

The Slack transport needs network access to port `8082`. Iron-proxy needs
access to port `8081`. Agent sandboxes receive neither real token.

Centaur OS checks the bearer token and the exact Slack workspace/channel pair.
Rejected surfaces are not stored.

Each ingested message sender may include an optional `avatar_url`. Centaur OS
accepts only an HTTP(S) reference, updates it idempotently on the sender's
existing Slack identity, and falls back to a deterministic local avatar when
the reference is absent or cannot be loaded. This does not grant Centaur OS new
Slack permissions or cause the backend to download avatar files.

## Agent tool

Load `tools/centaur_os` through Centaur's overlay mechanism. The tool supports:

```text
centaur-os get-context "<question>" --chat-object-id <chat-object-id> --limit 10
centaur-os search-objects "<query>" --limit 10
centaur-os read-object <object-id>
```

It cannot write or access PostgreSQL.

`get-context` fails closed unless the canonical Chat is active and its stored
`provider:workspace:channel:thread` identity matches
`X-Centaur-Thread-Key`. General `search-objects` remains independent of a Chat.

## Check it

1. Send a message on an approved Slack surface.
2. Reply `done` or `finished`, or wait 10 minutes.
3. Confirm the Curator Run completed in the UI.
4. Ask a related question in a new interaction.
5. Confirm the reply uses the saved context.

If Slack does not reply, check Centaur's Slack transport first. If it replies
without context, check the context URL, token, NetworkPolicy, and Curator Run.

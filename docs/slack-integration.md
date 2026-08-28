# Slack integration

Slack provides the conversation. Centaur OS provides the shared context.

## Flow

1. Centaur sends Slack thread updates to Centaur OS.
2. Centaur OS stores one Chat, its messages, and its human and agent Users.
3. An interaction closes when the user says `done` or `finished`, or after 10
   minutes without a new message.
4. The Curator writes one Memory and any confirmed Tasks, Entities, or
   Connections.
5. Before a later reply, Centaur requests a context packet of up to 10 Objects.

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

Use a Centaur build that supports `slackbotv2.interactionSink` and
`slackbotv2.contextBuilder`.

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

## Agent tool

Load `tools/centaur_os` through Centaur's overlay mechanism. The tool supports:

```text
centaur-os get-context "<question>" --limit 10
centaur-os search-objects "<query>" --limit 10
centaur-os read-object <object-id>
```

It cannot write or access PostgreSQL.

## Check it

1. Send a message on an approved Slack surface.
2. Reply `done` or `finished`, or wait 10 minutes.
3. Confirm the Curator Run completed in the UI.
4. Ask a related question in a new interaction.
5. Confirm the reply uses the saved context.

If Slack does not reply, check Centaur's Slack transport first. If it replies
without context, check the context URL, token, NetworkPolicy, and Curator Run.

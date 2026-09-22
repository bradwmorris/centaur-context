# Codex desktop bridge

Host-side Chat capture and exactly three MCP tools over the canonical Context
HTTP client. See [installation and boundaries](../../docs/codex.md).

Install both local packages with Python 3.11 or newer:

```sh
python3 -m pip install tools/centaur_context tools/codex_context
centaur-codex-context --config /private/config.json status
```

`hook` reads one lifecycle event from stdin; `flush` retries registered sessions
and bounded queued deliveries; `status` reports delivery and curation receipts;
`mcp` serves newline-delimited MCP on stdio. Keep configuration, credentials and
SQLite queue outside repositories. Install a host scheduler for periodic flushes
so reconnect recovery does not depend on another agent turn.

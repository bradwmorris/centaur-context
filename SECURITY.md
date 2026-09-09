# Security

## Supported versions

Centaur Context is pre-1.0 and does not yet publish stable releases. Security
fixes are made on the latest `main` branch. Do not expose the default deployment
to the public internet.

## Reporting a vulnerability

Use this repository's private vulnerability-reporting or Security Advisory
feature when available. Do not include credentials, private data, or a working
exploit in a public issue.

For a non-sensitive hardening suggestion, open a GitHub issue with the affected
version, configuration, impact, and a minimal reproduction.

## Deployment expectations

- Keep the human UI private and use localhost port forwarding or an equivalent
  authenticated private path.
- Give each listener its own purpose-bound credential; do not reuse tokens.
- Never give an agent sandbox a database DSN or raw service credential.
- Restrict Slack ingestion to exact approved workspace/channel pairs.
- Review NetworkPolicy and any optional model-provider egress before applying it.
- Back up and verify the Context database before upgrading.

See [Architecture](docs/architecture.md) and
[Setup and operations](docs/setup.md) for the full trust boundary.

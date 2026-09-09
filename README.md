# Centaur Context

Shared, durable context for Centaur agents—hosted by you.

Centaur runs your agents. Centaur Context keeps useful knowledge available across
conversations and agent runs. It turns completed conversations into structured
records, connects them to what is already known, and retrieves relevant context
for future work. A web UI lets you inspect and manage that knowledge.

The database schema is the ontology: it defines the things the system knows
about and how they relate. Humans and agents use the same shared structure.

Centaur Context is not a standalone agent platform. It is installed beside an
existing Centaur deployment. To install Centaur itself, use Centaur's official
[Quickstart](https://centaur.run/quickstart); this repository only documents
Centaur Context.

## Proposed Centaur App

Centaur Context is a working candidate for Centaur's proposed App model. It
already has its own container, web UI, API, database, background work, and agent
tool. Today it is installed manually beside Centaur because the App system is
not implemented in production. In the proposed future, Centaur could install,
route to, upgrade, and remove Context as one registered App.

The Context product remains outside Centaur core. Centaur only needs small,
generic points for sending completed interactions and requesting relevant
context before an agent turn. See [Architecture](docs/architecture.md) and the
[Centaur integration contract](docs/centaur-integration.md).

## Evaluate before installing

You do not need to create a database or run any commands to evaluate the design.
Start with [Architecture](docs/architecture.md) to understand what Context owns,
then read the [Centaur integration contract](docs/centaur-integration.md) to see
the two optional hooks it needs from Centaur.

Only follow [Setup and operations](docs/setup.md) when you already operate a
compatible Centaur revision and want to evaluate the working integration. The
current compatibility document is authoritative; historical fork commits are
not an installation recipe.

## Documentation

- [Architecture](docs/architecture.md) — the system, schema, and context loop.
- [Setup and operations](docs/setup.md) — install, connect, verify, and maintain
  Centaur Context beside an existing Centaur deployment.
- [Centaur integration contract](docs/centaur-integration.md) — the small
  connection between the proposed App and Centaur core.

Centaur Context runs alongside Centaur with its own PostgreSQL database. Centaur
owns agent execution; Context owns shared knowledge. Company-specific prompts,
workflows, and integrations belong in a private overlay.

Version 0.3.0 supports one organization on a local machine or trusted private
network. See [compatibility.toml](compatibility.toml) for the supported contract.

## Development

See [Contributing](CONTRIBUTING.md) for repository boundaries, development
checks, and contribution guidance. Agent-assisted contributors should also read
[AGENTS.md](AGENTS.md).

## License

[MIT](LICENSE). [Centaur](https://github.com/paradigmxyz/centaur) is separate
software with its own license.

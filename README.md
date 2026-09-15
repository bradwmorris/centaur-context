# Centaur Context

> [!IMPORTANT]
> **Centaur Context is an app/extension for [Centaur](https://github.com/paradigmxyz/centaur).** Centaur is an open-source control plane for running and owning your own agent infrastructure. We've created a [Centaur walkthrough video](https://youtu.be/993XrWfg34U).
>
> Install Centaur and become comfortable operating it before adding Centaur Context. If you haven't installed Centaur yet, start with [Centaur's official quickstart](https://centaur.run/quickstart). Then return here and follow the [Centaur Context setup instructions](docs/setup.md).

Centaur runs your agents. Centaur Context keeps useful knowledge available across
conversations and agent runs. It turns completed conversations into structured
records, connects them to what is already known, and retrieves relevant context
for future work. A web UI lets you inspect and manage that knowledge.

The database schema is the ontology: it defines the things the system knows
about and how they relate. Humans and agents use the same shared structure.

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

## What changed in the maintainer's Centaur fork

The working proof of concept uses changes in `bradwmorris/centaur`, **not** in
Paradigm's original `paradigmxyz/centaur` repository. Anyone can see the public
fork on GitHub, but that does not mean its changes were added to Paradigm's
repository. If you adapt Context for another Centaur installation, make the
changes in **your own fork** and review them before deploying. Do not treat a
GitHub comparison page as a request to modify the original repository.

The fork contains more than two modifications:

- Two Slackbot connection points send completed interactions to Context and
  fetch relevant Context before Centaur answers.
- Supporting changes keep one stable Context Chat per Slack thread, identify
  participants and triggering messages correctly, and record model usage,
  completed Runs, and reviewable retrieval/tool/instruction evidence.
- A separate private Curator API lets Context use Centaur's subscription-backed
  model access. Direct-provider mode does not need that fork change.
- Multi-Slack-app support is optional. YouTube caption access and sandbox proxy
  cleanup are fork changes but are not Context requirements.

For the exact changed files, reasons, and minimum changes another fork needs,
read `docs/FORK.md` in a local checkout of `bradwmorris/centaur`. The file is at
`../centaur/docs/FORK.md` when the repositories are side by side. It has not
been published to GitHub yet. The published
[fork audit](https://github.com/bradwmorris/centaur/blob/main/docs/fork-audit-2026-09-15.md)
provides the detailed evidence until then.

## Evaluate before installing

You do not need to create a database or run any commands to evaluate the design.
Start with [Architecture](docs/architecture.md) to understand what Context owns,
then read the [Centaur integration contract](docs/centaur-integration.md) to see
the two connection points and their supporting requirements.

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
- [UI modules](docs/ui-modules.md) — trusted compile-time views over canonical
  Context data.
- [Integration API](docs/api.md) — supported endpoints, credentials, headers,
  and minimal request examples.

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

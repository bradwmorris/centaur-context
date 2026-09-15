# Centaur Context

[Centaur](https://github.com/paradigmxyz/centaur) runs agents. Centaur Context
helps those agents reuse what the organization has learned. It runs **beside**
Centaur as a separate Rust service, with its own PostgreSQL database. Centaur's
database keeps operational state; Context's database keeps shared knowledge.
They do not share a database, though they could use the same PostgreSQL server.

![Centaur control plane with the separate Context layer](docs/images/centaur-control-and-context.png)

The green Context layer adds another kind of durable state below the existing
Centaur control plane. Centaur and Context communicate through an authenticated
HTTP API. Context is not part of an agent's execution sandbox, and agents do not
need a database password to use it.

## The simple loop

Before an agent answers, **Context Builder reads** useful existing knowledge and
returns a short context packet. After a conversation ends, a **separate Curator
agent may write** new knowledge for future conversations. That is how useful
company context can build up over time.

![Read before the answer and curate after the conversation](docs/images/context-read-write-loop.png)

The diagrams show the roles, not every network call. In the working integration
today, Centaur's **Slackbot** calls Context automatically. Other agent entry
points are not automatically connected just because Context is installed.

### Read before the answer

Context Builder searches the saved records using ordinary text search, optional
meaning-based search, and a small number of related records. It ranks the
results and gives Centaur a bounded packet to add to the agent's input. **This
retrieval step does not call an LLM.**

### Write after the conversation

For an approved Slack interaction, Context checks the request token and the
allowed workspace and channel. It finds or creates the matching Chat, stores
new messages, and records the agent interaction as a Run. Unapproved Slack
surfaces are rejected, not stored.

When the conversation is finished or inactive, Context creates a **separate
Curator Run**. The Curator reads the new messages and potentially related
records, asks a model what is worth keeping, and requires a strict JSON plan.
Context validates that plan before saving useful Objects, explained Connections,
and immutable Object Events.

The Chat, messages, and Runs are records of what happened. A lasting Memory is
created **only if there is something useful to keep**; not every interaction
becomes new shared knowledge. What is saved can be found by a later agent.

## Connecting it to your Centaur installation

This is a working MVP installed beside Centaur, not a built-in Centaur App.
The proof of concept uses changes in the maintainer's
[`bradwmorris/centaur` fork](https://github.com/bradwmorris/centaur), **not** in
Paradigm's original repository. Stock Centaur does not yet include the automatic
Context hooks. The fork's two Slackbot connection points fetch context before a
reply and send completed interactions afterward. Other fork changes support
stable Chat identity, accurate Runs and evidence, or optional integrations;
they are not all required for every installation.

If you already run Centaur, start with [setup](docs/setup.md). It asks you to
check your own deployment, network, credentials, and approved Slack surfaces
before choosing an installation method. For the exact integration requirements,
read the [integration guide](docs/centaur-integration.md),
[supported revision](compatibility.toml), and
[fork audit](https://github.com/bradwmorris/centaur/blob/main/docs/fork-audit-2026-09-15.md).
The detailed `docs/FORK.md` file is currently available only in a local checkout
of the maintainer's Centaur fork; it has not been published to GitHub yet. Make
any Centaur changes in **your own fork**, not Paradigm's repository.

Context can also be explored without installing it. See the
[architecture](docs/architecture.md), [API](docs/api.md), and
[UI modules](docs/ui-modules.md). Organization-specific prompts, policies, and
integrations belong in a separate private overlay. Version 0.3.0 supports one
organization on a local machine or trusted private network.

## Development and license

See [Contributing](CONTRIBUTING.md) and [agent guidance](AGENTS.md). Centaur
Context is [MIT licensed](LICENSE); Centaur is separate software with its own
license.

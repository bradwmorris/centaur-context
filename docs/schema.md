# Schema and ontology

I’ve tried a few approaches to agent memory: Neo4j and graph queries at one end, and a wiki with simple search tools at the other. The lesson for me is that the database choice matters less than whether the system captures useful information, retrieves it at the right time, and lets you inspect and correct what it stored.

Centaur Context takes a middle path. It uses PostgreSQL, but arranges knowledge like a small graph. The main things are **Objects**; **Connections** link them. Each Object also has a written description, so it can be found through text search and, when enabled, semantic search. For the relationships this proof of concept needs, there is no separate graph database.

![Centaur Context Objects, Connections, Object types, and supporting records](images/schema-diagram-v3.png)

## Objects

An Object is the stable record for one thing the system knows about. It has an ID, a type, a title, and a description. It also records who created or updated it, where the information came from, and its current revision. Fields particular to a type live in a matching record—for example, a Task has a status and priority.

The current Object types are:

- **Task:** agreed work and its status.
- **Chat:** a conversation thread.
- **User:** a person or agent.
- **Entity:** a subject such as a project, organization, product, or concept.
- **Memory:** an event or insight worth retaining.
- **Source:** a work used as evidence, such as an article or paper.
- **Note:** one atomic excerpt, insight, or question. Legacy Notes may remain
  unclassified until a human reviews them.
- **Theme:** an approved category for organizing knowledge.

These are the types the current application supports, not a promise that an installation can invent new types without changing the schema and code.

Descriptions matter. “Project Atlas” might mean several things; a concise description that says what it is and why it matters in the current Context makes the Object easier for both people and search to identify. A description is a current snapshot of at most 600 Unicode characters after trimming, not a running log. Immutable Object Events and Runs retain its history.

## Connections

A Connection joins two Objects. It names the relationship and includes an explanation of *why* the link exists. The current relationships are `involves`, `about`, `related_to`, `depends_on`, `derived_from`, and `themed`.

For example, a Chat might involve a User. A Task agreed in that conversation might be `derived_from` the Chat and `depend_on` another Task. These links let the system find an Object and then inspect nearby knowledge. The explanation makes each link reviewable; it is not merely an unexplained edge in a graph.

Ordinary authenticated `context_apply` callers may create explained `involves`
or `about` Connections from an active Source to an Entity, and `related_to`
Connections between active Entities, even if either endpoint is protected.
This creation-only permission leaves endpoint contents and protection unchanged;
it does not permit modifying existing Connections. Protected Connection edits,
archived or system-managed endpoints, and other relationship shapes remain
subject to their existing restrictions.

The Object types and allowed relationships form the **ontology**: the shared vocabulary for what the system can represent and how those things relate.

## Evidence and history

Objects are the main knowledge records, but they do not hold everything. Chat messages keep the underlying conversation. Artifacts can preserve captured text, files, or references attached to an Object. A Source identifies a work; its current Artifact is the canonical captured content selected by Source intake, while other Artifacts are supporting material and never silently replace it.

Atomic Notes have one of three intents: `excerpt`, `insight`, or `question`.
An Excerpt preserves exact wording and identifies one Source, one of that
Source's Artifacts, and a validated timestamp, page, section, or text-offset
locator. Insights and Questions can be `derived_from` Sources and other Notes.
This keeps source evidence separate from interpretation without introducing
parallel Object types.

Runs record operations such as curation. Immutable Object Events record changes to Objects and Connections, so you can see what changed and why. These supporting records are **not** additional Object types. They help keep the knowledge traceable and reviewable.

This structure is deliberately modest. PostgreSQL stores the Objects, relationships, evidence, and history together. If much deeper relationship traversal becomes a real requirement, a graph database may be worth revisiting. For now, the more important work is checking that the system stores the right knowledge and actually brings it back when an agent needs it.

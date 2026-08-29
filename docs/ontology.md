# Ontology

Centaur Context stores a business as Objects and Connections.

## Objects

Every first-class node starts with one row in `objects`. That row holds its
title, plain-language description, type, revision, lifecycle, and provenance.

An Object may have one matching subtype row:

| Type | Meaning |
| --- | --- |
| Task | Confirmed work to do |
| Chat | One conversation thread |
| User | A human or agent |
| Entity | A company, project, product, place, or other named thing |
| Memory | A simple record of what happened |
| Source | An article, paper, podcast, video, book, report, document, dataset, or web page used as evidence |
| Note | Useful human- or agent-authored Markdown or plain text |

The Object is canonical. The subtype stores fields specific to that type.

A Source stores only bounded bibliographic and artifact metadata in its subtype.
Immutable versions of normalized article text or transcripts live separately in
`source_contents`; the current version is selected without overwriting older
evidence. Original binary files remain outside PostgreSQL and are referenced by
an opaque artifact identifier and integrity hash. Source list and agent APIs
never return complete long-form text accidentally.

A Note keeps its concise identity and summary in the canonical Object row while
its bounded Markdown or plain-text body lives in the one-to-one `notes`
subtype. Authorized agents create Notes only through the dedicated authenticated
Note-write API; general Context agent access remains read-only.

A User may have provider identities such as Slack. Those identities retain the
provider/workspace key, display name, and an optional HTTP(S) avatar reference.
The UI always has a deterministic local fallback avatar, so the canonical User
does not depend on an external image remaining available.

## Supporting records

These are not Objects:

- Connections
- Chat messages
- Embeddings
- Curator Runs
- Audit events
- Source content versions

They support the graph but are not first-class business nodes.

## Connections

A Connection joins two Objects. It must use one of five types and include a
short explanation.

| Type | Use |
| --- | --- |
| `involves` | A person or agent took part |
| `about` | One Object is mainly about another |
| `related_to` | The Objects have a useful general link |
| `depends_on` | One Object needs another first |
| `derived_from` | An Object came from a source Chat or evidentiary Source |

## Rules

- Every Object needs a clear, concise description.
- Every Object has one primary type.
- Each subtype row belongs to exactly one Object.
- Each Curator Run creates exactly one primary Memory.
- Sources represent evidence; Memories represent events or derived insights.
- A Task is created only from an explicit instruction or commitment.
- Updates use revisions so newer changes are not overwritten.
- Every Curator change points back to its source Chat and messages.
- Source and User visuals are derived from stored Chat, message, owner,
  `involves`, `derived_from`, and supporting-message evidence. Display names
  alone never establish attribution.
- Curator Runs are atomic: all changes commit, or none do.
- Undo reverses a whole Curator Run without deleting its audit history.

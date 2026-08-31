# Ontology

Centaur Context stores a business as Objects and Connections.

## Objects

Every first-class node starts with one row in `objects`. That row holds its
title, explicit plain-language description, type, revision, attribution,
provenance, and archive time. `archived_at` is the persisted state: `NULL` means
active, while a timestamp means archived. APIs derive the human-facing
`lifecycle` value from it.

An Object may have one matching subtype row:

| Type | Meaning |
| --- | --- |
| Task | Confirmed work tracked as backlog, todo, doing, review, done, or blocked |
| Chat | One conversation thread |
| User | A human or agent |
| Entity | A named person, organization, product, project, publication, place, concept, or other subject |
| Memory | A simple record of what happened |
| Source | An article, paper, podcast episode, video, book, report, document, dataset, web page, or social post used as evidence |
| Note | Useful human- or agent-authored Markdown or plain text |
| Theme | A human-approved stable taxonomy category |

The Object is canonical. The subtype stores fields specific to that type.

A Source stores bounded bibliographic metadata in its subtype. Immutable
supporting material lives in `artifacts`. Artifacts can belong to any Object,
not only Sources, and cover transcripts, normalized text, files, URLs, and
other evidence. A Source may point at its current Artifact without overwriting
older Artifacts. Each Artifact carries its own SHA-256 digest, size, media type,
capture time, metadata, and optional predecessor. Long text is read only through
bounded Artifact windows.

A Note keeps its concise identity and summary in the canonical Object row while
its bounded Markdown or plain-text body lives in the one-to-one `notes`
subtype. Authorized agents create Notes only through the dedicated authenticated
Note-write API; general Context agent access remains read-only.

A User embeds zero or more provider identities such as Slack or GitHub in its
`identities` JSON array. The same User may have identities from many providers. They retain the
provider/workspace key, display name, and an optional HTTP(S) avatar reference.
The UI always has a deterministic local fallback avatar, so the canonical User
does not depend on an external image remaining available.

## Supporting records

These are not Objects:

- Connections
- Chat messages
- Embeddings
- Runs, including curator, evaluation, ingestion, external-action, and mutation Runs
- Object Events, the sole durable Object/Connection mutation history
- Artifacts

They support the graph but are not first-class business nodes.

## Connections

A Connection joins two Objects. It must use one of six types and include a
short explanation.

| Type | Use |
| --- | --- |
| `involves` | A person or agent took part |
| `about` | One Object is mainly about another |
| `related_to` | The Objects have a useful general link |
| `depends_on` | One Object needs another first |
| `derived_from` | An Object came from a source Chat or evidentiary Source |
| `themed` | A non-Theme Object is assigned to a human-approved Theme |

## Rules

- Every Object needs a direct, evidence-grounded 50–150 word description that
  says what it is, what it is about, and its current context.
- Every Object has one primary type.
- Each subtype row belongs to exactly one Object.
- A curator Run creates zero or more Memories only when the messages contain
  durable events or insights worth retaining.
- Sources represent evidence; Memories represent events or derived insights.
- A Task is created only from an explicit instruction or commitment.
- Updates use revisions so newer changes are not overwritten.
- Every durable Object or Connection change belongs to a Run and has a complete
  reversible before/after Object Event snapshot.
- Source and User visuals are derived from stored Chat, message, owner,
  `involves`, `derived_from`, and supporting-message evidence. Display names
  alone never establish attribution.
- Curator Runs are atomic: all changes commit, or none do.
- Undo creates a compensating child Run and new Object Events; it never rewrites history.

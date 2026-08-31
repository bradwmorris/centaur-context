# Context and search

Centaur Context keeps shared information in one PostgreSQL database. Humans and
agents use the same Objects, Notes, Sources, Chats, and Connections.

The goal is simple: find the smallest useful set of information for the current
question.

## How it works

When an agent asks for context, Centaur Context:

1. Searches Object titles and descriptions for matching words.
2. Optionally searches by meaning when embeddings are enabled.
3. Adds the current Chat, its participants, and directly connected Objects.
4. Adds a small amount of useful detail, such as Task status or a Note excerpt.
5. Returns no more than 10 Objects in a bounded response.

```text
Question
   ↓
Word search + optional meaning search
   ↓
Current Chat + people + Connections
   ↓
Small context packet for the agent
```

## What is searched

| Information | Search today |
| --- | --- |
| Objects | Title and description |
| Notes | Title, description, and Note body |
| Sources | Metadata and attached Artifact text |
| Connections | Used to expand conversation context |

A Source and its supporting material are kept separately:

- A **Source** is the article, video, paper, or other work.
- An **Artifact** is a transcript, captured full text, URL, file reference, or
  other immutable supporting item attached to the Source.

Each text Artifact is stored as one complete capture. It is not split into
chunks today, and the same Artifact model can support Tasks, Chats, or any other Object.

## Word search and meaning search

Word search is always available. It uses PostgreSQL to find words in titles,
descriptions, Note bodies, and textual Artifact content.

Meaning search is optional. It creates a vector from an Object's kind, title,
and description. This can find related ideas even when the wording is different.

If meaning search is unavailable or out of date, Centaur Context safely falls
back to word search.

## Context is conversation-aware

Search finds relevant Objects. Context Builder goes one step further.

It always considers the current Chat, the people in it, and Objects directly
connected to it. This prevents the agent from losing track of the conversation
even when the latest message is short or ambiguous.

The final response is deliberately small: up to 10 complete Objects and no more
than 12,000 characters. The agent can search or read an Object again when it
needs more detail.

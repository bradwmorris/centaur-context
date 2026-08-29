"""Small JSON CLI for the Centaur Context agent tool."""

from __future__ import annotations

import json
from typing import Any

import typer

from .client import _client

app = typer.Typer(
    name="centaur-context",
    help="Read shared context and create explicitly authorized Notes.",
    no_args_is_help=True,
)


def _print(value: Any) -> None:
    print(json.dumps(value, indent=2, ensure_ascii=False, default=str))


@app.command("get-context")
def get_context(
    query: str = typer.Argument(..., help="What the agent needs context for."),
    chat_object_id: str = typer.Option(
        ..., "--chat-object-id", help="Canonical Chat Object for the current thread."
    ),
    kind: str | None = typer.Option(None, help="Optional Object kind filter."),
    limit: int = typer.Option(10, min=1, max=10),
) -> None:
    """Build a concise packet of relevant Objects and their connections."""
    _print(
        _client().get_context(
            query, chat_object_id=chat_object_id, kind=kind, limit=limit
        )
    )


@app.command("search-objects")
def search_objects(
    query: str = typer.Argument(..., help="Text to find in record titles or bodies."),
    kind: str | None = typer.Option(None, help="Optional Object kind filter."),
    limit: int = typer.Option(20, min=1, max=100),
) -> None:
    """Search shared Objects."""
    _print(_client().search_objects(query, kind=kind, limit=limit))


@app.command("read-object")
def read_object(id: str = typer.Argument(..., help="Object UUID.")) -> None:
    """Read one shared Object."""
    _print(_client().read_object(id))


@app.command("search-sources")
def search_sources(
    query: str = typer.Argument(
        ..., help="Text to find in Source metadata or normalized content."
    ),
    limit: int = typer.Option(20, min=1, max=100),
    cursor: str | None = typer.Option(None, help="Opaque cursor from the prior page."),
) -> None:
    """Search Sources and return small attributed excerpts."""
    _print(_client().search_sources(query, limit=limit, cursor=cursor))


@app.command("read-source")
def read_source(
    source_id: str = typer.Argument(..., help="Canonical Source Object UUID."),
) -> None:
    """Read Source metadata without loading its long-form content."""
    _print(_client().read_source(source_id))


@app.command("read-source-content")
def read_source_content(
    source_id: str = typer.Argument(..., help="Canonical Source Object UUID."),
    version: int | None = typer.Option(
        None, min=1, help="Content version; omit to read the current version."
    ),
    offset: int = typer.Option(0, min=0, help="Zero-based character offset."),
    limit: int = typer.Option(8_000, min=1, max=20_000),
) -> None:
    """Read a bounded text window from one Source content version."""
    _print(
        _client().read_source_content(
            source_id, version=version, offset=offset, limit=limit
        )
    )


@app.command("search-notes")
def search_notes(
    query: str = typer.Argument(..., help="Text to find in Note metadata or content."),
    limit: int = typer.Option(20, min=1, max=100),
    cursor: str | None = typer.Option(None, help="Opaque cursor from the prior page."),
) -> None:
    """Search Notes and return bounded excerpts."""
    _print(_client().search_notes(query, limit=limit, cursor=cursor))


@app.command("read-note")
def read_note(
    note_id: str = typer.Argument(..., help="Canonical Note Object UUID."),
) -> None:
    """Read one Note and its content."""
    _print(_client().read_note(note_id))


@app.command("create-note")
def create_note(
    title: str = typer.Argument(..., help="Short Note title."),
    description: str = typer.Option(..., help="Concise description of the Note."),
    content: str = typer.Option(..., help="Markdown or plain-text Note content."),
    content_format: str = typer.Option("markdown", help="markdown or plain_text."),
    provenance_json: str = typer.Option(
        "{}", help="JSON object describing where the Note came from."
    ),
    idempotency_key: str = typer.Option(
        ..., help="Stable retry key, required for safe Note creation."
    ),
) -> None:
    """Create a Note using CENTAUR_CONTEXT_NOTE_WRITE_TOKEN."""
    try:
        provenance = json.loads(provenance_json)
    except json.JSONDecodeError as exc:
        raise ValueError("provenance_json must be valid JSON") from exc
    _print(
        _client().create_note(
            title,
            description,
            content,
            content_format=content_format,
            provenance=provenance,
            idempotency_key=idempotency_key,
        )
    )


def main() -> None:
    try:
        app()
    except (RuntimeError, ValueError) as exc:
        print(json.dumps({"error": str(exc)}, ensure_ascii=False))
        raise SystemExit(1) from None


if __name__ == "__main__":
    main()

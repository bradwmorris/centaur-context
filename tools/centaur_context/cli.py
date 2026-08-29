"""Small JSON CLI for the Centaur Context agent tool."""

from __future__ import annotations

import json
from typing import Any

import typer

from .client import _client

app = typer.Typer(
    name="centaur-context",
    help="Read concise shared context from Centaur Context.",
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


def main() -> None:
    try:
        app()
    except (RuntimeError, ValueError) as exc:
        print(json.dumps({"error": str(exc)}, ensure_ascii=False))
        raise SystemExit(1) from None


if __name__ == "__main__":
    main()

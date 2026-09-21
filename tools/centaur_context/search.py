"""The universal Context search tool."""

from __future__ import annotations

import argparse

from .client import _client
from .tool_common import run


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="context_search",
        description="Find canonical Centaur Context Objects.",
        epilog=(
            "example:\n"
            "  context_search 'deployment decision' --object-type task --limit 10"
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("query", help="Words to search for.")
    parser.add_argument(
        "--object-type",
        action="append",
        dest="object_types",
        help="Restrict results to this Object type; repeat for multiple types.",
    )
    parser.add_argument("--limit", type=int, default=20, help="Maximum results (default: 20).")
    parser.add_argument(
        "--lexical-only",
        action="store_true",
        help="Use exact text matching without semantic search.",
    )
    return parser


def app(argv: list[str] | None = None) -> None:
    values = _parser().parse_args(argv)
    run(
        lambda: _client().context_search(
            values.query,
            object_types=values.object_types,
            limit=values.limit,
            lexical_only=values.lexical_only,
        )
    )


def main() -> None:
    app()

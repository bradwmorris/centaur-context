"""The universal Context read tool."""

from __future__ import annotations

import argparse
import json

from .client import _client
from .tool_common import run


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="context_read",
        description="Read canonical Centaur Context Objects.",
        epilog=(
            "example:\n"
            "  context_read 00000000-0000-0000-0000-000000000001 "
            "--include connections"
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("object_ids", nargs="+", help="One or more Object UUIDs.")
    parser.add_argument(
        "--include",
        action="append",
        choices=("connections", "artifacts", "events", "messages"),
        help="Include related data; repeat for multiple kinds.",
    )
    parser.add_argument(
        "--artifact-window",
        action="append",
        help='JSON object with "artifact_id" and optional "offset" and "limit".',
    )
    parser.add_argument("--note-window", action="append", help='JSON object with "object_id" and optional "offset" and "limit".')
    return parser


def app(argv: list[str] | None = None) -> None:
    values = _parser().parse_args(argv)
    run(
        lambda: _client().context_read(
            values.object_ids,
            include=values.include,
            note_windows=[json.loads(value) for value in values.note_window or []],
            artifact_windows=[
                json.loads(value) for value in values.artifact_window or []
            ],
        )
    )


def main() -> None:
    app()

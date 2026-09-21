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
    parser.add_argument("query", nargs="?", default="", help="Words to search for.")
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
    parser.add_argument("--task-owner", help="Assigned canonical User Object ID.")
    parser.add_argument("--task-status", action="append", help="Task status; repeat to include more.")
    parser.add_argument("--task-priority", choices=["low", "medium", "high"])
    parser.add_argument("--task-due-before", help="RFC3339 latest due instant.")
    parser.add_argument("--ready", action="store_true", help="Agent-suitable, assigned, dated backlog/todo Tasks with a brief and no incomplete Task dependency; inspect permission before claiming.")
    parser.add_argument("--task-cursor", help="next_cursor from the preceding filtered page.")
    return parser


def app(argv: list[str] | None = None) -> None:
    values = _parser().parse_args(argv)
    filters = {key: value for key, value in {
        "owner_object_id": values.task_owner, "statuses": values.task_status,
        "priority": values.task_priority, "due_before": values.task_due_before,
        "ready": values.ready or None, "cursor": values.task_cursor,
    }.items() if value is not None}
    extra = {"task_filters": filters} if filters else {}
    run(
        lambda: _client().context_search(
            values.query,
            object_types=values.object_types,
            limit=values.limit,
            lexical_only=values.lexical_only,
            **extra,
        )
    )


def main() -> None:
    app()

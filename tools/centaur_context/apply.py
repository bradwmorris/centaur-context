"""The universal Context atomic write tool."""

from __future__ import annotations

import argparse

from .client import _client
from .tool_common import print_json, read_json_object, run


EXAMPLE_REQUEST = {
    "contract_version": "1.0.0",
    "idempotency_key": "replace-with-one-stable-key",
    "validate_only": True,
    "operations": [
        {
            "operation": "create_object",
            "local_ref": "task",
            "kind": "task",
            "title": "Review deployment",
            "description": "Review the proposed deployment and record the decision.",
            "fields": {"status": "todo", "priority": "medium"},
        },
        {
            "operation": "create_connection",
            "source": {"local_ref": "task"},
            "kind": "related_to",
            "target": {"object_id": "00000000-0000-0000-0000-000000000001"},
            "description": "The Task reviews this existing deployment record.",
        },
    ],
}


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="context_apply",
        description="Apply one atomic, idempotent Centaur Context write batch.",
        epilog=(
            "request root keys:\n"
            "  required: contract_version, idempotency_key, operations\n"
            "  optional: chat_object_id, validate_only\n\n"
            "supported operations:\n"
            "  create_object, update_object, archive_object, create_connection,\n"
            "  update_connection, archive_connection, append_artifact\n\n"
            "examples:\n"
            "  context_apply --file apply-request.json\n"
            "  context_apply --example\n"
            "  context_apply --schema"
        ),
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--json", help="Complete request as one JSON object.")
    source.add_argument("--file", help="Path to a file containing the request JSON object.")
    source.add_argument(
        "--example",
        action="store_true",
        help="Print a validation-only create-and-connect template; do not call the API.",
    )
    source.add_argument(
        "--schema",
        action="store_true",
        help="Read the live canonical contract, including exact operation fields; do not write.",
    )
    return parser


def app(argv: list[str] | None = None) -> None:
    values = _parser().parse_args(argv)
    if values.example:
        print_json(EXAMPLE_REQUEST)
        return
    if values.schema:
        run(lambda: _client().context_contract())
        return
    request = read_json_object(values.json, values.file)
    run(lambda: _client().context_apply(request))


def main() -> None:
    app()

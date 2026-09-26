"""The universal Context atomic write tool."""

from __future__ import annotations

import argparse

from .client import _client
from .tool_common import print_json, read_json_object, run


DESCRIPTION_EXAMPLES = {
    "task": "Define and test concise Object descriptions across creation, capture and maintenance paths.",
    "entity": "Jane Lee studies how agents retain and retrieve useful memories.",
    "source": "An interview with Jane Lee comparing retrieval evaluation methods for agent memory.",
    "note": "The current embedding trigger already invalidates vectors after title or description changes. Recorded to prevent a redundant re-indexing subsystem.",
    "theme": "How canonical Objects are found and ranked using their titles, descriptions and relationships.",
    "chat": "Alex and a research assistant compared retrieval methods and selected a metadata-only evaluation plan.",
    "user": "A research assistant that helps review sources and prepare evidence-backed notes.",
    "memory": "Alex approved the retrieval evaluation plan after reviewing its synthetic test cases."
}

EXAMPLE_DOCUMENT = {
    "standalone_idea": {
        "contract_version": "1.1.0",
        "idempotency_key": "replace-with-one-stable-note-key",
        "validate_only": True,
        "operations": [{
            "operation": "create_object", "local_ref": "idea", "kind": "note",
            "title": "Evaluate context ownership",
            "description": "A tentative idea about context ownership and switching costs. Saved for later investigation without claiming supporting evidence.",
            "fields": {"intent": "idea", "content": "Context ownership may affect switching costs."},
        }],
    },
    "description_policy": {
        "max_unicode_characters_after_trimming": 600,
        "guidance": "Current snapshot, not a log: identify the concrete subject and add concise distinguishing facts. Entity identity belongs in metadata; relationship context belongs on Connections.",
        "examples": DESCRIPTION_EXAMPLES,
    },
    "create_and_connect": {
        "contract_version": "1.1.0",
        "idempotency_key": "replace-with-one-stable-create-key",
        "validate_only": True,
        "operations": [
            {
                "operation": "create_object",
                "local_ref": "task",
                "kind": "task",
                "title": "Review deployment",
                "description": "Review the proposed deployment and record the decision. This Task keeps the release approval explicit and reviewable.",
                "fields": {"status": "todo", "priority": "medium",
                           "owner_object_id": "00000000-0000-0000-0000-000000000001",
                           "due_at": "2099-01-01T00:00:00Z",
                           "brief_markdown": "Review the proposed deployment; record acceptance evidence and the next action."},
            },
            {
                "operation": "create_connection",
                "source": {"local_ref": "task"},
                "kind": "related_to",
                "target": {"object_id": "00000000-0000-0000-0000-000000000001"},
                "description": "The Task reviews this existing deployment record.",
            },
        ],
    },
    "update_current_snapshot": {
        "contract_version": "1.1.0",
        "idempotency_key": "replace-with-one-stable-update-key",
        "validate_only": True,
        "operations": [
            {
                "operation": "update_object",
                "object_id": "00000000-0000-0000-0000-000000000001",
                "expected_revision": 3,
                "changes": {
                    "title": "Approve retrieval launch",
                    "description": "Approve the retrieval launch after the evaluation passes. The latest evidence removed the prior quality blocker.",
                },
            },
            {
                "operation": "create_connection",
                "source": {"object_id": "00000000-0000-0000-0000-000000000001"},
                "kind": "derived_from",
                "target": {"object_id": "00000000-0000-0000-0000-000000000002"},
                "description": "The updated launch decision is supported by this evaluation evidence.",
            },
        ],
    },
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
            "Object descriptions:\n"
            "  at most 600 Unicode characters after trimming\n"
            "  current snapshot, not a log: identify the Object and why it matters now\n"
            "  update materially stale descriptions with expected_revision\n\n"
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
        help="Print validation-only create/update requests and per-type description examples; do not call the API.",
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
        print_json(EXAMPLE_DOCUMENT)
        return
    if values.schema:
        run(lambda: _client().context_contract())
        return
    request = read_json_object(values.json, values.file)
    run(lambda: _client().context_apply(request))


def main() -> None:
    app()

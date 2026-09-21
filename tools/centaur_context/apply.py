"""The universal Context atomic write tool."""

from __future__ import annotations

import argparse

from .client import _client
from .tool_common import print_json, read_json_object, run


DESCRIPTION_EXAMPLES = {
    "task": "Define and test the Object-description contract across every write path. Created from Issue #51 to keep retrieval summaries explicit and current.",
    "entity": "Jane Lee, a researcher working on agent-memory evaluation. Relevant as the speaker in the Source supporting the retrieval design.",
    "source": "A YouTube interview with Jane Lee about retrieval evaluation for agent memory. Added as evidence for the semantic-search design decision.",
    "note": "The current embedding trigger already invalidates vectors after title or description changes. Recorded to prevent a redundant re-indexing subsystem.",
    "theme": "Work concerning how canonical Objects are found and ranked. Used to group decisions, tests, and Sources about retrieval quality.",
}

EXAMPLE_DOCUMENT = {
    "standalone_insight": {
        "contract_version": "1.1.0",
        "idempotency_key": "replace-with-one-stable-note-key",
        "validate_only": True,
        "operations": [{
            "operation": "create_object", "local_ref": "idea", "kind": "note",
            "title": "Evaluate context ownership",
            "description": "A tentative idea about context ownership and switching costs. Saved for later investigation without claiming supporting evidence.",
            "fields": {"intent": "insight", "content": "Context ownership may affect switching costs."},
        }],
    },
    "description_policy": {
        "max_unicode_characters_after_trimming": 600,
        "guidance": "Current snapshot, not a log: say what the Object is and why it matters in this Context now.",
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

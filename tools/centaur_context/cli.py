"""Small JSON CLI for the Centaur Context agent tool."""

from __future__ import annotations

import argparse
import json
from typing import Any

from .client import _client


def _print(value: Any) -> None:
    print(json.dumps(value, indent=2, ensure_ascii=False, default=str))


def _manifest(path: str) -> dict[str, Any]:
    with open(path, encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError("manifest file must contain a JSON object")
    return value


def get_context(
    query: str,
    chat_object_id: str,
    kind: str | None = None,
    limit: int = 10,
) -> None:
    """Build a concise packet of relevant Objects and their connections."""
    _print(
        _client().get_context(
            query, chat_object_id=chat_object_id, kind=kind, limit=limit
        )
    )


def search_objects(
    query: str,
    kind: str | None = None,
    limit: int = 20,
) -> None:
    """Search shared Objects."""
    _print(_client().search_objects(query, kind=kind, limit=limit))


def read_object(id: str) -> None:
    """Read one shared Object."""
    _print(_client().read_object(id))


def search_sources(
    query: str,
    limit: int = 20,
    cursor: str | None = None,
) -> None:
    """Search Sources and return small attributed excerpts."""
    _print(_client().search_sources(query, limit=limit, cursor=cursor))


def read_source(source_id: str, thread_key: str | None = None) -> None:
    """Read Source metadata without loading its long-form content."""
    _print(_client().read_source(source_id, thread_key=thread_key))


def read_source_content(
    source_id: str,
    version: int | None = None,
    offset: int = 0,
    limit: int = 8_000,
) -> None:
    """Read a bounded text window from one Source content version."""
    _print(
        _client().read_source_content(
            source_id, version=version, offset=offset, limit=limit
        )
    )


def search_notes(
    query: str,
    limit: int = 20,
    cursor: str | None = None,
) -> None:
    """Search Notes and return bounded excerpts."""
    _print(_client().search_notes(query, limit=limit, cursor=cursor))


def read_note(note_id: str) -> None:
    """Read one Note and its content."""
    _print(_client().read_note(note_id))


def create_note(
    title: str,
    description: str,
    content: str,
    content_format: str = "markdown",
    provenance_json: str = "{}",
    idempotency_key: str = "",
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


def source_intake_validate(manifest_file: str) -> None:
    """Validate one Enyu Source manifest without writes."""
    _print(_client().source_intake_validate(_manifest(manifest_file)))


def source_intake_commit(manifest_file: str) -> None:
    """Commit or safely replay one Enyu Source manifest."""
    _print(_client().source_intake_commit(_manifest(manifest_file)))


def source_intake_status(manifest_file: str) -> None:
    """Check commit and retrieval readiness for one Enyu Source manifest."""
    _print(_client().source_intake_status(_manifest(manifest_file)))


def list_themes(slug: str | None = None) -> None:
    """List approved Themes."""
    _print(_client().list_themes(slug=slug))


def read_theme(theme_id: str) -> None:
    """Read one approved Theme."""
    _print(_client().read_theme(theme_id))


def list_theme_objects(
    theme_id: str, kind: str | None = None, limit: int = 20
) -> None:
    """List Objects assigned to one approved Theme."""
    _print(_client().list_theme_objects(theme_id, kind=kind, limit=limit))


def _json_object(value: str, field: str) -> dict[str, Any]:
    try:
        parsed = json.loads(value)
    except json.JSONDecodeError as exc:
        raise ValueError(f"{field} must be valid JSON") from exc
    if not isinstance(parsed, dict):
        raise ValueError(f"{field} must be a JSON object")
    return parsed


def propose_theme(
    title: str,
    slug: str,
    description: str,
    rationale: str,
    evidence_json: str = "{}",
    provenance_json: str = "{}",
    idempotency_key: str = "",
) -> None:
    """Propose a Theme for human approval."""
    _print(
        _client().propose_theme(
            title=title,
            slug=slug,
            description=description,
            rationale=rationale,
            evidence=_json_object(evidence_json, "evidence_json"),
            provenance=_json_object(provenance_json, "provenance_json"),
            idempotency_key=idempotency_key,
        )
    )


def read_theme_proposal(proposal_id: str) -> None:
    """Read one Theme proposal and its decision status."""
    _print(_client().read_theme_proposal(proposal_id))


def assign_theme(
    object_id: str,
    theme_id: str,
    description: str,
    provenance_json: str = "{}",
    protected: bool = False,
    idempotency_key: str = "",
) -> None:
    """Assign an existing approved Theme to an Object."""
    _print(
        _client().assign_theme(
            object_id=object_id,
            theme_id=theme_id,
            description=description,
            provenance=_json_object(provenance_json, "provenance_json"),
            protected=protected,
            idempotency_key=idempotency_key,
        )
    )


def unassign_theme(
    assignment_id: str, expected_revision: int, idempotency_key: str = ""
) -> None:
    """Archive one existing Theme assignment."""
    _print(
        _client().unassign_theme(
            assignment_id,
            expected_revision=expected_revision,
            idempotency_key=idempotency_key,
        )
    )


def _bounded_int(minimum: int, maximum: int):
    def parse(value: str) -> int:
        parsed = int(value)
        if not minimum <= parsed <= maximum:
            raise argparse.ArgumentTypeError(
                f"must be between {minimum} and {maximum}"
            )
        return parsed

    return parse


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="centaur-context",
        description="Read shared context and create explicitly authorized Notes.",
    )
    commands = parser.add_subparsers(dest="command", required=True)

    command = commands.add_parser("get-context")
    command.add_argument("query")
    command.add_argument("--chat-object-id", required=True)
    command.add_argument("--kind")
    command.add_argument("--limit", type=_bounded_int(1, 10), default=10)

    command = commands.add_parser("search-objects")
    command.add_argument("query")
    command.add_argument("--kind")
    command.add_argument("--limit", type=_bounded_int(1, 100), default=20)

    command = commands.add_parser("read-object")
    command.add_argument("id")

    for name in ("search-sources", "search-notes"):
        command = commands.add_parser(name)
        command.add_argument("query")
        command.add_argument("--limit", type=_bounded_int(1, 100), default=20)
        command.add_argument("--cursor")

    command = commands.add_parser("read-source")
    command.add_argument("source_id")

    command = commands.add_parser("read-source-content")
    command.add_argument("source_id")
    command.add_argument("--version", type=_bounded_int(1, 2**31 - 1))
    command.add_argument("--offset", type=_bounded_int(0, 2**31 - 1), default=0)
    command.add_argument("--limit", type=_bounded_int(1, 20_000), default=8_000)

    command = commands.add_parser("read-note")
    command.add_argument("note_id")

    command = commands.add_parser("create-note")
    command.add_argument("title")
    command.add_argument("--description", required=True)
    command.add_argument("--content", required=True)
    command.add_argument("--content-format", default="markdown")
    command.add_argument("--provenance-json", default="{}")
    command.add_argument("--idempotency-key", required=True)

    for name in (
        "source-intake-validate",
        "source-intake-commit",
        "source-intake-status",
    ):
        command = commands.add_parser(name)
        command.add_argument("manifest_file")

    command = commands.add_parser("list-themes")
    command.add_argument("--slug")

    command = commands.add_parser("read-theme")
    command.add_argument("theme_id")

    command = commands.add_parser("list-theme-objects")
    command.add_argument("theme_id")
    command.add_argument("--kind")
    command.add_argument("--limit", type=_bounded_int(1, 100), default=20)

    command = commands.add_parser("propose-theme")
    command.add_argument("title")
    command.add_argument("--slug", required=True)
    command.add_argument("--description", required=True)
    command.add_argument("--rationale", required=True)
    command.add_argument("--evidence-json", default="{}")
    command.add_argument("--provenance-json", default="{}")
    command.add_argument("--idempotency-key", required=True)

    command = commands.add_parser("read-theme-proposal")
    command.add_argument("proposal_id")

    command = commands.add_parser("assign-theme")
    command.add_argument("object_id")
    command.add_argument("theme_id")
    command.add_argument("--description", required=True)
    command.add_argument("--provenance-json", default="{}")
    command.add_argument("--protected", action="store_true")
    command.add_argument("--idempotency-key", required=True)

    command = commands.add_parser("unassign-theme")
    command.add_argument("assignment_id")
    command.add_argument("--expected-revision", type=int, required=True)
    command.add_argument("--idempotency-key", required=True)
    return parser


def app(argv: list[str] | None = None) -> None:
    values = vars(_build_parser().parse_args(argv))
    command = values.pop("command").replace("-", "_")
    globals()[command](**values)


def main() -> None:
    try:
        app()
    except (RuntimeError, ValueError) as exc:
        print(json.dumps({"error": str(exc)}, ensure_ascii=False))
        raise SystemExit(1) from None


if __name__ == "__main__":
    main()

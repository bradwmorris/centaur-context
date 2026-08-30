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


def read_source(source_id: str) -> None:
    """Read Source metadata without loading its long-form content."""
    _print(_client().read_source(source_id))


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

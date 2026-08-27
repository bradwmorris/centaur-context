"""Small JSON CLI for the Centaur OS agent tool."""

from __future__ import annotations

import json
from typing import Any

import typer

from .client import _client

app = typer.Typer(
    name="centaur-os",
    help="Read and update shared Centaur OS context and tasks.",
    no_args_is_help=True,
)


def _json_object(value: str, field: str) -> dict[str, Any]:
    try:
        parsed = json.loads(value)
    except json.JSONDecodeError as exc:
        raise typer.BadParameter(f"{field} must be valid JSON") from exc
    if not isinstance(parsed, dict):
        raise typer.BadParameter(f"{field} must be a JSON object")
    return parsed


def _print(value: Any) -> None:
    print(json.dumps(value, indent=2, ensure_ascii=False, default=str))


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


@app.command("create-object")
def create_object(
    kind: str = typer.Option(..., help="Non-Task Object kind supported by the API."),
    title: str = typer.Option(...),
    body: str = typer.Option(""),
    provenance_json: str = typer.Option("{}", help="Provenance JSON object."),
    idempotency_key: str = typer.Option(..., help="Stable unique key for safe retries."),
) -> None:
    """Create one shared Object."""
    _print(
        _client().create_object(
            kind, title, body, _json_object(provenance_json, "provenance"), idempotency_key
        )
    )


@app.command("update-object")
def update_object(
    id: str = typer.Argument(..., help="Object UUID."),
    expected_revision: int = typer.Option(..., min=1),
    changes_json: str = typer.Option(..., help="Allowed field changes as a JSON object."),
    idempotency_key: str = typer.Option(..., help="Stable unique key for safe retries."),
) -> None:
    """Update one record without silently overwriting a newer revision."""
    _print(
        _client().update_object(
            id,
            expected_revision,
            _json_object(changes_json, "changes"),
            idempotency_key,
        )
    )


@app.command("list-connections")
def list_connections(id: str = typer.Argument(..., help="Object UUID.")) -> None:
    """List relationships for one record."""
    _print(_client().list_connections(id))


@app.command("create-connection")
def create_connection(
    source_id: str = typer.Option(...),
    kind: str = typer.Option(..., help="supports, depends_on, references, part_of, or supersedes."),
    target_id: str = typer.Option(...),
    reason: str = typer.Option(...),
    provenance_json: str = typer.Option("{}", help="Provenance JSON object."),
    idempotency_key: str = typer.Option(..., help="Stable unique key for safe retries."),
) -> None:
    """Create one explained relationship."""
    _print(
        _client().create_connection(
            source_id,
            kind,
            target_id,
            reason,
            _json_object(provenance_json, "provenance"),
            idempotency_key,
        )
    )


@app.command("list-tasks")
def list_tasks(
    status: str | None = typer.Option(None),
    agent_eligible: bool | None = typer.Option(None, "--agent-eligible/--any-eligibility"),
    limit: int = typer.Option(20, min=1, max=100),
) -> None:
    """List shared tasks."""
    _print(_client().list_tasks(status=status, agent_eligible=agent_eligible, limit=limit))


@app.command("read-task")
def read_task(id: str = typer.Argument(..., help="Task object UUID.")) -> None:
    """Read one shared task."""
    _print(_client().read_task(id))


@app.command("update-task")
def update_task(
    id: str = typer.Argument(..., help="Task object UUID."),
    expected_revision: int = typer.Option(..., min=1),
    changes_json: str = typer.Option(..., help="Allowed field changes as a JSON object."),
    idempotency_key: str = typer.Option(..., help="Stable unique key for safe retries."),
) -> None:
    """Update one task without silently overwriting a newer revision."""
    _print(
        _client().update_task(
            id,
            expected_revision,
            _json_object(changes_json, "changes"),
            idempotency_key,
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

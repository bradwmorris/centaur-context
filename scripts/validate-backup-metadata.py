#!/usr/bin/env python3

import json
import sys
from pathlib import Path


def fail(message: str) -> None:
    raise SystemExit(f"invalid backup metadata: {message}")


def allowed_database(name: object) -> bool:
    return isinstance(name, str) and (
        name == "centaur_context"
        or "centaur_context_test" in name
        or name == "centaur_os"
        or "centaur_os_test" in name
    )


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: validate-backup-metadata.py BACKUP.json")
    try:
        metadata = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        fail(str(exc))
    if not isinstance(metadata, dict):
        fail("expected one JSON object")
    if metadata.get("product") not in {"centaur-context", "centaur-os"}:
        fail("unsupported product discriminator")
    if metadata.get("format") != "pg_dump-custom":
        fail("unsupported backup format")
    if not allowed_database(metadata.get("database")):
        fail("unexpected source database")
    schema = metadata.get("schema_version")
    if not isinstance(schema, int) or isinstance(schema, bool) or not 1 <= schema <= 17:
        fail("unsupported schema version")
    if not isinstance(metadata.get("product_version"), str) or not metadata["product_version"]:
        fail("missing product version")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3

import json
import re
import sys
import tomllib
from pathlib import Path


def fail(message: str) -> None:
    raise SystemExit(f"invalid backup metadata: {message}")


def allowed_database(name: object) -> bool:
    return isinstance(name, str) and re.fullmatch(
        r"(?:centaur_context|centaur_os)(?:_[a-z0-9_]+)?", name
    ) is not None


def supported_schema_version() -> int:
    compatibility = Path(__file__).resolve().parents[1] / "compatibility.toml"
    with compatibility.open("rb") as handle:
        value = tomllib.load(handle)["centaur_context"]["database_schema_version"]
    if not isinstance(value, int) or isinstance(value, bool) or value < 1:
        fail("invalid compatibility schema version")
    return value


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
    if (
        not isinstance(schema, int)
        or isinstance(schema, bool)
        or not 1 <= schema <= supported_schema_version()
    ):
        fail("unsupported schema version")
    if not isinstance(metadata.get("product_version"), str) or not metadata["product_version"]:
        fail("missing product version")


if __name__ == "__main__":
    main()

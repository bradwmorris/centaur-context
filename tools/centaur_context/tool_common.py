"""Shared command helpers for the three universal Context tools."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any


def print_json(value: Any) -> None:
    print(json.dumps(value, indent=2, ensure_ascii=False, default=str))


def read_json_object(value: str | None, path: str | None) -> dict[str, Any]:
    if bool(value) == bool(path):
        raise ValueError("provide exactly one of --json or --file")
    if path:
        value = Path(path).read_text(encoding="utf-8")
    parsed = json.loads(value or "")
    if not isinstance(parsed, dict):
        raise ValueError("input must be a JSON object")
    return parsed


def run(action: Any) -> None:
    try:
        print_json(action())
    except (OSError, RuntimeError, ValueError, json.JSONDecodeError) as exc:
        print(json.dumps({"error": str(exc)}, ensure_ascii=False))
        raise SystemExit(1) from None

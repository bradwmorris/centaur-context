"""The universal Context atomic write tool."""

from __future__ import annotations

import argparse

from .client import _client
from .tool_common import read_json_object, run


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="context_apply",
        description="Apply one atomic, idempotent Centaur Context write batch.",
    )
    parser.add_argument("--json")
    parser.add_argument("--file")
    return parser


def app(argv: list[str] | None = None) -> None:
    values = _parser().parse_args(argv)
    request = read_json_object(values.json, values.file)
    run(lambda: _client().context_apply(request))


def main() -> None:
    app()

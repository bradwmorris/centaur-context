"""Workflow-only registration shim for the purpose-bound Context client."""

from __future__ import annotations

import importlib.util
from pathlib import Path
from types import ModuleType


def _load_shared_client() -> ModuleType:
    path = Path(__file__).resolve().parents[1] / "centaur_context" / "client.py"
    spec = importlib.util.spec_from_file_location(
        "centaur_context_workflow_shared_client", path
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load the shared Centaur Context client")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


_shared = _load_shared_client()
CentaurContextClient = _shared.CentaurContextClient


def _client():
    return _shared._client()


def main() -> None:
    raise SystemExit(
        "centaur-context is a workflow-only RPC adapter; call it through Centaur workflows"
    )

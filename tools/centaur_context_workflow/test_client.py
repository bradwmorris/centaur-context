import importlib.util
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def _module():
    path = ROOT / "tools/centaur_context_workflow/client.py"
    spec = importlib.util.spec_from_file_location("workflow_adapter_test", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_adapter_reuses_shared_client_and_preserves_workflow_methods():
    client = _module()._client()
    for method in (
        "workflow_run_start",
        "workflow_run_trace",
        "workflow_run_finish",
        "source_intake_resolve_connections",
        "source_intake_validate",
        "source_intake_commit",
        "source_intake_wait",
    ):
        assert callable(getattr(client, method))

"""The file helper verifies real bytes and reconciles an uncertain upload."""
import hashlib
from pathlib import Path

import pytest

from tools.centaur_context.visual import attach


class Client:
    def __init__(self, data: bytes, fail_after_commit: bool = False):
        self.data = data
        self.fail_after_commit = fail_after_commit
        self.artifacts = []
        self.revision = 1
        self.calls = 0

    def context_read(self, _ids, include):
        assert include == ["artifacts"]
        return {"objects": [{"object": {"kind": "source", "lifecycle": "active", "revision": self.revision}, "artifacts": self.artifacts}]}

    def attach_visual_bytes(self, source_id, data, **fields):
        self.calls += 1
        assert data == self.data
        self.revision += 1
        artifact = {
            "id": "visual-1", "object_id": source_id, "kind": "research_visual",
            "media_type": fields["media_type"], "sha256": hashlib.sha256(data).hexdigest(),
            "size_bytes": len(data),
            "metadata": {"creation_key": fields["idempotency_key"], "width": 100, "height": 80},
        }
        self.artifacts.insert(0, artifact)
        if self.fail_after_commit:
            raise RuntimeError("connection lost after commit")
        return {"artifact": artifact, "viewer_path": f"/sources/{source_id}/artifacts/visual-1"}

    def read_visual_bytes(self, _id):
        return self.data


def test_attach_reconciles_ambiguous_response_and_repeated_call(tmp_path: Path):
    data = b"\x89PNG\r\n\x1a\nsynthetic"
    path = tmp_path / "visual.png"
    path.write_bytes(data)
    client = Client(data, fail_after_commit=True)
    args = {"title": "Simple loop", "description": "One synthetic relationship.", "document_key": "loop"}
    first = attach(client, "source-1", path, **args)
    second = attach(client, "source-1", path, **args)
    assert first == second
    assert first["sha256"] == hashlib.sha256(data).hexdigest()
    assert client.calls == 1


def test_attach_rejects_missing_or_non_image_file(tmp_path: Path):
    client = Client(b"")
    args = {"title": "Visual", "description": "Test.", "document_key": "test"}
    with pytest.raises(FileNotFoundError):
        attach(client, "source-1", tmp_path / "missing.png", **args)
    path = tmp_path / "bad.svg"
    path.write_text("<svg></svg>")
    with pytest.raises(ValueError, match="PNG or JPEG"):
        attach(client, "source-1", path, **args)
    assert client.calls == 0

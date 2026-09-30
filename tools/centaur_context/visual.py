"""Attach and verify a local research visual without putting bytes in a prompt."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import stat
from typing import Any

from .client import _client
from .tool_common import run


def _file(path: Path) -> tuple[bytes, str]:
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode):
        raise ValueError("visual path must be a regular file")
    if not 0 < info.st_size <= 20 * 1024 * 1024:
        raise ValueError("visual must be between 1 byte and 20 MiB")
    data = path.read_bytes()
    if len(data) != info.st_size:
        raise ValueError("visual changed while it was read")
    if data.startswith(b"\x89PNG\r\n\x1a\n"):
        return data, "image/png"
    if data.startswith(b"\xff\xd8\xff"):
        return data, "image/jpeg"
    raise ValueError("visual must be a PNG or JPEG file")


def _receipt(source_id: str, artifact: dict[str, Any], viewer_path: str) -> dict[str, Any]:
    metadata = artifact.get("metadata") or {}
    return {
        "source_id": source_id,
        "artifact_id": artifact["id"],
        "viewer_path": viewer_path,
        "media_type": artifact["media_type"],
        "width": metadata["width"],
        "height": metadata["height"],
        "sha256": artifact["sha256"],
        "size_bytes": artifact["size_bytes"],
    }


def attach(
    client: Any, source_id: str, path: Path, *, title: str,
    description: str, document_key: str,
    supersedes_artifact_id: str | None = None, method: str | None = None,
) -> dict[str, Any]:
    data, media_type = _file(path)
    digest = hashlib.sha256(data).hexdigest()
    identity = json.dumps({
        "source_id": source_id, "sha256": digest, "title": title,
        "description": description, "document_key": document_key,
        "predecessor": supersedes_artifact_id, "method": method,
    }, sort_keys=True, ensure_ascii=False).encode()
    key = "visual-" + hashlib.sha256(identity).hexdigest()
    snapshot = client.context_read([source_id], include=["artifacts"])["objects"][0]
    source = snapshot["object"]
    if source["kind"] != "source" or source["lifecycle"] != "active":
        raise ValueError("visual parent must be an active Source")

    def reconcile(artifacts: list[dict[str, Any]]) -> dict[str, Any] | None:
        for artifact in artifacts:
            metadata = artifact.get("metadata") or {}
            if metadata.get("creation_key") != key:
                continue
            if (artifact["kind"] != "research_visual" or artifact["sha256"] != digest
                    or artifact["object_id"] != source_id):
                raise RuntimeError("visual idempotency key matched a different Artifact")
            if client.read_visual_bytes(artifact["id"]) != data:
                raise RuntimeError("visual readback differs from the local image")
            return _receipt(source_id, artifact, f"/sources/{source_id}/artifacts/{artifact['id']}")
        return None

    existing = reconcile(snapshot.get("artifacts") or [])
    if existing:
        return existing
    try:
        result = client.attach_visual_bytes(
            source_id, data, title=title, description=description,
            document_key=document_key, media_type=media_type,
            expected_revision=source["revision"], idempotency_key=key,
            supersedes_artifact_id=supersedes_artifact_id, method=method,
        )
    except RuntimeError:
        refreshed = client.context_read([source_id], include=["artifacts"])["objects"][0]
        found = reconcile(refreshed.get("artifacts") or [])
        if found:
            return found
        raise
    artifact = result["artifact"]
    if artifact["sha256"] != digest or artifact["size_bytes"] != len(data):
        raise RuntimeError("visual receipt did not match the local image")
    if client.read_visual_bytes(artifact["id"]) != data:
        raise RuntimeError("visual readback differs from the local image")
    return _receipt(source_id, artifact, result["viewer_path"])


def app(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(prog="context_visual", description="Attach and verify a local Source visual")
    parser.add_argument("source_id")
    parser.add_argument("path", type=Path)
    parser.add_argument("--title", required=True)
    parser.add_argument("--description", required=True)
    parser.add_argument("--document-key", required=True)
    parser.add_argument("--supersedes-artifact-id")
    parser.add_argument("--method", help="Actual generation or rendering method")
    args = parser.parse_args(argv)
    run(lambda: attach(
        _client(), args.source_id, args.path, title=args.title,
        description=args.description, document_key=args.document_key,
        supersedes_artifact_id=args.supersedes_artifact_id, method=args.method,
    ))


def main() -> None:
    app()

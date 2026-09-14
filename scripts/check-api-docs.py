#!/usr/bin/env python3

"""Fail when a documented supported route moves or disappears."""

from collections import Counter
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
SURFACES = {
    # source file, router start, router end, nested prefix, expected routes
    "Agent": ("src/api.rs", "pub fn agent_router", "pub fn note_write_router", "/api/v2", 17),
    "Note/Task writer": ("src/api.rs", "pub fn note_write_router", "fn service_router", "/api/v2", 3),
    "Slack ingestion": ("src/ingest.rs", "pub fn router", ".with_state", "", 2),
    "Source intake": ("src/source_intake.rs", "pub fn router", ".with_state", "", 7),
}
ROW = re.compile(
    r"^\| (Agent|Note/Task writer|Slack ingestion|Source intake) "
    r"\| (GET|POST|PATCH) \| `([^`]+)` \|",
    re.MULTILINE,
)


def failures(document: str, sources: dict[str, str]) -> list[str]:
    errors = []
    routes = ROW.findall(document)
    counts = Counter(surface for surface, _, _ in routes)
    for surface, (*_, expected) in SURFACES.items():
        if counts[surface] != expected:
            errors.append(f"{surface} documents {counts[surface]} routes; expected {expected}")

    for surface, method, public_path in routes:
        source, start, end, prefix, _ = SURFACES[surface]
        text = sources[source]
        if start not in text or end not in text.split(start, 1)[1]:
            errors.append(f"router markers missing in {source}: {start}")
            continue
        router = text.split(start, 1)[1].split(end, 1)[0]
        local_path = public_path.removeprefix(prefix)
        pattern = (
            rf'\.route\s*\(\s*"{re.escape(local_path)}"\s*,\s*'
            rf"(?:axum::routing::)?{method.lower()}\s*\("
        )
        if not re.search(pattern, router):
            errors.append(
                f"route missing from expected router: {source} {method} {public_path}"
            )
    return errors


def main() -> None:
    document = (ROOT / "docs/api.md").read_text(encoding="utf-8")
    sources = {
        source: (ROOT / source).read_text(encoding="utf-8")
        for source, *_ in SURFACES.values()
    }
    errors = failures(document, sources)
    if errors:
        raise SystemExit("\n".join(errors))

    # Prove that moving a documented route out of its expected router is rejected.
    moved = dict(sources)
    moved["src/api.rs"] = moved["src/api.rs"].replace(
        '"/context"', '"/moved-context"', 1
    )
    if not failures(document, moved):
        raise SystemExit("route-drift self-test did not detect a moved route")
    print("API documentation route checks passed.")


if __name__ == "__main__":
    main()

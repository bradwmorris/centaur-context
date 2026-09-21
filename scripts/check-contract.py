#!/usr/bin/env python3
"""Validate the canonical contract and deterministic generated artifacts."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONTRACT = ROOT / "contract/context-contract.json"
SCHEMA = ROOT / "contract/context-contract.schema.json"
INSTRUCTIONS = ROOT / "generated/context-agent-instructions.md"
RUNTIME_INSTRUCTIONS = ROOT / "services/sandbox/SYSTEM_PROMPT.md"

EXPECTED_TYPES = {"task", "chat", "user", "entity", "memory", "source", "note", "theme"}
EXPECTED_TOOLS = {"context_search", "context_read", "context_apply"}
EXPECTED_WRITABLE = {"task", "entity", "source", "note", "theme"}
EXPECTED_APPLY_OPERATIONS = {
    "create_object",
    "update_object",
    "archive_object",
    "create_connection",
    "update_connection",
    "archive_connection",
    "append_artifact",
}


def fail(message: str) -> None:
    raise SystemExit(message)


def validate(value: object, rule: dict[str, object], root: dict[str, object], path: str = "$") -> None:
    reference = rule.get("$ref")
    if isinstance(reference, str):
        if not reference.startswith("#/"):
            fail(f"unsupported schema reference at {path}: {reference}")
        target: object = root
        for part in reference[2:].split("/"):
            target = target[part]  # type: ignore[index]
        validate(value, target, root, path)  # type: ignore[arg-type]
        return
    if "const" in rule and value != rule["const"]:
        fail(f"{path} does not match its required constant")
    expected = rule.get("type")
    type_ok = {
        "object": isinstance(value, dict),
        "array": isinstance(value, list),
        "string": isinstance(value, str),
        "boolean": isinstance(value, bool),
        "integer": isinstance(value, int) and not isinstance(value, bool),
    }.get(expected, True)
    if not type_ok:
        fail(f"{path} must be {expected}")
    if isinstance(value, dict):
        required = rule.get("required", [])
        for key in required:  # type: ignore[union-attr]
            if key not in value:
                fail(f"{path} is missing required key {key}")
        minimum = rule.get("minProperties")
        if isinstance(minimum, int) and len(value) < minimum:
            fail(f"{path} has too few properties")
        properties = rule.get("properties", {})
        additional = rule.get("additionalProperties", True)
        for key, child in value.items():
            child_rule = properties.get(key) if isinstance(properties, dict) else None
            if child_rule is None:
                if additional is False:
                    fail(f"{path} contains unknown key {key}")
                child_rule = additional if isinstance(additional, dict) else None
            if isinstance(child_rule, dict):
                validate(child, child_rule, root, f"{path}.{key}")
    if isinstance(value, list):
        minimum = rule.get("minItems")
        if isinstance(minimum, int) and len(value) < minimum:
            fail(f"{path} has too few items")
        if rule.get("uniqueItems") and len({json.dumps(item, sort_keys=True) for item in value}) != len(value):
            fail(f"{path} contains duplicate items")
        item_rule = rule.get("items")
        if isinstance(item_rule, dict):
            for index, child in enumerate(value):
                validate(child, item_rule, root, f"{path}[{index}]")
    if isinstance(value, str):
        pattern = rule.get("pattern")
        if isinstance(pattern, str) and re.search(pattern, value) is None:
            fail(f"{path} does not match {pattern}")
        minimum = rule.get("minLength")
        if isinstance(minimum, int) and len(value) < minimum:
            fail(f"{path} is too short")
    if isinstance(value, int) and not isinstance(value, bool):
        minimum = rule.get("minimum")
        if isinstance(minimum, int) and value < minimum:
            fail(f"{path} is below its minimum")


def main() -> int:
    contract = json.loads(CONTRACT.read_text())
    schema = json.loads(SCHEMA.read_text())
    if schema.get("$schema") != "https://json-schema.org/draft/2020-12/schema":
        fail("contract schema must declare JSON Schema 2020-12")
    validate(contract, schema, schema)
    if set(contract["object_types"]) != EXPECTED_TYPES:
        fail("contract Object types do not match the canonical ontology")
    writable = {
        name for name, spec in contract["object_types"].items() if spec["interactive_write"]
    }
    if writable != EXPECTED_WRITABLE:
        fail("contract interactive-write classification drifted")
    if set(contract["tools"]) != EXPECTED_TOOLS:
        fail("contract must expose exactly the three universal Context tools")
    if set(contract["tools"]["context_apply"]["operations"]) != EXPECTED_APPLY_OPERATIONS:
        fail("contract context_apply operations drifted from the universal API")
    if contract["tools"]["context_apply"].get("object_reference") != {
        "exactly_one_of": ["object_id", "local_ref"]
    }:
        fail("contract context_apply Object reference shape drifted")
    if len(contract["connection_kinds"]) != len(set(contract["connection_kinds"])):
        fail("connection kinds must be unique")
    instructions = INSTRUCTIONS.read_text()
    if RUNTIME_INSTRUCTIONS.read_text() != instructions:
        fail("runtime Context instructions drifted from the generated fragment")
    for value in (contract["contract_version"], contract["ontology_version"], *EXPECTED_TOOLS):
        if value not in instructions:
            fail(f"generated instructions are missing {value}")
    direct_examples = (
        "context_search 'words to find' --object-type task --limit 10",
        "context_read OBJECT_UUID --include connections",
        "context_apply --file REQUEST.json",
        "context_apply --example",
        "context_apply --schema",
    )
    for example in direct_examples:
        if example not in instructions:
            fail(f"generated instructions are missing direct usage: {example}")
    if "do not inspect their executable or source code" not in instructions:
        fail("generated instructions must forbid tool implementation discovery")
    if len(instructions) > 1_800:
        fail("generated Context instructions exceed the 1,800-character budget")
    print("Context contract and generated instructions are consistent")
    return 0


if __name__ == "__main__":
    sys.exit(main())

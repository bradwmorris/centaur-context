from __future__ import annotations

import json

from tools.centaur_context import apply, read, search


def test_search_help_gives_a_direct_example(capsys):
    try:
        search.app(["--help"])
    except SystemExit as exc:
        assert exc.code == 0
    output = capsys.readouterr().out
    assert "context_search 'deployment decision' --object-type task --limit 10" in output
    assert "Words to search for" in output


def test_read_help_gives_a_direct_example(capsys):
    try:
        read.app(["--help"])
    except SystemExit as exc:
        assert exc.code == 0
    output = capsys.readouterr().out
    assert "context_read 00000000-0000-0000-0000-000000000001" in output
    assert "One or more Object UUIDs" in output


def test_apply_help_explains_request_and_operations(capsys):
    try:
        apply.app(["--help"])
    except SystemExit as exc:
        assert exc.code == 0
    output = capsys.readouterr().out
    assert "required: contract_version, idempotency_key, operations" in output
    assert "create_object, update_object, archive_object, create_connection" in output
    assert "context_apply --example" in output
    assert "context_apply --schema" in output


def test_apply_example_is_local_and_validation_only(monkeypatch, capsys):
    monkeypatch.setattr(apply, "_client", lambda: (_ for _ in ()).throw(AssertionError("API called")))
    apply.app(["--example"])
    request = json.loads(capsys.readouterr().out)
    assert request["contract_version"] == "1.0.0"
    assert request["validate_only"] is True
    assert [operation["operation"] for operation in request["operations"]] == [
        "create_object",
        "create_connection",
    ]


def test_apply_schema_reads_contract_without_applying(monkeypatch, capsys):
    class Client:
        def context_contract(self):
            return {
                "contract_version": "1.0.0",
                "tools": {
                    "context_apply": {
                        "operations": {
                            "create_object": {
                                "required": ["local_ref", "kind", "title", "description"]
                            }
                        }
                    }
                },
            }

        def context_apply(self, _request):
            raise AssertionError("write attempted")

    monkeypatch.setattr(apply, "_client", Client)
    apply.app(["--schema"])
    contract = json.loads(capsys.readouterr().out)
    assert contract["contract_version"] == "1.0.0"
    assert contract["tools"]["context_apply"]["operations"]["create_object"][
        "required"
    ] == ["local_ref", "kind", "title", "description"]

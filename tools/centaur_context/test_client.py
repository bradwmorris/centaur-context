from __future__ import annotations

import httpx
import pytest
from centaur_tool_centaur_context import cli
from centaur_tool_centaur_context import client as client_module
from centaur_tool_centaur_context.client import CentaurContextClient


def json_response(data, status_code: int = 200) -> httpx.Response:
    return httpx.Response(status_code, json=data)


def make_client(handler, **overrides) -> CentaurContextClient:
    return CentaurContextClient(
        base_url="http://centaur-context.test:8081",
        note_write_url="http://centaur-context.test:8084",
        intake_url="http://centaur-context.test:8085",
        source_intake_url="http://centaur-context.test:8086",
        theme_proposal_url="http://centaur-context.test:8087",
        theme_proposal_token="theme-proposal-token",
        external_action_url="http://centaur-context.test:8088",
        external_action_token="external-action-token",
        console_url="http://centaur-console.test:3000",
        token="placeholder-token",
        principal_id="principal-1",
        thread_key="slack:team:channel:thread",
        transport=httpx.MockTransport(handler),
        **overrides,
    )


def test_search_objects_sends_scoped_agent_context() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return json_response(
            {"data": {"query": "shared", "retrieval": "full_text", "objects": []}}
        )

    result = make_client(handler).search_objects("shared", kind="memory", limit=500)

    assert result == {"query": "shared", "retrieval": "full_text", "objects": []}
    request = requests[0]
    assert request.url.path == "/api/v1/search/objects"
    assert request.url.params["q"] == "shared"
    assert request.url.params["kind"] == "memory"
    assert request.url.params["limit"] == "100"
    assert request.headers["authorization"] == "Bearer placeholder-token"
    assert request.headers["x-centaur-principal-id"] == "principal-1"
    assert request.headers["x-centaur-thread-key"] == "slack:team:channel:thread"


def test_get_context_caps_the_packet_at_ten_objects() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return json_response(
            {"data": {"query": "shared", "retrieval": "full_text", "objects": []}}
        )

    result = make_client(handler).get_context(
        "shared", chat_object_id="11111111-1111-4111-8111-111111111111", limit=500
    )

    assert result["query"] == "shared"
    assert requests[0].url.path == "/api/v1/context"
    assert requests[0].url.params["limit"] == "10"
    assert (
        requests[0].url.params["chat_object_id"]
        == "11111111-1111-4111-8111-111111111111"
    )
    assert requests[0].method == "GET"


def test_theme_reads_use_the_standard_scoped_agent_api() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return json_response({"data": []})

    client = make_client(handler)
    assert client.list_themes(slug="research-ai") == []
    assert client.list_theme_objects("theme/id", kind="source", limit=500) == []

    assert requests[0].url.path == "/api/v1/themes"
    assert requests[0].url.params["slug"] == "research-ai"
    assert requests[1].url.raw_path.split(b"?", 1)[0] == b"/api/v1/themes/theme%2Fid/objects"
    assert requests[1].url.params["kind"] == "source"
    assert requests[1].url.params["limit"] == "100"


def test_theme_proposal_uses_the_narrow_credential_and_idempotency() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return json_response({"data": {"status": "pending"}}, 201)

    result = make_client(handler).propose_theme(
        title="AI Infrastructure",
        slug="ai-infrastructure",
        description="Research about infrastructure used to build and operate AI systems.",
        rationale="A recurring research vertical needs a stable retrieval boundary.",
        evidence={"source_ids": ["source-1"]},
        idempotency_key="proposal-1",
    )

    assert result["status"] == "pending"
    request = requests[0]
    assert request.url.port == 8087
    assert request.url.path == "/api/v1/theme-proposals"
    assert request.headers["authorization"] == "Bearer theme-proposal-token"
    assert request.headers["idempotency-key"] == "proposal-1"


def test_get_context_requires_a_chat_object_before_request() -> None:
    def handler(_request: httpx.Request) -> httpx.Response:
        raise AssertionError("request should not be sent")

    with pytest.raises(ValueError, match="chat_object_id is required"):
        make_client(handler).get_context("shared", chat_object_id="  ")


def test_resolves_principal_from_console_permissions() -> None:
    hosts: list[str] = []

    def handler(request: httpx.Request) -> httpx.Response:
        hosts.append(request.url.host or "")
        if request.url.host == "centaur-console.test":
            return json_response({"data": {"principal_id": "resolved-principal"}})
        return json_response({"data": []})

    client = CentaurContextClient(
        base_url="http://centaur-context.test:8081",
        console_url="http://centaur-console.test:3000",
        token="placeholder-token",
        thread_key="thread-1",
        transport=httpx.MockTransport(handler),
    )
    client.search_objects("context")

    assert hosts == ["centaur-console.test", "centaur-context.test"]


def test_search_rejects_an_empty_query_before_request() -> None:
    def handler(_request: httpx.Request) -> httpx.Response:
        raise AssertionError("request should not be sent")

    with pytest.raises(ValueError, match="query is required"):
        make_client(handler).search_objects("   ")


def test_search_sources_sends_authentication_and_pagination() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return json_response(
            {
                "data": {
                    "items": [{"id": "source-2", "excerpt": "bounded evidence"}],
                    "next_cursor": "source-2",
                }
            }
        )

    result = make_client(handler).search_sources(
        "evidence", limit=500, cursor=" source-1 "
    )

    assert result["next_cursor"] == "source-2"
    request = requests[0]
    assert request.method == "GET"
    assert request.url.path == "/api/v1/search/sources"
    assert request.url.params["q"] == "evidence"
    assert request.url.params["limit"] == "100"
    assert request.url.params["cursor"] == "source-1"
    assert request.headers["authorization"] == "Bearer placeholder-token"
    assert request.headers["x-centaur-principal-id"] == "principal-1"
    assert request.headers["x-centaur-thread-key"] == "slack:team:channel:thread"


def test_search_sources_omits_empty_cursor_and_requires_query() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return json_response({"data": {"items": [], "next_cursor": None}})

    make_client(handler).search_sources("paper", cursor="  ")

    assert "cursor" not in requests[0].url.params
    with pytest.raises(ValueError, match="query is required"):
        make_client(handler).search_sources("  ")
    assert len(requests) == 1


def test_read_source_reads_metadata_without_content() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return json_response({"data": {"id": "source/id", "title": "A paper"}})

    result = make_client(handler).read_source(
        "source/id", thread_key="workflow:run-123"
    )

    assert result["title"] == "A paper"
    assert requests[0].method == "GET"
    assert requests[0].url.raw_path == b"/api/v1/sources/source%2Fid"
    assert requests[0].url.query == b""
    assert requests[0].headers["x-centaur-thread-key"] == "workflow:run-123"


def test_read_source_can_target_an_explicit_context_service() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return json_response({"data": {"id": "source-1"}})

    result = make_client(handler).read_source(
        "source-1",
        thread_key="workflow:run-123",
        base_url="http://centaur-context-enyu.test:8081/",
    )

    assert result["id"] == "source-1"
    assert requests[0].url == httpx.URL(
        "http://centaur-context-enyu.test:8081/api/v1/sources/source-1"
    )


def test_read_source_content_sends_a_bounded_window() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return json_response(
            {
                "data": {
                    "version": 3,
                    "offset": 120,
                    "text": "bounded evidence",
                    "next_offset": 136,
                }
            }
        )

    result = make_client(handler).read_source_content(
        "source/id", version=3, offset=120, limit=50_000
    )

    assert result["next_offset"] == 136
    request = requests[0]
    assert request.method == "GET"
    assert request.url.raw_path.split(b"?", 1)[0] == (
        b"/api/v1/sources/source%2Fid/content"
    )
    assert request.url.params["version"] == "3"
    assert request.url.params["offset"] == "120"
    assert request.url.params["limit"] == "20000"


def test_read_source_content_defaults_to_current_version() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return json_response({"data": {"version": 2, "text": "current"}})

    make_client(handler).read_source_content("source-1")

    assert "version" not in requests[0].url.params
    assert requests[0].url.params["offset"] == "0"
    assert requests[0].url.params["limit"] == "8000"


@pytest.mark.parametrize(
    ("kwargs", "message"),
    [
        ({"offset": -1}, "offset must be zero or greater"),
        ({"limit": 0}, "limit must be at least one"),
        ({"version": 0}, "version must be at least one"),
    ],
)
def test_read_source_content_rejects_invalid_bounds_before_request(
    kwargs: dict[str, int], message: str
) -> None:
    def handler(_request: httpx.Request) -> httpx.Response:
        raise AssertionError("request should not be sent")

    with pytest.raises(ValueError, match=message):
        make_client(handler).read_source_content("source-1", **kwargs)


def test_read_source_content_preserves_missing_version_error() -> None:
    def handler(request: httpx.Request) -> httpx.Response:
        assert request.method == "GET"
        return json_response(
            {
                "error": {
                    "code": "source_content_not_found",
                    "message": "Source content version was not found.",
                }
            },
            404,
        )

    with pytest.raises(RuntimeError, match="Source content version was not found"):
        make_client(handler).read_source_content("source-1", version=99)


def test_search_notes_sends_read_authentication_and_pagination() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return json_response(
            {
                "data": {
                    "items": [{"id": "note-2", "excerpt": "bounded thought"}],
                    "next_cursor": "note-2",
                }
            }
        )

    result = make_client(handler).search_notes(
        "thought", limit=500, cursor=" note-1 "
    )

    assert result["next_cursor"] == "note-2"
    request = requests[0]
    assert request.method == "GET"
    assert request.url.path == "/api/v1/search/notes"
    assert request.url.params["q"] == "thought"
    assert request.url.params["limit"] == "100"
    assert request.url.params["cursor"] == "note-1"
    assert request.headers["authorization"] == "Bearer placeholder-token"


def test_search_notes_rejects_empty_or_oversized_query_before_request() -> None:
    def handler(_request: httpx.Request) -> httpx.Response:
        raise AssertionError("request should not be sent")

    client = make_client(handler)
    with pytest.raises(ValueError, match="query is required"):
        client.search_notes("  ")
    with pytest.raises(ValueError, match="at most 1000"):
        client.search_notes("x" * 1_001)


def test_read_note_reads_content_with_read_credential() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return json_response(
            {"data": {"id": "note/id", "title": "Idea", "content": "Body"}}
        )

    result = make_client(handler).read_note("note/id")

    assert result["content"] == "Body"
    assert requests[0].url.raw_path == b"/api/v1/notes/note%2Fid"
    assert requests[0].headers["authorization"] == "Bearer placeholder-token"


def test_create_note_uses_only_the_separate_write_credential() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return json_response({"data": {"id": "note-1", "title": "Research idea"}}, 201)

    result = make_client(
        handler, note_write_token="write-placeholder-token"
    ).create_note(
        " Research idea ",
        "A durable idea to investigate.",
        "# Evidence\n\nA bounded body.",
        content_format="markdown",
        provenance={"source_type": "agent", "source_ref": "turn-1"},
        idempotency_key=" note:turn-1:idea-1 ",
    )

    assert result["id"] == "note-1"
    request = requests[0]
    assert request.method == "POST"
    assert request.url.port == 8084
    assert request.url.path == "/api/v1/notes"
    assert request.headers["authorization"] == "Bearer write-placeholder-token"
    assert request.headers["authorization"] != "Bearer placeholder-token"
    assert request.headers["idempotency-key"] == "note:turn-1:idea-1"
    assert request.headers["x-centaur-principal-id"] == "principal-1"
    assert request.headers["x-centaur-thread-key"] == "slack:team:channel:thread"
    assert request.read() == (
        b'{"title":"Research idea","description":"A durable idea to investigate.",'
        b'"content":"# Evidence\\n\\nA bounded body.","content_format":"markdown",'
        b'"provenance":{"source_type":"agent","source_ref":"turn-1"}}'
    )


def test_intake_validate_commit_and_status_use_only_intake_credential() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return json_response({"data": {"status": "ok"}}, 201 if request.method == "POST" else 200)

    client = make_client(handler, intake_token="intake-placeholder-token")
    batch = {
        "batch_id": "enyu-source-1",
        "manifest_sha256": "a" * 64,
        "objects": [],
    }

    client.validate_intake_batch(batch)
    client.commit_intake_batch(batch)
    client.intake_batch_status("enyu-source-1")

    assert [request.url.path for request in requests] == [
        "/api/v1/intake/batches/validate",
        "/api/v1/intake/batches/commit",
        "/api/v1/intake/batches/enyu-source-1",
    ]
    assert all(request.url.port == 8085 for request in requests)
    assert all(request.headers["authorization"] == "Bearer intake-placeholder-token" for request in requests)
    assert all(request.headers["authorization"] != "Bearer placeholder-token" for request in requests)


def test_intake_never_falls_back_to_read_or_note_write_tokens(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("CENTAUR_CONTEXT_INTAKE_TOKEN", raising=False)
    monkeypatch.setattr(client_module, "_tool_secret", lambda _name: "")

    with pytest.raises(RuntimeError, match="CENTAUR_CONTEXT_INTAKE_TOKEN"):
        make_client(lambda _request: json_response({}), note_write_token="note-token").validate_intake_batch(
            {"batch_id": "batch-1", "manifest_sha256": "a" * 64}
        )


def test_source_intake_methods_use_only_the_enyu_workflow_credential() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return json_response({"data": {"status": "ok"}})

    client = make_client(
        handler, source_intake_token="source-intake-placeholder-token"
    )
    manifest = {
        "version": "centaur-context-source-intake-v1",
        "idempotency_key": "workflow:run-1",
        "source": {"title": "Example"},
    }

    client.source_intake_validate(manifest)
    client.source_intake_commit(manifest)
    client.source_intake_status(manifest)

    assert [request.url.path for request in requests] == [
        "/api/v1/source-intake/validate",
        "/api/v1/source-intake/commit",
        "/api/v1/source-intake/status",
    ]
    assert all(request.url.port == 8086 for request in requests)
    assert all(
        request.headers["authorization"]
        == "Bearer source-intake-placeholder-token"
        for request in requests
    )
    assert all(
        request.headers["authorization"] != "Bearer placeholder-token"
        for request in requests
    )


def test_source_intake_accepts_explicit_workflow_identity() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return json_response({"data": {"valid": True}})

    client = make_client(
        handler, source_intake_token="source-intake-placeholder-token"
    )
    client.source_intake_validate(
        {"version": "centaur-context-source-intake-v1"},
        principal_id="workflow-enyu-source-ingestion",
        thread_key="workflow:run-1",
    )

    assert requests[0].headers["x-centaur-principal-id"] == (
        "workflow-enyu-source-ingestion"
    )
    assert requests[0].headers["x-centaur-thread-key"] == "workflow:run-1"


def test_source_intake_never_falls_back_to_other_credentials(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("CENTAUR_CONTEXT_SOURCE_INTAKE_TOKEN", raising=False)
    monkeypatch.setattr(client_module, "_tool_secret", lambda _name: "")

    with pytest.raises(RuntimeError, match="CENTAUR_CONTEXT_SOURCE_INTAKE_TOKEN"):
        make_client(
            lambda _request: json_response({}),
            note_write_token="note-token",
            intake_token="intake-token",
        ).source_intake_validate({})


def test_source_intake_requires_a_json_object_before_request() -> None:
    def handler(_request: httpx.Request) -> httpx.Response:
        raise AssertionError("request should not be sent")

    with pytest.raises(ValueError, match="manifest must be a JSON object"):
        make_client(
            handler, source_intake_token="source-intake-token"
        ).source_intake_validate([])  # type: ignore[arg-type]


def test_external_action_methods_use_only_the_dedicated_credential() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return json_response({"data": {"object_id": "action-1", "state": "reserved"}})

    client = make_client(handler)
    manifest = {
        "version": "centaur-context-external-action-v1",
        "idempotency_key": "reserve-1",
    }
    client.reserve_external_action(manifest)
    client.append_external_action_event("action/1", manifest)
    client.read_external_action("action/1")

    assert [request.method for request in requests] == ["POST", "POST", "GET"]
    assert [request.url.raw_path.split(b"?", 1)[0] for request in requests] == [
        b"/api/v1/external-actions/reserve",
        b"/api/v1/external-actions/action%2F1/events",
        b"/api/v1/external-actions/action%2F1",
    ]
    assert all(request.url.port == 8088 for request in requests)
    assert all(
        request.headers["authorization"] == "Bearer external-action-token"
        for request in requests
    )
    assert all(
        request.headers["authorization"] != "Bearer placeholder-token"
        for request in requests
    )


def test_external_actions_never_fall_back_to_other_credentials(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("CENTAUR_CONTEXT_EXTERNAL_ACTION_TOKEN", raising=False)
    monkeypatch.setattr(client_module, "_tool_secret", lambda _name: "")

    client = make_client(lambda _request: json_response({}))
    client._explicit_external_action_token = ""
    with pytest.raises(RuntimeError, match="CENTAUR_CONTEXT_EXTERNAL_ACTION_TOKEN"):
        client.reserve_external_action({})


@pytest.mark.parametrize(
    ("batch", "message"),
    [
        ([], "batch must be a JSON object"),
        ({"manifest_sha256": "a" * 64}, "batch_id is required"),
        ({"batch_id": "batch-1", "manifest_sha256": "A" * 64}, "lowercase SHA-256"),
    ],
)
def test_intake_rejects_invalid_envelopes_before_request(batch, message: str) -> None:
    def handler(_request: httpx.Request) -> httpx.Response:
        raise AssertionError("request should not be sent")

    with pytest.raises(ValueError, match=message):
        make_client(handler, intake_token="intake-token").validate_intake_batch(batch)


def test_create_note_never_falls_back_to_the_read_token(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("CENTAUR_CONTEXT_NOTE_WRITE_TOKEN", raising=False)
    monkeypatch.setattr(client_module, "_tool_secret", lambda _name: "")

    def handler(_request: httpx.Request) -> httpx.Response:
        raise AssertionError("request should not be sent")

    with pytest.raises(RuntimeError, match="CENTAUR_CONTEXT_NOTE_WRITE_TOKEN"):
        make_client(handler).create_note(
            "Idea",
            "A durable idea to investigate.",
            "Body",
            idempotency_key="note-1",
        )


@pytest.mark.parametrize(
    ("overrides", "message"),
    [
        ({"title": " "}, "title is required"),
        ({"title": "x" * 301}, "title must be at most 300"),
        ({"description": " "}, "description is required"),
        ({"description": "x" * 1_001}, "description must be at most 1000"),
        ({"content": " "}, "content is required"),
        ({"content": "x" * 100_001}, "content must be at most 100000"),
        ({"content_format": "html"}, "content_format must be"),
        ({"provenance": []}, "provenance must be a JSON object"),
        ({"idempotency_key": " "}, "idempotency_key is required"),
        ({"idempotency_key": "x" * 201}, "idempotency_key must be at most 200"),
    ],
)
def test_create_note_rejects_invalid_input_before_resolving_write_token(
    overrides: dict[str, object], message: str
) -> None:
    def handler(_request: httpx.Request) -> httpx.Response:
        raise AssertionError("request should not be sent")

    values: dict[str, object] = {
        "title": "Idea",
        "description": "A durable idea to investigate.",
        "content": "Body",
        "content_format": "plain_text",
        "provenance": {},
        "idempotency_key": "note-1",
    }
    values.update(overrides)
    with pytest.raises(ValueError, match=message):
        make_client(handler).create_note(**values)  # type: ignore[arg-type]


def test_create_note_cli_parses_provenance_and_forwards_idempotency(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    calls: list[tuple[tuple[object, ...], dict[str, object]]] = []

    class FakeClient:
        def create_note(self, *args, **kwargs):
            calls.append((args, kwargs))
            return {"id": "note-1"}

    monkeypatch.setattr(cli, "_client", FakeClient)

    cli.create_note(
        "Idea",
        "A durable idea to investigate.",
        "Body",
        "plain_text",
        '{"source_type":"agent"}',
        "note-1",
    )

    assert calls == [
        (
            ("Idea", "A durable idea to investigate.", "Body"),
            {
                "content_format": "plain_text",
                "provenance": {"source_type": "agent"},
                "idempotency_key": "note-1",
            },
        )
    ]
    assert capsys.readouterr().out.strip() == '{\n  "id": "note-1"\n}'


def test_propose_theme_cli_parses_evidence_and_provenance(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    calls: list[dict[str, object]] = []

    class FakeClient:
        def propose_theme(self, **kwargs):
            calls.append(kwargs)
            return {"id": "proposal-1", "status": "pending"}

    monkeypatch.setattr(cli, "_client", FakeClient)

    cli.propose_theme(
        "AI Infrastructure",
        "ai-infrastructure",
        "Research about infrastructure for AI systems.",
        "A recurring vertical needs a stable retrieval boundary.",
        '{"source_ids":["source-1"]}',
        '{"source_type":"agent"}',
        "proposal-1",
    )

    assert calls == [
        {
            "title": "AI Infrastructure",
            "slug": "ai-infrastructure",
            "description": "Research about infrastructure for AI systems.",
            "rationale": "A recurring vertical needs a stable retrieval boundary.",
            "evidence": {"source_ids": ["source-1"]},
            "provenance": {"source_type": "agent"},
            "idempotency_key": "proposal-1",
        }
    ]
    assert '"status": "pending"' in capsys.readouterr().out


def test_missing_thread_key_fails_closed(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("CENTAUR_THREAD_KEY", raising=False)

    def handler(_request: httpx.Request) -> httpx.Response:
        raise AssertionError("request should not be sent")

    client = CentaurContextClient(
        base_url="http://centaur-context.test:8081",
        token="placeholder-token",
        principal_id="principal-1",
        transport=httpx.MockTransport(handler),
    )
    with pytest.raises(RuntimeError, match="CENTAUR_THREAD_KEY is required"):
        client.read_object("object-1")


def test_legacy_url_is_used_when_canonical_value_is_absent(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("CENTAUR_CONTEXT_URL", raising=False)
    monkeypatch.setenv("CENTAUR_OS_URL", "http://legacy-centaur.test:8081")

    client = CentaurContextClient(
        token="placeholder-token",
        principal_id="principal-1",
        thread_key="thread-1",
        transport=httpx.MockTransport(lambda _request: json_response({"data": {}})),
    )

    assert client.base_url == "http://legacy-centaur.test:8081"


def test_conflicting_canonical_and_legacy_urls_fail_closed(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("CENTAUR_CONTEXT_URL", "http://canonical.test:8081")
    monkeypatch.setenv("CENTAUR_OS_URL", "http://legacy.test:8081")

    with pytest.raises(RuntimeError, match="conflicting canonical and legacy"):
        CentaurContextClient()


def test_legacy_token_is_used_when_canonical_value_is_absent(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("CENTAUR_CONTEXT_API_TOKEN", raising=False)
    monkeypatch.setenv("CENTAUR_OS_API_TOKEN", "legacy-placeholder-token")
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return json_response({"data": {}})

    client = CentaurContextClient(
        base_url="http://centaur-context.test:8081",
        principal_id="principal-1",
        thread_key="thread-1",
        transport=httpx.MockTransport(handler),
    )
    client.read_object("object-1")

    assert requests[0].headers["authorization"] == "Bearer legacy-placeholder-token"


def test_api_error_preserves_safe_message() -> None:
    def handler(_request: httpx.Request) -> httpx.Response:
        return json_response(
            {"error": {"code": "revision_conflict", "message": "The record changed."}},
            409,
        )

    with pytest.raises(RuntimeError, match="The record changed"):
        make_client(handler).read_object("object-1")


def test_default_transport_uses_stdlib_without_runtime_packages(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    requests = []
    monkeypatch.delenv("HTTP_PROXY", raising=False)
    monkeypatch.delenv("http_proxy", raising=False)

    class FakeResponse:
        status = 200

        def __enter__(self):
            return self

        def __exit__(self, *_args):
            return None

        def read(self) -> bytes:
            return b'{"data":{"id":"object-1"}}'

    def fake_urlopen(request, timeout):
        requests.append((request, timeout))
        return FakeResponse()

    monkeypatch.setattr(client_module, "urlopen", fake_urlopen)
    client = CentaurContextClient(
        base_url="http://centaur-context.test:8081",
        token="placeholder-token",
        principal_id="principal-1",
        thread_key="thread-1",
        timeout=12,
    )

    assert client.read_object("object-1") == {"id": "object-1"}
    request, timeout = requests[0]
    assert request.full_url == "http://centaur-context.test:8081/api/v1/objects/object-1"
    assert request.get_header("Authorization") == "Bearer placeholder-token"
    assert timeout == 12


def test_default_transport_explicitly_uses_http_proxy(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    requests = []

    class FakeResponse:
        status = 200

        def __enter__(self):
            return self

        def __exit__(self, *_args):
            return None

        def read(self) -> bytes:
            return b'{"data":{"id":"object-1"}}'

    class FakeOpener:
        def open(self, request, timeout):
            requests.append((request, timeout))
            return FakeResponse()

    monkeypatch.setenv("HTTP_PROXY", "http://iron-proxy:80")
    monkeypatch.delenv("http_proxy", raising=False)
    monkeypatch.setattr(client_module, "build_opener", lambda *_args: FakeOpener())
    client = CentaurContextClient(
        base_url="http://centaur-context.test:8081",
        token="placeholder-token",
        principal_id="principal-1",
        thread_key="thread-1",
        timeout=12,
    )

    assert client.read_object("object-1") == {"id": "object-1"}
    request, timeout = requests[0]
    assert request.host == "iron-proxy:80"
    assert request.selector == "http://centaur-context.test:8081/api/v1/objects/object-1"
    assert timeout == 12


def test_cli_api_failure_exits_cleanly(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    def fail() -> None:
        raise RuntimeError("The API is unavailable.")

    monkeypatch.setattr(cli, "app", fail)

    with pytest.raises(SystemExit) as exc_info:
        cli.main()

    assert exc_info.value.code == 1
    assert capsys.readouterr().out.strip() == '{"error": "The API is unavailable."}'

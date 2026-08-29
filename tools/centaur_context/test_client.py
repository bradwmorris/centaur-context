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

    result = make_client(handler).read_source("source/id")

    assert result["title"] == "A paper"
    assert requests[0].method == "GET"
    assert requests[0].url.raw_path == b"/api/v1/sources/source%2Fid"
    assert requests[0].url.query == b""


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

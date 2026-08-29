from __future__ import annotations

import httpx
import pytest

from centaur_tool_centaur_os import cli
from centaur_tool_centaur_os.client import CentaurOsClient


def json_response(data, status_code: int = 200) -> httpx.Response:
    return httpx.Response(status_code, json=data)


def make_client(handler, **overrides) -> CentaurOsClient:
    return CentaurOsClient(
        base_url="http://centaur-os.test:8081",
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

    client = CentaurOsClient(
        base_url="http://centaur-os.test:8081",
        console_url="http://centaur-console.test:3000",
        token="placeholder-token",
        thread_key="thread-1",
        transport=httpx.MockTransport(handler),
    )
    client.search_objects("context")

    assert hosts == ["centaur-console.test", "centaur-os.test"]


def test_search_rejects_an_empty_query_before_request() -> None:
    def handler(_request: httpx.Request) -> httpx.Response:
        raise AssertionError("request should not be sent")

    with pytest.raises(ValueError, match="query is required"):
        make_client(handler).search_objects("   ")


def test_missing_thread_key_fails_closed(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("CENTAUR_THREAD_KEY", raising=False)

    def handler(_request: httpx.Request) -> httpx.Response:
        raise AssertionError("request should not be sent")

    client = CentaurOsClient(
        base_url="http://centaur-os.test:8081",
        token="placeholder-token",
        principal_id="principal-1",
        transport=httpx.MockTransport(handler),
    )
    with pytest.raises(RuntimeError, match="CENTAUR_THREAD_KEY is required"):
        client.read_object("object-1")


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

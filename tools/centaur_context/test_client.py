import httpx
import pytest

try:
    from tools.centaur_context.client import CentaurContextClient
except ModuleNotFoundError:
    from client import CentaurContextClient


def client(handler):
    return CentaurContextClient(
        base_url="http://context.test",
        token="x" * 32,
        principal_id="agent-test",
        thread_key="codex:test",
        transport=httpx.MockTransport(handler),
    )


def response(data):
    return httpx.Response(200, json={"data": data})


def privileged_client(handler):
    return CentaurContextClient(
        base_url="http://context.test",
        token="r" * 32,
        note_write_url="http://notes.test",
        note_write_token="n" * 32,
        intake_url="http://intake.test",
        intake_token="i" * 32,
        source_intake_url="http://source-intake.test",
        source_intake_token="s" * 32,
        external_action_url="http://actions.test",
        external_action_token="e" * 32,
        principal_id="agent-test",
        thread_key="codex:test",
        transport=httpx.MockTransport(handler),
    )


def test_search_uses_v2():
    requests = []
    def handler(request):
        requests.append(request)
        return response([])
    assert client(handler).search_objects(
        "shared",
        lexical_only=True,
        principal_id="workflow-source",
        thread_key="workflow:run-1",
    ) == []
    assert requests[0].url.path == "/api/v2/search/objects"
    assert requests[0].url.params["lexical_only"] == "true"
    assert requests[0].headers["x-centaur-principal-id"] == "workflow-source"
    assert requests[0].headers["x-centaur-thread-key"] == "workflow:run-1"


def test_explicit_lexical_search_tool_operation():
    requests = []
    def handler(request):
        requests.append(request)
        return response([])
    assert client(handler).search_objects_lexical(
        "Agents",
        principal_id="workflow-source",
        thread_key="workflow:run-2",
    ) == []
    assert requests[0].url.path == "/api/v2/search/objects"
    assert requests[0].url.params["lexical_only"] == "true"
    assert requests[0].headers["x-centaur-principal-id"] == "workflow-source"
    assert requests[0].headers["x-centaur-thread-key"] == "workflow:run-2"


def test_lists_generic_artifacts():
    requests = []
    def handler(request):
        requests.append(request)
        return response([])
    assert client(handler).list_artifacts("object/1") == []
    assert requests[0].url.raw_path == b"/api/v2/objects/object%2F1/artifacts"


def test_reads_provider_safe_embedding_status():
    requests = []
    def handler(request):
        requests.append(request)
        return response({"configured": False, "fallback": "full_text"})
    assert client(handler).embedding_status()["fallback"] == "full_text"
    assert requests[0].url.path == "/api/v2/embeddings/status"


def test_reads_artifact_by_id_with_bounded_window():
    requests = []
    def handler(request):
        requests.append(request)
        return response({"text": "hello"})
    value = client(handler).read_artifact("artifact-1", offset=4, limit=99_999)
    assert value["text"] == "hello"
    assert requests[0].url.path == "/api/v2/artifacts/artifact-1/content"
    assert requests[0].url.params["limit"] == "20000"


@pytest.mark.parametrize("kwargs", [{"artifact_id": "", "offset": 0}, {"artifact_id": "a", "offset": -1}, {"artifact_id": "a", "limit": 0}])
def test_artifact_validation(kwargs):
    with pytest.raises(ValueError):
        client(lambda _: response({})).read_artifact(**kwargs)


def test_theme_assignment_uses_main_agent_contract():
    requests = []
    def handler(request):
        requests.append(request)
        return response({"id": "connection-1"})
    client(handler).assign_theme(object_id="object-1", theme_id="theme-1", description="Relevant", idempotency_key="assignment-1")
    assert requests[0].url.path == "/api/v2/theme-assignments"
    assert requests[0].headers["authorization"] == "Bearer " + "x" * 32


def test_specialized_writes_keep_distinct_credentials_and_v2_routes():
    requests = []

    def handler(request):
        requests.append(request)
        return response({})

    scoped = privileged_client(handler)
    scoped.create_note(
        title="Research note",
        description="A source-grounded note created through the narrow write listener.",
        content="Evidence",
        idempotency_key="note-1",
    )
    scoped.validate_intake_batch({"batch_id": "batch-1", "manifest_sha256": "a" * 64})
    scoped.source_intake_validate({"version": "centaur-context-source-intake-v2"})
    scoped.reserve_external_action({"version": "centaur-context-external-action-v2"})

    assert [(request.url.host, request.url.path) for request in requests] == [
        ("notes.test", "/api/v2/notes"),
        ("intake.test", "/api/v2/intake/batches/validate"),
        ("source-intake.test", "/api/v2/source-intake/validate"),
        ("actions.test", "/api/v2/external-actions/reserve"),
    ]
    assert [request.headers["authorization"] for request in requests] == [
        "Bearer " + "n" * 32,
        "Bearer " + "i" * 32,
        "Bearer " + "s" * 32,
        "Bearer " + "e" * 32,
    ]


@pytest.mark.parametrize(
    "operation",
    [
        lambda value: value.create_note(
            title="Research note",
            description="A source-grounded note created through a narrow listener.",
            content="Evidence",
            idempotency_key="note-1",
        ),
        lambda value: value.validate_intake_batch(
            {"batch_id": "batch-1", "manifest_sha256": "a" * 64}
        ),
        lambda value: value.source_intake_validate({"version": "centaur-context-source-intake-v2"}),
        lambda value: value.reserve_external_action(
            {"version": "centaur-context-external-action-v2"}
        ),
    ],
)
def test_specialized_writes_never_fall_back_to_the_read_token(monkeypatch, operation):
    for name in [
        "CENTAUR_CONTEXT_NOTE_WRITE_TOKEN",
        "CENTAUR_CONTEXT_INTAKE_TOKEN",
        "CENTAUR_CONTEXT_SOURCE_INTAKE_TOKEN",
        "CENTAUR_CONTEXT_EXTERNAL_ACTION_TOKEN",
    ]:
        monkeypatch.delenv(name, raising=False)
    with pytest.raises(RuntimeError):
        operation(client(lambda _: response({})))

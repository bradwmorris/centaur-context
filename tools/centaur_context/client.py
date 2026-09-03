"""Standard authenticated agent client for the Centaur Context Rust API."""

from __future__ import annotations

import os
import time
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urlencode, urlparse
from urllib.request import ProxyHandler, Request, build_opener, urlopen

DEFAULT_CENTAUR_CONTEXT_URL = "http://centaur-context.centaur.svc.cluster.local:8081"
DEFAULT_NOTE_WRITE_URL = "http://centaur-context-note-write.centaur.svc.cluster.local:8084"
DEFAULT_INTAKE_URL = "http://centaur-context-intake.centaur.svc.cluster.local:8085"
DEFAULT_SOURCE_INTAKE_URL = "http://centaur-context-enyu.centaur.svc.cluster.local:8086"
DEFAULT_RESEARCH_MUTATION_URL = "http://centaur-context-enyu.centaur.svc.cluster.local:8087"
DEFAULT_EXTERNAL_ACTION_URL = "http://centaur-context-enyu.centaur.svc.cluster.local:8088"
DEFAULT_CONSOLE_URL = "http://centaur-console:3000"
SANDBOX_PERMISSIONS_PATH = "/api/v1/sandbox/permissions"
TOKEN_NAME = "CENTAUR_CONTEXT_API_TOKEN"
LEGACY_TOKEN_NAME = "CENTAUR_OS_API_TOKEN"
NOTE_WRITE_TOKEN_NAME = "CENTAUR_CONTEXT_NOTE_WRITE_TOKEN"
INTAKE_TOKEN_NAME = "CENTAUR_CONTEXT_INTAKE_TOKEN"
SOURCE_INTAKE_TOKEN_NAME = "CENTAUR_CONTEXT_SOURCE_INTAKE_TOKEN"
RESEARCH_MUTATION_TOKEN_NAME = "CENTAUR_CONTEXT_RESEARCH_MUTATION_TOKEN"
EXTERNAL_ACTION_TOKEN_NAME = "CENTAUR_CONTEXT_EXTERNAL_ACTION_TOKEN"
MAX_SOURCE_CONTENT_WINDOW = 20_000
MAX_NOTE_CONTENT = 100_000
PROVENANCE_KEYS = frozenset({"source_type", "source_ref", "note", "publication_allowed"})


def _validated_provenance(value: dict[str, Any] | None) -> dict[str, Any]:
    if value is None:
        return {}
    if not isinstance(value, dict):
        raise ValueError("provenance must be a JSON object")
    unknown = sorted(set(value) - PROVENANCE_KEYS)
    if unknown:
        allowed = ", ".join(sorted(PROVENANCE_KEYS))
        raise ValueError(
            f"unsupported provenance keys: {', '.join(unknown)}; accepted keys: {allowed}"
        )
    return value


class _UrllibResponse:
    def __init__(self, status_code: int, body: bytes) -> None:
        self.status_code = status_code
        self._body = body

    def json(self) -> Any:
        import json

        return json.loads(self._body.decode("utf-8"))


class _UrllibClient:
    def __init__(self, timeout: float) -> None:
        self.timeout = timeout

    def close(self) -> None:
        return None

    def get(self, url: str) -> _UrllibResponse:
        return self.request("GET", url)

    def request(
        self,
        method: str,
        url: str,
        *,
        params: dict[str, Any] | None = None,
        json: dict[str, Any] | None = None,
        headers: dict[str, str] | None = None,
    ) -> _UrllibResponse:
        import json as json_module

        if params:
            separator = "&" if "?" in url else "?"
            url = f"{url}{separator}{urlencode(params)}"
        body = None
        request_headers = dict(headers or {})
        if json is not None:
            body = json_module.dumps(json, separators=(",", ":")).encode("utf-8")
            request_headers["Content-Type"] = "application/json"
        request = Request(url, data=body, headers=request_headers, method=method)
        proxy = _clean(os.getenv("http_proxy") or os.getenv("HTTP_PROXY"))
        open_request = urlopen
        if proxy and urlparse(url).scheme == "http":
            proxy_url = urlparse(proxy)
            if not proxy_url.netloc:
                raise RuntimeError("HTTP proxy URL is invalid")
            request.set_proxy(proxy_url.netloc, "http")
            open_request = build_opener(ProxyHandler({})).open
        try:
            with open_request(request, timeout=self.timeout) as response:
                return _UrllibResponse(response.status, response.read())
        except HTTPError as exc:
            return _UrllibResponse(exc.code, exc.read())
        except URLError as exc:
            raise RuntimeError(str(exc.reason)) from exc


def _clean(value: str | None) -> str:
    return (value or "").strip()


def _required(value: str | None, field: str) -> str:
    cleaned = _clean(value)
    if not cleaned:
        raise ValueError(f"{field} is required")
    return cleaned


def _compatible_value(canonical: str, legacy: str, *, source: str) -> str:
    canonical_value = _clean(canonical)
    legacy_value = _clean(legacy)
    if canonical_value and legacy_value and canonical_value != legacy_value:
        raise RuntimeError(f"conflicting canonical and legacy {source} values")
    return canonical_value or legacy_value


def _compatible_env(canonical_name: str, legacy_name: str) -> str:
    return _compatible_value(
        os.getenv(canonical_name, ""), os.getenv(legacy_name, ""), source="environment"
    )


def _bounded_limit(value: int) -> int:
    return max(1, min(int(value), 100))


def _tool_secret(name: str) -> str:
    try:
        from centaur_sdk import secret
    except ImportError:
        try:
            from centaur_sdk.tool_sdk import secret
        except ImportError:
            return ""
    return _clean(secret(name, ""))


def _data(payload: Any) -> Any:
    if not isinstance(payload, dict) or "data" not in payload:
        raise RuntimeError("Centaur Context returned an invalid response")
    return payload["data"]


class CentaurContextClient:
    """Serialize approved operations; validation remains in the Rust API."""

    def __init__(
        self,
        base_url: str | None = None,
        token: str | None = None,
        note_write_url: str | None = None,
        note_write_token: str | None = None,
        intake_url: str | None = None,
        intake_token: str | None = None,
        source_intake_url: str | None = None,
        source_intake_token: str | None = None,
        research_mutation_url: str | None = None,
        research_mutation_token: str | None = None,
        external_action_url: str | None = None,
        external_action_token: str | None = None,
        principal_id: str | None = None,
        thread_key: str | None = None,
        console_url: str | None = None,
        timeout: float = 30.0,
        transport: Any | None = None,
    ) -> None:
        self.base_url = (
            _clean(base_url)
            or _compatible_env("CENTAUR_CONTEXT_URL", "CENTAUR_OS_URL")
            or DEFAULT_CENTAUR_CONTEXT_URL
        )
        self.base_url = self.base_url.rstrip("/")
        self.note_write_url = (
            _clean(note_write_url or os.getenv("CENTAUR_CONTEXT_NOTE_WRITE_URL"))
            or DEFAULT_NOTE_WRITE_URL
        ).rstrip("/")
        self.intake_url = (
            _clean(intake_url or os.getenv("CENTAUR_CONTEXT_INTAKE_URL"))
            or DEFAULT_INTAKE_URL
        ).rstrip("/")
        self.source_intake_url = (
            _clean(source_intake_url or os.getenv("CENTAUR_CONTEXT_SOURCE_INTAKE_URL"))
            or DEFAULT_SOURCE_INTAKE_URL
        ).rstrip("/")
        self.research_mutation_url = (
            _clean(
                research_mutation_url
                or os.getenv("CENTAUR_CONTEXT_RESEARCH_MUTATION_URL")
            )
            or DEFAULT_RESEARCH_MUTATION_URL
        ).rstrip("/")
        self.external_action_url = (
            _clean(external_action_url or os.getenv("CENTAUR_CONTEXT_EXTERNAL_ACTION_URL"))
            or DEFAULT_EXTERNAL_ACTION_URL
        ).rstrip("/")
        self.console_url = (
            _clean(console_url or os.getenv("CENTAUR_CONSOLE_URL")) or DEFAULT_CONSOLE_URL
        ).rstrip("/")
        self._explicit_token = _clean(token)
        self._explicit_note_write_token = _clean(note_write_token)
        self._explicit_intake_token = _clean(intake_token)
        self._explicit_source_intake_token = _clean(source_intake_token)
        self._explicit_research_mutation_token = _clean(research_mutation_token)
        self._explicit_external_action_token = _clean(external_action_token)
        self._explicit_principal_id = _clean(principal_id)
        self._explicit_thread_key = _clean(thread_key)
        if transport is None:
            self._http = _UrllibClient(timeout)
        else:
            try:
                import httpx
            except ImportError as exc:
                raise RuntimeError("httpx is required when a custom transport is used") from exc
            self._http = httpx.Client(timeout=timeout, transport=transport)

    def close(self) -> None:
        self._http.close()

    def _token(self) -> str:
        value = self._explicit_token or _compatible_env(TOKEN_NAME, LEGACY_TOKEN_NAME)
        if not value:
            value = _compatible_value(
                _tool_secret(TOKEN_NAME),
                _tool_secret(LEGACY_TOKEN_NAME),
                source="tool secret",
            )
        if not value:
            raise RuntimeError(f"{TOKEN_NAME} is required")
        return value

    def _note_write_token(self) -> str:
        value = self._explicit_note_write_token or _clean(
            os.getenv(NOTE_WRITE_TOKEN_NAME)
        )
        if not value:
            value = _tool_secret(NOTE_WRITE_TOKEN_NAME)
        if not value:
            raise RuntimeError(f"{NOTE_WRITE_TOKEN_NAME} is required to create notes")
        return value

    def _intake_token(self) -> str:
        value = self._explicit_intake_token or _clean(os.getenv(INTAKE_TOKEN_NAME))
        if not value:
            value = _tool_secret(INTAKE_TOKEN_NAME)
        if not value:
            raise RuntimeError(f"{INTAKE_TOKEN_NAME} is required for Context intake")
        return value

    def _source_intake_token(self) -> str:
        value = self._explicit_source_intake_token or _clean(
            os.getenv(SOURCE_INTAKE_TOKEN_NAME)
        )
        if not value:
            value = _tool_secret(SOURCE_INTAKE_TOKEN_NAME)
        if not value:
            raise RuntimeError(
                f"{SOURCE_INTAKE_TOKEN_NAME} is required for Enyu Source intake"
            )
        return value

    def _research_mutation_token(self) -> str:
        value = self._explicit_research_mutation_token or _clean(
            os.getenv(RESEARCH_MUTATION_TOKEN_NAME)
        )
        if not value:
            value = _tool_secret(RESEARCH_MUTATION_TOKEN_NAME)
        if not value:
            raise RuntimeError(
                f"{RESEARCH_MUTATION_TOKEN_NAME} is required for Research mutations"
            )
        return value

    def _external_action_token(self) -> str:
        value = self._explicit_external_action_token or _clean(
            os.getenv(EXTERNAL_ACTION_TOKEN_NAME)
        )
        if not value:
            value = _tool_secret(EXTERNAL_ACTION_TOKEN_NAME)
        if not value:
            raise RuntimeError(
                f"{EXTERNAL_ACTION_TOKEN_NAME} is required for External actions"
            )
        return value

    def _principal_id(self) -> str:
        explicit = self._explicit_principal_id or _clean(
            os.getenv("CENTAUR_MCP_PRINCIPAL_ID")
        ) or _clean(os.getenv("CENTAUR_PRINCIPAL_ID"))
        if explicit:
            return explicit

        try:
            response = self._http.get(f"{self.console_url}{SANDBOX_PERMISSIONS_PATH}")
            if response.status_code >= 400:
                raise RuntimeError(f"HTTP {response.status_code}")
            payload = response.json()
        except (RuntimeError, ValueError) as exc:
            raise RuntimeError("could not resolve the current Centaur principal") from exc
        data = payload.get("data") if isinstance(payload, dict) else None
        principal_id = _clean(data.get("principal_id")) if isinstance(data, dict) else ""
        if not principal_id:
            raise RuntimeError("Centaur permissions did not return a principal ID")
        return principal_id

    def _thread_key(self) -> str:
        value = self._explicit_thread_key or _clean(os.getenv("CENTAUR_THREAD_KEY"))
        if not value:
            raise RuntimeError("CENTAUR_THREAD_KEY is required")
        return value

    def _headers(
        self,
        idempotency_key: str | None = None,
        *,
        token: str | None = None,
        principal_id: str | None = None,
        thread_key: str | None = None,
    ) -> dict[str, str]:
        headers = {
            "Accept": "application/json",
            "Authorization": f"Bearer {token or self._token()}",
            "X-Centaur-Principal-Id": _clean(principal_id) or self._principal_id(),
            "X-Centaur-Thread-Key": _clean(thread_key) or self._thread_key(),
        }
        if idempotency_key is not None:
            key = _clean(idempotency_key)
            if not key:
                raise ValueError("idempotency_key is required")
            headers["Idempotency-Key"] = key
        return headers

    def _request(
        self,
        method: str,
        path: str,
        *,
        params: dict[str, Any] | None = None,
        json: dict[str, Any] | None = None,
        idempotency_key: str | None = None,
        token: str | None = None,
        base_url: str | None = None,
        principal_id: str | None = None,
        thread_key: str | None = None,
    ) -> Any:
        try:
            response = self._http.request(
                method,
                f"{base_url or self.base_url}{path}",
                params=params,
                json=json,
                headers=self._headers(
                    idempotency_key,
                    token=token,
                    principal_id=principal_id,
                    thread_key=thread_key,
                ),
            )
        except Exception as exc:
            raise RuntimeError(f"Centaur Context request failed: {exc}") from exc
        if response.status_code >= 400:
            try:
                payload = response.json()
                error = payload.get("error", {})
                message = error.get("message") or error.get("code")
            except (ValueError, AttributeError):
                message = None
            detail = message or f"HTTP {response.status_code}"
            raise RuntimeError(f"Centaur Context request failed: {detail}")
        try:
            return _data(response.json())
        except ValueError as exc:
            raise RuntimeError("Centaur Context returned invalid JSON") from exc

    def get_context(
        self,
        query: str,
        chat_object_id: str,
        kind: str | None = None,
        limit: int = 10,
    ) -> dict[str, Any]:
        """Build a concise context packet of at most ten canonical Objects."""
        query = _clean(query)
        if not query:
            raise ValueError("query is required")
        chat_object_id = _clean(chat_object_id)
        if not chat_object_id:
            raise ValueError("chat_object_id is required")
        params: dict[str, Any] = {
            "q": query,
            "chat_object_id": chat_object_id,
            "limit": min(_bounded_limit(limit), 10),
        }
        if _clean(kind):
            params["kind"] = _clean(kind)
        return self._request("GET", "/api/v2/context", params=params)

    def search_objects(
        self,
        query: str,
        kind: str | None = None,
        limit: int = 20,
        lexical_only: bool = False,
        principal_id: str | None = None,
        thread_key: str | None = None,
    ) -> dict[str, Any]:
        """Search canonical Objects without Context Builder importance boosting."""
        query = _clean(query)
        if not query:
            raise ValueError("query is required")
        params: dict[str, Any] = {"q": query, "limit": _bounded_limit(limit)}
        if _clean(kind):
            params["kind"] = _clean(kind)
        if lexical_only:
            params["lexical_only"] = "true"
        return self._request(
            "GET",
            "/api/v2/search/objects",
            params=params,
            principal_id=principal_id,
            thread_key=thread_key,
        )

    def search_objects_lexical(
        self,
        query: str,
        kind: str | None = None,
        limit: int = 20,
        principal_id: str | None = None,
        thread_key: str | None = None,
    ) -> dict[str, Any]:
        """Search canonical Objects using full-text retrieval only."""
        return self.search_objects(
            query,
            kind=kind,
            limit=limit,
            lexical_only=True,
            principal_id=principal_id,
            thread_key=thread_key,
        )

    def read_object(self, id: str) -> dict[str, Any]:
        """Read one shared record by ID."""
        return self._request("GET", f"/api/v2/objects/{quote(id, safe='')}")

    def list_themes(self, slug: str | None = None) -> list[dict[str, Any]]:
        """List approved Themes, optionally selecting one exact slug."""
        params = {"slug": _clean(slug)} if _clean(slug) else None
        return self._request("GET", "/api/v2/themes", params=params)

    def read_theme(self, theme_id: str) -> dict[str, Any]:
        """Read one approved Theme."""
        theme_id = _clean(theme_id)
        if not theme_id:
            raise ValueError("theme_id is required")
        return self._request("GET", f"/api/v2/themes/{quote(theme_id, safe='')}")

    def list_theme_objects(
        self, theme_id: str, *, kind: str | None = None, limit: int = 20
    ) -> list[dict[str, Any]]:
        """List active Objects assigned to an approved Theme."""
        theme_id = _clean(theme_id)
        if not theme_id:
            raise ValueError("theme_id is required")
        params: dict[str, Any] = {"limit": _bounded_limit(limit)}
        if _clean(kind):
            params["kind"] = _clean(kind)
        return self._request(
            "GET", f"/api/v2/themes/{quote(theme_id, safe='')}/objects", params=params
        )

    def assign_theme(
        self,
        *,
        object_id: str,
        theme_id: str,
        description: str,
        provenance: dict[str, Any] | None = None,
        protected: bool = False,
        idempotency_key: str,
    ) -> dict[str, Any]:
        """Assign an existing approved Theme to a non-Theme Object."""
        object_id = _clean(object_id)
        theme_id = _clean(theme_id)
        description = _clean(description)
        if not object_id:
            raise ValueError("object_id is required")
        if not theme_id:
            raise ValueError("theme_id is required")
        if not description:
            raise ValueError("description is required")
        if provenance is not None and not isinstance(provenance, dict):
            raise ValueError("provenance must be a JSON object")
        return self._request(
            "POST",
            "/api/v2/theme-assignments",
            json={
                "object_id": object_id,
                "theme_id": theme_id,
                "description": description,
                "provenance": provenance or {},
                "protected": bool(protected),
            },
            idempotency_key=idempotency_key,
        )

    def unassign_theme(
        self,
        assignment_id: str,
        *,
        expected_revision: int,
        idempotency_key: str,
    ) -> dict[str, Any]:
        """Archive one existing themed Connection."""
        assignment_id = _clean(assignment_id)
        if not assignment_id:
            raise ValueError("assignment_id is required")
        return self._request(
            "POST",
            f"/api/v2/theme-assignments/{quote(assignment_id, safe='')}/archive",
            json={"expected_revision": int(expected_revision)},
            idempotency_key=idempotency_key,
        )

    def search_sources(
        self,
        query: str,
        limit: int = 20,
        cursor: str | None = None,
    ) -> dict[str, Any]:
        """Search Source metadata and content, returning bounded excerpts."""
        query = _clean(query)
        if not query:
            raise ValueError("query is required")
        params: dict[str, Any] = {"q": query, "limit": _bounded_limit(limit)}
        if _clean(cursor):
            params["cursor"] = _clean(cursor)
        return self._request("GET", "/api/v2/search/sources", params=params)

    def read_source(
        self,
        source_id: str,
        *,
        thread_key: str | None = None,
        base_url: str | None = None,
    ) -> dict[str, Any]:
        """Read canonical Source metadata without loading long-form content."""
        source_id = _clean(source_id)
        if not source_id:
            raise ValueError("source_id is required")
        target_url = _clean(base_url).rstrip("/") or None
        return self._request(
            "GET",
            f"/api/v2/sources/{quote(source_id, safe='')}",
            thread_key=thread_key,
            base_url=target_url,
        )

    def list_artifacts(self, object_id: str) -> list[dict[str, Any]]:
        """List immutable supporting artifacts attached to any Object."""
        object_id = _clean(object_id)
        if not object_id:
            raise ValueError("object_id is required")
        return self._request(
            "GET", f"/api/v2/objects/{quote(object_id, safe='')}/artifacts"
        )

    def embedding_status(self) -> dict[str, Any]:
        """Read provider-safe embedding queue and lexical-fallback status."""
        return self._request("GET", "/api/v2/embeddings/status")

    def read_artifact(
        self,
        artifact_id: str,
        *,
        offset: int = 0,
        limit: int = 8_000,
    ) -> dict[str, Any]:
        """Read one bounded text window from an Artifact attached to any Object."""
        artifact_id = _clean(artifact_id)
        if not artifact_id:
            raise ValueError("artifact_id is required")
        offset = int(offset)
        if offset < 0:
            raise ValueError("offset must be zero or greater")
        limit = int(limit)
        if limit < 1:
            raise ValueError("limit must be at least one")
        params: dict[str, Any] = {
            "artifact_id": artifact_id,
            "offset": offset,
            "limit": min(limit, MAX_SOURCE_CONTENT_WINDOW),
        }
        return self._request(
            "GET",
            f"/api/v2/artifacts/{quote(artifact_id, safe='')}/content",
            params=params,
        )

    def search_notes(
        self,
        query: str,
        limit: int = 20,
        cursor: str | None = None,
    ) -> dict[str, Any]:
        """Search canonical Notes, returning bounded content excerpts."""
        query = _clean(query)
        if not query:
            raise ValueError("query is required")
        if len(query) > 1_000:
            raise ValueError("query must be at most 1000 characters")
        params: dict[str, Any] = {"q": query, "limit": _bounded_limit(limit)}
        if _clean(cursor):
            params["cursor"] = _clean(cursor)
        return self._request("GET", "/api/v2/search/notes", params=params)

    def read_note(self, note_id: str) -> dict[str, Any]:
        """Read one canonical Note and its content."""
        note_id = _clean(note_id)
        if not note_id:
            raise ValueError("note_id is required")
        return self._request("GET", f"/api/v2/notes/{quote(note_id, safe='')}")

    def create_note(
        self,
        title: str,
        description: str,
        content: str,
        *,
        content_format: str = "markdown",
        provenance: dict[str, Any] | None = None,
        originating_chat_object_id: str | None = None,
        derived_from_source_object_ids: list[str] | None = None,
        idempotency_key: str,
    ) -> dict[str, Any]:
        """Create a Note with the separate, narrowly scoped write credential."""
        title = _clean(title)
        description = _clean(description)
        content = _clean(content)
        content_format = _clean(content_format).lower()
        idempotency_key = _clean(idempotency_key)
        if not title:
            raise ValueError("title is required")
        if len(title) > 300:
            raise ValueError("title must be at most 300 characters")
        if not description:
            raise ValueError("description is required")
        if len(description) > 2_000:
            raise ValueError("description must be at most 2000 characters")
        if not content:
            raise ValueError("content is required")
        if len(content) > MAX_NOTE_CONTENT:
            raise ValueError("content must be at most 100000 characters")
        if content_format not in {"markdown", "plain_text"}:
            raise ValueError("content_format must be markdown or plain_text")
        provenance = _validated_provenance(provenance)
        if not idempotency_key:
            raise ValueError("idempotency_key is required")
        if len(idempotency_key) > 200:
            raise ValueError("idempotency_key must be at most 200 characters")
        return self._request(
            "POST",
            "/api/v2/notes",
            json={
                "title": title,
                "description": description,
                "content": content,
                "content_format": content_format,
                "provenance": provenance,
                "originating_chat_object_id": _clean(originating_chat_object_id),
                "derived_from_source_object_ids": derived_from_source_object_ids or [],
            },
            idempotency_key=idempotency_key,
            token=self._note_write_token(),
            base_url=self.note_write_url,
        )

    def create_task(
        self,
        title: str,
        description: str,
        *,
        priority: str = "medium",
        due_at: str | None = None,
        owner_object_id: str | None = None,
        agent_suitable: bool = False,
        brief_markdown: str | None = None,
        provenance: dict[str, Any] | None = None,
        originating_chat_object_id: str | None = None,
        derived_from_source_object_ids: list[str] | None = None,
        idempotency_key: str,
    ) -> dict[str, Any]:
        """Create an open Task with the separate, narrowly scoped write credential."""
        title = _clean(title)
        description = _clean(description)
        priority = _clean(priority).lower()
        due_at = _clean(due_at)
        owner_object_id = _clean(owner_object_id)
        brief_markdown = _clean(brief_markdown)
        idempotency_key = _clean(idempotency_key)
        if not title:
            raise ValueError("title is required")
        if len(title) > 300:
            raise ValueError("title must be at most 300 characters")
        if not description:
            raise ValueError("description is required")
        if len(description) > 2_000:
            raise ValueError("description must be at most 2000 characters")
        if priority not in {"low", "medium", "high", "urgent"}:
            raise ValueError("priority must be low, medium, high, or urgent")
        if len(brief_markdown) > MAX_NOTE_CONTENT:
            raise ValueError("brief_markdown must be at most 100000 characters")
        provenance = _validated_provenance(provenance)
        if not idempotency_key:
            raise ValueError("idempotency_key is required")
        if len(idempotency_key) > 200:
            raise ValueError("idempotency_key must be at most 200 characters")
        payload: dict[str, Any] = {
            "title": title,
            "description": description,
            "status": "todo",
            "priority": priority,
            "agent_suitable": bool(agent_suitable),
            "provenance": provenance,
            "originating_chat_object_id": _clean(originating_chat_object_id),
            "derived_from_source_object_ids": derived_from_source_object_ids or [],
        }
        if due_at:
            payload["due_at"] = due_at
        if owner_object_id:
            payload["owner_object_id"] = owner_object_id
        if brief_markdown:
            payload["brief_markdown"] = brief_markdown
        return self._request(
            "POST",
            "/api/v2/tasks",
            json=payload,
            idempotency_key=idempotency_key,
            token=self._note_write_token(),
            base_url=self.note_write_url,
        )

    def validate_intake_batch(self, batch: dict[str, Any]) -> dict[str, Any]:
        """Validate an atomic, bounded intake batch without writing rows."""
        payload = self._intake_batch(batch)
        return self._request(
            "POST",
            "/api/v2/intake/batches/validate",
            json=payload,
            token=self._intake_token(),
            base_url=self.intake_url,
        )

    def commit_intake_batch(self, batch: dict[str, Any]) -> dict[str, Any]:
        """Atomically commit or idempotently replay a validated intake batch."""
        payload = self._intake_batch(batch)
        return self._request(
            "POST",
            "/api/v2/intake/batches/commit",
            json=payload,
            token=self._intake_token(),
            base_url=self.intake_url,
        )

    def intake_batch_status(self, batch_id: str) -> dict[str, Any] | None:
        """Read server-ledger status for a previously committed intake batch."""
        batch_id = _clean(batch_id)
        if not batch_id:
            raise ValueError("batch_id is required")
        if len(batch_id) > 100:
            raise ValueError("batch_id must be at most 100 characters")
        return self._request(
            "GET",
            f"/api/v2/intake/batches/{quote(batch_id, safe='')}",
            token=self._intake_token(),
            base_url=self.intake_url,
        )

    def source_intake_validate(
        self,
        manifest: dict[str, Any],
        principal_id: str | None = None,
        thread_key: str | None = None,
    ) -> dict[str, Any]:
        """Validate one Enyu Source manifest without writing rows."""
        return self._source_intake_request(
            "validate", manifest, principal_id=principal_id, thread_key=thread_key
        )

    def source_intake_resolve_connections(
        self,
        queries: list[str],
        principal_id: str | None = None,
        thread_key: str | None = None,
    ) -> dict[str, Any]:
        """Resolve unique exact Entity and Theme titles through trusted Source intake."""
        if not isinstance(queries, list) or not queries:
            raise ValueError("queries must be a non-empty array")
        return self._request(
            "POST",
            "/api/v2/source-intake/resolve-connections",
            json={"queries": queries},
            token=self._source_intake_token(),
            base_url=self.source_intake_url,
            principal_id=principal_id,
            thread_key=thread_key,
        )

    def source_intake_commit(
        self,
        manifest: dict[str, Any],
        principal_id: str | None = None,
        thread_key: str | None = None,
    ) -> dict[str, Any]:
        """Atomically commit or replay one Enyu Source manifest."""
        return self._source_intake_request(
            "commit", manifest, principal_id=principal_id, thread_key=thread_key
        )

    def source_intake_status(
        self,
        manifest: dict[str, Any],
        principal_id: str | None = None,
        thread_key: str | None = None,
    ) -> dict[str, Any]:
        """Check commit and retrieval readiness for one Enyu Source manifest."""
        return self._source_intake_request(
            "status", manifest, principal_id=principal_id, thread_key=thread_key
        )

    def source_intake_wait(
        self,
        manifest: dict[str, Any],
        principal_id: str | None = None,
        thread_key: str | None = None,
        *,
        attempts: int = 36,
        interval_seconds: float = 5,
    ) -> dict[str, Any]:
        """Wait up to three minutes for intake readiness in one visible tool call."""
        if attempts < 1 or attempts > 60:
            raise ValueError("attempts must be between 1 and 60")
        if interval_seconds < 0 or interval_seconds > 30:
            raise ValueError("interval_seconds must be between 0 and 30")
        result: dict[str, Any] = {}
        for attempt in range(attempts):
            result = self.source_intake_status(
                manifest, principal_id=principal_id, thread_key=thread_key
            )
            if result.get("ready") is True:
                return result
            if attempt + 1 < attempts:
                time.sleep(interval_seconds)
        return result

    def edit_source(
        self,
        source_id: str,
        changes: dict[str, Any],
        idempotency_key: str,
        principal_id: str | None = None,
        thread_key: str | None = None,
    ) -> dict[str, Any]:
        """Edit one ingested Source through the dedicated Research workflow listener."""
        return self._research_mutation_request(
            "PATCH",
            f"/api/v2/sources/{quote(_required(source_id, 'source_id'), safe='')}",
            changes,
            idempotency_key,
            principal_id=principal_id,
            thread_key=thread_key,
        )

    def connect(
        self,
        connection: dict[str, Any],
        idempotency_key: str,
        principal_id: str | None = None,
        thread_key: str | None = None,
    ) -> dict[str, Any]:
        """Create or reuse one canonical Connection through the Research workflow listener."""
        return self._research_mutation_request(
            "POST",
            "/api/v2/connections",
            connection,
            idempotency_key,
            principal_id=principal_id,
            thread_key=thread_key,
        )

    def edit_connection(
        self,
        connection_id: str,
        changes: dict[str, Any],
        idempotency_key: str,
        principal_id: str | None = None,
        thread_key: str | None = None,
    ) -> dict[str, Any]:
        """Edit one canonical Connection through the Research workflow listener."""
        return self._research_mutation_request(
            "PATCH",
            f"/api/v2/connections/{quote(_required(connection_id, 'connection_id'), safe='')}",
            changes,
            idempotency_key,
            principal_id=principal_id,
            thread_key=thread_key,
        )

    def _research_mutation_request(
        self,
        method: str,
        path: str,
        payload: dict[str, Any],
        idempotency_key: str,
        *,
        principal_id: str | None,
        thread_key: str | None,
    ) -> dict[str, Any]:
        if not isinstance(payload, dict):
            raise ValueError("Research mutation payload must be a JSON object")
        idempotency_key = _required(idempotency_key, "idempotency_key")
        if len(idempotency_key) > 200:
            raise ValueError("idempotency_key must be at most 200 characters")
        return self._request(
            method,
            path,
            json=payload,
            idempotency_key=idempotency_key,
            token=self._research_mutation_token(),
            base_url=self.research_mutation_url,
            principal_id=principal_id,
            thread_key=thread_key,
        )

    def workflow_run_start(
        self,
        run: dict[str, Any],
        principal_id: str | None = None,
        thread_key: str | None = None,
    ) -> dict[str, Any]:
        """Create or reuse one durable workflow Run without storing source content."""
        return self._workflow_run_request(
            "POST",
            "/api/v2/source-intake/runs/start",
            run,
            principal_id=principal_id,
            thread_key=thread_key,
        )

    def workflow_run_trace(
        self,
        run_id: str,
        entry: dict[str, Any],
        principal_id: str | None = None,
        thread_key: str | None = None,
    ) -> dict[str, Any]:
        """Append one idempotent, privacy-minimized workflow trace entry."""
        run_id = _clean(run_id)
        if not run_id:
            raise ValueError("run_id is required")
        return self._workflow_run_request(
            "POST",
            f"/api/v2/source-intake/runs/{quote(run_id, safe='')}/trace",
            entry,
            principal_id=principal_id,
            thread_key=thread_key,
        )

    def workflow_run_finish(
        self,
        run_id: str,
        outcome: dict[str, Any],
        principal_id: str | None = None,
        thread_key: str | None = None,
    ) -> dict[str, Any]:
        """Complete or fail one durable workflow Run and link its child commit."""
        run_id = _clean(run_id)
        if not run_id:
            raise ValueError("run_id is required")
        return self._workflow_run_request(
            "POST",
            f"/api/v2/source-intake/runs/{quote(run_id, safe='')}/finish",
            outcome,
            principal_id=principal_id,
            thread_key=thread_key,
        )

    def _workflow_run_request(
        self,
        method: str,
        path: str,
        payload: dict[str, Any],
        *,
        principal_id: str | None,
        thread_key: str | None,
    ) -> dict[str, Any]:
        if not isinstance(payload, dict):
            raise ValueError("workflow Run payload must be a JSON object")
        return self._request(
            method,
            path,
            json=payload,
            token=self._source_intake_token(),
            base_url=self.source_intake_url,
            principal_id=principal_id,
            thread_key=thread_key,
        )

    def _source_intake_request(
        self,
        action: str,
        manifest: dict[str, Any],
        *,
        principal_id: str | None = None,
        thread_key: str | None = None,
    ) -> dict[str, Any]:
        if not isinstance(manifest, dict):
            raise ValueError("manifest must be a JSON object")
        return self._request(
            "POST",
            f"/api/v2/source-intake/{action}",
            json=manifest,
            token=self._source_intake_token(),
            base_url=self.source_intake_url,
            principal_id=principal_id,
            thread_key=thread_key,
        )

    def reserve_external_action(
        self,
        manifest: dict[str, Any],
        principal_id: str | None = None,
        thread_key: str | None = None,
    ) -> dict[str, Any]:
        """Reserve one immutable, privacy-minimized external action."""
        if not isinstance(manifest, dict):
            raise ValueError("manifest must be a JSON object")
        return self._request(
            "POST",
            "/api/v2/external-actions/reserve",
            json=manifest,
            token=self._external_action_token(),
            base_url=self.external_action_url,
            principal_id=principal_id,
            thread_key=thread_key,
        )

    def append_external_action_event(
        self,
        action_id: str,
        event: dict[str, Any],
        principal_id: str | None = None,
        thread_key: str | None = None,
    ) -> dict[str, Any]:
        """Append one idempotent event to an existing external action."""
        action_id = _clean(action_id)
        if not action_id:
            raise ValueError("action_id is required")
        if not isinstance(event, dict):
            raise ValueError("event must be a JSON object")
        return self._request(
            "POST",
            f"/api/v2/external-actions/{quote(action_id, safe='')}/events",
            json=event,
            token=self._external_action_token(),
            base_url=self.external_action_url,
            principal_id=principal_id,
            thread_key=thread_key,
        )

    def read_external_action(
        self,
        action_id: str,
        principal_id: str | None = None,
        thread_key: str | None = None,
    ) -> dict[str, Any]:
        """Read current state for one external action."""
        action_id = _clean(action_id)
        if not action_id:
            raise ValueError("action_id is required")
        return self._request(
            "GET",
            f"/api/v2/external-actions/{quote(action_id, safe='')}",
            token=self._external_action_token(),
            base_url=self.external_action_url,
            principal_id=principal_id,
            thread_key=thread_key,
        )

    @staticmethod
    def _intake_batch(batch: dict[str, Any]) -> dict[str, Any]:
        if not isinstance(batch, dict):
            raise ValueError("batch must be a JSON object")
        batch_id = _clean(batch.get("batch_id"))
        manifest_sha256 = _clean(batch.get("manifest_sha256"))
        if not batch_id:
            raise ValueError("batch_id is required")
        if len(batch_id) > 100:
            raise ValueError("batch_id must be at most 100 characters")
        if len(manifest_sha256) != 64 or any(
            character not in "0123456789abcdef" for character in manifest_sha256
        ):
            raise ValueError("manifest_sha256 must be a lowercase SHA-256 hex digest")
        payload = dict(batch)
        payload["batch_id"] = batch_id
        payload["manifest_sha256"] = manifest_sha256
        return payload


def _client() -> CentaurContextClient:
    return CentaurContextClient()

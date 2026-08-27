"""Standard authenticated agent client for the Centaur OS Rust API."""

from __future__ import annotations

import os
from typing import Any
from urllib.parse import quote

import httpx

DEFAULT_CENTAUR_OS_URL = "http://centaur-os.centaur.svc.cluster.local:8081"
DEFAULT_CONSOLE_URL = "http://centaur-console:3000"
SANDBOX_PERMISSIONS_PATH = "/api/v1/sandbox/permissions"
TOKEN_NAME = "CENTAUR_OS_API_TOKEN"

OBJECT_UPDATE_FIELDS = {"title", "description", "provenance", "protected", "archive"}
TASK_UPDATE_FIELDS = {
    "title",
    "description",
    "provenance",
    "status",
    "priority",
    "owner_object_id",
    "clear_owner",
    "agent_eligible",
    "due_at",
    "clear_due_at",
}


def _clean(value: str | None) -> str:
    return (value or "").strip()


def _bounded_limit(value: int) -> int:
    return max(1, min(int(value), 100))


def _tool_secret(name: str) -> str:
    try:
        from centaur_sdk import secret
    except ImportError:
        from centaur_sdk.tool_sdk import secret
    return _clean(secret(name, ""))


def _data(payload: Any) -> Any:
    if not isinstance(payload, dict) or "data" not in payload:
        raise RuntimeError("Centaur OS returned an invalid response")
    return payload["data"]


class CentaurOsClient:
    """Serialize approved operations; validation remains in the Rust API."""

    def __init__(
        self,
        base_url: str | None = None,
        token: str | None = None,
        principal_id: str | None = None,
        thread_key: str | None = None,
        console_url: str | None = None,
        timeout: float = 30.0,
        transport: httpx.BaseTransport | None = None,
    ) -> None:
        self.base_url = _clean(base_url or os.getenv("CENTAUR_OS_URL")) or DEFAULT_CENTAUR_OS_URL
        self.base_url = self.base_url.rstrip("/")
        self.console_url = (
            _clean(console_url or os.getenv("CENTAUR_CONSOLE_URL")) or DEFAULT_CONSOLE_URL
        ).rstrip("/")
        self._explicit_token = _clean(token)
        self._explicit_principal_id = _clean(principal_id)
        self._explicit_thread_key = _clean(thread_key)
        self._http = httpx.Client(timeout=timeout, transport=transport)

    def close(self) -> None:
        self._http.close()

    def _token(self) -> str:
        value = self._explicit_token or _clean(os.getenv(TOKEN_NAME)) or _tool_secret(TOKEN_NAME)
        if not value:
            raise RuntimeError(f"{TOKEN_NAME} is required")
        return value

    def _principal_id(self) -> str:
        explicit = self._explicit_principal_id or _clean(
            os.getenv("CENTAUR_MCP_PRINCIPAL_ID")
        ) or _clean(os.getenv("CENTAUR_PRINCIPAL_ID"))
        if explicit:
            return explicit

        try:
            response = self._http.get(f"{self.console_url}{SANDBOX_PERMISSIONS_PATH}")
            response.raise_for_status()
            payload = response.json()
        except (httpx.HTTPError, ValueError) as exc:
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

    def _headers(self, idempotency_key: str | None = None) -> dict[str, str]:
        headers = {
            "Accept": "application/json",
            "Authorization": f"Bearer {self._token()}",
            "X-Centaur-Principal-Id": self._principal_id(),
            "X-Centaur-Thread-Key": self._thread_key(),
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
    ) -> Any:
        try:
            response = self._http.request(
                method,
                f"{self.base_url}{path}",
                params=params,
                json=json,
                headers=self._headers(idempotency_key),
            )
            response.raise_for_status()
            return _data(response.json())
        except httpx.HTTPStatusError as exc:
            try:
                payload = exc.response.json()
                error = payload.get("error", {})
                message = error.get("message") or error.get("code")
            except (ValueError, AttributeError):
                message = None
            detail = message or f"HTTP {exc.response.status_code}"
            raise RuntimeError(f"Centaur OS request failed: {detail}") from exc
        except httpx.RequestError as exc:
            raise RuntimeError(f"Centaur OS request failed: {exc}") from exc
        except ValueError as exc:
            raise RuntimeError("Centaur OS returned invalid JSON") from exc

    def search_objects(self, query: str, kind: str | None = None, limit: int = 20) -> list[dict[str, Any]]:
        """Search shared records by title or description."""
        query = _clean(query)
        if not query:
            raise ValueError("query is required")
        params: dict[str, Any] = {"q": query, "limit": _bounded_limit(limit)}
        if _clean(kind):
            params["kind"] = _clean(kind)
        return self._request("GET", "/api/v1/objects", params=params)

    def read_object(self, id: str) -> dict[str, Any]:
        """Read one shared record by ID."""
        return self._request("GET", f"/api/v1/objects/{quote(id, safe='')}")

    def create_object(
        self,
        kind: str,
        title: str,
        description: str,
        provenance: dict[str, Any],
        idempotency_key: str,
    ) -> dict[str, Any]:
        """Create one non-Task Object supported by the Centaur OS API."""
        return self._request(
            "POST",
            "/api/v1/objects",
            json={
                "kind": kind,
                "title": title,
                "description": description,
                "provenance": provenance,
            },
            idempotency_key=idempotency_key,
        )

    def update_object(
        self,
        id: str,
        expected_revision: int,
        changes: dict[str, Any],
        idempotency_key: str,
    ) -> dict[str, Any]:
        """Update selected record fields with optimistic revision protection."""
        unknown = set(changes) - OBJECT_UPDATE_FIELDS
        if unknown:
            raise ValueError(f"unsupported object changes: {', '.join(sorted(unknown))}")
        payload = {"expected_revision": int(expected_revision), **changes}
        return self._request(
            "PATCH",
            f"/api/v1/objects/{quote(id, safe='')}",
            json=payload,
            idempotency_key=idempotency_key,
        )

    def list_connections(self, id: str) -> list[dict[str, Any]]:
        """List incoming and outgoing relationships for one record."""
        return self._request(
            "GET", f"/api/v1/objects/{quote(id, safe='')}/connections"
        )

    def create_connection(
        self,
        source_id: str,
        kind: str,
        target_id: str,
        description: str,
        provenance: dict[str, Any],
        idempotency_key: str,
    ) -> dict[str, Any]:
        """Create one explained relationship between two records."""
        return self._request(
            "POST",
            "/api/v1/connections",
            json={
                "source_object_id": source_id,
                "kind": kind,
                "target_object_id": target_id,
                "description": description,
                "provenance": provenance,
            },
            idempotency_key=idempotency_key,
        )

    def list_tasks(
        self,
        status: str | None = None,
        agent_eligible: bool | None = None,
        limit: int = 20,
    ) -> list[dict[str, Any]]:
        """List shared tasks with optional status and eligibility filters."""
        params: dict[str, Any] = {"limit": _bounded_limit(limit)}
        if _clean(status):
            params["status"] = _clean(status)
        if agent_eligible is not None:
            params["agent_eligible"] = agent_eligible
        return self._request("GET", "/api/v1/tasks", params=params)

    def read_task(self, id: str) -> dict[str, Any]:
        """Read one shared task by ID."""
        return self._request("GET", f"/api/v1/tasks/{quote(id, safe='')}")

    def update_task(
        self,
        id: str,
        expected_revision: int,
        changes: dict[str, Any],
        idempotency_key: str,
    ) -> dict[str, Any]:
        """Update selected task fields with optimistic revision protection."""
        unknown = set(changes) - TASK_UPDATE_FIELDS
        if unknown:
            raise ValueError(f"unsupported task changes: {', '.join(sorted(unknown))}")
        payload = {"expected_revision": int(expected_revision), **changes}
        return self._request(
            "PATCH",
            f"/api/v1/tasks/{quote(id, safe='')}",
            json=payload,
            idempotency_key=idempotency_key,
        )


def _client() -> CentaurOsClient:
    return CentaurOsClient()

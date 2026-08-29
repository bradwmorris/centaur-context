"""Standard authenticated agent client for the Centaur Context Rust API."""

from __future__ import annotations

import os
from typing import Any
from urllib.parse import quote

import httpx

DEFAULT_CENTAUR_CONTEXT_URL = "http://centaur-context.centaur.svc.cluster.local:8081"
DEFAULT_CONSOLE_URL = "http://centaur-console:3000"
SANDBOX_PERMISSIONS_PATH = "/api/v1/sandbox/permissions"
TOKEN_NAME = "CENTAUR_CONTEXT_API_TOKEN"
LEGACY_TOKEN_NAME = "CENTAUR_OS_API_TOKEN"

def _clean(value: str | None) -> str:
    return (value or "").strip()


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
        from centaur_sdk.tool_sdk import secret
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
        principal_id: str | None = None,
        thread_key: str | None = None,
        console_url: str | None = None,
        timeout: float = 30.0,
        transport: httpx.BaseTransport | None = None,
    ) -> None:
        self.base_url = (
            _clean(base_url)
            or _compatible_env("CENTAUR_CONTEXT_URL", "CENTAUR_OS_URL")
            or DEFAULT_CENTAUR_CONTEXT_URL
        )
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
            raise RuntimeError(f"Centaur Context request failed: {detail}") from exc
        except httpx.RequestError as exc:
            raise RuntimeError(f"Centaur Context request failed: {exc}") from exc
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
        return self._request("GET", "/api/v1/context", params=params)

    def search_objects(self, query: str, kind: str | None = None, limit: int = 20) -> dict[str, Any]:
        """Search canonical Objects without Context Builder importance boosting."""
        query = _clean(query)
        if not query:
            raise ValueError("query is required")
        params: dict[str, Any] = {"q": query, "limit": _bounded_limit(limit)}
        if _clean(kind):
            params["kind"] = _clean(kind)
        return self._request("GET", "/api/v1/search/objects", params=params)

    def read_object(self, id: str) -> dict[str, Any]:
        """Read one shared record by ID."""
        return self._request("GET", f"/api/v1/objects/{quote(id, safe='')}")

def _client() -> CentaurContextClient:
    return CentaurContextClient()

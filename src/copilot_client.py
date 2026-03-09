"""HTTP client for the GitHub Copilot Chat API."""

import json
from collections.abc import AsyncIterator

import httpx

from .auth import CopilotAuth

COPILOT_CHAT_URL = "https://api.githubcopilot.com/chat/completions"
COPILOT_MODELS_URL = "https://api.githubcopilot.com/models"

DEFAULT_MODELS = [
    "gpt-4o",
    "gpt-4o-mini",
    "gpt-4",
    "gpt-3.5-turbo",
    "claude-3.5-sonnet",
    "claude-3.5-haiku",
    "o1-preview",
    "o1-mini",
    "o3-mini",
]


class CopilotClient:
    def __init__(self, auth: CopilotAuth) -> None:
        self._auth = auth
        self._http = httpx.AsyncClient(timeout=httpx.Timeout(120.0, connect=10.0))

    async def _headers(self) -> dict[str, str]:
        token = await self._auth.get_copilot_token()
        return {
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/json",
            "Accept": "application/json",
            "User-Agent": "GitHubCopilotChat/0.1",
            "Editor-Version": "vscode/1.95.0",
            "Editor-Plugin-Version": "copilot-chat/0.22.0",
            "Openai-Intent": "conversation-panel",
            "Copilot-Integration-Id": "vscode-chat",
        }

    async def chat_completions(self, payload: dict) -> dict:
        """Non-streaming chat completion."""
        headers = await self._headers()
        payload["stream"] = False

        resp = await self._http.post(COPILOT_CHAT_URL, headers=headers, json=payload)
        resp.raise_for_status()
        return resp.json()

    async def chat_completions_stream(self, payload: dict) -> AsyncIterator[str]:
        """Streaming chat completion — yields SSE `data: ...` lines."""
        headers = await self._headers()
        headers["Accept"] = "text/event-stream"
        payload["stream"] = True

        async with self._http.stream("POST", COPILOT_CHAT_URL, headers=headers, json=payload) as resp:
            resp.raise_for_status()
            async for line in resp.aiter_lines():
                line = line.strip()
                if not line:
                    continue
                if line.startswith("data: "):
                    data = line[6:]
                    if data == "[DONE]":
                        yield "data: [DONE]\n\n"
                        break
                    # Validate it's JSON, then forward
                    try:
                        json.loads(data)
                        yield f"data: {data}\n\n"
                    except json.JSONDecodeError:
                        continue

    async def list_models(self) -> list[dict]:
        """Fetch available models from Copilot, with fallback to defaults."""
        try:
            headers = await self._headers()
            resp = await self._http.get(COPILOT_MODELS_URL, headers=headers)
            if resp.status_code == 200:
                data = resp.json()
                if isinstance(data, dict) and "data" in data:
                    return data["data"]
                if isinstance(data, list):
                    return data
        except Exception:
            pass

        return [{"id": m, "object": "model", "created": 0, "owned_by": "github-copilot"} for m in DEFAULT_MODELS]

    async def close(self) -> None:
        await self._http.aclose()

"""GitHub Copilot authentication via VSCode OAuth Device Flow."""

import json
import logging
import os
import time
from pathlib import Path

import httpx

GITHUB_CLIENT_ID = "Iv1.b507a08c87ecfe98"
GITHUB_SCOPES = "read:user"
GITHUB_BASE_URL = "https://github.com"
GITHUB_API_URL = "https://api.github.com"
COPILOT_TOKEN_URL = f"{GITHUB_API_URL}/copilot_internal/v2/token"

COPILOT_VERSION = "0.26.7"
EDITOR_PLUGIN_VERSION = f"copilot-chat/{COPILOT_VERSION}"
USER_AGENT = f"GitHubCopilotChat/{COPILOT_VERSION}"
VSCODE_VERSION = "1.95.0"

TOKEN_REFRESH_MARGIN = 300

logger = logging.getLogger("copilot-wrapper")


def _token_path() -> Path:
    """Path where the GitHub OAuth token is persisted."""
    config_dir = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
    token_dir = config_dir / "copilot-wrapper"
    token_dir.mkdir(parents=True, exist_ok=True)
    return token_dir / "github_token"


class CopilotAuthError(Exception):
    pass


class CopilotAuth:
    def __init__(self, github_token: str | None = None) -> None:
        self._github_token: str | None = github_token
        self._copilot_token: str | None = None
        self._copilot_token_expires_at: float = 0.0

    def _github_headers(self) -> dict[str, str]:
        return {
            "Content-Type": "application/json",
            "Accept": "application/json",
            "Authorization": f"token {self._github_token}",
            "Editor-Version": f"vscode/{VSCODE_VERSION}",
            "Editor-Plugin-Version": EDITOR_PLUGIN_VERSION,
            "User-Agent": USER_AGENT,
        }

    async def get_github_token(self) -> str:
        """Return the GitHub OAuth token, loading from disk or running device flow."""
        if self._github_token:
            return self._github_token

        # Try loading from disk
        path = _token_path()
        if path.exists():
            token = path.read_text().strip()
            if token:
                self._github_token = token
                logger.info("Loaded GitHub token from %s", path)
                return self._github_token

        # Run device flow
        self._github_token = await self._device_flow()
        path.write_text(self._github_token)
        logger.info("GitHub token saved to %s", path)
        return self._github_token

    async def _device_flow(self) -> str:
        """Run GitHub OAuth Device Flow using the VSCode client ID."""
        async with httpx.AsyncClient() as client:
            # Step 1: Request device code
            resp = await client.post(
                f"{GITHUB_BASE_URL}/login/device/code",
                json={"client_id": GITHUB_CLIENT_ID, "scope": GITHUB_SCOPES},
                headers={"Content-Type": "application/json", "Accept": "application/json"},
            )
            if resp.status_code != 200:
                raise CopilotAuthError(f"Failed to start device flow: {resp.text}")

            data = resp.json()
            user_code = data["user_code"]
            device_code = data["device_code"]
            verification_uri = data["verification_uri"]
            interval = data.get("interval", 5) + 1

            # Print instructions for the user
            print(f"\n{'='*50}")
            print(f"  Please enter code:  {user_code}")
            print(f"  at:  {verification_uri}")
            print(f"{'='*50}\n")

            # Try to open browser
            try:
                import webbrowser
                webbrowser.open(verification_uri)
            except Exception:
                pass

            # Step 2: Poll for access token
            while True:
                time.sleep(interval)
                resp = await client.post(
                    f"{GITHUB_BASE_URL}/login/oauth/access_token",
                    json={
                        "client_id": GITHUB_CLIENT_ID,
                        "device_code": device_code,
                        "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
                    },
                    headers={"Content-Type": "application/json", "Accept": "application/json"},
                )

                if resp.status_code != 200:
                    continue

                result = resp.json()
                access_token = result.get("access_token")
                if access_token:
                    logger.info("GitHub authentication successful!")
                    return access_token

                error = result.get("error")
                if error == "authorization_pending":
                    continue
                elif error == "slow_down":
                    interval += 5
                    continue
                elif error == "expired_token":
                    raise CopilotAuthError("Device code expired. Please restart and try again.")
                elif error == "access_denied":
                    raise CopilotAuthError("Authorization was denied by the user.")
                else:
                    continue

    async def get_copilot_token(self) -> str:
        if self._copilot_token and time.time() < self._copilot_token_expires_at - TOKEN_REFRESH_MARGIN:
            return self._copilot_token

        github_token = await self.get_github_token()

        headers = self._github_headers()

        async with httpx.AsyncClient() as client:
            resp = await client.get(COPILOT_TOKEN_URL, headers=headers)

        if resp.status_code == 401 or resp.status_code == 404:
            # Token may be stale — delete and re-auth
            logger.warning("Copilot token request failed (HTTP %s), re-authenticating...", resp.status_code)
            self._github_token = None
            path = _token_path()
            if path.exists():
                path.unlink()
            github_token = await self.get_github_token()
            headers = self._github_headers()
            async with httpx.AsyncClient() as client:
                resp = await client.get(COPILOT_TOKEN_URL, headers=headers)

        if resp.status_code != 200:
            raise CopilotAuthError(
                f"Failed to fetch Copilot token (HTTP {resp.status_code}): {resp.text}\n"
                "Make sure you have an active GitHub Copilot subscription."
            )

        data = resp.json()

        try:
            self._copilot_token = data["token"]
            self._copilot_token_expires_at = float(data["expires_at"])
        except (KeyError, ValueError, TypeError) as exc:
            raise CopilotAuthError(
                f"Unexpected response from Copilot token endpoint: {data}"
            ) from exc

        return self._copilot_token

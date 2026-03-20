# Changelog

## [0.1.1] – 2026-03-20

### Changed
- Replaced `uvicorn[standard]` with explicit `uvicorn` + `uvloop` + `httptools` to drop unused `watchfiles` and `websockets` dependencies (~40 MB idle RSS)
- Added `loop="uvloop"` and `workers=1` to uvicorn config for minimal resource usage
- Added `[tool.setuptools] packages = ["src"]` so the `copilot-wrapper` entry point resolves correctly

### Added
- `com.github.copilot-wrapper.plist` — macOS launchd daemon config (auto-start on login, auto-restart on crash, Nice=10, background I/O priority)
- `daemon.sh` — install/uninstall/start/stop/status/logs helper script

## [0.1.0] – 2026-03-09

### Added
- Initial release: FastAPI-based OpenAI-compatible API wrapper for GitHub Copilot
- Streaming and non-streaming chat completions via `/v1/chat/completions`
- Model listing via `/v1/models`
- GitHub OAuth Device Flow authentication
- Environment-based configuration (`.env` support)

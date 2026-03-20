# Changelog

## [0.2.0] – 2026-03-20

### Added
- Complete Rust reimplementation — now the only implementation
  - `axum` HTTP server with single-threaded tokio runtime
  - GitHub OAuth Device Flow authentication
  - Copilot token exchange with auto-refresh
  - `/health`, `/v1/models`, `/v1/chat/completions` (streaming + non-streaming)
  - CORS via `tower-http`
  - `.env` file support
  - `open` browser on macOS during device flow
- Test framework: 14 unit tests + 7 integration tests with `wiremock` mock server
  - Model serialization/deserialization roundtrips
  - Config defaults and edge cases
  - Auth header generation and token caching
  - Full API integration: health, models (upstream + fallback), chat completions (stream + non-stream), error handling, CORS
- `lib.rs` / `main.rs` split for testability (`build_app()` + `new_with_base_urls()`)

### Removed
- Python implementation entirely (`src/*.py`, `pyproject.toml`, `.venv`)
- `debug_stream.py`, `test_e2e.py` (replaced by Rust integration tests)

### Changed
- Rust source moved from `rust/src/` to `src/` (top-level Cargo project)
- `daemon.sh` now includes `build` command; auto-builds if binary missing on `install`
- Release binary: 2.4 MB, stripped, LTO, ~5 MB RSS idle

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

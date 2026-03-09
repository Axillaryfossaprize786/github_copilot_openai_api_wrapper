<div align="center">

# 🤖 Copilot Gateway

**Use GitHub Copilot with any OpenAI-compatible client**

[![Python 3.11+](https://img.shields.io/badge/python-3.11+-3776AB?logo=python&logoColor=white)](https://python.org)
[![FastAPI](https://img.shields.io/badge/FastAPI-009688?logo=fastapi&logoColor=white)](https://fastapi.tiangolo.com)
[![License: MIT](https://img.shields.io/badge/license-MIT-yellow.svg)](LICENSE)
[![GitHub Copilot](https://img.shields.io/badge/GitHub%20Copilot-000?logo=githubcopilot&logoColor=white)](https://github.com/features/copilot)

A local proxy server that exposes OpenAI-compatible API endpoints, forwarding requests to the GitHub Copilot Chat API. Use GitHub Copilot with any client that supports the OpenAI API — Open WebUI, Chatbox, BoltAI, Elephas, Cline, Aider, and more.

[Getting Started](#getting-started) · [Configuration](#configuration) · [Client Setup](#client-setup) · [API Reference](#api-reference)

</div>

---

## ✨ Features

- 🔄 **OpenAI-compatible API** — drop-in replacement for `api.openai.com`
- 🌊 **Streaming support** — real-time SSE streaming, just like OpenAI
- 🔐 **Automatic auth** — GitHub OAuth Device Flow, no tokens to manage
- 🧠 **40+ models** — GPT-4o, Claude, Gemini, o3-mini, and more via Copilot
- ⚡ **Fast & lightweight** — async Python with FastAPI + uvicorn
- 🔌 **Zero config** — works out of the box, just run and go

## Prerequisites

- Python 3.11+
- An active [GitHub Copilot](https://github.com/features/copilot) subscription

> **Note:** The GitHub CLI (`gh`) is **not** required. The server uses its own OAuth Device Flow.

## Getting Started

### Installation

```bash
git clone https://github.com/trsdn/github_copilot_openai_api_wrapper.git
cd github_copilot_openai_api_wrapper

python3 -m venv .venv
source .venv/bin/activate
pip install -e .
```

### First Run

```bash
copilot-wrapper
```

On first launch, the server will guide you through GitHub authentication:

1. A one-time code is displayed (e.g. `ABCD-1234`)
2. Your browser opens to https://github.com/login/device
3. Enter the code and authorize
4. Done! Your token is saved to `~/.config/copilot-wrapper/github_token`

The server is now running at **http://127.0.0.1:8080** 🚀

## Configuration

Configure via environment variables or a `.env` file:

```bash
cp .env.example .env
```

| Variable | Default | Description |
|---|---|---|
| `HOST` | `127.0.0.1` | Server bind address |
| `PORT` | `8080` | Server port |
| `LOG_LEVEL` | `info` | Log level (`debug`, `info`, `warning`, `error`) |
| `GITHUB_TOKEN` | — | Optional: skip Device Flow by providing a token directly |

## Client Setup

Point your OpenAI-compatible client at the local server:

| Setting | Value |
|---|---|
| **API Base URL** | `http://127.0.0.1:8080/v1` |
| **API Key** | anything (e.g. `x`) — not validated |
| **Model** | `gpt-4o`, `claude-3.5-sonnet`, `o3-mini`, etc. |

> **💡 Tip:** Some clients (like Elephas) automatically append `/v1` to the base URL. In that case, use `http://127.0.0.1:8080` without the `/v1` suffix.

### Tested Clients

| Client | Status | Notes |
|---|---|---|
| [Open WebUI](https://github.com/open-webui/open-webui) | ✅ | Set base URL to `http://127.0.0.1:8080/v1` |
| [Chatbox](https://chatboxai.app/) | ✅ | Works out of the box |
| [Elephas](https://elephas.app/) | ✅ | Use `http://127.0.0.1:8080` (no `/v1`) |
| [BoltAI](https://boltai.com/) | ✅ | Works out of the box |
| [Aider](https://aider.chat/) | ✅ | Set `--openai-api-base` |
| [Cline](https://github.com/cline/cline) | ✅ | Use OpenAI-compatible provider |
| curl | ✅ | See examples below |

## API Reference

### `POST /v1/chat/completions`

OpenAI-compatible chat completions. Supports both streaming and non-streaming.

```bash
# Non-streaming
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'

# Streaming (SSE)
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o",
    "messages": [{"role": "user", "content": "Hello!"}],
    "stream": true
  }'
```

**Supported parameters:** `model`, `messages`, `temperature`, `top_p`, `max_tokens`, `stream`, `stop`, `n`

### `GET /v1/models`

List all available models from GitHub Copilot.

```bash
curl http://localhost:8080/v1/models
```

### `GET /health`

Health check endpoint.

```bash
curl http://localhost:8080/health
# → {"status": "ok"}
```

## Architecture

```
┌──────────────────┐     ┌─────────────────────┐     ┌──────────────────────────┐
│   Chat Client    │────▶│   Copilot Gateway    │────▶│  api.githubcopilot.com   │
│  (any OpenAI-    │◀────│  localhost:8080      │◀────│  GitHub Copilot API      │
│   compatible)    │     │  FastAPI + uvicorn   │     │                          │
└──────────────────┘     └─────────────────────┘     └──────────────────────────┘
                              │
                              ▼
                         OAuth Device Flow
                         (first run only)
```

## Project Structure

```
src/
├── main.py            # FastAPI app, endpoints, startup
├── config.py          # Configuration (pydantic-settings)
├── auth.py            # GitHub OAuth Device Flow + Copilot token management
├── copilot_client.py  # Async HTTP client for Copilot API
├── models.py          # Pydantic request/response models
└── middleware.py       # CORS, logging, error handling
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## Disclaimer

This project is for **educational and personal use**. It relies on internal GitHub Copilot API endpoints that are not officially documented. Use at your own risk and ensure compliance with [GitHub's Terms of Service](https://docs.github.com/en/site-policy/github-terms/github-terms-of-service).

## License

[MIT](LICENSE)

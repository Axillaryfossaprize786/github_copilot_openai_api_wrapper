"""FastAPI application — OpenAI-compatible wrapper for GitHub Copilot."""

import json
import logging
import time
import uuid
from contextlib import asynccontextmanager

import uvicorn
from fastapi import FastAPI, HTTPException, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import StreamingResponse

from .auth import CopilotAuth, CopilotAuthError
from .config import get_settings
from .copilot_client import CopilotClient
from .middleware import RequestLoggingMiddleware
from .models import (
    ChatCompletionRequest,
    ChatCompletionResponse,
    ModelInfo,
    ModelList,
)

logger = logging.getLogger("copilot-wrapper")

client: CopilotClient | None = None


@asynccontextmanager
async def lifespan(app: FastAPI):
    global client
    settings = get_settings()
    auth = CopilotAuth(github_token=settings.github_token)
    client = CopilotClient(auth)
    logger.info("Copilot OpenAI wrapper started on %s:%s", settings.host, settings.port)
    yield
    await client.close()
    logger.info("Copilot OpenAI wrapper stopped")


app = FastAPI(
    title="GitHub Copilot OpenAI Wrapper",
    version="0.1.0",
    lifespan=lifespan,
)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)
app.add_middleware(RequestLoggingMiddleware)


@app.get("/health")
async def health():
    return {"status": "ok"}


@app.get("/v1/models")
async def list_models():
    assert client is not None
    raw_models = await client.list_models()
    models = []
    for m in raw_models:
        if isinstance(m, dict):
            models.append(ModelInfo(
                id=m.get("id", m.get("name", "unknown")),
                created=m.get("created", 0),
                owned_by=m.get("owned_by", "github-copilot"),
            ))
        else:
            models.append(ModelInfo(id=str(m)))
    return ModelList(data=models)


@app.post("/v1/chat/completions")
async def chat_completions(request: ChatCompletionRequest):
    assert client is not None

    payload = request.model_dump(exclude_none=True)

    try:
        if request.stream:
            return StreamingResponse(
                _stream_response(payload),
                media_type="text/event-stream",
                headers={
                    "Cache-Control": "no-cache",
                    "Connection": "keep-alive",
                    "X-Accel-Buffering": "no",
                },
            )
        else:
            result = await client.chat_completions(payload)
            return result

    except CopilotAuthError as exc:
        raise HTTPException(status_code=401, detail=str(exc))
    except Exception as exc:
        logger.exception("Copilot API error")
        raise HTTPException(status_code=502, detail=f"Copilot API error: {exc}")


async def _stream_response(payload: dict):
    assert client is not None
    try:
        async for chunk in client.chat_completions_stream(payload):
            yield chunk
    except CopilotAuthError as exc:
        error = {"error": {"message": str(exc), "type": "auth_error", "code": 401}}
        yield f"data: {json.dumps(error)}\n\n"
    except Exception as exc:
        logger.exception("Streaming error")
        error = {"error": {"message": str(exc), "type": "server_error", "code": 502}}
        yield f"data: {json.dumps(error)}\n\n"


def cli():
    """Entry point for the `copilot-wrapper` command."""
    settings = get_settings()
    logging.basicConfig(
        level=getattr(logging, settings.log_level.upper(), logging.INFO),
        format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    )
    uvicorn.run(
        "src.main:app",
        host=settings.host,
        port=settings.port,
        log_level=settings.log_level,
    )


if __name__ == "__main__":
    cli()

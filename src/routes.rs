/// Axum route handlers for the OpenAI-compatible API.
use std::sync::Arc;

use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use futures_util::StreamExt;
use serde_json::Value;

use crate::models::{ApiError, ApiErrorDetail, ModelInfo, ModelList};
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health))
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
}

async fn health() -> Json<Value> {
    Json(serde_json::json!({"status": "ok"}))
}

async fn list_models(State(state): State<Arc<AppState>>) -> Json<Value> {
    let raw = state.client.list_models().await;

    let models: Vec<ModelInfo> = raw
        .iter()
        .map(|m| ModelInfo {
            id: m
                .get("id")
                .or_else(|| m.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            object: "model".into(),
            created: m.get("created").and_then(|v| v.as_u64()).unwrap_or(0),
            owned_by: m
                .get("owned_by")
                .and_then(|v| v.as_str())
                .unwrap_or("github-copilot")
                .to_string(),
        })
        .collect();

    Json(serde_json::to_value(ModelList {
        object: "list".into(),
        data: models,
    }).unwrap())
}

async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<Value>,
) -> Response {
    let stream = payload
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if stream {
        match state.client.chat_completions_stream(payload).await {
            Ok(sse_stream) => {
                let body = Body::from_stream(sse_stream.map(Ok::<_, std::io::Error>));
                Response::builder()
                    .header("Content-Type", "text/event-stream")
                    .header("Cache-Control", "no-cache")
                    .header("Connection", "keep-alive")
                    .header("X-Accel-Buffering", "no")
                    .body(body)
                    .unwrap()
            }
            Err(e) => error_response(StatusCode::BAD_GATEWAY, "server_error", &e),
        }
    } else {
        match state.client.chat_completions(&mut payload).await {
            Ok(result) => Json(result).into_response(),
            Err(e) if e.contains("auth") || e.contains("401") => {
                error_response(StatusCode::UNAUTHORIZED, "auth_error", &e)
            }
            Err(e) => error_response(StatusCode::BAD_GATEWAY, "server_error", &e),
        }
    }
}

fn error_response(status: StatusCode, error_type: &str, message: &str) -> Response {
    let body = ApiError {
        error: ApiErrorDetail {
            message: message.to_string(),
            error_type: error_type.to_string(),
            code: status.as_u16(),
        },
    };
    (status, Json(serde_json::to_value(body).unwrap())).into_response()
}

//! Integration tests for the copilot-wrapper API routes.
//!
//! These tests spin up the axum app with a mock Copilot backend (wiremock)
//! so no real GitHub/Copilot credentials are needed.

use std::sync::Arc;

use axum::body::Body;
use http_body_util::BodyExt;
use hyper::Request;
use serde_json::{json, Value};
use tower::ServiceExt;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use copilot_wrapper::auth::CopilotAuth;
use copilot_wrapper::copilot::CopilotClient;
use copilot_wrapper::{AppState, build_app};

/// Build a test app backed by a wiremock mock for the Copilot API.
async fn test_app() -> (axum::Router, MockServer) {
    let mock_server = MockServer::start().await;

    let auth = CopilotAuth::new(Some("ghp_fake_test_token".into()));
    {
        use copilot_wrapper::auth::now_secs;
        *auth.copilot_token.write().await = Some("cp_test_token".into());
        *auth.copilot_expires_at.write().await = now_secs() + 7200.0;
    }

    let client = CopilotClient::new_with_base_urls(
        auth,
        &format!("{}/chat/completions", mock_server.uri()),
        &format!("{}/models", mock_server.uri()),
    );
    let state = Arc::new(AppState { client });
    let app = build_app(state);

    (app, mock_server)
}

/// Helper: send a GET request and return (status, body as Value).
async fn get_json(app: axum::Router, uri: &str) -> (u16, Value) {
    let resp = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let value: Value = serde_json::from_slice(&body).unwrap();
    (status, value)
}

/// Helper: send a POST request with JSON body.
async fn post_json(app: axum::Router, uri: &str, payload: &Value) -> (u16, Vec<u8>) {
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let body = resp.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, body)
}

#[tokio::test]
async fn health_returns_ok() {
    let (app, _mock) = test_app().await;
    let (status, body) = get_json(app, "/health").await;
    assert_eq!(status, 200);
    assert_eq!(body, json!({"status": "ok"}));
}

#[tokio::test]
async fn models_returns_list_from_upstream() {
    let (app, mock) = test_app().await;

    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                {"id": "gpt-4o", "object": "model", "created": 1000, "owned_by": "github-copilot"},
                {"id": "gpt-4o-mini", "object": "model", "created": 1001, "owned_by": "github-copilot"},
            ]
        })))
        .mount(&mock)
        .await;

    let (status, body) = get_json(app, "/v1/models").await;
    assert_eq!(status, 200);
    assert_eq!(body["object"], "list");
    let data = body["data"].as_array().unwrap();
    assert_eq!(data.len(), 2);
    assert_eq!(data[0]["id"], "gpt-4o");
}

#[tokio::test]
async fn models_fallback_when_upstream_fails() {
    let (app, mock) = test_app().await;

    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock)
        .await;

    let (status, body) = get_json(app, "/v1/models").await;
    assert_eq!(status, 200);
    let data = body["data"].as_array().unwrap();
    assert!(!data.is_empty());
    let ids: Vec<&str> = data.iter().filter_map(|m| m["id"].as_str()).collect();
    assert!(ids.contains(&"gpt-4o"));
    assert!(ids.contains(&"gpt-4o-mini"));
}

#[tokio::test]
async fn chat_completions_non_streaming() {
    let (app, mock) = test_app().await;

    let upstream_response = json!({
        "id": "chatcmpl-test123",
        "object": "chat.completion",
        "created": 1234567890,
        "model": "gpt-4o-mini",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "Hello!"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&upstream_response))
        .mount(&mock)
        .await;

    let payload = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Hi"}],
        "stream": false
    });
    let (status, raw) = post_json(app, "/v1/chat/completions", &payload).await;
    let body: Value = serde_json::from_slice(&raw).unwrap();

    assert_eq!(status, 200);
    assert_eq!(body["id"], "chatcmpl-test123");
    assert_eq!(body["choices"][0]["message"]["content"], "Hello!");
}

#[tokio::test]
async fn chat_completions_streaming() {
    let (app, mock) = test_app().await;

    let sse_body = [
        "data: {\"id\":\"chatcmpl-s1\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}],\"created\":1234,\"model\":\"gpt-4o-mini\"}\n\n",
        "data: {\"id\":\"chatcmpl-s1\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hi\"}}],\"created\":1234,\"model\":\"gpt-4o-mini\"}\n\n",
        "data: {\"id\":\"chatcmpl-s1\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"!\"}}],\"created\":1234,\"model\":\"gpt-4o-mini\"}\n\n",
        "data: [DONE]\n\n",
    ].concat();

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Type", "text/event-stream")
                .set_body_string(sse_body),
        )
        .mount(&mock)
        .await;

    let payload = json!({
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Hi"}],
        "stream": true
    });
    let (status, raw) = post_json(app, "/v1/chat/completions", &payload).await;
    let body_text = String::from_utf8(raw).unwrap();

    assert_eq!(status, 200);
    assert!(body_text.contains("data: "), "Should contain SSE data frames");
    assert!(body_text.contains("[DONE]"), "Should contain [DONE] marker");
    assert!(body_text.contains("\"content\":\"Hi\""), "Should contain streamed content");
}

#[tokio::test]
async fn chat_completions_upstream_error_returns_502() {
    let (app, mock) = test_app().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
        .mount(&mock)
        .await;

    let payload = json!({
        "model": "gpt-4o",
        "messages": [{"role": "user", "content": "Hi"}],
        "stream": false
    });
    let (status, raw) = post_json(app, "/v1/chat/completions", &payload).await;
    let body: Value = serde_json::from_slice(&raw).unwrap();

    assert_eq!(status, 502);
    assert_eq!(body["error"]["code"], 502);
}

#[tokio::test]
async fn cors_headers_present() {
    let (app, _mock) = test_app().await;
    let resp = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("access-control-allow-origin").unwrap(),
        "*"
    );
}

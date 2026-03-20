/// HTTP client for the GitHub Copilot Chat API.
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::Value;

use crate::auth::CopilotAuth;

const COPILOT_CHAT_URL: &str = "https://api.githubcopilot.com/chat/completions";
const COPILOT_MODELS_URL: &str = "https://api.githubcopilot.com/models";

const DEFAULT_MODELS: &[&str] = &[
    "gpt-4o",
    "gpt-4o-mini",
    "gpt-4",
    "gpt-3.5-turbo",
    "claude-3.5-sonnet",
    "claude-3.5-haiku",
    "o1-preview",
    "o1-mini",
    "o3-mini",
];

pub struct CopilotClient {
    auth: CopilotAuth,
    http: Client,
}

impl CopilotClient {
    pub fn new(auth: CopilotAuth) -> Self {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap();
        Self { auth, http }
    }

    async fn headers(&self) -> Result<Vec<(&'static str, String)>, String> {
        let token = self.auth.get_copilot_token().await?;
        Ok(vec![
            ("Authorization", format!("Bearer {token}")),
            ("Content-Type", "application/json".into()),
            ("Accept", "application/json".into()),
            ("User-Agent", "GitHubCopilotChat/0.1".into()),
            ("Editor-Version", "vscode/1.95.0".into()),
            ("Editor-Plugin-Version", "copilot-chat/0.22.0".into()),
            ("Openai-Intent", "conversation-panel".into()),
            ("Copilot-Integration-Id", "vscode-chat".into()),
        ])
    }

    /// Non-streaming chat completion — returns raw JSON value.
    pub async fn chat_completions(&self, payload: &mut Value) -> Result<Value, String> {
        payload["stream"] = Value::Bool(false);
        let headers = self.headers().await?;

        let mut req = self.http.post(COPILOT_CHAT_URL);
        for (k, v) in &headers {
            req = req.header(*k, v);
        }

        let resp = req
            .json(payload)
            .send()
            .await
            .map_err(|e| format!("Copilot API error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Copilot API error (HTTP {status}): {body}"));
        }

        resp.json().await.map_err(|e| format!("JSON parse error: {e}"))
    }

    /// Streaming chat completion — returns a stream of SSE `data: ...` strings.
    pub async fn chat_completions_stream(
        &self,
        mut payload: Value,
    ) -> Result<impl futures_util::Stream<Item = String> + use<>, String> {
        payload["stream"] = Value::Bool(true);
        let mut headers = self.headers().await?;
        // Override Accept for SSE
        for h in &mut headers {
            if h.0 == "Accept" {
                h.1 = "text/event-stream".into();
            }
        }

        let mut req = self.http.post(COPILOT_CHAT_URL);
        for (k, v) in &headers {
            req = req.header(*k, v);
        }

        let resp = req
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Copilot streaming error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Copilot API error (HTTP {status}): {body}"));
        }

        // Stream bytes → lines → SSE data frames
        let byte_stream = resp.bytes_stream();

        let sse_stream = async_stream(byte_stream);

        Ok(sse_stream)
    }

    /// List available models, with fallback to defaults.
    pub async fn list_models(&self) -> Vec<Value> {
        let result: Result<Vec<Value>, _> = async {
            let headers = self.headers().await?;
            let mut req = self.http.get(COPILOT_MODELS_URL);
            for (k, v) in &headers {
                req = req.header(*k, v);
            }
            let resp = req.send().await.map_err(|e| format!("{e}"))?;
            if resp.status().as_u16() != 200 {
                return Err("non-200".into());
            }
            let data: Value = resp.json().await.map_err(|e| format!("{e}"))?;
            if let Some(arr) = data.get("data").and_then(|d| d.as_array()) {
                return Ok(arr.clone());
            }
            if let Some(arr) = data.as_array() {
                return Ok(arr.clone());
            }
            Err("unexpected format".into())
        }
        .await;

        result.unwrap_or_else(|_: String| {
            DEFAULT_MODELS
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "id": m,
                        "object": "model",
                        "created": 0,
                        "owned_by": "github-copilot",
                    })
                })
                .collect()
        })
    }
}

/// Convert a reqwest byte stream into an SSE line stream.
fn async_stream(
    byte_stream: impl futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
) -> impl futures_util::Stream<Item = String> {
    // Accumulate partial lines
    let mut buffer = String::new();

    futures_util::stream::unfold(
        (byte_stream, buffer),
        |(mut stream, mut buf)| async move {
            loop {
                // Try to extract a complete line from the buffer
                if let Some(pos) = buf.find('\n') {
                    let line = buf[..pos].trim().to_string();
                    buf = buf[pos + 1..].to_string();

                    if line.is_empty() {
                        continue;
                    }

                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            return Some(("data: [DONE]\n\n".to_string(), (stream, buf)));
                        }
                        // Validate JSON
                        if serde_json::from_str::<Value>(data).is_ok() {
                            return Some((
                                format!("data: {data}\n\n"),
                                (stream, buf),
                            ));
                        }
                        continue;
                    }
                    continue;
                }

                // Need more data
                match stream.next().await {
                    Some(Ok(chunk)) => {
                        buf.push_str(&String::from_utf8_lossy(&chunk));
                    }
                    _ => return None, // Stream ended
                }
            }
        },
    )
}

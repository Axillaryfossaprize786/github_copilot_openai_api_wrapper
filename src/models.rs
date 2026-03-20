/// Serde models for OpenAI-compatible API types.
use serde::{Deserialize, Serialize};

// ── Request ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

// ── Response (non-streaming) ─────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChatCompletionChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatCompletionChoice {
    pub index: u32,
    pub message: ChatMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// ── Models list ──────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}

#[derive(Debug, Serialize)]
pub struct ModelList {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

// ── Error ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: ApiErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ApiErrorDetail {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    pub code: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_request_deserialize_minimal() {
        let json = r#"{"model":"gpt-4o","messages":[{"role":"user","content":"hi"}]}"#;
        let req: ChatCompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "gpt-4o");
        assert_eq!(req.messages.len(), 1);
        assert!(!req.stream);
        assert!(req.temperature.is_none());
        assert!(req.max_tokens.is_none());
    }

    #[test]
    fn chat_request_deserialize_full() {
        let json = r#"{
            "model": "gpt-4o-mini",
            "messages": [{"role": "system", "content": "be helpful"}, {"role": "user", "content": "hi"}],
            "temperature": 0.7,
            "top_p": 0.9,
            "max_tokens": 100,
            "stream": true,
            "n": 2
        }"#;
        let req: ChatCompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "gpt-4o-mini");
        assert_eq!(req.messages.len(), 2);
        assert!(req.stream);
        assert_eq!(req.temperature, Some(0.7));
        assert_eq!(req.top_p, Some(0.9));
        assert_eq!(req.max_tokens, Some(100));
        assert_eq!(req.n, Some(2));
    }

    #[test]
    fn chat_request_roundtrip() {
        let req = ChatCompletionRequest {
            model: "gpt-4o".into(),
            messages: vec![ChatMessage { role: "user".into(), content: "test".into() }],
            temperature: Some(0.5),
            top_p: None,
            max_tokens: Some(50),
            stream: false,
            stop: None,
            n: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("top_p")); // skip_serializing_if
        assert!(!json.contains("stop"));
        assert!(json.contains("\"temperature\":0.5"));
    }

    #[test]
    fn model_list_serialize() {
        let list = ModelList {
            object: "list".into(),
            data: vec![
                ModelInfo { id: "gpt-4o".into(), object: "model".into(), created: 0, owned_by: "github-copilot".into() },
            ],
        };
        let v = serde_json::to_value(&list).unwrap();
        assert_eq!(v["object"], "list");
        assert_eq!(v["data"][0]["id"], "gpt-4o");
    }

    #[test]
    fn api_error_serialize() {
        let err = ApiError {
            error: ApiErrorDetail {
                message: "bad".into(),
                error_type: "server_error".into(),
                code: 502,
            },
        };
        let v = serde_json::to_value(&err).unwrap();
        assert_eq!(v["error"]["type"], "server_error");
        assert_eq!(v["error"]["code"], 502);
    }

    #[test]
    fn chat_completion_response_deserialize() {
        let json = r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1234567890,
            "model": "gpt-4o",
            "choices": [{"index": 0, "message": {"role": "assistant", "content": "Hello"}, "finish_reason": "stop"}]
        }"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, "chatcmpl-123");
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(resp.choices[0].message.content, "Hello");
        assert_eq!(resp.choices[0].finish_reason, Some("stop".to_string()));
        assert!(resp.usage.is_none());
    }
}

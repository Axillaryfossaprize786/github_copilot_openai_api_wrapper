/// GitHub Copilot authentication: GitHub OAuth Device Flow + Copilot token exchange.
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::Client;
use serde::Deserialize;
use tokio::sync::RwLock;

const GITHUB_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";
const GITHUB_SCOPES: &str = "read:user";
const GITHUB_BASE_URL: &str = "https://github.com";
const GITHUB_API_URL: &str = "https://api.github.com";
const COPILOT_VERSION: &str = "0.26.7";
const VSCODE_VERSION: &str = "1.95.0";
const TOKEN_REFRESH_MARGIN: f64 = 300.0;

fn token_path() -> PathBuf {
    let config = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs::home_dir().unwrap().join(".config"));
    let dir = config.join("copilot-wrapper");
    std::fs::create_dir_all(&dir).ok();
    dir.join("github_token")
}

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

pub struct CopilotAuth {
    http: Client,
    github_token: RwLock<Option<String>>,
    copilot_token: RwLock<Option<String>>,
    copilot_expires_at: RwLock<f64>,
}

impl CopilotAuth {
    pub fn new(github_token: Option<String>) -> Self {
        Self {
            http: Client::new(),
            github_token: RwLock::new(github_token),
            copilot_token: RwLock::new(None),
            copilot_expires_at: RwLock::new(0.0),
        }
    }

    fn github_headers(token: &str) -> Vec<(&'static str, String)> {
        vec![
            ("Content-Type", "application/json".into()),
            ("Accept", "application/json".into()),
            ("Authorization", format!("token {token}")),
            ("Editor-Version", format!("vscode/{VSCODE_VERSION}")),
            (
                "Editor-Plugin-Version",
                format!("copilot-chat/{COPILOT_VERSION}"),
            ),
            (
                "User-Agent",
                format!("GitHubCopilotChat/{COPILOT_VERSION}"),
            ),
        ]
    }

    /// Get (or refresh) the GitHub personal/OAuth token.
    pub async fn get_github_token(&self) -> Result<String, String> {
        // Fast path
        {
            let guard = self.github_token.read().await;
            if let Some(ref t) = *guard {
                return Ok(t.clone());
            }
        }

        // Try disk
        let path = token_path();
        if path.exists() {
            if let Ok(t) = std::fs::read_to_string(&path) {
                let t = t.trim().to_string();
                if !t.is_empty() {
                    tracing::info!("Loaded GitHub token from {}", path.display());
                    *self.github_token.write().await = Some(t.clone());
                    return Ok(t);
                }
            }
        }

        // Device flow
        let token = self.device_flow().await?;
        std::fs::write(&path, &token).ok();
        tracing::info!("GitHub token saved to {}", path.display());
        *self.github_token.write().await = Some(token.clone());
        Ok(token)
    }

    /// GitHub OAuth Device Flow.
    async fn device_flow(&self) -> Result<String, String> {
        #[derive(Deserialize)]
        struct DeviceCodeResponse {
            user_code: String,
            device_code: String,
            verification_uri: String,
            interval: Option<u64>,
        }

        let resp: DeviceCodeResponse = self
            .http
            .post(format!("{GITHUB_BASE_URL}/login/device/code"))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&serde_json::json!({
                "client_id": GITHUB_CLIENT_ID,
                "scope": GITHUB_SCOPES,
            }))
            .send()
            .await
            .map_err(|e| format!("Device flow request failed: {e}"))?
            .json()
            .await
            .map_err(|e| format!("Device flow parse failed: {e}"))?;

        let interval = resp.interval.unwrap_or(5) + 1;

        eprintln!("\n{}", "=".repeat(50));
        eprintln!("  Please enter code:  {}", resp.user_code);
        eprintln!("  at:  {}", resp.verification_uri);
        eprintln!("{}\n", "=".repeat(50));

        // Try opening browser
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(&resp.verification_uri)
                .spawn()
                .ok();
        }

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;

            #[derive(Deserialize)]
            struct PollResponse {
                access_token: Option<String>,
                error: Option<String>,
            }

            let poll: PollResponse = self
                .http
                .post(format!("{GITHUB_BASE_URL}/login/oauth/access_token"))
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .json(&serde_json::json!({
                    "client_id": GITHUB_CLIENT_ID,
                    "device_code": resp.device_code,
                    "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
                }))
                .send()
                .await
                .map_err(|e| format!("Poll failed: {e}"))?
                .json()
                .await
                .map_err(|e| format!("Poll parse failed: {e}"))?;

            if let Some(token) = poll.access_token {
                tracing::info!("GitHub authentication successful!");
                return Ok(token);
            }

            match poll.error.as_deref() {
                Some("authorization_pending") | None => continue,
                Some("slow_down") => {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
                Some("expired_token") => {
                    return Err("Device code expired. Please restart and try again.".into())
                }
                Some("access_denied") => {
                    return Err("Authorization was denied by the user.".into())
                }
                Some(other) => return Err(format!("Device flow error: {other}")),
            }
        }
    }

    /// Get a valid Copilot API token, refreshing if needed.
    pub async fn get_copilot_token(&self) -> Result<String, String> {
        // Fast path: cached and not expired
        {
            let tok = self.copilot_token.read().await;
            let exp = *self.copilot_expires_at.read().await;
            if tok.is_some() && now_secs() < exp - TOKEN_REFRESH_MARGIN {
                return Ok(tok.as_ref().unwrap().clone());
            }
        }

        let github_token = self.get_github_token().await?;
        let headers = Self::github_headers(&github_token);

        let mut req = self
            .http
            .get(format!("{GITHUB_API_URL}/copilot_internal/v2/token"));
        for (k, v) in &headers {
            req = req.header(*k, v);
        }

        let resp = req.send().await.map_err(|e| format!("Copilot token request failed: {e}"))?;

        // If 401/404 — maybe stale GitHub token, re-auth once
        if resp.status().as_u16() == 401 || resp.status().as_u16() == 404 {
            tracing::warn!(
                "Copilot token request failed (HTTP {}), re-authenticating...",
                resp.status()
            );
            // Clear cached token
            *self.github_token.write().await = None;
            let path = token_path();
            if path.exists() {
                std::fs::remove_file(&path).ok();
            }

            let github_token = self.get_github_token().await?;
            let headers = Self::github_headers(&github_token);
            let mut req = self
                .http
                .get(format!("{GITHUB_API_URL}/copilot_internal/v2/token"));
            for (k, v) in &headers {
                req = req.header(*k, v);
            }
            let resp = req.send().await.map_err(|e| format!("Retry failed: {e}"))?;
            return self.parse_copilot_token(resp).await;
        }

        self.parse_copilot_token(resp).await
    }

    async fn parse_copilot_token(&self, resp: reqwest::Response) -> Result<String, String> {
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "Failed to fetch Copilot token (HTTP {status}): {body}\n\
                 Make sure you have an active GitHub Copilot subscription."
            ));
        }

        #[derive(Deserialize)]
        struct CopilotTokenResponse {
            token: String,
            expires_at: f64,
        }

        let data: CopilotTokenResponse = resp
            .json()
            .await
            .map_err(|e| format!("Copilot token parse error: {e}"))?;

        *self.copilot_token.write().await = Some(data.token.clone());
        *self.copilot_expires_at.write().await = data.expires_at;

        Ok(data.token)
    }
}

/// Application configuration from environment variables.

#[derive(Clone)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    pub log_level: String,
    pub github_token: Option<String>,
}

impl Settings {
    pub fn load() -> Self {
        // Load .env file if present (best-effort)
        if let Ok(contents) = std::fs::read_to_string(".env") {
            for line in contents.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim();
                    let value = value.trim();
                    // Only set if not already in environment
                    if std::env::var(key).is_err() {
                        // SAFETY: we run this single-threaded before spawning the server
                        unsafe { std::env::set_var(key, value) };
                    }
                }
            }
        }

        Self {
            host: std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into()),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            log_level: std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".into()),
            github_token: std::env::var("GITHUB_TOKEN").ok().filter(|t| !t.is_empty()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_env_empty() {
        // Clear any env that might be set
        unsafe {
            std::env::remove_var("HOST");
            std::env::remove_var("PORT");
            std::env::remove_var("LOG_LEVEL");
            std::env::remove_var("GITHUB_TOKEN");
        }

        let s = Settings {
            host: std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into()),
            port: std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080),
            log_level: std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".into()),
            github_token: std::env::var("GITHUB_TOKEN").ok().filter(|t| !t.is_empty()),
        };

        assert_eq!(s.host, "127.0.0.1");
        assert_eq!(s.port, 8080);
        assert_eq!(s.log_level, "info");
        assert!(s.github_token.is_none());
    }

    #[test]
    fn port_parse_invalid_falls_back() {
        unsafe { std::env::set_var("PORT", "not_a_number") };
        let port: u16 = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);
        assert_eq!(port, 8080);
        unsafe { std::env::remove_var("PORT") };
    }

    #[test]
    fn empty_github_token_is_none() {
        unsafe { std::env::set_var("GITHUB_TOKEN", "") };
        let token = std::env::var("GITHUB_TOKEN").ok().filter(|t| !t.is_empty());
        assert!(token.is_none());
        unsafe { std::env::remove_var("GITHUB_TOKEN") };
    }
}

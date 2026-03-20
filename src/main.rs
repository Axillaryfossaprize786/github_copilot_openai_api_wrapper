use std::sync::Arc;
use tracing_subscriber::EnvFilter;

use copilot_wrapper::auth::CopilotAuth;
use copilot_wrapper::config::Settings;
use copilot_wrapper::copilot::CopilotClient;
use copilot_wrapper::{AppState, build_app};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let settings = Settings::load();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(&settings.log_level)),
        )
        .init();

    let auth = CopilotAuth::new(settings.github_token.clone());
    let client = CopilotClient::new(auth);
    let state = Arc::new(AppState { client });

    let app = build_app(state);

    let addr = format!("{}:{}", settings.host, settings.port);
    tracing::info!("Copilot OpenAI wrapper listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    tracing::info!("Copilot OpenAI wrapper stopped");
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.ok();
    tracing::info!("Shutdown signal received");
}

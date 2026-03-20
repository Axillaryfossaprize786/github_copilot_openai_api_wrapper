mod auth;
mod config;
mod copilot;
mod models;
mod routes;

use std::sync::Arc;
use axum::Router;
use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;

use auth::CopilotAuth;
use config::Settings;
use copilot::CopilotClient;

/// Shared application state.
pub struct AppState {
    pub client: CopilotClient,
}

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

    let app = Router::new()
        .merge(routes::router())
        .layer(CorsLayer::permissive())
        .with_state(state);

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

use axum;
use std::sync::Arc;
use std::time::Instant;
use tokio;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tower_http::trace::TraceLayer;

use graph::{
    api,
    config::Config,
    api::ApiState,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    println!("Starting temporal knowledge graph API server...");

    // Load config for testing/local development
    let config = Config::for_testing();

    // Create ApiState using initialize
    let state = Arc::new(ApiState::initialize(&config).await?);

    // Create router with just the health check endpoint
    let app = api::create_router(state)
        .layer(TraceLayer::new_for_http());

    // Start server
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("Listening on {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
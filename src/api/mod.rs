pub mod handlers;
pub mod models;
pub mod middleware;
pub mod error;

use axum::{
    middleware::{from_fn, from_fn_with_state},
    routing::{get, post},
    Router,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::types::{Node, Edge, NodeId, EdgeId, EntityId, Properties, TemporalRange, Timestamp, EntityType};
use self::{
    models::*,
    handlers::*,
    middleware::{auth, RateLimiter, rate_limit, request_logger},
    error::{ApiError, ApiResult},
};

/// API state shared across handlers
pub struct ApiState {
    start_time: Instant,
    // Add more state fields as needed
}

impl ApiState {
    /// Create a new API state
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }
    
    /// Get API uptime
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        health_check,
        version,
        create_node,
        get_node,
        update_node,
        delete_node,
        create_edge,
        get_edge,
        update_edge,
        delete_edge,
        query_knowledge,
        store_information,
    ),
    components(
        schemas(
            Node, Edge, NodeId, EdgeId, EntityId, Properties, TemporalRange, Timestamp, EntityType,
            VersionInfo, HealthCheckResponse, ComponentHealth, ComponentStatus,
            CreateNodeRequest, UpdateNodeRequest, 
            CreateEdgeRequest, UpdateEdgeRequest,
            BatchCreateNodesRequest, BatchCreateEdgesRequest, BatchOperationError,
            QueryRequest, QueryResponse, QueryResult, StoreRequest,
        )
    ),
    tags(
        (name = "health", description = "Health and version endpoints"),
        (name = "nodes", description = "Node management endpoints"),
        (name = "edges", description = "Edge management endpoints"),
        (name = "knowledge", description = "Knowledge graph operations"),
    ),
    info(
        title = "Temporal Knowledge Graph API",
        description = "API for managing a temporal knowledge graph with dynamic memory capabilities",
        version = "0.1.0"
    )
)]
struct ApiDoc;

/// Create the API router
pub fn create_router() -> Router {
    // Create API state
    let state = Arc::new(ApiState::new());
    
    // Create a basic router with health endpoints
    let app = Router::new()
        // Health routes
        .route("/health", get(handlers::health_check))
        .route("/version", get(handlers::version))
        
        // Swagger UI for API documentation
        .merge(SwaggerUi::new("/swagger-ui")
            .url("/api-docs/openapi.json", ApiDoc::openapi()))
        
        // Add request logging and tracing
        .layer(from_fn(request_logger))
        .layer(TraceLayer::new_for_http())
        .with_state(state);
        
    app
} 
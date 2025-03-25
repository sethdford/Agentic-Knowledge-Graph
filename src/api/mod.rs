pub mod handlers;
pub mod models;
pub mod middleware;

use axum::{
    middleware::from_fn_with_state,
    routing::{get, post, patch, delete},
    Router,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use self::models::*;
use self::handlers::*;
use self::middleware::{auth, RateLimiter, rate_limit, request_logger};

/// API error types
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Bad request: {0}")]
    BadRequest(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
    
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
}

/// Result type for API operations
pub type ApiResult<T> = Result<T, ApiError>;

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
        handlers::health_check,
        handlers::version,
        handlers::create_node,
        handlers::get_node,
        handlers::update_node,
        handlers::delete_node,
        handlers::create_nodes_batch,
        handlers::create_edge,
        handlers::get_edge,
        handlers::update_edge,
        handlers::delete_edge,
        handlers::create_edges_batch,
        handlers::query_knowledge,
        handlers::store_information,
    ),
    components(
        schemas(
            CreateNodeRequest, UpdateNodeRequest,
            CreateEdgeRequest, UpdateEdgeRequest,
            BatchCreateNodesRequest, BatchCreateEdgesRequest,
            BatchOperationFailure, BatchOperationResponse, 
            Node, Edge, NodeId, EdgeId, EntityId, Properties,
            ComponentStatus, ComponentHealth, HealthCheckResponse, VersionInfo,
            QueryRequest, QueryResult, QueryResponse, StoreRequest,
            TemporalRange, Timestamp
        )
    ),
    tags(
        (name = "health", description = "Health and monitoring endpoints"),
        (name = "nodes", description = "Node CRUD operations"),
        (name = "edges", description = "Edge CRUD operations"),
        (name = "knowledge", description = "Knowledge graph operations")
    ),
    info(
        title = "Knowledge Graph API",
        version = "1.0.0",
        description = "REST API for a temporal knowledge graph with entity and relationship handling"
    )
)]
pub struct ApiDoc;

/// Request to store information in the graph
#[derive(Debug, serde::Deserialize)]
pub struct StoreRequest {
    /// Text to process and store in the graph
    pub text: String,
    /// Optional context for entity extraction
    pub context: Option<String>,
    /// Optional metadata to attach to the stored information
    pub metadata: Option<serde_json::Value>,
}

/// Create router with all API routes
pub fn create_router() -> Router {
    let state = Arc::new(ApiState::new());
    
    // Create rate limiter
    let rate_limiter = RateLimiter::new(100, Duration::from_secs(60));
    
    // API routes that require authentication
    let authenticated_routes = Router::new()
        // Node routes
        .route("/nodes", post(handlers::create_node))
        .route("/nodes/batch", post(handlers::create_nodes_batch))
        .route("/nodes/:id", get(handlers::get_node)
            .patch(handlers::update_node)
            .delete(handlers::delete_node))
        
        // Edge routes
        .route("/edges", post(handlers::create_edge))
        .route("/edges/batch", post(handlers::create_edges_batch))
        .route("/edges/:id", get(handlers::get_edge)
            .patch(handlers::update_edge)
            .delete(handlers::delete_edge))
        
        // Knowledge graph routes
        .route("/knowledge/query", post(handlers::query_knowledge))
        .route("/knowledge/store", post(handlers::store_information))
        
        // Apply authentication middleware
        .route_layer(axum::middleware::from_fn(auth))
        
        // Apply rate limiting with state
        .route_layer(from_fn_with_state(rate_limiter, rate_limit));
    
    // Public routes that don't require authentication
    let public_routes = Router::new()
        // Health routes
        .route("/health", get(handlers::health_check))
        .route("/version", get(handlers::version));
    
    // API documentation
    let swagger_ui = SwaggerUi::new("/swagger-ui")
        .url("/api-docs/openapi.json", ApiDoc::openapi());
    
    // Combine all routes and apply common middleware
    Router::new()
        .merge(authenticated_routes)
        .merge(public_routes)
        .merge(swagger_ui)
        .layer(axum::middleware::from_fn(request_logger))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
} 
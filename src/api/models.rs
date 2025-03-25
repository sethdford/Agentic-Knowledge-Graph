use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use utoipa::ToSchema;

use crate::types::{Node, Edge, EntityId, Timestamp, TemporalRange, NodeId, EntityType, Properties, EdgeId};

/// Request to store information in the graph
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreRequest {
    /// Text to process and store in the graph
    pub text: String,
    /// Optional context for entity extraction
    pub context: Option<String>,
    /// Optional metadata to attach to the stored information
    pub metadata: Option<serde_json::Value>,
}

/// Request to query knowledge from the graph
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct QueryRequest {
    /// Query text
    pub query: String,
    /// Optional timestamp to query at
    pub timestamp: Option<DateTime<Utc>>,
    /// Optional time window in seconds
    pub time_window: Option<i64>,
}

/// Response from a knowledge query
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct QueryResponse {
    /// Query results
    pub results: Vec<QueryResult>,
    /// Relevant context
    pub context: Vec<String>,
}

/// Single result from a knowledge query
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct QueryResult {
    /// Node ID
    pub id: Uuid,
    /// Content
    pub content: String,
    /// Confidence score
    pub confidence: f64,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Request to create a new node
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateNodeRequest {
    /// Entity type
    pub entity_type: EntityType,
    /// Node label
    pub label: String,
    /// Node properties
    pub properties: Properties,
    /// Temporal validity range
    #[schema(inline)]
    pub valid_time: Option<TemporalRange>,
}

/// Request to create a new edge
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateEdgeRequest {
    /// Source node ID
    pub source_id: Uuid,
    /// Target node ID
    pub target_id: Uuid,
    /// Edge label
    pub label: String,
    /// Edge properties
    pub properties: Properties,
    /// Temporal validity range
    #[schema(inline)]
    pub valid_time: Option<TemporalRange>,
}

/// Request to update a node
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateNodeRequest {
    /// Node label
    pub label: Option<String>,
    /// Node properties to update
    pub properties: Option<Properties>,
    /// Valid time range
    pub valid_time: Option<TemporalRange>,
}

/// Request to update an edge
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateEdgeRequest {
    /// Edge label
    pub label: Option<String>,
    /// Edge properties to update
    pub properties: Option<Properties>,
    /// Valid time range
    pub valid_time: Option<TemporalRange>,
}

/// Request for batch node creation
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct BatchCreateNodesRequest {
    /// List of nodes to create
    pub nodes: Vec<CreateNodeRequest>,
}

/// Request for batch edge creation
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct BatchCreateEdgesRequest {
    /// List of edges to create
    pub edges: Vec<CreateEdgeRequest>,
}

/// Response for batch operations
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BatchOperationResponse {
    /// Number of successful operations
    pub success_count: usize,
    /// List of failed operations with error messages
    pub failures: Vec<BatchOperationError>,
}

/// Error details for batch operations
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BatchOperationError {
    /// Index of the failed operation
    pub index: usize,
    /// Error message
    pub error: String,
}

/// API version information
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct VersionInfo {
    /// API version
    pub version: String,
    /// Build timestamp
    pub build_timestamp: DateTime<Utc>,
    /// Git commit hash
    pub commit_hash: String,
}

/// Health check response
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HealthCheckResponse {
    /// Status of the service
    pub status: String,
    /// Version information
    pub version: String,
    /// Uptime in seconds
    pub uptime: u64,
    /// Component health statuses
    pub components: Vec<ComponentHealth>,
}

/// Health status of a system component
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ComponentHealth {
    /// Component name
    pub name: String,
    /// Component status
    pub status: ComponentStatus,
    /// Additional details
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum ComponentStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

// Implement custom schemas for imported types
impl<'s> utoipa::ToSchema<'s> for TemporalRange {
    fn schema() -> (&'s str, utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>) {
        (
            "TemporalRange",
            utoipa::openapi::RefOr::T(utoipa::openapi::schema::Schema::Object(
                utoipa::openapi::schema::ObjectBuilder::new()
                    .description(Some("Temporal validity range"))
                    .build()
            ))
        )
    }
}

impl<'s> utoipa::ToSchema<'s> for Timestamp {
    fn schema() -> (&'s str, utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>) {
        (
            "Timestamp",
            utoipa::openapi::RefOr::T(utoipa::openapi::schema::Schema::Object(
                utoipa::openapi::schema::ObjectBuilder::new()
                    .description(Some("ISO 8601 / RFC 3339 date-time"))
                    .build()
            ))
        )
    }
}

// Implement custom schemas for remaining imported types
impl<'s> utoipa::ToSchema<'s> for NodeId {
    fn schema() -> (&'s str, utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>) {
        (
            "NodeId",
            utoipa::openapi::RefOr::T(utoipa::openapi::schema::Schema::Object(
                utoipa::openapi::schema::ObjectBuilder::new()
                    .description(Some("UUID identifier for a node"))
                    .build()
            ))
        )
    }
}

impl<'s> utoipa::ToSchema<'s> for EdgeId {
    fn schema() -> (&'s str, utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>) {
        (
            "EdgeId",
            utoipa::openapi::RefOr::T(utoipa::openapi::schema::Schema::Object(
                utoipa::openapi::schema::ObjectBuilder::new()
                    .description(Some("UUID identifier for an edge"))
                    .build()
            ))
        )
    }
}

impl<'s> utoipa::ToSchema<'s> for EntityId {
    fn schema() -> (&'s str, utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>) {
        (
            "EntityId",
            utoipa::openapi::RefOr::T(utoipa::openapi::schema::Schema::Object(
                utoipa::openapi::schema::ObjectBuilder::new()
                    .description(Some("Unique identifier for an entity"))
                    .build()
            ))
        )
    }
}

impl<'s> utoipa::ToSchema<'s> for Properties {
    fn schema() -> (&'s str, utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>) {
        (
            "Properties",
            utoipa::openapi::RefOr::T(utoipa::openapi::schema::Schema::Object(
                utoipa::openapi::schema::ObjectBuilder::new()
                    .description(Some("Key-value properties for an entity"))
                    .build()
            ))
        )
    }
}

impl<'s> utoipa::ToSchema<'s> for Node {
    fn schema() -> (&'s str, utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>) {
        (
            "Node",
            utoipa::openapi::RefOr::T(utoipa::openapi::schema::Schema::Object(
                utoipa::openapi::schema::ObjectBuilder::new()
                    .description(Some("Node in the graph"))
                    .build()
            ))
        )
    }
}

impl<'s> utoipa::ToSchema<'s> for Edge {
    fn schema() -> (&'s str, utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>) {
        (
            "Edge",
            utoipa::openapi::RefOr::T(utoipa::openapi::schema::Schema::Object(
                utoipa::openapi::schema::ObjectBuilder::new()
                    .description(Some("Edge connecting two nodes in the graph"))
                    .build()
            ))
        )
    }
}

impl<'s> utoipa::ToSchema<'s> for EntityType {
    fn schema() -> (&'s str, utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>) {
        (
            "EntityType",
            utoipa::openapi::RefOr::T(utoipa::openapi::schema::Schema::Object(
                utoipa::openapi::schema::ObjectBuilder::new()
                    .description(Some("Type of entity (Node, Edge, etc.)"))
                    .build()
            ))
        )
    }
} 
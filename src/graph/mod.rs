use std::sync::Arc;
use futures::future::Future;
use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration};
use std::time::Duration as StdDuration;
use tracing::{debug, error, info, warn};
use std::collections::HashMap;
use gremlin_client::{GValue, ToGValue, GResultSet};

use crate::{
    error::{Error, Result},
    types::{FromLocalResultSet, TimeRange},
    Config,
};

// Re-export common types for external use
pub use crate::types::{Node, Edge, NodeId, EdgeId, TemporalRange, Properties, EntityType, Timestamp};

pub mod neptune;
pub mod query;

/// Core trait defining graph operations
#[async_trait]
pub trait Graph: Send + Sync {
    /// Create a new node
    async fn create_node(&self, node: Node) -> Result<NodeId>;
    
    /// Get a node by ID
    async fn get_node(&self, id: NodeId) -> Result<Node>;
    
    /// Update a node
    async fn update_node(&self, node: Node) -> Result<()>;
    
    /// Delete a node
    async fn delete_node(&self, id: NodeId) -> Result<()>;
    
    /// Create a new edge
    async fn create_edge(&self, edge: Edge) -> Result<EdgeId>;
    
    /// Get an edge by ID
    async fn get_edge(&self, id: EdgeId) -> Result<Edge>;
    
    /// Update an edge
    async fn update_edge(&self, edge: Edge) -> Result<()>;
    
    /// Delete an edge
    async fn delete_edge(&self, id: EdgeId) -> Result<()>;
    
    /// Get all edges connected to a node within a temporal range
    async fn get_edges_for_node(
        &self,
        node_id: NodeId,
        temporal_range: Option<TemporalRange>,
    ) -> Result<Vec<Edge>>;
    
    /// Get all nodes connected to a node within a temporal range
    async fn get_connected_nodes(
        &self,
        node_id: NodeId,
        temporal_range: Option<TemporalRange>,
    ) -> Result<Vec<Node>>;

    /// Execute a query
    async fn execute_query<T>(&self, query: &str, params: &[(&str, &dyn ToGValue)]) -> Result<T> 
    where
        T: FromLocalResultSet;

    /// Execute a query with built-in retry mechanism
    async fn execute_query_with_retry<T>(&self, query: &str, params: &[(&str, &dyn ToGValue)]) -> Result<T>
    where
        T: FromLocalResultSet;

    /// Get all nodes with a given label
    async fn get_nodes_by_label(&self, label: &str) -> Result<Vec<Node>>;

    /// Get all edges with a given label
    async fn get_edges_by_label(&self, label: &str) -> Result<Vec<Edge>>;

    /// Get all edges between two nodes
    async fn get_edges_between(&self, from: NodeId, to: NodeId) -> Result<Vec<Edge>>;

    /// Get all edges from a node
    async fn get_edges_from(&self, from: NodeId) -> Result<Vec<Edge>>;

    /// Get all edges to a node
    async fn get_edges_to(&self, to: NodeId) -> Result<Vec<Edge>>;

    /// Get a vertex by ID
    async fn get_vertex(&self, id: &str) -> Result<Option<Node>>;

    /// Execute a Gremlin query
    async fn execute_gremlin_query(&self, query: &str, params: &[(&str, &dyn ToGValue)]) -> Result<GResultSet>;
}

/// Factory function to create a new graph instance
pub async fn new_graph(config: &crate::Config) -> Result<impl Graph> {
    neptune::NeptuneGraph::new(config).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{EntityType, Properties};
    use std::collections::HashMap;
    use chrono::Utc;
    use uuid::Uuid;

    // Helper function to create a test node
    fn create_test_node() -> Node {
        Node {
            id: NodeId(Uuid::new_v4()),
            entity_type: EntityType::Event,
            label: "test".to_string(),
            properties: Properties(HashMap::new()),
            valid_time: TemporalRange {
                start: Some(Timestamp(Utc::now())),
                end: None,
            },
            transaction_time: TemporalRange {
                start: Some(Timestamp(Utc::now())),
                end: None,
            },
        }
    }

    // Helper function to create a test edge
    fn create_test_edge() -> Edge {
        Edge {
            id: EdgeId(Uuid::new_v4()),
            source_id: NodeId(Uuid::new_v4()),
            target_id: NodeId(Uuid::new_v4()),
            label: "test".to_string(),
            properties: Properties(HashMap::new()),
            valid_time: TemporalRange {
                start: Some(Timestamp(Utc::now())),
                end: None,
            },
            transaction_time: TemporalRange {
                start: Some(Timestamp(Utc::now())),
                end: None,
            },
        }
    }
} 
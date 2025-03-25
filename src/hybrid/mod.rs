//! Hybrid Vector + Graph Architecture
//! 
//! This module implements a hybrid retrieval system that combines the temporal graph
//! with dense vector embeddings to outperform pure vector or pure graph approaches.

use std::sync::Arc;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use serde_json::Value;

use crate::{
    Config,
    error::{Error, Result},
    graph::{Graph, Node, Edge, NodeId, EdgeId, TemporalRange},
    memory::{Memory, MemoryEntry, MemoryOperations},
    temporal::TemporalIndex,
    types::{EntityType, Properties, Timestamp},
};

pub mod store;
pub mod models;
pub mod query;
pub mod fusion;

/// Re-export key types for external use
pub use store::HybridStore;
pub use models::{VectorizedNode, VectorizedEdge, EmbeddingFunction};
pub use query::{HybridQuery, QueryResult, SimilarityMetric};
pub use fusion::{FusionStrategy, WeightedFusion, RankFusion};

/// Core trait for hybrid vector+graph operations
#[async_trait]
pub trait HybridGraph: Send + Sync {
    /// Create a node with vector embedding
    async fn create_node(&self, node: VectorizedNode) -> Result<NodeId>;
    
    /// Get a node by ID, including its vector embedding
    async fn get_node(&self, id: NodeId) -> Result<VectorizedNode>;
    
    /// Update a node, including its vector embedding
    async fn update_node(&self, node: VectorizedNode) -> Result<()>;
    
    /// Create an edge with vector embedding
    async fn create_edge(&self, edge: VectorizedEdge) -> Result<EdgeId>;
    
    /// Get an edge by ID, including its vector embedding
    async fn get_edge(&self, id: EdgeId) -> Result<VectorizedEdge>;
    
    /// Update an edge, including its vector embedding
    async fn update_edge(&self, edge: VectorizedEdge) -> Result<()>;
    
    /// Find similar nodes using vector similarity
    async fn find_similar_nodes(
        &self, 
        embedding: Vec<f32>, 
        limit: usize, 
        filter: Option<Value>
    ) -> Result<Vec<VectorizedNode>>;
    
    /// Execute a hybrid query combining graph traversal and vector similarity
    async fn execute_hybrid_query(
        &self,
        query: HybridQuery,
        fusion_strategy: Option<Box<dyn FusionStrategy>>,
    ) -> Result<QueryResult>;
    
    /// Get knowledge within time range (combining temporal and vector similarity)
    async fn get_knowledge_in_time_range(
        &self,
        time_range: TemporalRange,
        query_embedding: Option<Vec<f32>>,
        limit: usize,
    ) -> Result<Vec<VectorizedNode>>;
}

/// Factory function to create a new hybrid graph instance
pub async fn new_hybrid_graph(
    config: &Config,
    graph: impl Graph + 'static,
    memory: impl Memory + 'static,
    temporal_index: impl TemporalIndex + 'static,
) -> Result<impl HybridGraph> {
    store::HybridStore::new(config, graph, memory, temporal_index).await
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Tests will be implemented once the core modules are complete
} 
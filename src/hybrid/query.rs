//! Query module for hybrid vector+graph operations
//!
//! This module provides the query capabilities for the hybrid vector+graph system,
//! enabling combined searches across both graph structure and vector similarity.

use std::collections::{HashMap, HashSet};
use serde::{Serialize, Deserialize};
use serde_json::Value;

use crate::{
    error::{Error, Result},
    graph::{NodeId, EdgeId, TemporalRange},
    hybrid::models::{VectorizedNode, VectorizedEdge},
    types::EntityType,
};

/// Supported similarity metrics for vector comparisons
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SimilarityMetric {
    /// Cosine similarity (dot product of normalized vectors)
    Cosine,
    /// Euclidean distance (L2 norm of difference)
    Euclidean,
    /// Dot product (unnormalized)
    DotProduct,
}

impl Default for SimilarityMetric {
    fn default() -> Self {
        Self::Cosine
    }
}

/// Represents a hybrid query combining graph and vector operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridQuery {
    /// Optional starting node ID for graph traversal
    pub start_node_id: Option<NodeId>,
    /// Optional query embedding for vector similarity
    pub query_embedding: Option<Vec<f32>>,
    /// Optional text to generate embedding from
    pub query_text: Option<String>,
    /// Graph traversal steps to follow
    pub traversal_steps: Vec<TraversalStep>,
    /// Filter for node types to include
    pub node_type_filter: Option<Vec<EntityType>>,
    /// Temporal range constraint
    pub temporal_range: Option<TemporalRange>,
    /// Maximum number of results to return
    pub limit: usize,
    /// Similarity metric to use
    pub similarity_metric: SimilarityMetric,
    /// Minimum similarity threshold (0.0 to 1.0)
    pub min_similarity: Option<f32>,
    /// Whether to include graph structure in results
    pub include_graph_structure: bool,
    /// Custom filter query in JSON format
    pub custom_filter: Option<Value>,
}

impl Default for HybridQuery {
    fn default() -> Self {
        Self {
            start_node_id: None,
            query_embedding: None,
            query_text: None,
            traversal_steps: Vec::new(),
            node_type_filter: None,
            temporal_range: None,
            limit: 10,
            similarity_metric: SimilarityMetric::default(),
            min_similarity: Some(0.7),
            include_graph_structure: false,
            custom_filter: None,
        }
    }
}

/// Represents a step in graph traversal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraversalStep {
    /// Edge label to follow
    pub edge_label: Option<String>,
    /// Direction of traversal
    pub direction: TraversalDirection,
    /// Maximum depth for this step
    pub max_depth: Option<usize>,
    /// Filter for node types at this step
    pub node_type_filter: Option<Vec<EntityType>>,
    /// Whether to use vector similarity for ranking at this step
    pub use_vector_similarity: bool,
    /// Properties required at this step
    pub required_properties: Option<Vec<String>>,
}

/// Direction for graph traversal
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TraversalDirection {
    /// Outgoing edges (from current node to others)
    Out,
    /// Incoming edges (from others to current node)
    In,
    /// Both directions
    Both,
}

impl Default for TraversalDirection {
    fn default() -> Self {
        Self::Out
    }
}

/// Result of a hybrid query operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// Nodes found by the query
    pub nodes: Vec<ScoredNode>,
    /// Edges connecting the result nodes
    pub edges: Vec<ScoredEdge>,
    /// Total number of results before limit was applied
    pub total_count: usize,
    /// Query execution time in milliseconds
    pub execution_time_ms: u64,
    /// Whether the query was executed successfully
    pub success: bool,
    /// Error message if the query failed
    pub error_message: Option<String>,
    /// Additional metadata about the query execution
    pub metadata: Option<Value>,
}

/// A node with a similarity score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredNode {
    /// The vectorized node
    pub node: VectorizedNode,
    /// Similarity score (0.0 to 1.0)
    pub score: f32,
    /// Path information if this came from graph traversal
    pub path: Option<TraversalPath>,
}

/// An edge with a similarity score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredEdge {
    /// The vectorized edge
    pub edge: VectorizedEdge,
    /// Similarity score (0.0 to 1.0)
    pub score: f32,
}

/// Path information for graph traversal results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraversalPath {
    /// Path of nodes from start to this node
    pub node_ids: Vec<NodeId>,
    /// Path of edges from start to this node
    pub edge_ids: Vec<EdgeId>,
    /// Depth from the start node
    pub depth: usize,
}

/// Builder for creating hybrid queries
#[derive(Default)]
pub struct HybridQueryBuilder {
    query: HybridQuery,
}

impl HybridQueryBuilder {
    /// Create a new query builder
    pub fn new() -> Self {
        Self {
            query: HybridQuery::default(),
        }
    }

    /// Set the start node for graph traversal
    pub fn start_from(mut self, node_id: NodeId) -> Self {
        self.query.start_node_id = Some(node_id);
        self
    }

    /// Set the query embedding for vector similarity
    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.query.query_embedding = Some(embedding);
        self.query.query_text = None; // Clear text if embedding is provided
        self
    }

    /// Set the query text for generating an embedding
    pub fn with_text(mut self, text: String) -> Self {
        self.query.query_text = Some(text);
        self.query.query_embedding = None; // Clear embedding if text is provided
        self
    }

    /// Add a traversal step
    pub fn add_step(mut self, step: TraversalStep) -> Self {
        self.query.traversal_steps.push(step);
        self
    }

    /// Add a simple outgoing traversal step
    pub fn follow_outgoing(mut self, edge_label: Option<String>) -> Self {
        self.query.traversal_steps.push(TraversalStep {
            edge_label,
            direction: TraversalDirection::Out,
            max_depth: None,
            node_type_filter: None,
            use_vector_similarity: true,
            required_properties: None,
        });
        self
    }

    /// Add a simple incoming traversal step
    pub fn follow_incoming(mut self, edge_label: Option<String>) -> Self {
        self.query.traversal_steps.push(TraversalStep {
            edge_label,
            direction: TraversalDirection::In,
            max_depth: None,
            node_type_filter: None,
            use_vector_similarity: true,
            required_properties: None,
        });
        self
    }

    /// Filter by node types
    pub fn filter_node_types(mut self, types: Vec<EntityType>) -> Self {
        self.query.node_type_filter = Some(types);
        self
    }

    /// Set temporal range constraint
    pub fn in_time_range(mut self, range: TemporalRange) -> Self {
        self.query.temporal_range = Some(range);
        self
    }

    /// Set result limit
    pub fn limit(mut self, limit: usize) -> Self {
        self.query.limit = limit;
        self
    }

    /// Set similarity metric
    pub fn with_similarity_metric(mut self, metric: SimilarityMetric) -> Self {
        self.query.similarity_metric = metric;
        self
    }

    /// Set minimum similarity threshold
    pub fn min_similarity(mut self, threshold: f32) -> Self {
        self.query.min_similarity = Some(threshold);
        self
    }

    /// Include graph structure in results
    pub fn include_graph_structure(mut self, include: bool) -> Self {
        self.query.include_graph_structure = include;
        self
    }

    /// Set custom filter
    pub fn with_custom_filter(mut self, filter: Value) -> Self {
        self.query.custom_filter = Some(filter);
        self
    }

    /// Build the final query
    pub fn build(self) -> HybridQuery {
        self.query
    }
}

/// Helper functions for vector similarity calculations
pub mod vector_similarity {
    /// Compute cosine similarity between two vectors
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        
        let dot_product = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>();
        let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        
        dot_product / (norm_a * norm_b)
    }
    
    /// Compute Euclidean distance between two vectors (normalized to 0-1 range)
    pub fn euclidean_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        
        let distance = a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt();
            
        // Convert distance to similarity (1.0 means identical, 0.0 means very different)
        // Using 1.0 / (1.0 + distance) to normalize to 0-1 range
        1.0 / (1.0 + distance)
    }
    
    /// Compute dot product similarity (normalized)
    pub fn dot_product_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        
        let dot_product = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>();
        
        // Normalize to 0-1 range using sigmoid-like function
        0.5 + 0.5 * (dot_product / (1.0 + dot_product.abs()))
    }
    
    /// Compute similarity based on the specified metric
    pub fn compute_similarity(a: &[f32], b: &[f32], metric: super::SimilarityMetric) -> f32 {
        match metric {
            super::SimilarityMetric::Cosine => cosine_similarity(a, b),
            super::SimilarityMetric::Euclidean => euclidean_similarity(a, b),
            super::SimilarityMetric::DotProduct => dot_product_similarity(a, b),
        }
    }
} 
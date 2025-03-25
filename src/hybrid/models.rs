//! Models for the hybrid vector+graph system
//!
//! This module defines the core data structures for entities that combine
//! graph properties with vector embeddings.

use std::sync::Arc;
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

use crate::{
    error::{Error, Result},
    graph::{Node, Edge, NodeId, EdgeId, TemporalRange},
    types::{EntityType, Properties},
};

/// Default embedding dimension used for vectors
pub const DEFAULT_EMBEDDING_DIM: usize = 768;

/// Represents a node with a vector embedding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorizedNode {
    /// The underlying graph node
    pub node: Node,
    /// Vector embedding for similarity search
    pub embedding: Option<Vec<f32>>,
    /// Additional metadata for the embedding
    pub embedding_metadata: Option<EmbeddingMetadata>,
}

/// Represents an edge with a vector embedding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorizedEdge {
    /// The underlying graph edge
    pub edge: Edge,
    /// Vector embedding for similarity search
    pub embedding: Option<Vec<f32>>,
    /// Additional metadata for the embedding
    pub embedding_metadata: Option<EmbeddingMetadata>,
}

/// Metadata for vector embeddings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingMetadata {
    /// The model used to generate the embedding
    pub model: String,
    /// The version of the model
    pub model_version: String,
    /// Timestamp when the embedding was generated
    pub generated_at: chrono::DateTime<chrono::Utc>,
    /// Dimension of the embedding
    pub dimension: usize,
    /// Additional properties of the embedding
    pub properties: Properties,
}

impl VectorizedNode {
    /// Create a new vectorized node from a regular node
    pub fn new(node: Node, embedding: Option<Vec<f32>>) -> Self {
        Self {
            node,
            embedding,
            embedding_metadata: None,
        }
    }

    /// Create a vectorized node with metadata
    pub fn with_metadata(node: Node, embedding: Vec<f32>, metadata: EmbeddingMetadata) -> Self {
        Self {
            node,
            embedding: Some(embedding),
            embedding_metadata: Some(metadata),
        }
    }

    /// Get the node ID
    pub fn id(&self) -> NodeId {
        self.node.id
    }

    /// Get the embedding, returning an error if not present
    pub fn get_embedding(&self) -> Result<&Vec<f32>> {
        self.embedding.as_ref().ok_or_else(|| Error::Generic("Node has no embedding".to_string()))
    }
}

impl VectorizedEdge {
    /// Create a new vectorized edge from a regular edge
    pub fn new(edge: Edge, embedding: Option<Vec<f32>>) -> Self {
        Self {
            edge,
            embedding,
            embedding_metadata: None,
        }
    }

    /// Create a vectorized edge with metadata
    pub fn with_metadata(edge: Edge, embedding: Vec<f32>, metadata: EmbeddingMetadata) -> Self {
        Self {
            edge,
            embedding: Some(embedding),
            embedding_metadata: Some(metadata),
        }
    }

    /// Get the edge ID
    pub fn id(&self) -> EdgeId {
        self.edge.id
    }

    /// Get the embedding, returning an error if not present
    pub fn get_embedding(&self) -> Result<&Vec<f32>> {
        self.embedding.as_ref().ok_or_else(|| Error::Generic("Edge has no embedding".to_string()))
    }
}

/// Trait for embedding generation functions
#[async_trait]
pub trait EmbeddingFunction: Send + Sync {
    /// Generate an embedding for text content
    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>>;
    
    /// Get the dimension of embeddings produced by this function
    fn embedding_dim(&self) -> usize;
    
    /// Get the name of the model used for embeddings
    fn model_name(&self) -> String;
    
    /// Get the version of the model
    fn model_version(&self) -> String;
}

/// Implementation of a simple embedding function that uses the rust-bert library
pub struct RustBertEmbeddings {
    /// The model used for embeddings
    model_name: String,
    /// The model version
    model_version: String,
    /// The dimension of the embeddings
    embedding_dim: usize,
}

impl RustBertEmbeddings {
    /// Create a new RustBertEmbeddings instance
    pub fn new(model_name: String, model_version: String, embedding_dim: usize) -> Self {
        Self {
            model_name,
            model_version,
            embedding_dim,
        }
    }
}

#[async_trait]
impl EmbeddingFunction for RustBertEmbeddings {
    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        // This is a placeholder implementation
        // In a real implementation, we would use rust-bert to generate embeddings
        // Since this requires loading large ML models, we'll implement this separately
        
        // For now, return a random embedding of the correct dimension
        let mut rng = rand::thread_rng();
        let embedding: Vec<f32> = (0..self.embedding_dim)
            .map(|_| rand::Rng::gen_range(&mut rng, -1.0..1.0))
            .collect();
        
        Ok(embedding)
    }
    
    fn embedding_dim(&self) -> usize {
        self.embedding_dim
    }
    
    fn model_name(&self) -> String {
        self.model_name.clone()
    }
    
    fn model_version(&self) -> String {
        self.model_version.clone()
    }
}

/// Factory function to create a new embedding function
pub fn create_embedding_function(
    model_name: Option<String>,
    embedding_dim: Option<usize>,
) -> Box<dyn EmbeddingFunction> {
    let model_name = model_name.unwrap_or_else(|| "all-MiniLM-L6-v2".to_string());
    let embedding_dim = embedding_dim.unwrap_or(DEFAULT_EMBEDDING_DIM);
    
    Box::new(RustBertEmbeddings::new(
        model_name,
        "1.0".to_string(),
        embedding_dim,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    
    #[test]
    fn test_vectorized_node() {
        let node = Node {
            id: NodeId(Uuid::new_v4()),
            entity_type: EntityType::Person,
            label: "test".to_string(),
            properties: Properties::new(),
            valid_time: TemporalRange {
                start: Some(crate::types::Timestamp(Utc::now())),
                end: None,
            },
            transaction_time: TemporalRange {
                start: Some(crate::types::Timestamp(Utc::now())),
                end: None,
            },
        };
        
        let embedding = vec![0.1, 0.2, 0.3];
        let vectorized_node = VectorizedNode::new(node.clone(), Some(embedding.clone()));
        
        assert_eq!(vectorized_node.id(), node.id);
        assert_eq!(vectorized_node.get_embedding().unwrap(), &embedding);
    }
} 
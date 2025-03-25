//! Store implementation for hybrid vector+graph system
//!
//! This module provides the main implementation of the hybrid graph store
//! that combines graph and vector operations.

use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Instant, Duration};

use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::RwLock;
use serde_json::{json, Value};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::{
    Config,
    error::{Error, Result},
    graph::{Graph, Node, Edge, NodeId, EdgeId, TemporalRange},
    memory::{Memory, MemoryEntry, MemoryOperations},
    temporal::TemporalIndex,
    types::{EntityType, Properties, Timestamp},
    hybrid::{
        models::{VectorizedNode, VectorizedEdge, EmbeddingFunction, EmbeddingMetadata, create_embedding_function},
        query::{HybridQuery, QueryResult, ScoredNode, ScoredEdge, TraversalPath, SimilarityMetric},
        fusion::{FusionStrategy, default_fusion_strategy},
    },
};

/// Main implementation of the hybrid graph store
pub struct HybridStore {
    /// Reference to the underlying graph store
    graph: Arc<dyn Graph>,
    /// Reference to the vector memory store
    memory: Arc<dyn Memory>,
    /// Reference to the temporal index
    temporal_index: Arc<dyn TemporalIndex>,
    /// Embedding function for generating vector embeddings
    embedding_function: Arc<dyn EmbeddingFunction>,
    /// Configuration
    config: Config,
}

impl HybridStore {
    /// Create a new hybrid store
    pub async fn new(
        config: &Config,
        graph: impl Graph + 'static,
        memory: impl Memory + 'static,
        temporal_index: impl TemporalIndex + 'static,
    ) -> Result<Self> {
        let embedding_function = create_embedding_function(
            None, // Use default model
            None, // Use default dimension
        );
        
        Ok(Self {
            graph: Arc::new(graph),
            memory: Arc::new(memory),
            temporal_index: Arc::new(temporal_index),
            embedding_function: Arc::new(embedding_function),
            config: config.clone(),
        })
    }
    
    /// Create a new hybrid store with a specific embedding function
    pub async fn with_embedding_function(
        config: &Config,
        graph: impl Graph + 'static,
        memory: impl Memory + 'static,
        temporal_index: impl TemporalIndex + 'static,
        embedding_function: impl EmbeddingFunction + 'static,
    ) -> Result<Self> {
        Ok(Self {
            graph: Arc::new(graph),
            memory: Arc::new(memory),
            temporal_index: Arc::new(temporal_index),
            embedding_function: Arc::new(embedding_function),
            config: config.clone(),
        })
    }
    
    /// Generate a vector embedding for a node
    async fn generate_node_embedding(&self, node: &Node) -> Result<Vec<f32>> {
        // Extract text representation of the node for embedding
        let text = format!(
            "{} {}\n{:?}",
            node.label,
            node.entity_type.to_string(),
            node.properties,
        );
        
        self.embedding_function.generate_embedding(&text).await
    }
    
    /// Generate a vector embedding for an edge
    async fn generate_edge_embedding(&self, edge: &Edge) -> Result<Vec<f32>> {
        // Extract text representation of the edge for embedding
        let text = format!(
            "{}\n{:?}",
            edge.label,
            edge.properties,
        );
        
        self.embedding_function.generate_embedding(&text).await
    }
    
    /// Create embedding metadata
    fn create_embedding_metadata(&self) -> EmbeddingMetadata {
        EmbeddingMetadata {
            model: self.embedding_function.model_name(),
            model_version: self.embedding_function.model_version(),
            generated_at: Utc::now(),
            dimension: self.embedding_function.embedding_dim(),
            properties: Properties::new(),
        }
    }
    
    /// Store node embedding in vector store
    async fn store_node_embedding(&self, node: &Node, embedding: &[f32]) -> Result<()> {
        let metadata = json!({
            "id": node.id.0.to_string(),
            "type": "node",
            "entity_type": node.entity_type.to_string(),
            "label": node.label,
            "valid_time_start": node.valid_time.start.map(|ts| ts.0.to_rfc3339()),
            "valid_time_end": node.valid_time.end.map(|ts| ts.0.to_rfc3339()),
            "transaction_time_start": node.transaction_time.start.map(|ts| ts.0.to_rfc3339()),
            "transaction_time_end": node.transaction_time.end.map(|ts| ts.0.to_rfc3339()),
        });
        
        let mut meta_map = HashMap::new();
        meta_map.insert("metadata".to_string(), metadata);
        
        let memory_entry = MemoryEntry {
            id: node.id.0.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            content: node.label.clone(),
            metadata: meta_map,
            embedding: Some(embedding.to_vec()),
            node_type: Some(node.entity_type),
        };
        
        self.memory.store(memory_entry).await
    }
    
    /// Store edge embedding in vector store
    async fn store_edge_embedding(&self, edge: &Edge, embedding: &[f32]) -> Result<()> {
        let metadata = json!({
            "id": edge.id.0.to_string(),
            "type": "edge",
            "source_id": edge.source_id.0.to_string(),
            "target_id": edge.target_id.0.to_string(),
            "label": edge.label,
            "valid_time_start": edge.valid_time.start.map(|ts| ts.0.to_rfc3339()),
            "valid_time_end": edge.valid_time.end.map(|ts| ts.0.to_rfc3339()),
            "transaction_time_start": edge.transaction_time.start.map(|ts| ts.0.to_rfc3339()),
            "transaction_time_end": edge.transaction_time.end.map(|ts| ts.0.to_rfc3339()),
        });
        
        let mut meta_map = HashMap::new();
        meta_map.insert("metadata".to_string(), metadata);
        
        let memory_entry = MemoryEntry {
            id: edge.id.0.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            content: edge.label.clone(),
            metadata: meta_map,
            embedding: Some(embedding.to_vec()),
            node_type: None, // Edges don't have a node_type
        };
        
        self.memory.store(memory_entry).await
    }
    
    /// Find node embedding in vector store
    async fn find_node_embedding(&self, node_id: &NodeId) -> Result<Option<Vec<f32>>> {
        let result = self.memory.get(node_id.0.to_string().as_str()).await?;
        
        match result {
            Some(entry) => Ok(entry.embedding),
            None => Ok(None),
        }
    }
    
    /// Find edge embedding in vector store
    async fn find_edge_embedding(&self, edge_id: &EdgeId) -> Result<Option<Vec<f32>>> {
        let result = self.memory.get(edge_id.0.to_string().as_str()).await?;
        
        match result {
            Some(entry) => Ok(entry.embedding),
            None => Ok(None),
        }
    }
    
    /// Convert a MemoryEntry to a VectorizedNode
    async fn memory_entry_to_vectorized_node(&self, entry: MemoryEntry) -> Result<VectorizedNode> {
        let node_id = entry.id.parse::<Uuid>()
            .map_err(|_| Error::Generic(format!("Invalid node ID: {}", entry.id)))?;
            
        let node = self.graph.get_node(NodeId(node_id)).await?;
        
        let metadata = if let Some(ref embedding) = entry.embedding {
            Some(EmbeddingMetadata {
                model: self.embedding_function.model_name(),
                model_version: self.embedding_function.model_version(),
                generated_at: entry.updated_at,
                dimension: embedding.len(),
                properties: Properties::new(),
            })
        } else {
            None
        };
        
        Ok(VectorizedNode {
            node,
            embedding: entry.embedding,
            embedding_metadata: metadata,
        })
    }
    
    /// Execute vector similarity search
    async fn execute_vector_search(
        &self,
        embedding: &[f32],
        limit: usize,
        filter: Option<Value>,
    ) -> Result<Vec<MemoryEntry>> {
        self.memory.search_similar(embedding.to_vec(), limit, filter).await
    }
    
    /// Convert vector search results to scored nodes
    async fn vector_results_to_scored_nodes(
        &self,
        entries: Vec<MemoryEntry>,
        query_embedding: &[f32],
        similarity_metric: SimilarityMetric,
    ) -> Result<Vec<ScoredNode>> {
        use crate::hybrid::query::vector_similarity::compute_similarity;
        
        let mut results = Vec::with_capacity(entries.len());
        
        for entry in entries {
            if let Some(ref entry_embedding) = entry.embedding {
                let node = self.memory_entry_to_vectorized_node(entry).await?;
                
                let score = compute_similarity(query_embedding, entry_embedding, similarity_metric);
                
                results.push(ScoredNode {
                    node,
                    score,
                    path: None, // No path for vector results
                });
            }
        }
        
        Ok(results)
    }
}

#[async_trait]
impl super::HybridGraph for HybridStore {
    async fn create_node(&self, mut node: VectorizedNode) -> Result<NodeId> {
        // Create the node in the graph
        let node_id = self.graph.create_node(node.node.clone()).await?;
        
        // Generate embedding if not provided
        if node.embedding.is_none() {
            let embedding = self.generate_node_embedding(&node.node).await?;
            node.embedding = Some(embedding);
            node.embedding_metadata = Some(self.create_embedding_metadata());
        }
        
        // Store the embedding
        if let Some(ref embedding) = node.embedding {
            self.store_node_embedding(&node.node, embedding).await?;
        }
        
        Ok(node_id)
    }
    
    async fn get_node(&self, id: NodeId) -> Result<VectorizedNode> {
        // Get the node from the graph
        let node = self.graph.get_node(id).await?;
        
        // Find embedding in vector store
        let embedding = self.find_node_embedding(&id).await?;
        
        // Create metadata if embedding exists
        let embedding_metadata = if let Some(ref emb) = embedding {
            Some(EmbeddingMetadata {
                model: self.embedding_function.model_name(),
                model_version: self.embedding_function.model_version(),
                generated_at: Utc::now(), // We don't know when it was generated
                dimension: emb.len(),
                properties: Properties::new(),
            })
        } else {
            None
        };
        
        Ok(VectorizedNode {
            node,
            embedding,
            embedding_metadata,
        })
    }
    
    async fn update_node(&self, mut node: VectorizedNode) -> Result<()> {
        // Update the node in the graph
        self.graph.update_node(node.node.clone()).await?;
        
        // Generate embedding if not provided
        if node.embedding.is_none() {
            let embedding = self.generate_node_embedding(&node.node).await?;
            node.embedding = Some(embedding);
            node.embedding_metadata = Some(self.create_embedding_metadata());
        }
        
        // Store the embedding
        if let Some(ref embedding) = node.embedding {
            self.store_node_embedding(&node.node, embedding).await?;
        }
        
        Ok(())
    }
    
    async fn create_edge(&self, mut edge: VectorizedEdge) -> Result<EdgeId> {
        // Create the edge in the graph
        let edge_id = self.graph.create_edge(edge.edge.clone()).await?;
        
        // Generate embedding if not provided
        if edge.embedding.is_none() {
            let embedding = self.generate_edge_embedding(&edge.edge).await?;
            edge.embedding = Some(embedding);
            edge.embedding_metadata = Some(self.create_embedding_metadata());
        }
        
        // Store the embedding
        if let Some(ref embedding) = edge.embedding {
            self.store_edge_embedding(&edge.edge, embedding).await?;
        }
        
        Ok(edge_id)
    }
    
    async fn get_edge(&self, id: EdgeId) -> Result<VectorizedEdge> {
        // Get the edge from the graph
        let edge = self.graph.get_edge(id).await?;
        
        // Find embedding in vector store
        let embedding = self.find_edge_embedding(&id).await?;
        
        // Create metadata if embedding exists
        let embedding_metadata = if let Some(ref emb) = embedding {
            Some(EmbeddingMetadata {
                model: self.embedding_function.model_name(),
                model_version: self.embedding_function.model_version(),
                generated_at: Utc::now(), // We don't know when it was generated
                dimension: emb.len(),
                properties: Properties::new(),
            })
        } else {
            None
        };
        
        Ok(VectorizedEdge {
            edge,
            embedding,
            embedding_metadata,
        })
    }
    
    async fn update_edge(&self, mut edge: VectorizedEdge) -> Result<()> {
        // Update the edge in the graph
        self.graph.update_edge(edge.edge.clone()).await?;
        
        // Generate embedding if not provided
        if edge.embedding.is_none() {
            let embedding = self.generate_edge_embedding(&edge.edge).await?;
            edge.embedding = Some(embedding);
            edge.embedding_metadata = Some(self.create_embedding_metadata());
        }
        
        // Store the embedding
        if let Some(ref embedding) = edge.embedding {
            self.store_edge_embedding(&edge.edge, embedding).await?;
        }
        
        Ok(())
    }
    
    async fn find_similar_nodes(
        &self, 
        embedding: Vec<f32>, 
        limit: usize, 
        filter: Option<Value>
    ) -> Result<Vec<VectorizedNode>> {
        let entries = self.execute_vector_search(&embedding, limit, filter).await?;
        
        let mut nodes = Vec::with_capacity(entries.len());
        
        for entry in entries {
            let node = self.memory_entry_to_vectorized_node(entry).await?;
            nodes.push(node);
        }
        
        Ok(nodes)
    }
    
    async fn execute_hybrid_query(
        &self,
        query: HybridQuery,
        fusion_strategy: Option<Box<dyn FusionStrategy>>,
    ) -> Result<QueryResult> {
        let start_time = Instant::now();
        
        // Validate the query requirements
        if query.query_embedding.is_none() && query.query_text.is_none() && query.start_node_id.is_none() {
            return Err(Error::Generic(
                "Hybrid query must have either a query embedding, query text, or a start node ID".to_string()
            ));
        }
        
        // Generate embedding from text if provided
        let query_embedding = if let Some(text) = &query.query_text {
            Some(self.embedding_function.generate_embedding(text).await?)
        } else {
            query.query_embedding
        };
        
        let mut vector_results = Vec::new();
        let mut graph_results = Vec::new();
        
        // Execute vector similarity search if we have an embedding
        if let Some(embedding) = &query_embedding {
            let filter = if let Some(node_types) = &query.node_type_filter {
                let types: Vec<String> = node_types.iter()
                    .map(|t| t.to_string())
                    .collect();
                    
                Some(json!({
                    "filter": {
                        "terms": {
                            "node_type": types
                        }
                    }
                }))
            } else {
                query.custom_filter.clone()
            };
            
            let entries = self.execute_vector_search(embedding, query.limit * 2, filter).await?;
            
            vector_results = self.vector_results_to_scored_nodes(
                entries,
                embedding,
                query.similarity_metric,
            ).await?;
        }
        
        // Execute graph traversal if we have a start node
        if let Some(start_node_id) = query.start_node_id {
            // This would execute graph traversal and collect results
            // For now, we'll just fetch connected nodes as a simple example
            
            let connected_nodes = if let Some(range) = query.temporal_range {
                self.graph.get_connected_nodes(start_node_id, Some(range)).await?
            } else {
                self.graph.get_connected_nodes(start_node_id, None).await?
            };
            
            // Convert to vectorized nodes and assign scores
            for node in connected_nodes {
                let embedding = self.find_node_embedding(&node.id).await?;
                
                let score = if let (Some(node_emb), Some(query_emb)) = (&embedding, &query_embedding) {
                    // Calculate similarity if we have both embeddings
                    use crate::hybrid::query::vector_similarity::compute_similarity;
                    compute_similarity(node_emb, query_emb, query.similarity_metric)
                } else {
                    // Assign default score if we don't have embeddings
                    0.5
                };
                
                // Create path information
                let path = Some(TraversalPath {
                    node_ids: vec![start_node_id, node.id],
                    edge_ids: vec![], // We don't have edge IDs in this simplified example
                    depth: 1,
                });
                
                let metadata = if let Some(ref emb) = embedding {
                    Some(EmbeddingMetadata {
                        model: self.embedding_function.model_name(),
                        model_version: self.embedding_function.model_version(),
                        generated_at: Utc::now(),
                        dimension: emb.len(),
                        properties: Properties::new(),
                    })
                } else {
                    None
                };
                
                graph_results.push(ScoredNode {
                    node: VectorizedNode {
                        node,
                        embedding,
                        embedding_metadata: metadata,
                    },
                    score,
                    path,
                });
            }
        }
        
        // Apply fusion strategy
        let fusion = fusion_strategy.unwrap_or_else(default_fusion_strategy);
        
        let result = fusion.fuse_with_context(
            vector_results,
            graph_results,
            query.limit,
        ).await?;
        
        Ok(result)
    }
    
    async fn get_knowledge_in_time_range(
        &self,
        time_range: TemporalRange,
        query_embedding: Option<Vec<f32>>,
        limit: usize,
    ) -> Result<Vec<VectorizedNode>> {
        // Get entity IDs in time range from temporal index
        let entries = self.temporal_index.get_in_time_range(time_range, limit * 2).await?;
        
        let mut nodes = Vec::with_capacity(entries.len());
        
        // Fetch nodes from graph store
        for entry in entries {
            if let Ok(uuid) = Uuid::parse_str(&entry.entity_id) {
                if let Ok(node) = self.graph.get_node(NodeId(uuid)).await {
                    // Find embedding for this node
                    let embedding = self.find_node_embedding(&node.id).await?;
                    
                    // Create metadata if embedding exists
                    let embedding_metadata = if let Some(ref emb) = embedding {
                        Some(EmbeddingMetadata {
                            model: self.embedding_function.model_name(),
                            model_version: self.embedding_function.model_version(),
                            generated_at: Utc::now(),
                            dimension: emb.len(),
                            properties: Properties::new(),
                        })
                    } else {
                        None
                    };
                    
                    nodes.push(VectorizedNode {
                        node,
                        embedding,
                        embedding_metadata,
                    });
                }
            }
        }
        
        // Sort by vector similarity if query embedding provided
        if let Some(query_emb) = query_embedding {
            use crate::hybrid::query::vector_similarity::{compute_similarity, SimilarityMetric};
            
            // Filter to nodes with embeddings
            let mut nodes_with_scores: Vec<_> = nodes.into_iter()
                .filter_map(|node| {
                    node.embedding.as_ref().map(|emb| {
                        let score = compute_similarity(emb, &query_emb, SimilarityMetric::Cosine);
                        (node, score)
                    })
                })
                .collect();
                
            // Sort by score
            nodes_with_scores.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
            
            // Take top results
            nodes = nodes_with_scores.into_iter()
                .take(limit)
                .map(|(node, _)| node)
                .collect();
        } else {
            // Limit results if no embedding to sort by
            nodes.truncate(limit);
        }
        
        Ok(nodes)
    }
} 
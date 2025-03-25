//! Fusion strategies for combining graph and vector results
//!
//! This module provides different algorithms for fusing results from graph traversal
//! and vector similarity searches into a coherent ranked list.

use std::collections::{HashMap, HashSet};
use async_trait::async_trait;

use crate::{
    error::Result,
    hybrid::query::{ScoredNode, ScoredEdge, QueryResult},
};

/// Trait defining fusion strategy for combining graph and vector results
#[async_trait]
pub trait FusionStrategy: Send + Sync {
    /// Fuse vector and graph results into a unified ranked list
    async fn fuse(
        &self,
        vector_nodes: Vec<ScoredNode>,
        graph_nodes: Vec<ScoredNode>,
        limit: usize,
    ) -> Result<Vec<ScoredNode>>;
    
    /// Fuse results and provide additional context information
    async fn fuse_with_context(
        &self,
        vector_nodes: Vec<ScoredNode>,
        graph_nodes: Vec<ScoredNode>,
        limit: usize,
    ) -> Result<QueryResult>;
    
    /// Name of the fusion strategy
    fn name(&self) -> &'static str;
    
    /// Description of the fusion strategy
    fn description(&self) -> &'static str;
}

/// Weighted fusion strategy that combines scores with configurable weights
pub struct WeightedFusion {
    /// Weight for vector similarity scores (0.0 to 1.0)
    vector_weight: f32,
    /// Weight for graph relevance scores (0.0 to 1.0)
    graph_weight: f32,
    /// Whether to favor nodes that appear in both result sets
    boost_common: bool,
    /// Amount to boost common nodes by
    common_boost_factor: f32,
}

impl WeightedFusion {
    /// Create a new weighted fusion strategy
    pub fn new(vector_weight: f32, graph_weight: f32) -> Self {
        Self {
            vector_weight,
            graph_weight,
            boost_common: true,
            common_boost_factor: 1.5,
        }
    }
    
    /// Create a balanced fusion strategy (equal weights)
    pub fn balanced() -> Self {
        Self::new(0.5, 0.5)
    }
    
    /// Create a vector-focused fusion strategy
    pub fn vector_focused() -> Self {
        Self::new(0.8, 0.2)
    }
    
    /// Create a graph-focused fusion strategy
    pub fn graph_focused() -> Self {
        Self::new(0.2, 0.8)
    }
    
    /// Set whether to boost nodes that appear in both result sets
    pub fn with_common_boost(mut self, boost: bool, factor: Option<f32>) -> Self {
        self.boost_common = boost;
        if let Some(factor) = factor {
            self.common_boost_factor = factor;
        }
        self
    }
}

#[async_trait]
impl FusionStrategy for WeightedFusion {
    async fn fuse(
        &self,
        vector_nodes: Vec<ScoredNode>,
        graph_nodes: Vec<ScoredNode>,
        limit: usize,
    ) -> Result<Vec<ScoredNode>> {
        // Create a map for vector scores
        let mut vector_scores = HashMap::new();
        for node in &vector_nodes {
            vector_scores.insert(node.node.id(), node.score);
        }
        
        // Create a map for graph scores
        let mut graph_scores = HashMap::new();
        for node in &graph_nodes {
            graph_scores.insert(node.node.id(), node.score);
        }
        
        // Merge results and compute combined scores
        let mut all_nodes = HashMap::new();
        
        // Add vector nodes
        for node in vector_nodes {
            let id = node.node.id();
            let graph_score = graph_scores.get(&id).copied().unwrap_or(0.0);
            let vector_score = node.score;
            
            // Compute combined score
            let combined_score = (vector_score * self.vector_weight) + (graph_score * self.graph_weight);
            
            // Apply boost if node is in both result sets
            let final_score = if self.boost_common && graph_scores.contains_key(&id) {
                combined_score * self.common_boost_factor
            } else {
                combined_score
            };
            
            all_nodes.insert(id, (node, final_score));
        }
        
        // Add graph nodes that weren't in vector results
        for node in graph_nodes {
            let id = node.node.id();
            if !all_nodes.contains_key(&id) {
                let graph_score = node.score;
                let vector_score = vector_scores.get(&id).copied().unwrap_or(0.0);
                
                // Compute combined score
                let combined_score = (vector_score * self.vector_weight) + (graph_score * self.graph_weight);
                
                all_nodes.insert(id, (node, combined_score));
            }
        }
        
        // Convert to a vector and sort by score
        let mut results: Vec<_> = all_nodes.into_values().collect();
        results.sort_by(|(_, a_score), (_, b_score)| b_score.partial_cmp(a_score).unwrap_or(std::cmp::Ordering::Equal));
        
        // Create result nodes with updated scores
        let result_nodes = results.into_iter()
            .take(limit)
            .map(|(mut node, score)| {
                node.score = score;
                node
            })
            .collect();
            
        Ok(result_nodes)
    }
    
    async fn fuse_with_context(
        &self,
        vector_nodes: Vec<ScoredNode>,
        graph_nodes: Vec<ScoredNode>,
        limit: usize,
    ) -> Result<QueryResult> {
        let start_time = std::time::Instant::now();
        
        let total_count = vector_nodes.len() + graph_nodes.len();
        
        let nodes = self.fuse(vector_nodes, graph_nodes, limit).await?;
        
        // Collect all unique edges from nodes with paths
        let edges = Vec::new(); // In a real implementation, we would collect edges here
        
        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        
        Ok(QueryResult {
            nodes,
            edges,
            total_count,
            execution_time_ms,
            success: true,
            error_message: None,
            metadata: None,
        })
    }
    
    fn name(&self) -> &'static str {
        "weighted_fusion"
    }
    
    fn description(&self) -> &'static str {
        "Combines vector and graph scores using configurable weights"
    }
}

/// Rank-based fusion strategy that combines results based on their rank in each list
pub struct RankFusion {
    /// Constant k for CombSUM fusion algorithm
    k: f32,
}

impl RankFusion {
    /// Create a new rank fusion strategy
    pub fn new(k: f32) -> Self {
        Self { k }
    }
    
    /// Create a default rank fusion strategy
    pub fn default_params() -> Self {
        Self { k: 60.0 }
    }
}

#[async_trait]
impl FusionStrategy for RankFusion {
    async fn fuse(
        &self,
        vector_nodes: Vec<ScoredNode>,
        graph_nodes: Vec<ScoredNode>,
        limit: usize,
    ) -> Result<Vec<ScoredNode>> {
        // Create a map for all nodes
        let mut all_nodes = HashMap::new();
        
        // Assign ranks to vector nodes (1-based)
        for (rank, node) in vector_nodes.iter().enumerate() {
            let rank_score = 1.0 / (rank as f32 + self.k);
            all_nodes
                .entry(node.node.id())
                .or_insert_with(|| (node.clone(), 0.0))
                .1 += rank_score;
        }
        
        // Assign ranks to graph nodes (1-based)
        for (rank, node) in graph_nodes.iter().enumerate() {
            let rank_score = 1.0 / (rank as f32 + self.k);
            all_nodes
                .entry(node.node.id())
                .or_insert_with(|| (node.clone(), 0.0))
                .1 += rank_score;
        }
        
        // Convert to a vector and sort by fused rank score
        let mut results: Vec<_> = all_nodes.into_values().collect();
        results.sort_by(|(_, a_score), (_, b_score)| b_score.partial_cmp(a_score).unwrap_or(std::cmp::Ordering::Equal));
        
        // Take top N results and create scored nodes
        let result_nodes = results.into_iter()
            .take(limit)
            .map(|(mut node, score)| {
                // Normalize score to 0-1 range (assuming maximum possible score is 2.0)
                node.score = score / 2.0;
                node
            })
            .collect();
            
        Ok(result_nodes)
    }
    
    async fn fuse_with_context(
        &self,
        vector_nodes: Vec<ScoredNode>,
        graph_nodes: Vec<ScoredNode>,
        limit: usize,
    ) -> Result<QueryResult> {
        let start_time = std::time::Instant::now();
        
        let total_count = vector_nodes.len() + graph_nodes.len();
        
        let nodes = self.fuse(vector_nodes, graph_nodes, limit).await?;
        
        // Collect all unique edges from nodes with paths
        let edges = Vec::new(); // In a real implementation, we would collect edges here
        
        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        
        Ok(QueryResult {
            nodes,
            edges,
            total_count,
            execution_time_ms,
            success: true,
            error_message: None,
            metadata: None,
        })
    }
    
    fn name(&self) -> &'static str {
        "rank_fusion"
    }
    
    fn description(&self) -> &'static str {
        "Combines results based on their rank position using reciprocal rank fusion"
    }
}

/// Create a default fusion strategy
pub fn default_fusion_strategy() -> Box<dyn FusionStrategy> {
    Box::new(WeightedFusion::balanced())
} 
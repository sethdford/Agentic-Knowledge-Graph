use std::sync::Arc;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use metrics::{counter, gauge, histogram};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;
use std::collections::HashMap;
use serde_json::json;

use crate::{
    error::{Error, Result},
    graph::{Node, Edge, EntityType},
    temporal::{TemporalGraphStore, TemporalRange},
    types::{Node, Edge, EntityType, Properties, NodeId, EdgeId, EntityId, Timestamp},
};

/// Represents a node's neighborhood
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Neighborhood {
    /// Center node ID
    pub center_id: Uuid,
    /// Incoming edges and nodes
    pub incoming: Vec<(Edge, Node)>,
    /// Outgoing edges and nodes
    pub outgoing: Vec<(Edge, Node)>,
    /// Timestamp when this neighborhood was computed
    pub timestamp: DateTime<Utc>,
    /// Features computed for this neighborhood
    pub features: Vec<f32>,
}

/// Cache entry for neighborhood data
#[derive(Debug)]
struct CacheEntry {
    /// The neighborhood data
    neighborhood: Neighborhood,
    /// Number of times this entry was accessed
    access_count: u64,
    /// Last access timestamp
    last_access: DateTime<Utc>,
}

/// Configuration for neighborhood operations
#[derive(Debug, Clone)]
pub struct NeighborhoodConfig {
    /// Maximum cache size
    pub max_cache_size: usize,
    /// Cache eviction threshold (access count)
    pub cache_eviction_threshold: u64,
    /// Cache entry time-to-live
    pub cache_ttl_seconds: i64,
    /// Feature vector dimension
    pub feature_dim: usize,
}

impl Default for NeighborhoodConfig {
    fn default() -> Self {
        Self {
            max_cache_size: 10000,
            cache_eviction_threshold: 5,
            cache_ttl_seconds: 3600,
            feature_dim: 128,
        }
    }
}

/// Manager for neighborhood operations
#[derive(Clone)]
pub struct NeighborhoodManager {
    /// Graph store
    graph: Arc<TemporalGraphStore>,
    /// Configuration
    config: NeighborhoodConfig,
    /// Cache for neighborhood data
    cache: Arc<DashMap<String, CacheEntry>>,
    /// Cache for feature vectors
    feature_cache: Arc<DashMap<String, Vec<f32>>>,
    /// Statistics
    stats: Arc<RwLock<NeighborhoodStats>>,
}

/// Statistics for neighborhood operations
#[derive(Debug, Default)]
struct NeighborhoodStats {
    /// Number of cache hits
    cache_hits: u64,
    /// Number of cache misses
    cache_misses: u64,
    /// Number of cache evictions
    cache_evictions: u64,
    /// Average neighborhood size
    avg_neighborhood_size: f64,
    /// Total neighborhoods computed
    total_neighborhoods: u64,
}

#[async_trait]
pub trait NeighborhoodOps {
    /// Get neighborhood for a node at a specific time
    async fn get_neighborhood(
        &self,
        node_id: Uuid,
        timestamp: DateTime<Utc>,
    ) -> Result<Neighborhood>;
    
    /// Get neighborhoods for multiple nodes
    async fn get_neighborhoods(
        &self,
        node_ids: &[Uuid],
        timestamp: DateTime<Utc>,
    ) -> Result<Vec<Neighborhood>>;
    
    /// Compute features for a neighborhood
    async fn compute_features(
        &self,
        neighborhood: &Neighborhood,
    ) -> Result<Vec<f32>>;
    
    /// Update cache statistics
    fn update_stats(&self);
}

impl NeighborhoodManager {
    /// Create a new neighborhood manager
    pub fn new(graph: Arc<TemporalGraphStore>, config: NeighborhoodConfig) -> Self {
        // Register metrics
        counter!("neighborhood_cache_hits_total");
        counter!("neighborhood_cache_misses_total");
        counter!("neighborhood_cache_evictions_total");
        gauge!("neighborhood_cache_size", 0.0);
        histogram!("neighborhood_computation_duration_seconds");
        
        Self {
            graph,
            config,
            cache: Arc::new(DashMap::new()),
            feature_cache: Arc::new(DashMap::new()),
            stats: Arc::new(RwLock::new(NeighborhoodStats::default())),
        }
    }
    
    /// Check if a cache entry is valid
    fn is_cache_valid(&self, entry: &CacheEntry, current_time: DateTime<Utc>) -> bool {
        let age = current_time
            .signed_duration_since(entry.last_access)
            .num_seconds();
            
        age <= self.config.cache_ttl_seconds
    }
    
    /// Evict old entries from cache
    async fn evict_old_entries(&self) {
        let current_time = Utc::now();
        let mut evicted = 0;
        
        self.cache.retain(|_, entry| {
            let valid = self.is_cache_valid(entry, current_time);
            if !valid {
                evicted += 1;
            }
            valid
        });
        
        if evicted > 0 {
            let mut stats = self.stats.write().await;
            stats.cache_evictions += evicted as u64;
            counter!("neighborhood_cache_evictions_total").increment(evicted as u64);
        }
        
        gauge!("neighborhood_cache_size", self.cache.len() as f64);
    }
    
    /// Update cache entry
    async fn update_cache(
        &self,
        node_id: Uuid,
        neighborhood: Neighborhood,
    ) {
        let current_time = Utc::now();
        
        // Evict old entries if cache is full
        if self.cache.len() >= self.config.max_cache_size {
            self.evict_old_entries().await;
        }
        
        self.cache.insert(format!("{}_{}", node_id, neighborhood.timestamp.timestamp()), CacheEntry {
            neighborhood,
            access_count: 1,
            last_access: current_time,
        });
        
        gauge!("neighborhood_cache_size", self.cache.len() as f64);
    }
    
    /// Compute neighborhood features
    async fn compute_neighborhood_features(
        &self,
        neighborhood: &Neighborhood,
    ) -> Result<Vec<f32>> {
        // Check feature cache first
        if let Some(features) = self.feature_cache.get(&format!("{}_{}", neighborhood.center_id, neighborhood.timestamp.timestamp())) {
            return Ok(features.clone());
        }
        
        // Initialize feature vector
        let mut features = vec![0.0; self.config.feature_dim];
        
        // Compute features based on neighborhood structure
        // This is a placeholder implementation - replace with actual feature computation
        features[0] = neighborhood.incoming.len() as f32;
        features[1] = neighborhood.outgoing.len() as f32;
        
        // Add node type distributions
        let mut type_counts = std::collections::HashMap::new();
        for (_, node) in &neighborhood.incoming {
            *type_counts.entry(&node.node_type).or_insert(0) += 1;
        }
        for (_, node) in &neighborhood.outgoing {
            *type_counts.entry(&node.node_type).or_insert(0) += 1;
        }
        
        // Store in cache
        self.feature_cache.insert(format!("{}_{}", neighborhood.center_id, neighborhood.timestamp.timestamp()), features.clone());
        
        Ok(features)
    }
}

#[async_trait]
impl NeighborhoodOps for NeighborhoodManager {
    async fn get_neighborhood(
        &self,
        node_id: Uuid,
        timestamp: DateTime<Utc>,
    ) -> Result<Neighborhood> {
        let cache_key = format!("{}_{}", node_id, timestamp.timestamp());
        
        // Check cache first
        if let Some(entry) = self.cache.get_mut(&cache_key) {
            if self.is_cache_valid(entry, timestamp) {
                // Update access statistics
                entry.access_count += 1;
                entry.last_access = timestamp;
                
                let mut stats = self.stats.write().await;
                stats.cache_hits += 1;
                counter!("neighborhood_cache_hits_total").increment(1);
                
                return Ok(entry.neighborhood.clone());
            }
        }
        
        // Cache miss - compute neighborhood
        let timer = metrics::histogram!("neighborhood_computation_duration_seconds").start_timer();
        
        // Get incoming edges and nodes
        let incoming_edges = self.graph
            .get_edges_at(timestamp, None, Some(node_id))
            .await?;
            
        let mut incoming = Vec::new();
        for edge in incoming_edges {
            if let Some(node) = self.graph
                .get_latest_node(edge.source_id)
                .await?
            {
                incoming.push((edge, node));
            }
        }
        
        // Get outgoing edges and nodes
        let outgoing_edges = self.graph
            .get_edges_at(timestamp, Some(node_id), None)
            .await?;
            
        let mut outgoing = Vec::new();
        for edge in outgoing_edges {
            if let Some(node) = self.graph
                .get_latest_node(edge.target_id)
                .await?
            {
                outgoing.push((edge, node));
            }
        }
        
        // Create neighborhood
        let neighborhood = Neighborhood {
            center_id: node_id,
            incoming,
            outgoing,
            timestamp,
            features: Vec::new(),
        };
        
        // Compute features
        let features = self.compute_neighborhood_features(&neighborhood).await?;
        let neighborhood = Neighborhood { features, ..neighborhood };
        
        // Update cache
        self.update_cache(node_id, neighborhood.clone()).await;
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.cache_misses += 1;
            counter!("neighborhood_cache_misses_total").increment(1);
        }
        
        timer.observe_duration();
        
        Ok(neighborhood)
    }
    
    async fn get_neighborhoods(
        &self,
        node_ids: &[Uuid],
        timestamp: DateTime<Utc>,
    ) -> Result<Vec<Neighborhood>> {
        let mut neighborhoods = Vec::new();
        
        // Process nodes in parallel
        let mut handles = Vec::new();
        for &node_id in node_ids {
            let self_clone = self.clone();
            handles.push(tokio::spawn(async move {
                self_clone.get_neighborhood(node_id, timestamp).await
            }));
        }
        
        // Collect results
        for handle in handles {
            neighborhoods.push(handle.await??);
        }
        
        Ok(neighborhoods)
    }
    
    async fn compute_features(
        &self,
        neighborhood: &Neighborhood,
    ) -> Result<Vec<f32>> {
        self.compute_neighborhood_features(neighborhood).await
    }
    
    fn update_stats(&self) {
        gauge!("neighborhood_cache_size", self.cache.len() as f64);
    }
}

impl Clone for NeighborhoodManager {
    fn clone(&self) -> Self {
        Self {
            graph: self.graph.clone(),
            config: self.config.clone(),
            cache: self.cache.clone(),
            feature_cache: self.feature_cache.clone(),
            stats: self.stats.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_config;
    use aws_sdk_dynamodb;

    #[tokio::test]
    async fn test_neighborhood_operations() {
        // Create DynamoDB client for testing
        let config = aws_config::load_from_env().await;
        let client = Arc::new(aws_sdk_dynamodb::Client::new(&config));
        
        // Create temporal graph store
        let graph = TemporalGraphStore::new(
            client,
            "test_neighborhood".to_string(),
        );
        
        let now = Utc::now();
        let valid_time = TemporalRange {
            start: Some(Timestamp(now)),
            end: Some(Timestamp(now + chrono::Duration::hours(1))),
        };
        
        // Create test nodes
        let node1 = Node {
            id: NodeId(Uuid::new_v4()),
            entity_type: EntityType::Person,
            label: "Node 1".to_string(),
            properties: Properties::new(),
            valid_time: valid_time.clone(),
            transaction_time: valid_time.clone(),
        };
        
        let node2 = Node {
            id: NodeId(Uuid::new_v4()),
            entity_type: EntityType::Person,
            label: "Node 2".to_string(),
            properties: Properties::new(),
            valid_time: valid_time.clone(),
            transaction_time: valid_time.clone(),
        };
        
        // Store nodes
        graph.store(EntityId(node1.id.0), node1.clone(), valid_time.clone()).await.unwrap();
        graph.store(EntityId(node2.id.0), node2.clone(), valid_time.clone()).await.unwrap();
        
        // Create edge between nodes
        let edge = Edge {
            id: EdgeId(Uuid::new_v4()),
            source_id: node1.id,
            target_id: node2.id,
            label: "related_to".to_string(),
            properties: Properties::new(),
            valid_time: valid_time.clone(),
            transaction_time: valid_time.clone(),
        };
        
        // Store edge
        graph.store(EntityId(edge.id.0), edge.clone(), valid_time.clone()).await.unwrap();
        
        // Query neighbors
        let neighbors = graph.get_neighbors(node1.id, now).await.unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].id, node2.id);
    }
} 
use std::sync::Arc;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde::{Deserialize, Serialize};

use crate::{
    error::{Error, Result},
    temporal::{TemporalGraphStore, TemporalRange},
    types::{Node, Edge, EntityType, Properties, NodeId, EdgeId, EntityId, Timestamp},
};

/// Represents a subgraph type in the hierarchical system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SubgraphType {
    /// Raw episode data
    Episode,
    /// Semantic entity information
    Entity,
    /// Community-level information
    Community,
}

/// Represents a subgraph in the hierarchical system
#[derive(Debug)]
pub struct Subgraph {
    /// The type of subgraph
    pub subgraph_type: SubgraphType,
    /// The temporal graph store for this subgraph
    graph_store: Arc<TemporalGraphStore>,
    /// Parent subgraph (if any)
    parent: Option<Arc<Subgraph>>,
    /// Child subgraphs
    children: Vec<Arc<Subgraph>>,
}

/// Trait for subgraph operations
#[async_trait]
pub trait SubgraphOperations {
    /// Add a node to the subgraph
    async fn add_node(&self, node: Node, valid_time: TemporalRange) -> Result<()>;
    
    /// Add an edge to the subgraph
    async fn add_edge(&self, edge: Edge, valid_time: TemporalRange) -> Result<()>;
    
    /// Get nodes at a specific time
    async fn get_nodes_at(&self, timestamp: DateTime<Utc>, node_type: Option<EntityType>) -> Result<Vec<Node>>;
    
    /// Get edges at a specific time
    async fn get_edges_at(
        &self,
        timestamp: DateTime<Utc>,
        source_id: Option<Uuid>,
        target_id: Option<Uuid>,
    ) -> Result<Vec<Edge>>;
    
    /// Get nodes between two timestamps
    async fn get_nodes_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        node_type: Option<EntityType>,
    ) -> Result<Vec<Node>>;
    
    /// Get edges between two timestamps
    async fn get_edges_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        source_id: Option<Uuid>,
        target_id: Option<Uuid>,
    ) -> Result<Vec<Edge>>;
    
    /// Propagate changes up the hierarchy
    async fn propagate_up(&self, timestamp: DateTime<Utc>) -> Result<()>;
    
    /// Propagate changes down the hierarchy
    async fn propagate_down(&self, timestamp: DateTime<Utc>) -> Result<()>;
}

#[async_trait]
impl SubgraphOperations for Subgraph {
    async fn add_node(&self, node: Node, valid_time: TemporalRange) -> Result<()> {
        // Store node in current subgraph
        self.graph_store.store(EntityId(node.id.0), node.clone(), valid_time.clone()).await?;
        
        // Propagate to parent if exists
        if let Some(parent) = &self.parent {
            parent.add_node(node, valid_time).await?;
        }
        
        Ok(())
    }
    
    async fn add_edge(&self, edge: Edge, valid_time: TemporalRange) -> Result<()> {
        // Store edge in current subgraph
        self.graph_store.store(EntityId(edge.id.0), edge.clone(), valid_time.clone()).await?;
        
        // Propagate to parent if exists
        if let Some(parent) = &self.parent {
            parent.add_edge(edge, valid_time).await?;
        }
        
        Ok(())
    }
    
    async fn get_nodes_at(&self, timestamp: DateTime<Utc>, node_type: Option<EntityType>) -> Result<Vec<Node>> {
        self.graph_store.get_nodes_at(timestamp, node_type).await
    }
    
    async fn get_edges_at(
        &self,
        timestamp: DateTime<Utc>,
        source_id: Option<Uuid>,
        target_id: Option<Uuid>,
    ) -> Result<Vec<Edge>> {
        self.graph_store.get_edges_at(timestamp, source_id, target_id).await
    }
    
    async fn get_nodes_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        node_type: Option<EntityType>,
    ) -> Result<Vec<Node>> {
        self.graph_store.get_nodes_between(start, end, node_type).await
    }
    
    async fn get_edges_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        source_id: Option<Uuid>,
        target_id: Option<Uuid>,
    ) -> Result<Vec<Edge>> {
        self.graph_store.get_edges_between(start, end, source_id, target_id).await
    }
    
    async fn propagate_up(&self, timestamp: DateTime<Utc>) -> Result<()> {
        if let Some(parent) = &self.parent {
            // Get all nodes and edges at this timestamp
            let nodes = self.get_nodes_at(timestamp, None).await?;
            let edges = self.get_edges_at(timestamp, None, None).await?;
            
            // Create valid time range for propagation
            let valid_time = TemporalRange {
                start: Some(Timestamp(timestamp)),
                end: None, // Open-ended until next update
            };
            
            // Propagate nodes and edges to parent
            for node in nodes {
                parent.add_node(node, valid_time.clone()).await?;
            }
            
            for edge in edges {
                parent.add_edge(edge, valid_time.clone()).await?;
            }
        }
        
        Ok(())
    }
    
    async fn propagate_down(&self, timestamp: DateTime<Utc>) -> Result<()> {
        // Get all nodes and edges at this timestamp
        let nodes = self.get_nodes_at(timestamp, None).await?;
        let edges = self.get_edges_at(timestamp, None, None).await?;
        
        // Create valid time range for propagation
        let valid_time = TemporalRange {
            start: Some(Timestamp(timestamp)),
            end: None, // Open-ended until next update
        };
        
        // Propagate to all children
        for child in &self.children {
            for node in &nodes {
                child.add_node(node.clone(), valid_time.clone()).await?;
            }
            
            for edge in &edges {
                child.add_edge(edge.clone(), valid_time.clone()).await?;
            }
            
            // Recursively propagate down
            child.propagate_down(timestamp).await?;
        }
        
        Ok(())
    }
}

impl Subgraph {
    /// Create a new subgraph
    pub fn new(
        subgraph_type: SubgraphType,
        graph_store: Arc<TemporalGraphStore>,
        parent: Option<Arc<Subgraph>>,
    ) -> Self {
        Self {
            subgraph_type,
            graph_store,
            parent,
            children: Vec::new(),
        }
    }
    
    /// Add a child subgraph
    pub fn add_child(&mut self, child: Arc<Subgraph>) {
        self.children.push(child);
    }
    
    /// Get the subgraph type
    pub fn subgraph_type(&self) -> &SubgraphType {
        &self.subgraph_type
    }
    
    /// Get a reference to the parent subgraph
    pub fn parent(&self) -> Option<&Arc<Subgraph>> {
        self.parent.as_ref()
    }
    
    /// Get a reference to the child subgraphs
    pub fn children(&self) -> &[Arc<Subgraph>] {
        &self.children
    }
}

impl SubgraphManager {
    /// Store a subgraph with temporal information
    pub async fn store_subgraph(
        &self,
        subgraph: Subgraph,
        valid_time: TemporalRange,
    ) -> Result<()> {
        // Store nodes
        for node in &subgraph.nodes {
            self.graph_store.store(EntityId(node.id.0), node.clone(), valid_time.clone()).await?;
        }
        
        // Store edges
        for edge in &subgraph.edges {
            self.graph_store.store(EntityId(edge.id.0), edge.clone(), valid_time.clone()).await?;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_config;
    use aws_sdk_dynamodb;
    use chrono::Duration;
    
    #[tokio::test]
    async fn test_hierarchical_subgraph() {
        // Create DynamoDB client for testing
        let config = aws_config::load_from_env().await;
        let client = Arc::new(aws_sdk_dynamodb::Client::new(&config));
        
        // Create graph stores for different subgraph levels
        let episode_store = Arc::new(TemporalGraphStore::new(
            client.clone(),
            "test_episode".to_string(),
        ));
        
        let entity_store = Arc::new(TemporalGraphStore::new(
            client.clone(),
            "test_entity".to_string(),
        ));
        
        let community_store = Arc::new(TemporalGraphStore::new(
            client.clone(),
            "test_community".to_string(),
        ));
        
        // Create hierarchical subgraphs
        let community = Arc::new(Subgraph::new(
            SubgraphType::Community,
            community_store,
            None,
        ));
        
        let entity = Arc::new(Subgraph::new(
            SubgraphType::Entity,
            entity_store,
            Some(community.clone()),
        ));
        
        let episode = Arc::new(Subgraph::new(
            SubgraphType::Episode,
            episode_store,
            Some(entity.clone()),
        ));
        
        let now = Utc::now();
        
        // Create test node
        let node = Node {
            id: NodeId(Uuid::new_v4()),
            entity_type: EntityType::Person,
            label: "Test Node".to_string(),
            properties: Properties::new(),
            valid_time: TemporalRange {
                start: Some(Timestamp(now)),
                end: Some(Timestamp(now + Duration::hours(1))),
            },
            transaction_time: TemporalRange {
                start: Some(Timestamp(now)),
                end: Some(Timestamp(now + Duration::hours(1))),
            },
        };
        
        // Add node to episode subgraph
        assert!(episode.add_node(node.clone(), node.valid_time).await.is_ok());
        
        // Verify node exists in all levels
        let episode_nodes = episode.get_nodes_at(now, Some(EntityType::Person)).await.unwrap();
        let entity_nodes = entity.get_nodes_at(now, Some(EntityType::Person)).await.unwrap();
        let community_nodes = community.get_nodes_at(now, Some(EntityType::Person)).await.unwrap();
        
        assert_eq!(episode_nodes.len(), 1);
        assert_eq!(entity_nodes.len(), 1);
        assert_eq!(community_nodes.len(), 1);
        
        // Test edge propagation
        let edge = Edge {
            id: EdgeId(Uuid::new_v4()),
            source_id: node.id,
            target_id: Uuid::new_v4(),
            edge_type: "test_edge".to_string(),
            properties: serde_json::json!({}),
        };
        
        assert!(episode.add_edge(edge.clone(), node.valid_time).await.is_ok());
        
        // Verify edge exists in all levels
        let episode_edges = episode.get_edges_at(now, Some(node.id), None).await.unwrap();
        let entity_edges = entity.get_edges_at(now, Some(node.id), None).await.unwrap();
        let community_edges = community.get_edges_at(now, Some(node.id), None).await.unwrap();
        
        assert_eq!(episode_edges.len(), 1);
        assert_eq!(entity_edges.len(), 1);
        assert_eq!(community_edges.len(), 1);
    }
} 
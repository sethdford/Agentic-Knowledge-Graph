use uuid::Uuid;
use async_trait::async_trait;
use aws_config;
use aws_sdk_dynamodb::Client;
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

use crate::{
    error::{Error, Result},
    types::{Node, Edge, EntityType, Properties, NodeId, EdgeId, EntityId, TemporalRange, Timestamp},
};

use super::{
    ConsistencyCheckResult, DynamoDBTemporal, Temporal, TemporalQueryResult,
    query::OptimizedQuery,
};

/// Trait for temporal graph operations
#[async_trait]
pub trait TemporalGraph {
    /// Get nodes at a specific time
    async fn get_nodes_at(&self, timestamp: DateTime<Utc>, node_type: Option<EntityType>) -> Result<Vec<Node>>;
    
    /// Get edges at a specific time
    async fn get_edges_at(&self, timestamp: DateTime<Utc>, source_id: Option<Uuid>, target_id: Option<Uuid>) -> Result<Vec<Edge>>;
    
    /// Get nodes between two points in time
    async fn get_nodes_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        node_type: Option<EntityType>,
    ) -> Result<Vec<Node>>;
    
    /// Get edges between two points in time
    async fn get_edges_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Edge>>;
    
    /// Store a node with temporal information
    async fn store(&self, entity_id: EntityId, data: impl Serialize + Send + Sync + 'static, valid_time: TemporalRange) -> Result<()>;

    /// Get node evolution over time
    async fn get_node_evolution(&self, node_id: NodeId, time_range: &TemporalRange) -> Result<Vec<Node>>;

    /// Get edge evolution over time
    async fn get_edge_evolution(&self, edge_id: EdgeId, time_range: &TemporalRange) -> Result<Vec<Edge>>;
}

/// Temporal graph implementation for efficient node and edge queries
pub struct TemporalGraphImpl {
    nodes: Arc<DynamoDBTemporal<Node, Client>>,
    edges: Arc<DynamoDBTemporal<Edge, Client>>,
}

impl TemporalGraphImpl {
    pub async fn new(
        nodes: Arc<DynamoDBTemporal<Node, Client>>,
        edges: Arc<DynamoDBTemporal<Edge, Client>>,
    ) -> Self {
        Self { nodes, edges }
    }

    pub async fn new_test() -> Result<Self> {
        let config = aws_config::load_from_env().await;
        let client = Arc::new(Client::new(&config));
        
        Ok(TemporalGraphImpl {
            nodes: Arc::new(DynamoDBTemporal::new(
                client.clone(),
                "test_temporal_graph_nodes".to_string(),
            )),
            edges: Arc::new(DynamoDBTemporal::new(
                client.clone(),
                "test_temporal_graph_edges".to_string(),
            )),
        })
    }

    /// Store data with temporal information
    pub async fn store(&self, entity_id: EntityId, data: impl Serialize + Send + Sync + 'static, valid_time: TemporalRange) -> Result<()> {
        let json_data = serde_json::to_value(&data)?;
        match entity_id.entity_type {
            EntityType::Node | EntityType::Person | EntityType::Organization | EntityType::Location |
            EntityType::Event | EntityType::Topic | EntityType::Document | EntityType::Vertex => {
                let node: Node = serde_json::from_value(json_data)?;
                self.nodes.store(&entity_id, &valid_time, &node).await
            }
            EntityType::Edge => {
                let edge: Edge = serde_json::from_value(json_data)?;
                self.edges.store(&entity_id, &valid_time, &edge).await
            }
            EntityType::Custom(_) => {
                let node: Node = serde_json::from_value(json_data)?;
                self.nodes.store(&entity_id, &valid_time, &node).await
            }
        }
    }

    /// Get nodes valid at a specific timestamp
    pub async fn get_nodes_at(
        &self,
        timestamp: DateTime<Utc>,
        node_type: Option<EntityType>,
    ) -> Result<Vec<Node>> {
        let mut results = Vec::new();
        let mut last_evaluated_key = None;

        loop {
            let mut query = OptimizedQuery::new(self.nodes.table_name().to_string())
                .with_key_condition("valid_time_start <= :ts AND valid_time_end >= :ts".to_string())
                .with_values(serde_json::json!({
                    ":ts": timestamp.timestamp().to_string(),
                }));

            if let Some(ref nt) = node_type {
                query = query
                    .with_filter("entity_type = :et".to_string())
                    .with_values(serde_json::json!({
                        ":et": nt.to_string(),
                    }));
            }

            let result = self.nodes.query_with_options(query, last_evaluated_key).await?;
            
            for node in result.items {
                results.push(node.data);
            }

            last_evaluated_key = result.last_evaluated_key;
            if last_evaluated_key.is_none() {
                break;
            }
        }

        Ok(results)
    }

    /// Get edges valid at a specific timestamp
    pub async fn get_edges_at(
        &self,
        timestamp: DateTime<Utc>,
        source_id: Option<Uuid>,
        target_id: Option<Uuid>,
    ) -> Result<Vec<Edge>> {
        let mut results = Vec::new();
        let mut last_evaluated_key = None;

        loop {
            let mut query = OptimizedQuery::new(self.edges.table_name().to_string())
                .with_key_condition("valid_time_start <= :ts AND valid_time_end >= :ts".to_string())
                .with_values(serde_json::json!({
                    ":ts": timestamp.timestamp().to_string(),
                }));

            let mut filter_conditions = Vec::new();
            let mut filter_values = serde_json::Map::new();

            if let Some(sid) = source_id {
                filter_conditions.push("source_id = :sid");
                filter_values.insert(":sid".to_string(), serde_json::Value::String(sid.to_string()));
            }

            if let Some(tid) = target_id {
                filter_conditions.push("target_id = :tid");
                filter_values.insert(":tid".to_string(), serde_json::Value::String(tid.to_string()));
            }

            if !filter_conditions.is_empty() {
                query = query
                    .with_filter(filter_conditions.join(" AND "))
                    .with_values(serde_json::Value::Object(filter_values));
            }

            let result = self.edges.query_with_options(query, last_evaluated_key).await?;
            
            for edge in result.items {
                results.push(edge.data);
            }

            last_evaluated_key = result.last_evaluated_key;
            if last_evaluated_key.is_none() {
                break;
            }
        }

        Ok(results)
    }

    /// Get nodes within a temporal range
    pub async fn get_nodes_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        node_type: Option<EntityType>,
    ) -> Result<Vec<Node>> {
        let mut results = Vec::new();
        let mut last_evaluated_key = None;

        loop {
            let mut query = OptimizedQuery::new(self.nodes.table_name().to_string())
                .with_key_condition("valid_time_start <= :end AND valid_time_end >= :start".to_string())
                .with_values(serde_json::json!({
                    ":start": start.timestamp().to_string(),
                    ":end": end.timestamp().to_string(),
                }));

            if let Some(ref nt) = node_type {
                query = query
                    .with_filter("entity_type = :et".to_string())
                    .with_values(serde_json::json!({
                        ":et": nt.to_string(),
                    }));
            }

            let result = self.nodes.query_with_options(query, last_evaluated_key).await?;
            
            for node in result.items {
                results.push(node.data);
            }

            last_evaluated_key = result.last_evaluated_key;
            if last_evaluated_key.is_none() {
                break;
            }
        }

        Ok(results)
    }

    /// Get edges within a temporal range
    pub async fn get_edges_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        source_id: Option<Uuid>,
        target_id: Option<Uuid>,
    ) -> Result<Vec<Edge>> {
        let mut results = Vec::new();
        let mut last_evaluated_key = None;

        loop {
            let mut query = OptimizedQuery::new(self.edges.table_name().to_string())
                .with_key_condition("valid_time_start <= :end AND valid_time_end >= :start".to_string())
                .with_values(serde_json::json!({
                    ":start": start.timestamp().to_string(),
                    ":end": end.timestamp().to_string(),
                }));

            let mut filter_conditions = Vec::new();
            let mut filter_values = serde_json::Map::new();

            if let Some(sid) = source_id {
                filter_conditions.push("source_id = :sid");
                filter_values.insert(":sid".to_string(), serde_json::Value::String(sid.to_string()));
            }

            if let Some(tid) = target_id {
                filter_conditions.push("target_id = :tid");
                filter_values.insert(":tid".to_string(), serde_json::Value::String(tid.to_string()));
            }

            if !filter_conditions.is_empty() {
                query = query
                    .with_filter(filter_conditions.join(" AND "))
                    .with_values(serde_json::Value::Object(filter_values));
            }

            let result = self.edges.query_with_options(query, last_evaluated_key).await?;
            
            for edge in result.items {
                results.push(edge.data);
            }

            last_evaluated_key = result.last_evaluated_key;
            if last_evaluated_key.is_none() {
                break;
            }
        }

        Ok(results)
    }

    /// Get the evolution of a node over time
    pub async fn get_node_evolution(
        &self,
        node_id: NodeId,
        range: &TemporalRange,
    ) -> Result<Vec<Node>> {
        self.nodes.get_node_evolution(node_id, range).await
    }

    /// Get the evolution of an edge over time
    pub async fn get_edge_evolution(
        &self,
        edge_id: EdgeId,
        range: &TemporalRange,
    ) -> Result<Vec<Edge>> {
        self.edges.get_edge_evolution(edge_id, range).await
    }

    /// Get the latest version of a node
    pub async fn get_latest_node(&self, node_id: Uuid) -> Result<Option<Node>> {
        let result = self.nodes
            .query_latest(&EntityId { entity_type: EntityType::Node, id: node_id.to_string() })
            .await?;
        
        Ok(result.map(|r| r.data))
    }

    /// Get the latest version of an edge
    pub async fn get_latest_edge(&self, edge_id: Uuid) -> Result<Option<Edge>> {
        let result = self.edges
            .query_latest(&EntityId { entity_type: EntityType::Edge, id: edge_id.to_string() })
            .await?;
        
        Ok(result.map(|r| r.data))
    }

    /// Validate temporal consistency of the graph
    pub async fn validate_consistency(&self) -> Result<ConsistencyCheckResult> {
        let node_consistency = self.nodes.validate_consistency().await?;
        let edge_consistency = self.edges.validate_consistency().await?;

        let mut violations = node_consistency.violations;
        violations.extend(edge_consistency.violations);

        Ok(ConsistencyCheckResult {
            passed: node_consistency.passed && edge_consistency.passed,
            violations,
        })
    }
}

#[async_trait]
impl TemporalGraph for TemporalGraphImpl {
    async fn get_nodes_at(&self, timestamp: DateTime<Utc>, node_type: Option<EntityType>) -> Result<Vec<Node>> {
        let mut query = OptimizedQuery::new(self.nodes.table_name().to_string())
            .with_key_condition("valid_time_start <= :ts AND valid_time_end >= :ts".to_string())
            .with_values(serde_json::json!({
                ":ts": timestamp.timestamp().to_string(),
            }));

        if let Some(nt) = node_type {
            query = query
                .with_filter("entity_type = :et".to_string())
                .with_values(serde_json::json!({
                    ":et": nt.to_string(),
                }));
        }

        let result = self.nodes.query_with_options(query, None).await?;
        Ok(result.items.into_iter().map(|r| r.data).collect())
    }

    async fn get_edges_at(&self, timestamp: DateTime<Utc>, source_id: Option<Uuid>, target_id: Option<Uuid>) -> Result<Vec<Edge>> {
        let mut results = Vec::new();
        let mut last_evaluated_key = None;

        loop {
            let mut query = OptimizedQuery::new(self.edges.table_name().to_string())
                .with_key_condition("valid_time_start <= :ts AND valid_time_end >= :ts".to_string())
                .with_values(serde_json::json!({
                    ":ts": timestamp.timestamp().to_string(),
                }));

            let mut filter_conditions = Vec::new();
            let mut filter_values = serde_json::Map::new();

            if let Some(sid) = source_id {
                filter_conditions.push("source_id = :sid");
                filter_values.insert(":sid".to_string(), serde_json::Value::String(sid.to_string()));
            }

            if let Some(tid) = target_id {
                filter_conditions.push("target_id = :tid");
                filter_values.insert(":tid".to_string(), serde_json::Value::String(tid.to_string()));
            }

            if !filter_conditions.is_empty() {
                query = query
                    .with_filter(filter_conditions.join(" AND "))
                    .with_values(serde_json::Value::Object(filter_values));
            }

            let result = self.edges.query_with_options(query, last_evaluated_key).await?;
            
            for edge in result.items {
                results.push(edge.data);
            }

            last_evaluated_key = result.last_evaluated_key;
            if last_evaluated_key.is_none() {
                break;
            }
        }

        Ok(results)
    }

    async fn get_nodes_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        node_type: Option<EntityType>,
    ) -> Result<Vec<Node>> {
        let mut query = OptimizedQuery::new(self.nodes.table_name().to_string())
            .with_key_condition("valid_time_start <= :end AND valid_time_end >= :start".to_string())
            .with_values(serde_json::json!({
                ":start": start.timestamp().to_string(),
                ":end": end.timestamp().to_string(),
            }));

        if let Some(nt) = node_type {
            query = query
                .with_filter("entity_type = :et".to_string())
                .with_values(serde_json::json!({
                    ":et": nt.to_string(),
                }));
        }

        let result = self.nodes.query_with_options(query, None).await?;
        Ok(result.items.into_iter().map(|r| r.data).collect())
    }

    async fn get_edges_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Edge>> {
        let query = OptimizedQuery::new(self.edges.table_name().to_string())
            .with_key_condition("valid_time_start <= :end AND valid_time_end >= :start".to_string())
            .with_values(serde_json::json!({
                ":start": start.timestamp().to_string(),
                ":end": end.timestamp().to_string(),
            }));

        let result = self.edges.query_with_options(query, None).await?;
        Ok(result.items.into_iter().map(|r| r.data).collect())
    }

    async fn store(&self, entity_id: EntityId, data: impl Serialize + Send + Sync + 'static, valid_time: TemporalRange) -> Result<()> {
        self.store(entity_id, data, valid_time).await
    }

    async fn get_node_evolution(&self, node_id: NodeId, time_range: &TemporalRange) -> Result<Vec<Node>> {
        self.nodes.get_node_evolution(node_id, time_range).await
    }

    async fn get_edge_evolution(&self, edge_id: EdgeId, time_range: &TemporalRange) -> Result<Vec<Edge>> {
        self.edges.get_edge_evolution(edge_id, time_range).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[tokio::test]
    async fn test_temporal_graph_operations() {
        // Create DynamoDB client for testing
        let config = aws_config::load_from_env().await;
        let client = Arc::new(Client::new(&config));
        
        let graph = TemporalGraphImpl {
            nodes: Arc::new(DynamoDBTemporal::new(
                client.clone(),
                "test_temporal_graph_nodes".to_string(),
            )),
            edges: Arc::new(DynamoDBTemporal::new(
                client.clone(),
                "test_temporal_graph_edges".to_string(),
            )),
        };

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

        // Store node with temporal information
        let valid_time = TemporalRange {
            start: Some(Timestamp(now)),
            end: Some(Timestamp(now + Duration::hours(1))),
        };

        assert!(graph.store(EntityId {
            entity_type: EntityType::Node,
            id: node.id.0.to_string()
        }, node.clone(), valid_time.clone()).await.is_ok());

        // Test querying nodes
        let nodes = graph.get_nodes_at(now, Some(EntityType::Person)).await.unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, node.id);

        // Create test edge
        let edge = Edge {
            id: EdgeId(Uuid::new_v4()),
            source_id: node.id,
            target_id: NodeId(Uuid::new_v4()),
            label: "test_edge".to_string(),
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

        // Store edge with temporal information
        assert!(graph.store(EntityId {
            entity_type: EntityType::Edge,
            id: edge.id.0.to_string()
        }, edge.clone(), valid_time).await.is_ok());

        // Test querying edges
        let edges = graph.get_edges_at(now, Some(node.id.0), Some(edge.target_id.0)).await.unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].id, edge.id);

        // Test temporal range queries
        let nodes = graph
            .get_nodes_between(now, now + Duration::hours(1), Some(EntityType::Person))
            .await
            .unwrap();
        assert_eq!(nodes.len(), 1);

        // Test evolution queries
        let range = TemporalRange {
            start: Some(Timestamp(now)),
            end: Some(Timestamp(now + Duration::hours(1))),
        };
        let node_evolution = graph
            .get_node_evolution(
                NodeId(node.id.0),
                &range,
            )
            .await
            .unwrap();
        assert_eq!(node_evolution.len(), 1);

        // Test latest queries
        let latest_node = graph.get_latest_node(node.id.0).await.unwrap();
        assert!(latest_node.is_some());
        assert_eq!(latest_node.unwrap().id, node.id);

        // Test consistency validation
        let consistency = graph.validate_consistency().await.unwrap();
        assert!(consistency.passed);
    }
} 
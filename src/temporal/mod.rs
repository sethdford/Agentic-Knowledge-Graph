mod consistency;
mod index;
mod query;
mod query_builder;
mod dynamodb;
pub mod graph;

pub use consistency::{ConsistencyChecker, ConsistencyCheckResult, ConsistencyViolation, ConsistencyViolationType};
pub use index::{TemporalIndex, TemporalIndexEntry};
pub use query::OptimizedQuery;
pub use query_builder::TemporalQueryBuilder;
pub use dynamodb::DynamoDBTemporal;
pub use graph::TemporalGraph;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use std::sync::Arc;
use aws_sdk_dynamodb::Client as DynamoClient;
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    error::{Error, Result},
    graph::{Edge, Graph, Node},
    types::{EntityId, EntityType, EdgeId, NodeId, TemporalRange, Timestamp, TemporalQueryResult},
    Config,
};

/// Represents a temporal query range
#[derive(Debug, Clone)]
pub struct TimeRange {
    pub start: Timestamp,
    pub end: Timestamp,
}

impl TimeRange {
    /// Create a new time range
    pub fn new(start: Timestamp, end: Timestamp) -> Self {
        Self { start, end }
    }
    
    /// Create a time range for a specific duration before a given timestamp
    pub fn before(timestamp: Timestamp, duration: Duration) -> Self {
        let start_time = timestamp.0 - duration;
        Self {
            start: Timestamp(start_time),
            end: timestamp,
        }
    }
    
    /// Create a time range for a specific duration after a given timestamp
    pub fn after(timestamp: Timestamp, duration: Duration) -> Self {
        let end_time = timestamp.0 + duration;
        Self {
            start: timestamp,
            end: Timestamp(end_time),
        }
    }
}

/// Represents a temporal query operation
#[derive(Debug, Clone)]
pub enum TemporalOperation {
    /// Get state at a specific point in time
    At(DateTime<Utc>),
    /// Get state between two points in time
    Between(DateTime<Utc>, DateTime<Utc>),
    /// Get evolution over time
    Evolution(TemporalRange),
    /// Get latest state
    Latest,
}

/// Core trait for temporal operations
#[async_trait]
pub trait Temporal {
    /// The type of data being stored
    type Data;

    /// Store data with temporal information
    async fn store(
        &self,
        entity_id: EntityId,
        data: Self::Data,
        valid_time: TemporalRange,
    ) -> Result<()>;

    /// Query data at a specific point in time
    async fn query_at(
        &self,
        entity_id: &EntityId,
        timestamp: DateTime<Utc>,
    ) -> Result<Vec<TemporalQueryResult<Self::Data>>>;

    /// Query data between two points in time
    async fn query_between(
        &self,
        entity_id: &EntityId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TemporalQueryResult<Self::Data>>>;

    /// Query the evolution of data over time
    async fn query_evolution(
        &self,
        entity_id: &EntityId,
        range: &TemporalRange,
    ) -> Result<Vec<TemporalQueryResult<Self::Data>>>;

    /// Query the latest version of data
    async fn query_latest(
        &self,
        entity_id: &EntityId,
    ) -> Result<Option<TemporalQueryResult<Self::Data>>>;

    /// Validate temporal consistency
    async fn validate_consistency(&self) -> Result<ConsistencyCheckResult>;
}

/// Create a new temporal instance with DynamoDB backend
pub async fn new_temporal_dynamodb<T>(config: &Config) -> Result<DynamoDBTemporal<T, DynamoClient>>
where
    T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    let shared_config = aws_config::from_env().load().await;
    let client = DynamoClient::new(&shared_config);
    let table_name = config.dynamodb_table.clone();

    Ok(DynamoDBTemporal::new(Arc::new(client), table_name))
}

/// Temporal operations trait defining the interface for time-based operations
#[async_trait]
pub trait TemporalOperations: Send + Sync {
    /// Get nodes valid within a time range
    async fn get_nodes_in_range(&self, range: TimeRange) -> Result<Vec<Node>>;
    
    /// Get edges valid within a time range
    async fn get_edges_in_range(&self, range: TimeRange) -> Result<Vec<Edge>>;
    
    /// Get the temporal evolution of a node
    async fn get_node_evolution(&self, node_id: uuid::Uuid, range: TimeRange) -> Result<Vec<Node>>;
    
    /// Get the temporal evolution of relationships between nodes
    async fn get_relationship_evolution(
        &self,
        source_id: uuid::Uuid,
        target_id: uuid::Uuid,
        range: TimeRange,
    ) -> Result<Vec<Edge>>;
}

/// Temporal graph implementation combining graph operations with temporal queries
pub struct TemporalGraphImpl<G>
where
    G: Graph,
{
    graph: Arc<G>,
}

impl<G> TemporalGraphImpl<G>
where
    G: Graph,
{
    /// Create a new TemporalGraph instance
    pub fn new(graph: Arc<G>) -> Self {
        Self { graph }
    }
    
    /// Helper to check if a timestamp falls within a range
    fn is_in_range(timestamp: &Timestamp, range: &TimeRange) -> bool {
        timestamp.0 >= range.start.0 && timestamp.0 <= range.end.0
    }
}

#[async_trait]
impl<G> TemporalOperations for TemporalGraphImpl<G>
where
    G: Graph + Send + Sync,
{
    async fn get_nodes_in_range(&self, range: TimeRange) -> Result<Vec<Node>> {
        // TODO: Implement efficient temporal range query for nodes
        // This is a placeholder implementation
        Ok(vec![])
    }
    
    async fn get_edges_in_range(&self, range: TimeRange) -> Result<Vec<Edge>> {
        // TODO: Implement efficient temporal range query for edges
        // This is a placeholder implementation
        Ok(vec![])
    }
    
    async fn get_node_evolution(&self, node_id: uuid::Uuid, range: TimeRange) -> Result<Vec<Node>> {
        let node_id = NodeId(node_id);
        let node = self.graph.get_node(node_id).await?;
        
        // Check if the node's valid time overlaps with the requested range
        if let Some(overlap) = utils::temporal_overlap(&range, &TimeRange {
            start: node.valid_time.start.unwrap_or(Timestamp(Utc::now())),
            end: node.valid_time.end.unwrap_or(Timestamp(Utc::now()))
        }) {
            Ok(vec![node])
        } else {
            Ok(vec![])
        }
    }
    
    async fn get_relationship_evolution(
        &self,
        source_id: uuid::Uuid,
        target_id: uuid::Uuid,
        range: TimeRange,
    ) -> Result<Vec<Edge>> {
        // TODO: Implement relationship evolution tracking
        // This should:
        // 1. Get all edges between source and target nodes
        // 2. Filter by time range
        // 3. Sort by timestamp to show evolution
        Ok(vec![])
    }
}

/// Utility functions for temporal operations
pub mod utils {
    use super::*;
    
    /// Calculate the temporal overlap between two time ranges
    pub fn temporal_overlap(range1: &TimeRange, range2: &TimeRange) -> Option<TimeRange> {
        let start = range1.start.0.max(range2.start.0);
        let end = range1.end.0.min(range2.end.0);
        
        if start <= end {
            Some(TimeRange {
                start: Timestamp(start),
                end: Timestamp(end),
            })
        } else {
            None
        }
    }
    
    /// Calculate the temporal distance between two timestamps
    pub fn temporal_distance(t1: &Timestamp, t2: &Timestamp) -> Duration {
        if t1.0 > t2.0 {
            t1.0 - t2.0
        } else {
            t2.0 - t1.0
        }
    }
    
    /// Check if two time ranges are adjacent
    pub fn is_adjacent(range1: &TimeRange, range2: &TimeRange) -> bool {
        range1.end.0 == range2.start.0 || range1.start.0 == range2.end.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;

    #[tokio::test]
    async fn test_temporal_operations() {
        let config = Config::for_testing();
        let temporal = new_temporal_dynamodb::<String>(&config).await.unwrap();

        let entity_id = EntityId::new(EntityType::Node, "test-node".to_string());
        let now = Utc::now();
        let data = "test data".to_string();

        // Test store
        let valid_time = TemporalRange {
            start: Some(Timestamp(now)),
            end: Some(Timestamp(now + Duration::hours(1))),
        };

        assert!(Temporal::store(&temporal, entity_id.clone(), data.clone(), valid_time).await.is_ok());

        // Test querying
        let result = temporal.query_at(&entity_id, now).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].data, data);

        // Test consistency validation
        let result = temporal.validate_consistency().await.unwrap();
        assert!(result.passed);
    }
} 
mod consistency;
mod index;
mod query;
mod query_builder;
mod query_executor;
mod dynamodb;
pub mod graph;

pub use consistency::{ConsistencyChecker, ConsistencyCheckResult, ConsistencyViolation, ConsistencyViolationType};
pub use index::{TemporalIndex, TemporalIndexEntry};
pub use query::OptimizedQuery;
pub use query_builder::{
    TemporalQueryBuilder,
    PropertyOperator,
    RelationshipDirection,
    SortOrder,
    PropertyFilter,
    RelationshipFilter,
    SortField,
};
pub use query_executor::{TemporalQueryExecutor, QueryExecutorConfig};
pub use dynamodb::DynamoDBTemporal;
pub use graph::TemporalGraph;

// Add RelationshipType trait
pub trait RelationshipType: ToString + Send + Sync {}

// Implement for String and &str
impl RelationshipType for String {}
impl RelationshipType for &str {}

use async_trait::async_trait;
use chrono::{DateTime, Duration, TimeZone, Utc, NaiveDateTime};
use std::sync::Arc;
use aws_sdk_dynamodb::{Client as DynamoClient};
use serde::{de::DeserializeOwned, Serialize};
use std::any::Any;
use std::fmt;
use uuid::Uuid;

use crate::{
    aws::dynamodb::DynamoDBClient,
    error::{Error, Result},
    graph::{Edge, Graph, Node},
    types::{EntityId, EntityType, EdgeId, NodeId, TemporalRange, Timestamp, TemporalQueryResult},
    Config,
};

/// A time range defined by start and end timestamps
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeRange {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self { start, end }
    }

    /// Create a time range that starts at a very old time and ends at the given time
    pub fn before(end: DateTime<Utc>) -> Self {
        Self {
            start: DateTime::<Utc>::MIN_UTC,
            end,
        }
    }

    /// Create a time range that starts at the given time and ends at the current time
    pub fn after(start: DateTime<Utc>) -> Self {
        Self {
            start,
            end: DateTime::<Utc>::MAX_UTC,
        }
    }
    
    /// Check if this time range overlaps with another time range
    /// Ranges are considered to overlap if one range's start is less than or equal to the other's end
    /// and its end is greater than or equal to the other's start
    pub fn overlaps_with(&self, other: &TimeRange) -> bool {
        self.start < other.end && self.end > other.start
    }
    
    /// Calculate the temporal distance between this time range and another in nanoseconds
    /// Returns 0 if the ranges overlap or are adjacent
    pub fn distance_from(&self, other: &TimeRange) -> i64 {
        if self.overlaps_with(other) || self.is_adjacent_to(other) {
            return 0;
        }
    
        // If self is before other
        if self.end < other.start {
            (other.start - self.end).num_nanoseconds().unwrap_or(i64::MAX)
        }
        // If self is after other
        else {
            (self.start - other.end).num_nanoseconds().unwrap_or(i64::MAX)
        }
    }
    
    /// Check if this time range is adjacent to another
    /// Two ranges are adjacent if the end of one is exactly equal to the start of the other
    pub fn is_adjacent_to(&self, other: &TimeRange) -> bool {
        // Two ranges are adjacent if the end of one is exactly equal to the start of the other
        self.end == other.start || other.end == self.start
    }
}

impl fmt::Display for TimeRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{} to {}]", self.start, self.end)
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
    T: DeserializeOwned + Serialize + Send + Sync + 'static,
{
    let shared_config = aws_config::from_env().load().await;
    let client = DynamoClient::new(&shared_config);
    
    let temporal = DynamoDBTemporal::new(
        Arc::new(client),
        config.dynamodb_table.clone(),
    );
    
    Ok(temporal)
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
pub struct GenericTemporalGraph<T, G, C>
where
    T: DeserializeOwned + Serialize + Send + Sync + 'static,
    G: Graph + Send + Sync,
    C: DynamoDBClient + Send + Sync + 'static,
{
    graph: Arc<G>,
    temporal: DynamoDBTemporal<T, C>,
}

impl<T, G, C> GenericTemporalGraph<T, G, C>
where
    T: DeserializeOwned + Serialize + Send + Sync + 'static,
    G: Graph + Send + Sync,
    C: DynamoDBClient + Send + Sync + 'static,
{
    /// Create a new TemporalGraph instance
    pub fn new(graph: Arc<G>, temporal: DynamoDBTemporal<T, C>) -> Self {
        Self { graph, temporal }
    }
    
    /// Helper to check if a timestamp falls within a range
    fn is_in_range(timestamp: &DateTime<Utc>, range: &TimeRange) -> bool {
        // Create a point-in-time range and check if it overlaps with the given range
        let point_range = TimeRange::new(*timestamp, *timestamp);
        range.overlaps_with(&point_range)
    }
}

/// Check if two time ranges overlap
pub fn temporal_overlap(range1: &TimeRange, range2: &TimeRange) -> bool {
    range1.overlaps_with(range2)
}

/// Calculate the temporal distance between two time ranges in nanoseconds
pub fn temporal_distance(range1: &TimeRange, range2: &TimeRange) -> i64 {
    range1.distance_from(range2)
}

/// Check if two time ranges are adjacent
pub fn is_adjacent(range1: &TimeRange, range2: &TimeRange) -> bool {
    range1.is_adjacent_to(range2)
}

#[async_trait]
impl<T, G, C> TemporalOperations for GenericTemporalGraph<T, G, C>
where
    T: DeserializeOwned + Serialize + Send + Sync + 'static,
    G: Graph + Send + Sync,
    C: DynamoDBClient + Send + Sync + 'static,
{
    async fn get_nodes_in_range(&self, range: TimeRange) -> Result<Vec<Node>> {
        // We need to implement this without relying on graph.get_nodes()
        // Since the Graph trait doesn't have that method
        let mut nodes = Vec::new();
        
        // In a full implementation, we would need to get nodes another way
        // For now, return an empty vector
        
        Ok(nodes)
    }
    
    async fn get_edges_in_range(&self, range: TimeRange) -> Result<Vec<Edge>> {
        // We need to implement this without relying on graph.get_edges()
        // Since the Graph trait doesn't have that method
        let mut edges = Vec::new();
        
        // In a full implementation, we would need to get edges another way
        // For now, return an empty vector
        
        Ok(edges)
    }
    
    async fn get_node_evolution(&self, node_id: uuid::Uuid, range: TimeRange) -> Result<Vec<Node>> {
        // Get the node from the underlying graph
        let node_id = NodeId(node_id);
        match self.graph.get_node(node_id).await {
            Ok(node) => {
                // For now, just check if the node exists
                // In a full implementation, we would check if it falls within the time range
                // using proper fields like valid_time
                Ok(vec![node])
            },
            Err(_) => Ok(vec![]),
        }
    }
    
    async fn get_relationship_evolution(
        &self,
        source_id: uuid::Uuid,
        target_id: uuid::Uuid,
        range: TimeRange,
    ) -> Result<Vec<Edge>> {
        // Since Graph doesn't have a method to get edges between nodes,
        // we'll implement a stub that returns an empty vector
        let mut edges = Vec::new();
        
        // In a full implementation, we would do something like:
        // let all_edges = self.graph.get_edges_between(source_id, target_id).await?;
        // Then filter the edges based on timestamp
        
        Ok(edges)
    }
}

/// Utility functions for temporal operations
pub mod utils {
    use super::*;
    
    // Other utility functions can go here
}

/// Trait for data that can be stored in the temporal database
pub trait StorableData: Send + Sync + 'static {
    /// Convert to Any for downcasting
    fn as_any(&self) -> &dyn Any;
}

impl StorableData for Node {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl StorableData for Edge {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Duration, TimeZone, Utc};

    #[test]
    fn test_time_range() {
        // Create a TimeRange with a start and end time
        let now = Utc::now();
        let future = now + Duration::days(1);
        let range = TimeRange::new(now, future);

        // Verify the range properties
        assert_eq!(range.start, now);
        assert_eq!(range.end, future);

        // Test the before() and after() constructors
        let range2 = TimeRange::before(future);
        assert_eq!(range2.end, future);
        assert_eq!(range2.start, DateTime::<Utc>::MIN_UTC);

        let range3 = TimeRange::after(now);
        assert_eq!(range3.start, now);
        assert_eq!(range3.end, DateTime::<Utc>::MAX_UTC);

        // Test overlaps_with
        let same_range = TimeRange::new(now, future);
        assert!(range.overlaps_with(&same_range), "Range should overlap with itself");
        
        let contained_range = TimeRange::new(now, now + Duration::hours(12));
        assert!(range.overlaps_with(&contained_range), 
                "Range should overlap with range within its bounds");
        
        let overlapping_range = TimeRange::new(now - Duration::hours(12), now + Duration::hours(12));
        assert!(range.overlaps_with(&overlapping_range), 
                "Range should overlap with range that extends beyond it");
        
        // Create a non-overlapping range
        let past_range = TimeRange::new(now - Duration::days(2), now - Duration::days(1));
        assert!(!range.overlaps_with(&past_range), "Ranges should not overlap");
        
        // Test adjacency - these should not be considered overlapping
        let adjacent_range = TimeRange::new(future, future + Duration::days(1));
        assert!(!range.overlaps_with(&adjacent_range), 
                "Adjacent ranges should not be considered overlapping");
        
        // Test distance_from
        assert_eq!(range.distance_from(&same_range), 0);
        assert_eq!(range.distance_from(&past_range), 
                  (now - (now - Duration::days(1))).num_nanoseconds().unwrap_or(i64::MAX));
        
        // Test is_adjacent_to
        assert!(range.is_adjacent_to(&adjacent_range), "Ranges should be adjacent");
        assert!(!range.is_adjacent_to(&past_range), "Ranges should not be adjacent");
        
        // Verify the standalone functions still work the same
        assert_eq!(temporal_overlap(&range, &same_range), range.overlaps_with(&same_range));
        assert_eq!(temporal_distance(&range, &same_range), range.distance_from(&same_range));
        assert_eq!(is_adjacent(&range, &adjacent_range), range.is_adjacent_to(&adjacent_range));
    }

    #[test]
    fn test_non_overlapping_ranges() {
        let now = Utc::now();
        let range1 = TimeRange::new(now, now + Duration::days(1));
        let range2 = TimeRange::new(now + Duration::days(2), now + Duration::days(3));
        
        assert!(!range1.overlaps_with(&range2), "Ranges should not overlap");
        assert!(!range1.is_adjacent_to(&range2), "Ranges should not be adjacent");
        assert_eq!(range1.distance_from(&range2), 
                  Duration::days(1).num_nanoseconds().unwrap_or(i64::MAX));
        
        // Test case for exact adjacency
        let adjacent_range = TimeRange::new(now + Duration::days(1), now + Duration::days(2));
        assert!(!range1.overlaps_with(&adjacent_range), "Adjacent ranges should not overlap");
        assert!(range1.is_adjacent_to(&adjacent_range), "Ranges should be adjacent");
        assert_eq!(range1.distance_from(&adjacent_range), 0, "Distance between adjacent ranges should be 0");
    }
    
    // Integration test is skipped as it requires DynamoDB
    #[ignore]
    #[tokio::test]
    async fn test_temporal_operations() {
        // This test is skipped because it requires DynamoDB
        // Implementation would go here
    }
} 
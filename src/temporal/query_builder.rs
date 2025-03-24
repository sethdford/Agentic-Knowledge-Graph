use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    error::{Error, Result},
    types::{EntityId, EntityType, TemporalRange, Timestamp},
};

use super::query::OptimizedQuery;

/// Builder for constructing temporal queries
#[derive(Debug, Clone)]
pub struct TemporalQueryBuilder {
    /// Entity ID to query
    entity_id: Option<EntityId>,
    /// Entity type filter
    entity_type: Option<EntityType>,
    /// Point in time to query
    timestamp: Option<DateTime<Utc>>,
    /// Time range start
    range_start: Option<DateTime<Utc>>,
    /// Time range end
    range_end: Option<DateTime<Utc>>,
    /// Property filters
    property_filters: HashMap<String, String>,
    /// Maximum results to return
    limit: Option<u32>,
    /// Sort direction (true = ascending)
    ascending: bool,
}

impl Default for TemporalQueryBuilder {
    fn default() -> Self {
        Self {
            entity_id: None,
            entity_type: None,
            timestamp: None,
            range_start: None,
            range_end: None,
            property_filters: HashMap::new(),
            limit: None,
            ascending: true,
        }
    }
}

impl TemporalQueryBuilder {
    /// Create a new temporal query builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the entity ID to query
    pub fn entity_id(mut self, entity_id: EntityId) -> Self {
        self.entity_id = Some(entity_id);
        self
    }

    /// Set the entity type filter
    pub fn entity_type(mut self, entity_type: EntityType) -> Self {
        self.entity_type = Some(entity_type);
        self
    }

    /// Set a specific point in time to query
    pub fn at(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = Some(timestamp);
        self.range_start = None;
        self.range_end = None;
        self
    }

    /// Set a time range to query
    pub fn between(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Self> {
        if start > end {
            return Err(Error::InvalidTemporalRange(
                "Start time must be before end time".to_string(),
            ));
        }
        self.range_start = Some(start);
        self.range_end = Some(end);
        self.timestamp = None;
        Ok(self)
    }

    /// Add a property filter
    pub fn property_filter(mut self, key: String, value: String) -> Self {
        self.property_filters.insert(key, value);
        self
    }

    /// Set maximum number of results
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set sort direction
    pub fn ascending(mut self, ascending: bool) -> Self {
        self.ascending = ascending;
        self
    }

    /// Build the optimized query
    pub fn build(self) -> Result<OptimizedQuery> {
        let mut query = OptimizedQuery::new("temporal_entities".to_string());

        // Build key condition
        if let Some(entity_id) = self.entity_id {
            query = query.with_key_condition("entity_id = :eid".to_string());
            query = query.with_values(serde_json::json!({
                ":eid": entity_id.id.to_string(),
            }));
        }

        // Build filter expression
        let mut filter_values = serde_json::Map::new();
        let mut conditions = Vec::new();
        
        // Add temporal conditions
        if let Some(ts) = self.timestamp {
            conditions.push("valid_time_start <= :ts AND valid_time_end >= :ts".to_string());
            filter_values.insert(":ts".to_string(), serde_json::Value::String(ts.timestamp().to_string()));
        } else if let (Some(start), Some(end)) = (self.range_start, self.range_end) {
            conditions.push("valid_time_start <= :end AND valid_time_end >= :start".to_string());
            filter_values.insert(":start".to_string(), serde_json::Value::String(start.timestamp().to_string()));
            filter_values.insert(":end".to_string(), serde_json::Value::String(end.timestamp().to_string()));
        }

        // Add entity type filter
        if let Some(et) = self.entity_type {
            conditions.push("entity_type = :et".to_string());
            filter_values.insert(":et".to_string(), serde_json::Value::String(et.to_string()));
        }

        // Add property filters
        for (i, (key, value)) in self.property_filters.iter().enumerate() {
            let placeholder = format!(":pv{}", i);
            conditions.push(format!("{} = {}", key, placeholder).to_string());
            filter_values.insert(placeholder.to_string(), serde_json::Value::String(value.clone()));
        }

        if !conditions.is_empty() {
            query = query.with_filter(conditions.join(" AND "));
            query = query.with_values(serde_json::Value::Object(filter_values));
        }

        // Set limit and sort direction
        if let Some(limit) = self.limit {
            query = query.with_limit(limit);
        }
        query = query.with_scan_direction(self.ascending);

        Ok(query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_point_in_time_query() {
        let now = Utc::now();
        let entity_id = EntityId::new(EntityType::Node, "test-node".to_string());
        
        let query = TemporalQueryBuilder::new()
            .entity_id(entity_id)
            .at(now)
            .build()
            .unwrap();

        assert!(query.key_condition.contains("entity_id = :eid"));
        let filter = query.filter_expression.unwrap();
        assert!(filter.contains("valid_time_start <= :ts"));
        assert!(filter.contains("valid_time_end >= :ts"));
    }

    #[test]
    fn test_time_range_query() {
        let start = Utc::now();
        let end = start + Duration::hours(1);
        let entity_id = EntityId::new(EntityType::Node, "test-node".to_string());
        
        let query = TemporalQueryBuilder::new()
            .entity_id(entity_id)
            .between(start, end)
            .unwrap()
            .build()
            .unwrap();

        assert!(query.key_condition.contains("entity_id = :eid"));
        let filter = query.filter_expression.unwrap();
        assert!(filter.contains("valid_time_start <= :end"));
        assert!(filter.contains("valid_time_end >= :start"));
    }

    #[test]
    fn test_invalid_time_range() {
        let end = Utc::now();
        let start = end + Duration::hours(1);
        let entity_id = EntityId::new(EntityType::Node, "test-node".to_string());
        
        let result = TemporalQueryBuilder::new()
            .entity_id(entity_id)
            .between(start, end);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::InvalidTemporalRange(_)));
    }

    #[test]
    fn test_property_filters() {
        let now = Utc::now();
        let entity_id = EntityId::new(EntityType::Node, "test-node".to_string());
        
        let query = TemporalQueryBuilder::new()
            .entity_id(entity_id)
            .at(now)
            .property_filter("status".to_string(), "active".to_string())
            .build()
            .unwrap();

        let filter = query.filter_expression.unwrap();
        assert!(filter.contains("status = :pv0"));
    }
} 
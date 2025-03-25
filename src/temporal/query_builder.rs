use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

use crate::{
    error::{Error, Result},
    types::{EntityId, EntityType, TemporalRange, Timestamp},
};

use super::query::OptimizedQuery;

/// Property filter operator for comparing values
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PropertyOperator {
    /// Equal to
    Equal,
    /// Not equal to
    NotEqual,
    /// Greater than
    GreaterThan,
    /// Greater than or equal to
    GreaterThanOrEqual,
    /// Less than
    LessThan,
    /// Less than or equal to
    LessThanOrEqual,
    /// Contains substring
    Contains,
    /// Starts with substring
    StartsWith,
    /// Ends with substring
    EndsWith,
    /// In a set of values
    In,
    /// Not in a set of values
    NotIn,
}

impl fmt::Display for PropertyOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PropertyOperator::Equal => write!(f, "="),
            PropertyOperator::NotEqual => write!(f, "!="),
            PropertyOperator::GreaterThan => write!(f, ">"),
            PropertyOperator::GreaterThanOrEqual => write!(f, ">="),
            PropertyOperator::LessThan => write!(f, "<"),
            PropertyOperator::LessThanOrEqual => write!(f, "<="),
            PropertyOperator::Contains => write!(f, "contains"),
            PropertyOperator::StartsWith => write!(f, "begins_with"),
            PropertyOperator::EndsWith => write!(f, "ends_with"),
            PropertyOperator::In => write!(f, "IN"),
            PropertyOperator::NotIn => write!(f, "NOT IN"),
        }
    }
}

/// Property filter for querying entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyFilter {
    /// Property name
    pub property_name: String,
    /// Filter operator
    pub operator: PropertyOperator,
    /// Filter value
    pub value: serde_json::Value,
}

/// Relationship filter for querying connected entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipFilter {
    /// Relationship type
    pub relationship_type: String,
    /// Direction of the relationship
    pub direction: RelationshipDirection,
    /// Target entity type
    pub target_entity_type: Option<EntityType>,
    /// Property filters for the relationship
    pub property_filters: Vec<PropertyFilter>,
}

/// Direction of relationship for filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationshipDirection {
    /// Outgoing relationship
    Outgoing,
    /// Incoming relationship
    Incoming,
    /// Either direction
    Any,
}

/// Sort order for query results
#[derive(Debug, Clone)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Sort field for ordering results
#[derive(Debug, Clone)]
pub struct SortField {
    pub field: String,
    pub order: SortOrder,
}

/// Builder for constructing temporal queries
#[derive(Debug, Clone)]
pub struct TemporalQueryBuilder {
    /// Entity ID to query
    pub entity_id: Option<EntityId>,
    /// Entity type filter
    pub entity_type: Option<EntityType>,
    /// Point in time to query
    pub point_in_time: Option<DateTime<Utc>>,
    /// Time range start
    pub time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    /// Property filters
    pub property_filters: Vec<PropertyFilter>,
    /// Relationship filters
    pub relationship_filters: Vec<RelationshipFilter>,
    /// Sort fields
    pub sort_fields: Vec<SortField>,
    /// Maximum results to return
    pub limit: Option<usize>,
    /// Sort direction (true = ascending)
    pub ascending: bool,
    /// Page size for pagination
    pub page_size: Option<u32>,
    /// Page token for pagination
    pub page_token: Option<String>,
}

impl Default for TemporalQueryBuilder {
    fn default() -> Self {
        Self {
            entity_id: None,
            entity_type: None,
            point_in_time: None,
            time_range: None,
            property_filters: Vec::new(),
            relationship_filters: Vec::new(),
            sort_fields: Vec::new(),
            limit: None,
            ascending: true,
            page_size: None,
            page_token: None,
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
        self.point_in_time = Some(timestamp);
        self.time_range = None;
        self
    }

    /// Set a time range to query
    pub fn between(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Self> {
        if start > end {
            return Err(Error::InvalidTemporalRange(
                "Start time must be before end time".to_string(),
            ));
        }
        self.time_range = Some((start, end));
        self.point_in_time = None;
        Ok(self)
    }

    /// Add a property filter with a specific operator
    pub fn add_property_filter_with_operator(
        mut self,
        property: impl Into<String>,
        operator: PropertyOperator,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.property_filters.push(PropertyFilter {
            property_name: property.into(),
            operator,
            value: value.into(),
        });
        self
    }

    /// Add a relationship filter
    pub fn add_relationship_filter(
        mut self,
        relationship_type: impl Into<String>,
        direction: RelationshipDirection,
        target_type: Option<EntityType>,
        target_id: Option<impl Into<String>>,
    ) -> Self {
        self.relationship_filters.push(RelationshipFilter {
            relationship_type: relationship_type.into(),
            direction,
            target_entity_type: target_type,
            property_filters: Vec::new(),
        });
        self
    }

    /// Add a sort field
    pub fn add_sort_field(
        mut self,
        field: impl Into<String>,
        order: SortOrder,
    ) -> Self {
        self.sort_fields.push(SortField {
            field: field.into(),
            order,
        });
        self
    }

    /// Set the page size for pagination
    pub fn page_size(mut self, size: u32) -> Self {
        self.page_size = Some(size);
        self
    }

    /// Set the page token for pagination
    pub fn page_token(mut self, token: impl Into<String>) -> Self {
        self.page_token = Some(token.into());
        self
    }

    /// Build the optimized query with all filters and options
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
        if let Some(ts) = self.point_in_time {
            conditions.push("valid_time_start <= :ts AND valid_time_end >= :ts".to_string());
            filter_values.insert(":ts".to_string(), serde_json::Value::String(ts.timestamp().to_string()));
        } else if let Some((start, end)) = self.time_range {
            // Add time range conditions to filter expression, not key condition
            conditions.push("valid_time_start <= :end".to_string());
            conditions.push("valid_time_end >= :start".to_string());
            filter_values.insert(":start".to_string(), serde_json::Value::String(start.timestamp().to_string()));
            filter_values.insert(":end".to_string(), serde_json::Value::String(end.timestamp().to_string()));
        }

        // Add entity type filter
        if let Some(et) = self.entity_type {
            conditions.push("entity_type = :et".to_string());
            filter_values.insert(":et".to_string(), serde_json::Value::String(et.to_string()));
        }

        // Add property filters with operators
        for filter in self.property_filters {
            let condition = match filter.operator {
                PropertyOperator::Equal => format!("{} = :{}", filter.property_name, filter.property_name),
                PropertyOperator::NotEqual => format!("{} <> :{}", filter.property_name, filter.property_name),
                PropertyOperator::GreaterThan => format!("{} > :{}", filter.property_name, filter.property_name),
                PropertyOperator::LessThan => format!("{} < :{}", filter.property_name, filter.property_name),
                PropertyOperator::GreaterThanOrEqual => format!("{} >= :{}", filter.property_name, filter.property_name),
                PropertyOperator::LessThanOrEqual => format!("{} <= :{}", filter.property_name, filter.property_name),
                PropertyOperator::Contains => format!("contains({}, :{})", filter.property_name, filter.property_name),
                PropertyOperator::StartsWith => format!("begins_with({}, :{})", filter.property_name, filter.property_name),
                PropertyOperator::EndsWith => format!("ends_with({}, :{})", filter.property_name, filter.property_name),
                PropertyOperator::In => format!("{} IN :{}", filter.property_name, filter.property_name),
                PropertyOperator::NotIn => format!("NOT {} IN :{}", filter.property_name, filter.property_name),
            };
            conditions.push(condition);
            filter_values.insert(format!(":{}", filter.property_name), filter.value.clone());
        }

        // Add relationship filters
        if !self.relationship_filters.is_empty() {
            let mut relationship_conditions = Vec::new();
            for (i, filter) in self.relationship_filters.iter().enumerate() {
                let direction_condition = match filter.direction {
                    RelationshipDirection::Outgoing => format!("relationship_type = :rel_type_{} AND source_id = :entity_id", i),
                    RelationshipDirection::Incoming => format!("relationship_type = :rel_type_{} AND target_id = :entity_id", i),
                    RelationshipDirection::Any => format!("relationship_type = :rel_type_{} AND (source_id = :entity_id OR target_id = :entity_id)", i),
                };
                relationship_conditions.push(direction_condition);
                filter_values.insert(format!(":rel_type_{}", i), serde_json::Value::String(filter.relationship_type.clone()));

                if let Some(target_type) = &filter.target_entity_type {
                    filter_values.insert(format!(":target_type_{}", i), serde_json::Value::String(target_type.to_string()));
                    relationship_conditions.push(format!("target_type = :target_type_{}", i));
                }

                let mut property_conditions = Vec::new();
                for property_filter in &filter.property_filters {
                    let property_condition = format!("{} = :{}", property_filter.property_name, property_filter.property_name);
                    property_conditions.push(property_condition);
                    filter_values.insert(format!(":{}", property_filter.property_name), property_filter.value.clone());
                }
                relationship_conditions.push(format!("({})", property_conditions.join(" AND ")));
            }
            conditions.push(format!("({})", relationship_conditions.join(" AND ")));
        }

        if !conditions.is_empty() {
            query = query.with_filter(conditions.join(" AND "));
            query = query.with_values(serde_json::Value::Object(filter_values));
        }

        // Set limit and sort direction
        if let Some(limit) = self.limit {
            query = query.with_limit(limit as i32);
        }
        query = query.with_scan_direction(self.ascending);

        // Add sort fields
        if !self.sort_fields.is_empty() {
            let sort_conditions: Vec<String> = self.sort_fields
                .iter()
                .map(|sort| {
                    match sort.order {
                        SortOrder::Ascending => format!("{} ASC", sort.field),
                        SortOrder::Descending => format!("{} DESC", sort.field),
                    }
                })
                .collect();
            query = query.with_sort_key_condition(sort_conditions.join(", "));
        }

        // Add pagination
        if let Some(size) = self.page_size {
            query = query.with_limit(size as i32);
        }

        if let Some(token) = self.page_token {
            query = query.with_exclusive_start_key(token);
        }

        Ok(query)
    }

    /// Get property filters
    pub fn property_filters(&self) -> &[PropertyFilter] {
        &self.property_filters
    }

    /// Get relationship filters
    pub fn relationship_filters(&self) -> &[RelationshipFilter] {
        &self.relationship_filters
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

        assert!(query.key_condition.as_ref().unwrap().contains("entity_id = :eid"));
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

        assert!(query.key_condition.as_ref().unwrap().contains("entity_id = :eid"));
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
    fn test_query_builder_with_advanced_filters() {
        let now = Utc::now();
        let query = TemporalQueryBuilder::new()
            .entity_type(EntityType::Person)
            .between(now, now + chrono::Duration::days(1))
            .unwrap()
            .add_property_filter_with_operator(String::from("age"), PropertyOperator::GreaterThan, 25)
            .add_property_filter_with_operator(String::from("name"), PropertyOperator::Contains, String::from("John"))
            .add_relationship_filter(
                String::from("FRIEND_OF"),
                RelationshipDirection::Outgoing,
                Some(EntityType::Person),
                None::<String>,
            )
            .add_sort_field(String::from("name"), SortOrder::Ascending)
            .page_size(10)
            .build()
            .unwrap();

        assert!(query.filter_expression.as_ref().unwrap().contains("age > :age"));
        assert!(query.filter_expression.as_ref().unwrap().contains("contains(name, :name)"));
        assert!(query.filter_expression.as_ref().unwrap().contains("relationship_type = :rel_type_0"));
        assert_eq!(query.limit.unwrap(), 10);
    }
} 
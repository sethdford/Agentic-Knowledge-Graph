use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

use crate::{
    error::{Error, Result},
    types::TemporalRange,
};

use super::TemporalOperation;

/// Represents an optimized temporal query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizedQuery {
    /// DynamoDB table name
    pub table: String,
    /// Primary key condition
    pub key_condition: String,
    /// Filter expression
    pub filter_expression: Option<String>,
    /// Expression attribute values
    pub expression_values: serde_json::Value,
    /// Index to use
    pub index_name: Option<String>,
    /// Limit results
    pub limit: Option<u32>,
    /// Scan direction
    pub scan_forward: bool,
}

impl OptimizedQuery {
    /// Create a new optimized query
    pub fn new(table: String) -> Self {
        Self {
            table,
            key_condition: String::new(),
            filter_expression: None,
            expression_values: serde_json::json!({}),
            index_name: None,
            limit: None,
            scan_forward: true,
        }
    }

    /// Add a key condition
    pub fn with_key_condition(mut self, condition: String) -> Self {
        self.key_condition = condition;
        self
    }

    /// Add a filter expression
    pub fn with_filter(mut self, filter: String) -> Self {
        self.filter_expression = Some(filter);
        self
    }

    /// Add expression values
    pub fn with_values(mut self, values: serde_json::Value) -> Self {
        self.expression_values = values;
        self
    }

    /// Use a specific index
    pub fn with_index(mut self, index: String) -> Self {
        self.index_name = Some(index);
        self
    }

    /// Set result limit
    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set scan direction
    pub fn with_scan_direction(mut self, forward: bool) -> Self {
        self.scan_forward = forward;
        self
    }
}

/// Optimize a temporal query based on the operation
pub fn optimize_temporal_query(
    operation: &TemporalOperation,
    table: String,
) -> Result<OptimizedQuery> {
    let mut query = OptimizedQuery::new(table);

    match operation {
        TemporalOperation::At(timestamp) => {
            query = query
                .with_key_condition("valid_time_start <= :ts AND valid_time_end >= :ts".to_string())
                .with_values(serde_json::json!({
                    ":ts": timestamp.timestamp()
                }))
                .with_index("valid_time_index".to_string());
        }
        TemporalOperation::Between(start, end) => {
            if start > end {
                return Err(Error::InvalidTemporalRange(
                    "Start time must be before end time".to_string(),
                ));
            }

            query = query
                .with_key_condition(
                    "valid_time_start <= :end AND valid_time_end >= :start".to_string(),
                )
                .with_values(serde_json::json!({
                    ":start": start.timestamp(),
                    ":end": end.timestamp()
                }))
                .with_index("valid_time_index".to_string());
        }
        TemporalOperation::Evolution(range) => {
            if let (Some(start), Some(end)) = (range.start, range.end) {
                if start.0 > end.0 {
                    return Err(Error::InvalidTemporalRange(
                        "Start time must be before end time".to_string(),
                    ));
                }

                query = query
                    .with_key_condition(
                        "valid_time_start >= :start AND valid_time_start <= :end".to_string(),
                    )
                    .with_values(serde_json::json!({
                        ":start": start.0.timestamp(),
                        ":end": end.0.timestamp()
                    }))
                    .with_index("valid_time_index".to_string())
                    .with_scan_direction(true);
            }
        }
        TemporalOperation::Latest => {
            query = query
                .with_key_condition("transaction_time_end IS NULL".to_string())
                .with_index("transaction_time_index".to_string())
                .with_limit(1)
                .with_scan_direction(false);
        }
    }

    Ok(query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_optimize_at_query() {
        let now = Utc::now();
        let operation = TemporalOperation::At(now);
        let query = optimize_temporal_query(&operation, "test_table".to_string()).unwrap();

        assert!(query.key_condition.contains("valid_time_start"));
        assert!(query.key_condition.contains("valid_time_end"));
        assert_eq!(query.index_name, Some("valid_time_index".to_string()));
    }

    #[test]
    fn test_optimize_between_query() {
        let start = Utc::now();
        let end = start + Duration::hours(1);
        let operation = TemporalOperation::Between(start, end);
        let query = optimize_temporal_query(&operation, "test_table".to_string()).unwrap();

        assert!(query.key_condition.contains("valid_time_start"));
        assert!(query.key_condition.contains("valid_time_end"));
        assert_eq!(query.index_name, Some("valid_time_index".to_string()));
    }

    #[test]
    fn test_optimize_evolution_query() {
        let now = Utc::now();
        let range = TemporalRange {
            start: Some(crate::types::Timestamp(now)),
            end: Some(crate::types::Timestamp(now + Duration::hours(1))),
        };
        let operation = TemporalOperation::Evolution(range);
        let query = optimize_temporal_query(&operation, "test_table".to_string()).unwrap();

        assert!(query.key_condition.contains("valid_time_start"));
        assert_eq!(query.index_name, Some("valid_time_index".to_string()));
        assert!(query.scan_forward);
    }

    #[test]
    fn test_optimize_latest_query() {
        let operation = TemporalOperation::Latest;
        let query = optimize_temporal_query(&operation, "test_table".to_string()).unwrap();

        assert!(query.key_condition.contains("transaction_time_end"));
        assert_eq!(query.index_name, Some("transaction_time_index".to_string()));
        assert_eq!(query.limit, Some(1));
        assert!(!query.scan_forward);
    }

    #[test]
    fn test_invalid_temporal_range() {
        let end = Utc::now();
        let start = end + Duration::hours(1);
        let operation = TemporalOperation::Between(start, end);
        let result = optimize_temporal_query(&operation, "test_table".to_string());

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::InvalidTemporalRange(_)));
    }
} 
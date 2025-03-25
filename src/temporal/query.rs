use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use serde_json::Value;

use crate::{
    error::{Error, Result},
    types::TemporalRange,
};

use super::TemporalOperation;

/// Optimized query for temporal data
#[derive(Clone, Debug)]
pub struct OptimizedQuery {
    /// Table name
    pub table_name: String,
    /// Key condition expression
    pub key_condition: Option<String>,
    /// Filter expression
    pub filter_expression: Option<String>,
    /// Expression values
    pub expression_values: Option<Value>,
    /// Scan direction (true for ascending, false for descending)
    pub scan_direction: Option<bool>,
    /// Limit
    pub limit: Option<i32>,
}

impl OptimizedQuery {
    /// Create a new optimized query
    pub fn new(table_name: String) -> Self {
        Self {
            table_name,
            key_condition: None,
            filter_expression: None,
            expression_values: None,
            scan_direction: None,
            limit: None,
        }
    }

    /// Set key condition
    pub fn with_key_condition(mut self, condition: String) -> Self {
        self.key_condition = Some(condition);
        self
    }

    /// Set filter expression
    pub fn with_filter(mut self, filter: String) -> Self {
        self.filter_expression = Some(filter);
        self
    }

    /// Set expression values
    pub fn with_values(mut self, values: Value) -> Self {
        self.expression_values = Some(values);
        self
    }

    /// Set scan direction
    pub fn with_scan_direction(mut self, ascending: bool) -> Self {
        self.scan_direction = Some(ascending);
        self
    }

    /// Set limit
    pub fn with_limit(mut self, limit: i32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set sort key condition
    pub fn with_sort_key_condition(mut self, condition: String) -> Self {
        // Sort key condition is part of the key condition
        if let Some(existing) = self.key_condition {
            self.key_condition = Some(format!("{} AND {}", existing, condition));
        } else {
            self.key_condition = Some(condition);
        }
        self
    }

    /// Set exclusive start key
    pub fn with_exclusive_start_key(mut self, token: String) -> Self {
        // Store the token in expression values
        let mut values = self.expression_values.unwrap_or(serde_json::json!({}));
        if let serde_json::Value::Object(ref mut map) = values {
            map.insert(":exclusive_start_key".to_string(), serde_json::Value::String(token));
        }
        self.expression_values = Some(values);
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
                .with_scan_direction(true);
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
                .with_scan_direction(true);
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
                    .with_scan_direction(true);
            }
        }
        TemporalOperation::Latest => {
            query = query
                .with_key_condition("transaction_time_end IS NULL".to_string())
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

        assert!(query.key_condition.is_some());
        assert!(query.key_condition.as_ref().unwrap().contains("valid_time_start"));
        assert!(query.key_condition.as_ref().unwrap().contains("valid_time_end"));
        assert!(query.scan_direction.is_some());
        assert!(query.scan_direction.unwrap());
    }

    #[test]
    fn test_optimize_between_query() {
        let start = Utc::now();
        let end = start + Duration::hours(1);
        let operation = TemporalOperation::Between(start, end);
        let query = optimize_temporal_query(&operation, "test_table".to_string()).unwrap();

        assert!(query.key_condition.is_some());
        assert!(query.key_condition.as_ref().unwrap().contains("valid_time_start"));
        assert!(query.key_condition.as_ref().unwrap().contains("valid_time_end"));
        assert!(query.scan_direction.is_some());
        assert!(query.scan_direction.unwrap());
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

        assert!(query.key_condition.is_some());
        assert!(query.key_condition.as_ref().unwrap().contains("valid_time_start"));
        assert!(query.scan_direction.is_some());
        assert!(query.scan_direction.unwrap());
    }

    #[test]
    fn test_optimize_latest_query() {
        let operation = TemporalOperation::Latest;
        let query = optimize_temporal_query(&operation, "test_table".to_string()).unwrap();

        assert!(query.key_condition.is_some());
        assert!(query.key_condition.as_ref().unwrap().contains("transaction_time_end"));
        assert!(query.scan_direction.is_some());
        assert!(!query.scan_direction.unwrap());
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
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

use crate::{
    error::{Error, Result},
    types::{EntityId, EntityType, TemporalRange},
};

use super::TemporalIndexEntry;

/// Represents a consistency check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyCheckResult {
    /// Whether the check passed
    pub passed: bool,
    /// List of violations found
    pub violations: Vec<ConsistencyViolation>,
}

/// Represents a consistency violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyViolation {
    /// Type of violation
    pub violation_type: ConsistencyViolationType,
    /// Description of the violation
    pub description: String,
    /// Entity ID involved
    pub entity_id: EntityId,
    /// Timestamp when violation occurred
    pub timestamp: DateTime<Utc>,
}

/// Types of consistency violations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsistencyViolationType {
    /// Temporal overlap between entries
    TemporalOverlap,
    /// Gap in temporal coverage
    TemporalGap,
    /// Invalid temporal range
    InvalidRange,
    /// Missing required data
    MissingData,
    /// Inconsistent transaction time
    TransactionTimeInconsistency,
}

/// Consistency checker for temporal operations
pub struct ConsistencyChecker {
    /// Cache of validated ranges
    validated_ranges: RwLock<HashMap<EntityId, Vec<(DateTime<Utc>, DateTime<Utc>)>>>,
}

impl ConsistencyChecker {
    /// Create a new consistency checker
    pub fn new() -> Self {
        Self {
            validated_ranges: RwLock::new(HashMap::new()),
        }
    }

    /// Check temporal consistency for a set of entries
    pub async fn check_consistency(&self, entries: &[TemporalIndexEntry]) -> Result<ConsistencyCheckResult> {
        let mut violations = Vec::new();
        
        // Check for temporal overlaps
        self.check_temporal_overlaps(entries, &mut violations).await?;
        
        // Check for temporal gaps
        self.check_temporal_gaps(entries, &mut violations).await?;
        
        // Check transaction time consistency
        self.check_transaction_time_consistency(entries, &mut violations).await?;
        
        Ok(ConsistencyCheckResult {
            passed: violations.is_empty(),
            violations,
        })
    }

    /// Check for temporal overlaps between entries
    async fn check_temporal_overlaps(
        &self,
        entries: &[TemporalIndexEntry],
        violations: &mut Vec<ConsistencyViolation>,
    ) -> Result<()> {
        for (i, entry1) in entries.iter().enumerate() {
            for entry2 in entries.iter().skip(i + 1) {
                if entry1.entity_id == entry2.entity_id && 
                   entry1.is_current() && 
                   entry2.is_current() {
                    if (entry1.valid_time_start <= entry2.valid_time_end && 
                        entry1.valid_time_end >= entry2.valid_time_start) {
                        violations.push(ConsistencyViolation {
                            violation_type: ConsistencyViolationType::TemporalOverlap,
                            description: format!(
                                "Temporal overlap between versions {} and {}",
                                entry1.version_id, entry2.version_id
                            ),
                            entity_id: entry1.entity_id.clone(),
                            timestamp: entry1.valid_time_start,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    /// Check for temporal gaps in coverage
    async fn check_temporal_gaps(
        &self,
        entries: &[TemporalIndexEntry],
        violations: &mut Vec<ConsistencyViolation>,
    ) -> Result<()> {
        let mut entity_ranges: HashMap<EntityId, Vec<(DateTime<Utc>, DateTime<Utc>)>> = HashMap::new();
        
        // Group ranges by entity
        for entry in entries.iter().filter(|e| e.is_current()) {
            entity_ranges
                .entry(entry.entity_id.clone())
                .or_insert_with(Vec::new)
                .push((entry.valid_time_start, entry.valid_time_end));
        }
        
        // Check for gaps in each entity's timeline
        for (entity_id, mut ranges) in entity_ranges {
            ranges.sort_by(|a, b| a.0.cmp(&b.0));
            
            for window in ranges.windows(2) {
                if let [range1, range2] = window {
                    if range1.1 < range2.0 {
                        violations.push(ConsistencyViolation {
                            violation_type: ConsistencyViolationType::TemporalGap,
                            description: format!(
                                "Temporal gap between {} and {}",
                                range1.1, range2.0
                            ),
                            entity_id: entity_id.clone(),
                            timestamp: range1.1,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    /// Check transaction time consistency
    async fn check_transaction_time_consistency(
        &self,
        entries: &[TemporalIndexEntry],
        violations: &mut Vec<ConsistencyViolation>,
    ) -> Result<()> {
        for entry in entries {
            // Check that transaction_time_start is before transaction_time_end
            if let Some(end_time) = entry.transaction_time_end {
                if end_time <= entry.transaction_time_start {
                    violations.push(ConsistencyViolation {
                        violation_type: ConsistencyViolationType::TransactionTimeInconsistency,
                        description: "Transaction end time is before or equal to start time".to_string(),
                        entity_id: entry.entity_id.clone(),
                        timestamp: entry.transaction_time_start,
                    });
                }
            }
            
            // Check that valid time range is valid
            if entry.valid_time_end <= entry.valid_time_start {
                violations.push(ConsistencyViolation {
                    violation_type: ConsistencyViolationType::InvalidRange,
                    description: "Valid time end is before or equal to start time".to_string(),
                    entity_id: entry.entity_id.clone(),
                    timestamp: entry.valid_time_start,
                });
            }
        }
        Ok(())
    }

    /// Validate and cache a temporal range
    pub async fn validate_range(
        &self,
        entity_id: &EntityId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<()> {
        if start >= end {
            return Err(Error::InvalidTemporalRange(
                "Start time must be before end time".to_string(),
            ));
        }

        let mut ranges = self.validated_ranges.write().await;
        let entity_ranges = ranges.entry(entity_id.clone()).or_insert_with(Vec::new);
        
        // Check for overlaps with validated ranges
        for (existing_start, existing_end) in entity_ranges.iter() {
            if start <= *existing_end && end >= *existing_start {
                return Err(Error::TemporalOverlap(
                    "Range overlaps with previously validated range".to_string(),
                ));
            }
        }
        
        entity_ranges.push((start, end));
        entity_ranges.sort_by(|a, b| a.0.cmp(&b.0));
        
        Ok(())
    }

    /// Clear validated ranges for an entity
    pub async fn clear_validated_ranges(&self, entity_id: &EntityId) {
        let mut ranges = self.validated_ranges.write().await;
        ranges.remove(entity_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use uuid::Uuid;

    fn create_test_entry(
        entity_id: EntityId,
        valid_start: DateTime<Utc>,
        valid_end: DateTime<Utc>,
        transaction_start: DateTime<Utc>,
        transaction_end: Option<DateTime<Utc>>,
    ) -> TemporalIndexEntry {
        TemporalIndexEntry {
            entity_id,
            valid_time_start: valid_start,
            valid_time_end: valid_end,
            transaction_time_start: transaction_start,
            transaction_time_end: transaction_end,
            version_id: Uuid::new_v4(),
        }
    }

    #[tokio::test]
    async fn test_temporal_overlap_detection() {
        let checker = ConsistencyChecker::new();
        let now = Utc::now();
        let entity_id = EntityId::new(EntityType::Node, "test-node".to_string());
        
        let entries = vec![
            create_test_entry(
                entity_id.clone(),
                now,
                now + Duration::hours(1),
                now,
                None,
            ),
            create_test_entry(
                entity_id.clone(),
                now + Duration::minutes(30),
                now + Duration::hours(2),
                now,
                None,
            ),
        ];
        
        let result = checker.check_consistency(&entries).await.unwrap();
        assert!(!result.passed);
        assert_eq!(result.violations.len(), 1);
        assert!(matches!(
            result.violations[0].violation_type,
            ConsistencyViolationType::TemporalOverlap
        ));
    }

    #[tokio::test]
    async fn test_temporal_gap_detection() {
        let checker = ConsistencyChecker::new();
        let now = Utc::now();
        let entity_id = EntityId::new(EntityType::Node, "test-node".to_string());
        
        let entries = vec![
            create_test_entry(
                entity_id.clone(),
                now,
                now + Duration::hours(1),
                now,
                None,
            ),
            create_test_entry(
                entity_id.clone(),
                now + Duration::hours(2),
                now + Duration::hours(3),
                now,
                None,
            ),
        ];
        
        let result = checker.check_consistency(&entries).await.unwrap();
        assert!(!result.passed);
        assert_eq!(result.violations.len(), 1);
        assert!(matches!(
            result.violations[0].violation_type,
            ConsistencyViolationType::TemporalGap
        ));
    }

    #[tokio::test]
    async fn test_transaction_time_consistency() {
        let checker = ConsistencyChecker::new();
        let now = Utc::now();
        let entity_id = EntityId::new(EntityType::Node, "test-node".to_string());
        
        let entries = vec![
            create_test_entry(
                entity_id.clone(),
                now,
                now + Duration::hours(1),
                now,
                Some(now - Duration::hours(1)),
            ),
        ];
        
        let result = checker.check_consistency(&entries).await.unwrap();
        assert!(!result.passed);
        assert_eq!(result.violations.len(), 1);
        assert!(matches!(
            result.violations[0].violation_type,
            ConsistencyViolationType::TransactionTimeInconsistency
        ));
    }

    #[tokio::test]
    async fn test_range_validation() {
        let checker = ConsistencyChecker::new();
        let now = Utc::now();
        let entity_id = EntityId::new(EntityType::Node, "test-node".to_string());
        
        // Valid range
        assert!(checker
            .validate_range(
                &entity_id,
                now,
                now + Duration::hours(1),
            )
            .await
            .is_ok());
        
        // Overlapping range
        assert!(checker
            .validate_range(
                &entity_id,
                now + Duration::minutes(30),
                now + Duration::hours(2),
            )
            .await
            .is_err());
        
        // Invalid range (start >= end)
        assert!(checker
            .validate_range(
                &entity_id,
                now + Duration::hours(1),
                now,
            )
            .await
            .is_err());
    }
} 
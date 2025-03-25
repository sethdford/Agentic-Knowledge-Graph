use std::collections::BTreeMap;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    error::{Error, Result},
    types::{EntityId, EntityType, TemporalRange, Timestamp},
};

/// Represents a temporal index entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalIndexEntry {
    /// Entity ID
    pub entity_id: EntityId,
    /// Valid time start
    pub valid_time_start: DateTime<Utc>,
    /// Valid time end
    pub valid_time_end: DateTime<Utc>,
    /// Transaction time start
    pub transaction_time_start: DateTime<Utc>,
    /// Transaction time end (None if current)
    pub transaction_time_end: Option<DateTime<Utc>>,
    /// Version identifier
    pub version_id: Uuid,
}

impl TemporalIndexEntry {
    /// Create a new temporal index entry
    pub fn new(
        entity_id: EntityId,
        valid_time_start: DateTime<Utc>,
        valid_time_end: DateTime<Utc>,
        transaction_time_start: DateTime<Utc>,
    ) -> Self {
        Self {
            entity_id,
            valid_time_start,
            valid_time_end,
            transaction_time_start,
            transaction_time_end: None,
            version_id: Uuid::new_v4(),
        }
    }

    /// Check if this entry is valid at the given timestamp
    pub fn is_valid_at(&self, timestamp: &DateTime<Utc>) -> bool {
        self.valid_time_start <= *timestamp && self.valid_time_end >= *timestamp
    }

    /// Check if this entry is current (not superseded)
    pub fn is_current(&self) -> bool {
        self.transaction_time_end.is_none()
    }
}

/// Temporal index for managing temporal data
pub struct TemporalIndex {
    /// Index entries organized by entity ID
    entries: RwLock<BTreeMap<EntityId, Vec<TemporalIndexEntry>>>,
}

impl TemporalIndex {
    /// Create a new temporal index
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(BTreeMap::new()),
        }
    }

    /// Add a new entry to the index
    pub async fn add_entry(&self, entry: TemporalIndexEntry) -> Result<()> {
        let mut entries = self.entries.write().await;
        
        // Get or create vector for this entity
        let entity_entries = entries
            .entry(entry.entity_id.clone())
            .or_insert_with(Vec::new);

        // Validate temporal consistency
        for existing in entity_entries.iter() {
            if existing.is_current() && 
               ((entry.valid_time_start >= existing.valid_time_start && 
                 entry.valid_time_start <= existing.valid_time_end) ||
                (entry.valid_time_end >= existing.valid_time_start && 
                 entry.valid_time_end <= existing.valid_time_end)) {
                return Err(Error::TemporalOverlap(
                    "New entry overlaps with existing temporal range".to_string(),
                ));
            }
        }

        entity_entries.push(entry);
        Ok(())
    }

    /// Get entries valid at a specific timestamp
    pub async fn get_at(&self, entity_id: &EntityId, timestamp: &DateTime<Utc>) -> Result<Vec<TemporalIndexEntry>> {
        let entries = self.entries.read().await;
        
        Ok(entries
            .get(entity_id)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|entry| entry.is_valid_at(timestamp) && entry.is_current())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Get entries within a temporal range
    pub async fn get_between(
        &self,
        entity_id: &EntityId,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> Result<Vec<TemporalIndexEntry>> {
        if start > end {
            return Err(Error::InvalidTemporalRange(
                "Start time must be before end time".to_string(),
            ));
        }

        let entries = self.entries.read().await;
        
        Ok(entries
            .get(entity_id)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|entry| {
                        entry.is_current() &&
                        entry.valid_time_start <= *end &&
                        entry.valid_time_end >= *start
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Get the evolution of an entity over time
    pub async fn get_evolution(
        &self,
        entity_id: &EntityId,
        range: &TemporalRange,
    ) -> Result<Vec<TemporalIndexEntry>> {
        let entries = self.entries.read().await;
        
        Ok(entries
            .get(entity_id)
            .map(|entries| {
                let mut filtered: Vec<_> = entries
                    .iter()
                    .filter(|entry| {
                        entry.is_current() &&
                        range.start.map_or(true, |start| entry.valid_time_start >= start.0) &&
                        range.end.map_or(true, |end| entry.valid_time_start <= end.0)
                    })
                    .cloned()
                    .collect();
                
                filtered.sort_by(|a, b| a.valid_time_start.cmp(&b.valid_time_start));
                filtered
            })
            .unwrap_or_default())
    }

    /// Get the latest version of an entity
    pub async fn get_latest(&self, entity_id: &EntityId) -> Result<Option<TemporalIndexEntry>> {
        let entries = self.entries.read().await;
        
        Ok(entries
            .get(entity_id)
            .and_then(|entries| {
                entries
                    .iter()
                    .filter(|entry| entry.is_current())
                    .max_by_key(|entry| entry.valid_time_end)
                    .cloned()
            }))
    }

    /// Supersede an entry with a new version
    pub async fn supersede(
        &self,
        entity_id: &EntityId,
        version_id: &Uuid,
        transaction_time_end: DateTime<Utc>,
    ) -> Result<()> {
        let mut entries = self.entries.write().await;
        
        if let Some(entity_entries) = entries.get_mut(entity_id) {
            if let Some(entry) = entity_entries
                .iter_mut()
                .find(|e| e.version_id == *version_id && e.is_current())
            {
                entry.transaction_time_end = Some(transaction_time_end);
                Ok(())
            } else {
                Err(Error::VersionNotFound(
                    "Version not found or already superseded".to_string(),
                ))
            }
        } else {
            Err(Error::EntityNotFound(
                "Entity not found in temporal index".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[tokio::test]
    async fn test_add_and_get_entry() {
        let index = TemporalIndex::new();
        let now = Utc::now();
        let entity_id = EntityId::new(EntityType::Node, "test-node".to_string());
        
        let entry = TemporalIndexEntry::new(
            entity_id.clone(),
            now,
            now + Duration::hours(1),
            now,
        );
        
        index.add_entry(entry.clone()).await.unwrap();
        
        let result = index.get_at(&entity_id, &now).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].version_id, entry.version_id);
    }

    #[tokio::test]
    async fn test_temporal_overlap_detection() {
        let index = TemporalIndex::new();
        let now = Utc::now();
        let entity_id = EntityId::new(EntityType::Node, "test-node".to_string());
        
        let entry1 = TemporalIndexEntry::new(
            entity_id.clone(),
            now,
            now + Duration::hours(1),
            now,
        );
        
        let entry2 = TemporalIndexEntry::new(
            entity_id.clone(),
            now + Duration::minutes(30),
            now + Duration::hours(2),
            now,
        );
        
        index.add_entry(entry1).await.unwrap();
        let result = index.add_entry(entry2).await;
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::TemporalOverlap(_)));
    }

    #[tokio::test]
    async fn test_get_between() {
        let index = TemporalIndex::new();
        let now = Utc::now();
        let entity_id = EntityId::new(EntityType::Node, "test-node".to_string());
        
        let entry = TemporalIndexEntry::new(
            entity_id.clone(),
            now,
            now + Duration::hours(1),
            now,
        );
        
        index.add_entry(entry).await.unwrap();
        
        let result = index
            .get_between(
                &entity_id,
                &(now + Duration::minutes(30)),
                &(now + Duration::minutes(45)),
            )
            .await
            .unwrap();
            
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_get_evolution() {
        let index = TemporalIndex::new();
        let now = Utc::now();
        let entity_id = EntityId::new(EntityType::Node, "test-node".to_string());
        
        let entry1 = TemporalIndexEntry::new(
            entity_id.clone(),
            now,
            now + Duration::hours(1),
            now,
        );
        
        let entry2 = TemporalIndexEntry::new(
            entity_id.clone(),
            now + Duration::hours(2),
            now + Duration::hours(3),
            now,
        );
        
        index.add_entry(entry1).await.unwrap();
        index.add_entry(entry2).await.unwrap();
        
        let range = TemporalRange {
            start: Some(Timestamp(now)),
            end: Some(Timestamp(now + Duration::hours(3))),
        };
        
        let result = index.get_evolution(&entity_id, &range).await.unwrap();
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn test_supersede() {
        let index = TemporalIndex::new();
        let now = Utc::now();
        let entity_id = EntityId::new(EntityType::Node, "test-node".to_string());
        
        let entry = TemporalIndexEntry::new(
            entity_id.clone(),
            now,
            now + Duration::hours(1),
            now,
        );
        let version_id = entry.version_id;
        
        index.add_entry(entry).await.unwrap();
        
        index
            .supersede(&entity_id, &version_id, now + Duration::hours(2))
            .await
            .unwrap();
            
        // Check that the entry exists and transaction_time_end is set
        let entries = index.entries.read().await;
        let entry = entries.get(&entity_id)
            .and_then(|entries| entries.iter().find(|e| e.version_id == version_id))
            .unwrap();
        
        assert!(entry.transaction_time_end.is_some());
        assert_eq!(entry.transaction_time_end.unwrap(), now + Duration::hours(2));
    }
} 
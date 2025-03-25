use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    error::Result,
    memory::{Memory, MemoryEntry},
    types::{EntityType, TemporalRange},
};

/// Mock implementation of the Memory trait for testing
pub struct MockMemory {}

impl MockMemory {
    /// Create a new MockMemory instance
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Memory for MockMemory {
    async fn store(&self, _entry: MemoryEntry) -> Result<()> {
        // Mock implementation - does nothing
        Ok(())
    }
    
    async fn store_bulk(&self, _entries: Vec<MemoryEntry>) -> Result<()> {
        // Mock implementation - does nothing
        Ok(())
    }
    
    async fn search_similar(&self, _embedding: Vec<f32>, _k: usize, _filter: Option<Value>) -> Result<Vec<MemoryEntry>> {
        // Return empty vec for mock implementation
        Ok(Vec::new())
    }
    
    async fn get_by_node_type(&self, _node_type: EntityType, _limit: usize) -> Result<Vec<MemoryEntry>> {
        // Return empty vec for mock implementation
        Ok(Vec::new())
    }
    
    async fn get_by_time_range(&self, _range: TemporalRange, _limit: usize) -> Result<Vec<MemoryEntry>> {
        // Return empty vec for mock implementation
        Ok(Vec::new())
    }
    
    async fn get_for_node(&self, _node_id: Uuid, _limit: usize) -> Result<Vec<MemoryEntry>> {
        // Return empty vec for mock implementation
        Ok(Vec::new())
    }
    
    async fn get_for_edge(&self, _source_id: Uuid, _target_id: Uuid, _limit: usize) -> Result<Vec<MemoryEntry>> {
        // Return empty vec for mock implementation
        Ok(Vec::new())
    }
} 
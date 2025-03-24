//! Zep Temporal Knowledge Graph
//! 
//! A temporal knowledge graph system that provides dynamic memory capabilities for AI agents.
//! Built on AWS managed services with bi-temporal model support.

pub mod api;
pub mod aws;
pub mod config;
pub mod context;
pub mod error;
pub mod graph;
pub mod memory;
pub mod mcp;
pub mod rag;
pub mod temporal;
pub mod types;

pub use crate::{
    config::Config,
    context::Context,
    error::{Error, Result},
};

pub use types::{EntityId, Timestamp, TemporalRange};
pub use graph::Graph;
pub use types::{Node, Edge, NodeId, EdgeId};
pub use temporal::{DynamoDBTemporal as TemporalGraphStore, TemporalIndex, TemporalIndexEntry};
pub use memory::{MemorySystem, MemoryEntry, Memory};
pub use rag::{RAGSystem, RAGConfig, ExtractedEntity, DetectedRelationship};

/// Re-export common types
pub use types::Properties;

/// Version of the library
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
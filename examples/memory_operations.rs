use chrono::Utc;
use graph::{
    Config,
    error::Result,
    memory::{MemoryEntry, MemorySystem, OpenSearchMemory},
    types::EntityType,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize configuration
    let config = Config::default();
    
    // Initialize memory system
    println!("Initializing memory system...");
    let opensearch_memory = OpenSearchMemory::new(&config).await?;
    
    // Create memory system with 3 arguments:
    // 1. Memory storage (Arc<impl MemoryOperations>)
    // 2. Index name (String)
    // 3. Embedding dimension (usize)
    let memory = MemorySystem::new(
        Arc::new(opensearch_memory),
        "example_memory".to_string(),
        768 // embedding dimension
    ).await?;
    
    // Create a memory entry 
    let id = Uuid::new_v4();
    let node_type = EntityType::new("Person");
    
    let mut metadata = HashMap::new();
    metadata.insert("metadata".to_string(), json!({
        "id": id.to_string(),
        "type": "Person",
        "name": "John Doe"
    }));
    
    // Create and store memory entry
    println!("Storing memory entry...");
    let entry = MemoryEntry {
        id: id.to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        content: "John Doe is a software engineer with expertise in Rust programming.",
        metadata,
        embedding: Some(vec![0.1f32; 768]), // Dummy embedding
        node_type: Some(node_type),
    };
    
    memory.store(entry).await?;
    println!("Memory entry stored successfully!");
    
    // Retrieve the entry - Note: MemorySystem.get() expects a String, not &str
    println!("Retrieving the memory entry...");
    let retrieved_entry = memory.get(id.to_string()).await?;
    
    if let Some(entry) = retrieved_entry {
        println!("Retrieved entry: {}", entry.content);
        println!("Created at: {}", entry.created_at);
        println!("Metadata: {:?}", entry.metadata);
    } else {
        println!("Entry not found!");
    }
    
    // Demonstrate searching for similar entries
    println!("\nSearching for similar entries...");
    let query_embedding = vec![0.1f32; 768]; // Similar to our stored entry
    let results = memory.search_similar(query_embedding, 5, 0.0).await?;
    
    println!("Found {} similar entries:", results.len());
    for (i, result) in results.iter().enumerate() {
        println!("Result {}: {}", i+1, result.content);
        println!("  Score: {}", result.score.unwrap_or(0.0));
        println!("  Type: {:?}", result.node_type);
    }
    
    println!("\nOperation completed successfully!");
    Ok(())
} 
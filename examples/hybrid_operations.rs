//! Example demonstrating hybrid vector+graph operations
//! 
//! This example shows how to use the hybrid architecture to combine graph traversal
//! with vector similarity for more powerful knowledge retrieval.

use chrono::{DateTime, Utc};
use graph::{
    Config, 
    error::Result,
    graph::{Graph, Node, Edge, NodeId, EdgeId, TemporalRange},
    graph::neptune::NeptuneGraph,
    memory::{MemoryEntry, MemorySystem, OpenSearchMemory},
    types::{EntityType, Properties, Timestamp},
};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize configuration and services
    println!("Initializing services for hybrid operation example...");
    
    // Load configuration
    let config = Config::default();
    
    // Initialize graph database - using concrete NeptuneGraph type
    let graph = graph::graph::new_graph(&config).await?;
    
    // Initialize vector store (OpenSearch)
    let opensearch_memory = OpenSearchMemory::new(&config).await?;
    let memory = MemorySystem::new(
        Arc::new(opensearch_memory),
        "example_memory".to_string(),
        768 // embedding dimension
    ).await?;
    
    // Create some example data
    println!("Creating example data...");
    
    // Create nodes in graph database
    let node1_id = graph.create_node(Node {
        id: NodeId(Uuid::new_v4()),
        label: "Product".to_string(),
        entity_type: EntityType::new("Product"),
        properties: Properties::from_json(json!({
            "name": "Smartphone XYZ",
            "price": 799.99,
            "released": "2023-06-15"
        })),
        valid_time: TemporalRange::new(
            Some(Timestamp(Utc::now())),
            None,
        ),
        transaction_time: TemporalRange::new(
            Some(Timestamp(Utc::now())),
            None,
        ),
    }).await?;
    
    let node2_id = graph.create_node(Node {
        id: NodeId(Uuid::new_v4()),
        label: "Category".to_string(),
        entity_type: EntityType::new("Category"),
        properties: Properties::from_json(json!({
            "name": "Electronics",
            "description": "Electronic devices and gadgets"
        })),
        valid_time: TemporalRange::new(
            Some(Timestamp(Utc::now())),
            None,
        ),
        transaction_time: TemporalRange::new(
            Some(Timestamp(Utc::now())),
            None,
        ),
    }).await?;
    
    // Create an edge between nodes
    let edge_id = graph.create_edge(Edge {
        id: EdgeId(Uuid::new_v4()),
        label: "BELONGS_TO".to_string(),
        source_id: node1_id,
        target_id: node2_id,
        properties: Properties::from_json(json!({
            "since": "2023-06-15"
        })),
        valid_time: TemporalRange::new(
            Some(Timestamp(Utc::now())),
            None,
        ),
        transaction_time: TemporalRange::new(
            Some(Timestamp(Utc::now())),
            None,
        ),
    }).await?;
    
    // Store the same data in vector database with embeddings
    // In a real implementation, these would be actual embeddings from a model
    let embedding1 = vec![0.1f32; 768]; // Dummy embedding
    let mut metadata1 = HashMap::new();
    metadata1.insert("metadata".to_string(), json!({
        "id": node1_id.0.to_string(),
        "type": "node",
        "entity_type": "Product",
        "label": "Product"
    }));
    
    memory.store(MemoryEntry {
        id: node1_id.0.to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        content: "Smartphone XYZ is a high-end mobile device with advanced features released in 2023.".to_string(),
        metadata: metadata1,
        embedding: Some(embedding1),
        node_type: Some(EntityType::new("Product")),
    }).await?;
    
    let embedding2 = vec![0.2f32; 768]; // Dummy embedding
    let mut metadata2 = HashMap::new();
    metadata2.insert("metadata".to_string(), json!({
        "id": node2_id.0.to_string(),
        "type": "node",
        "entity_type": "Category",
        "label": "Category"
    }));
    
    memory.store(MemoryEntry {
        id: node2_id.0.to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        content: "Electronics category includes various electronic devices and gadgets.".to_string(),
        metadata: metadata2,
        embedding: Some(embedding2),
        node_type: Some(EntityType::new("Category")),
    }).await?;
    
    // Demonstrate hybrid queries - manually combining graph and vector operations
    println!("\nDemonstrating hybrid query capabilities:");
    
    // 1. Graph query - get node by ID
    let node1 = graph.get_node(node1_id).await?;
    println!("\n1. Graph Query - Get node by ID:");
    println!("   Node: {} ({})", node1.label, node1.id.0);
    println!("   Properties: {}", serde_json::to_string_pretty(&node1.properties)?);
    
    // 2. Vector query - find similar content
    let query_embedding = vec![0.15f32; 768]; // Dummy query embedding close to product
    let similar_entries = memory.search_similar(query_embedding, 5, 0.0).await?;
    
    println!("\n2. Vector Query - Find similar content:");
    for (i, entry) in similar_entries.iter().enumerate() {
        println!("   Result {}: {} (score: {})", i+1, entry.content, entry.score.unwrap_or(0.0));
    }
    
    // 3. Combined approach - use graph to find relationships then vector to find similar
    println!("\n3. Combined Graph + Vector approach:");
    
    // First get relationships from graph
    let connected_nodes = graph.get_connected_nodes(node1_id, None).await?;
    println!("   Connected nodes to Product from graph: {}", connected_nodes.len());
    
    for node in &connected_nodes {
        println!("   - {} ({})", node.label, node.id.0);
        
        // For each connected node, find similar entries in vector store
        if let Ok(Some(entry)) = memory.get(node.id.0.to_string()).await {
            if let Some(embedding) = &entry.embedding {
                let similar = memory.search_similar(embedding.clone(), 2, 0.0).await?;
                println!("     Similar items in vector store:");
                for sim in similar {
                    println!("       * {} (score: {})", sim.content, sim.score.unwrap_or(0.0));
                }
            }
        }
    }
    
    println!("\nHybrid operations example completed successfully!");
    Ok(())
} 
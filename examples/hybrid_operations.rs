//! Example demonstrating hybrid vector+graph operations
//! 
//! This example shows how to use the hybrid architecture to combine graph traversal
//! with vector similarity for more powerful knowledge retrieval.

use chrono::{DateTime, Utc};
use graph::{
    Config, 
    graph::{Graph, Node, Edge, NodeId, EdgeId, TemporalRange},
    hybrid::{
        HybridGraph, 
        VectorizedNode, 
        VectorizedEdge,
        query::{HybridQueryBuilder, SimilarityMetric},
        fusion::{WeightedFusion, RankFusion},
    },
    memory::{Memory, MemoryEntry},
    temporal::TemporalIndex,
    types::{EntityType, Properties, Timestamp},
};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize configuration and services
    println!("Initializing hybrid graph system...");
    
    // Load configuration
    let config = Config::default();
    
    // Initialize underlying services
    let graph = graph::graph::new_graph(&config).await?;
    let memory = graph::memory::MemorySystem::new(
        graph::memory::OpenSearchMemory::new(&config).await?
    ).await?;
    let temporal_index = graph::temporal::DynamoDBTemporal::new(&config).await?;
    
    // Create hybrid graph
    let hybrid_graph = graph::hybrid::new_hybrid_graph(
        &config,
        graph,
        memory,
        temporal_index,
    ).await?;
    
    // Create sample nodes with vector embeddings
    println!("Creating sample nodes...");
    
    // Create a person node
    let person_node = create_person_node("Alice", 30, "Software Engineer")?;
    let vectorized_person = VectorizedNode::new(
        person_node,
        Some(vec![0.1, 0.2, 0.3, 0.4, 0.5]),
    );
    let person_id = hybrid_graph.create_node(vectorized_person).await?;
    println!("Created person node with ID: {}", person_id.0);
    
    // Create a company node
    let company_node = create_company_node("TechCorp", "Technology", "San Francisco")?;
    let vectorized_company = VectorizedNode::new(
        company_node,
        Some(vec![0.2, 0.3, 0.4, 0.5, 0.6]),
    );
    let company_id = hybrid_graph.create_node(vectorized_company).await?;
    println!("Created company node with ID: {}", company_id.0);
    
    // Create a product node
    let product_node = create_product_node("AI Assistant", "Software", 299.99)?;
    let vectorized_product = VectorizedNode::new(
        product_node,
        Some(vec![0.5, 0.6, 0.7, 0.8, 0.9]),
    );
    let product_id = hybrid_graph.create_node(vectorized_product).await?;
    println!("Created product node with ID: {}", product_id.0);
    
    // Create edges between nodes
    println!("Creating relationships between nodes...");
    
    // Person works at company
    let works_at_edge = create_edge(
        person_id,
        company_id,
        "WORKS_AT",
        json!({"start_date": "2020-01-15", "position": "Senior Engineer"}),
    )?;
    let vectorized_works_at = VectorizedEdge::new(
        works_at_edge,
        Some(vec![0.3, 0.4, 0.5, 0.6, 0.7]),
    );
    let works_at_id = hybrid_graph.create_edge(vectorized_works_at).await?;
    println!("Created WORKS_AT relationship with ID: {}", works_at_id.0);
    
    // Company produces product
    let produces_edge = create_edge(
        company_id,
        product_id,
        "PRODUCES",
        json!({"release_date": "2022-06-30", "version": "1.0"}),
    )?;
    let vectorized_produces = VectorizedEdge::new(
        produces_edge,
        Some(vec![0.4, 0.5, 0.6, 0.7, 0.8]),
    );
    let produces_id = hybrid_graph.create_edge(vectorized_produces).await?;
    println!("Created PRODUCES relationship with ID: {}", produces_id.0);
    
    // Person develops product
    let develops_edge = create_edge(
        person_id,
        product_id,
        "DEVELOPS",
        json!({"role": "Lead Developer", "contribution": "Architecture"}),
    )?;
    let vectorized_develops = VectorizedEdge::new(
        develops_edge,
        Some(vec![0.6, 0.7, 0.8, 0.9, 1.0]),
    );
    let develops_id = hybrid_graph.create_edge(vectorized_develops).await?;
    println!("Created DEVELOPS relationship with ID: {}", develops_id.0);
    
    // Demonstrate vector similarity search
    println!("\nDemonstrating vector similarity search...");
    
    let query_embedding = vec![0.5, 0.6, 0.7, 0.8, 0.9]; // Similar to product node
    let similar_nodes = hybrid_graph.find_similar_nodes(
        query_embedding,
        5,
        None,
    ).await?;
    
    println!("Found {} similar nodes:", similar_nodes.len());
    for node in &similar_nodes {
        println!("- {} ({}): {:?}", node.node.label, node.node.id.0, node.embedding.as_ref().map(|e| e.len()));
    }
    
    // Demonstrate hybrid query
    println!("\nDemonstrating hybrid query...");
    
    let query = HybridQueryBuilder::new()
        .start_from(person_id)
        .with_embedding(vec![0.5, 0.6, 0.7, 0.8, 0.9])
        .follow_outgoing(None) // Follow all outgoing edges
        .limit(10)
        .with_similarity_metric(SimilarityMetric::Cosine)
        .include_graph_structure(true)
        .build();
    
    let fusion_strategy = Box::new(WeightedFusion::balanced());
    
    let query_result = hybrid_graph.execute_hybrid_query(query, Some(fusion_strategy)).await?;
    
    println!("Hybrid query found {} nodes and {} edges in {}ms:",
        query_result.nodes.len(),
        query_result.edges.len(),
        query_result.execution_time_ms
    );
    
    for (i, scored_node) in query_result.nodes.iter().enumerate() {
        println!("{}. {} (score: {:.4}): {:?}",
            i + 1,
            scored_node.node.node.label,
            scored_node.score,
            scored_node.path.as_ref().map(|p| p.depth)
        );
    }
    
    // Demonstrate temporal query
    println!("\nDemonstrating temporal hybrid query...");
    
    let now = Utc::now();
    let one_year_ago = now - chrono::Duration::days(365);
    
    let temporal_range = TemporalRange {
        start: Some(Timestamp(one_year_ago)),
        end: Some(Timestamp(now)),
    };
    
    let temporal_results = hybrid_graph.get_knowledge_in_time_range(
        temporal_range,
        Some(vec![0.5, 0.6, 0.7, 0.8, 0.9]),
        5,
    ).await?;
    
    println!("Temporal query found {} nodes:", temporal_results.len());
    for node in &temporal_results {
        println!("- {}: {} ({})",
            node.node.entity_type,
            node.node.label,
            node.node.valid_time.start
                .map(|ts| ts.0.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "unknown".to_string())
        );
    }
    
    println!("\nHybrid vector+graph example completed successfully!");
    
    Ok(())
}

// Helper functions to create nodes and edges

fn create_person_node(name: &str, age: u32, occupation: &str) -> Result<Node, Box<dyn std::error::Error>> {
    let mut props = HashMap::new();
    props.insert("name".to_string(), json!(name));
    props.insert("age".to_string(), json!(age));
    props.insert("occupation".to_string(), json!(occupation));
    
    let now = Utc::now();
    
    Ok(Node {
        id: NodeId(Uuid::new_v4()),
        entity_type: EntityType::Person,
        label: name.to_string(),
        properties: Properties(props),
        valid_time: TemporalRange {
            start: Some(Timestamp(now)),
            end: None,
        },
        transaction_time: TemporalRange {
            start: Some(Timestamp(now)),
            end: None,
        },
    })
}

fn create_company_node(name: &str, industry: &str, location: &str) -> Result<Node, Box<dyn std::error::Error>> {
    let mut props = HashMap::new();
    props.insert("name".to_string(), json!(name));
    props.insert("industry".to_string(), json!(industry));
    props.insert("location".to_string(), json!(location));
    
    let now = Utc::now();
    
    Ok(Node {
        id: NodeId(Uuid::new_v4()),
        entity_type: EntityType::Organization,
        label: name.to_string(),
        properties: Properties(props),
        valid_time: TemporalRange {
            start: Some(Timestamp(now)),
            end: None,
        },
        transaction_time: TemporalRange {
            start: Some(Timestamp(now)),
            end: None,
        },
    })
}

fn create_product_node(name: &str, category: &str, price: f64) -> Result<Node, Box<dyn std::error::Error>> {
    let mut props = HashMap::new();
    props.insert("name".to_string(), json!(name));
    props.insert("category".to_string(), json!(category));
    props.insert("price".to_string(), json!(price));
    
    let now = Utc::now();
    
    Ok(Node {
        id: NodeId(Uuid::new_v4()),
        entity_type: EntityType::Product,
        label: name.to_string(),
        properties: Properties(props),
        valid_time: TemporalRange {
            start: Some(Timestamp(now)),
            end: None,
        },
        transaction_time: TemporalRange {
            start: Some(Timestamp(now)),
            end: None,
        },
    })
}

fn create_edge(
    source_id: NodeId,
    target_id: NodeId,
    label: &str,
    properties_json: serde_json::Value,
) -> Result<Edge, Box<dyn std::error::Error>> {
    let mut props = HashMap::new();
    
    if let serde_json::Value::Object(map) = properties_json {
        for (key, value) in map {
            props.insert(key, value);
        }
    }
    
    let now = Utc::now();
    
    Ok(Edge {
        id: EdgeId(Uuid::new_v4()),
        source_id,
        target_id,
        label: label.to_string(),
        properties: Properties(props),
        valid_time: TemporalRange {
            start: Some(Timestamp(now)),
            end: None,
        },
        transaction_time: TemporalRange {
            start: Some(Timestamp(now)),
            end: None,
        },
    })
} 
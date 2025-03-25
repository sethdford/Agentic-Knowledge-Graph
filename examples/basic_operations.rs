use graph::{
    Config,
    graph::{Graph, new_graph},
    types::{Node, Edge, NodeId, EdgeId, EntityType, Properties, TemporalRange, Timestamp},
};
use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create test configuration
    let config = Config::for_testing();
    
    println!("🚀 Initializing Temporal Knowledge Graph...");
    
    // Initialize graph
    let graph = new_graph(&config).await.expect("Failed to create graph");
    
    // Create timestamps for our operations
    let now = Timestamp(Utc::now());
    
    println!("📝 Creating nodes...");
    
    // Create a person node
    let mut person_props = HashMap::new();
    person_props.insert("name".to_string(), "John Doe".into());
    person_props.insert("age".to_string(), 30.into());
    
    let person_node = Node {
        id: NodeId(Uuid::new_v4()),
        entity_type: EntityType::Person,
        label: "John Doe".to_string(),
        properties: Properties(person_props),
        valid_time: TemporalRange {
            start: Some(now),
            end: None,
        },
        transaction_time: TemporalRange {
            start: Some(now),
            end: None,
        },
    };
    
    // Create an organization node
    let mut org_props = HashMap::new();
    org_props.insert("name".to_string(), "Acme Corp".into());
    org_props.insert("founded".to_string(), 2010.into());
    
    let org_node = Node {
        id: NodeId(Uuid::new_v4()),
        entity_type: EntityType::Organization,
        label: "Acme Corp".to_string(),
        properties: Properties(org_props),
        valid_time: TemporalRange {
            start: Some(now),
            end: None,
        },
        transaction_time: TemporalRange {
            start: Some(now),
            end: None,
        },
    };
    
    // Create nodes in the graph
    let person_id = graph.create_node(person_node.clone()).await?;
    let org_id = graph.create_node(org_node.clone()).await?;
    
    println!("✅ Created Person node with ID: {}", person_id.0);
    println!("✅ Created Organization node with ID: {}", org_id.0);
    
    println!("🔗 Creating relationship between nodes...");
    
    // Create an edge representing employment
    let mut edge_props = HashMap::new();
    edge_props.insert("role".to_string(), "Software Engineer".into());
    edge_props.insert("start_date".to_string(), "2020-01-15".into());
    
    let employment_edge = Edge {
        id: EdgeId(Uuid::new_v4()),
        source_id: person_id,
        target_id: org_id,
        label: "WORKS_AT".to_string(),
        properties: Properties(edge_props),
        valid_time: TemporalRange {
            start: Some(now),
            end: None,
        },
        transaction_time: TemporalRange {
            start: Some(now),
            end: None,
        },
    };
    
    // Create the edge in the graph
    let edge_id = graph.create_edge(employment_edge.clone()).await?;
    
    println!("✅ Created WORKS_AT relationship with ID: {}", edge_id.0);
    
    println!("🔍 Querying the graph...");
    
    // Retrieve nodes
    let retrieved_person = graph.get_node(person_id).await?;
    let retrieved_org = graph.get_node(org_id).await?;
    
    println!("👤 Person: {} ({})", 
        retrieved_person.label, 
        retrieved_person.properties.0.get("name").unwrap()
    );
    
    println!("🏢 Organization: {} (founded: {})", 
        retrieved_org.label, 
        retrieved_org.properties.0.get("founded").unwrap()
    );
    
    // Get connected nodes
    let connected_orgs = graph.get_connected_nodes(person_id, None).await?;
    println!("🔗 {} is connected to {} organizations", retrieved_person.label, connected_orgs.len());
    
    for org in connected_orgs {
        println!("  └─ Connected to: {}", org.label);
    }
    
    // Get edges for a node
    let person_edges = graph.get_edges_for_node(person_id, None).await?;
    println!("↔️ {} has {} relationships", retrieved_person.label, person_edges.len());
    
    for edge in person_edges {
        println!("  └─ Relationship: {} (role: {})", 
            edge.label, 
            edge.properties.0.get("role").unwrap()
        );
    }
    
    println!("🧪 Demonstrating temporal features...");
    
    // Create a new version of the person node (e.g., after a promotion)
    let one_year_later = Timestamp(Utc::now() + chrono::Duration::days(365));
    
    let mut updated_props = retrieved_person.properties.0.clone();
    updated_props.insert("age".to_string(), 31.into());
    updated_props.insert("promotion_date".to_string(), one_year_later.0.to_rfc3339().into());
    
    let updated_person = Node {
        id: person_id,
        entity_type: EntityType::Person,
        label: "John Doe".to_string(),
        properties: Properties(updated_props),
        valid_time: TemporalRange {
            start: Some(one_year_later),
            end: None,
        },
        transaction_time: TemporalRange {
            start: Some(Timestamp(Utc::now())),
            end: None,
        },
    };
    
    // Update the person node
    graph.update_node(updated_person).await?;
    println!("✅ Updated person node with new properties (future valid time)");
    
    // Create a new version of the edge (e.g., changed role)
    let mut updated_edge_props = employment_edge.properties.0.clone();
    updated_edge_props.insert("role".to_string(), "Senior Software Engineer".into());
    updated_edge_props.insert("promotion_date".to_string(), one_year_later.0.to_rfc3339().into());
    
    let updated_edge = Edge {
        id: edge_id,
        source_id: person_id,
        target_id: org_id,
        label: "WORKS_AT".to_string(),
        properties: Properties(updated_edge_props),
        valid_time: TemporalRange {
            start: Some(one_year_later),
            end: None,
        },
        transaction_time: TemporalRange {
            start: Some(Timestamp(Utc::now())),
            end: None,
        },
    };
    
    // Update the edge
    graph.update_edge(updated_edge).await?;
    println!("✅ Updated edge with new properties (future valid time)");
    
    println!("\n✨ Example completed successfully!");
    
    Ok(())
} 
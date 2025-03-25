use reqwest;
use serde_json::{json, Value};
use uuid::Uuid;
use chrono::Utc;

// API client to interact with the temporal knowledge graph API
struct TemporalGraphClient {
    base_url: String,
    client: reqwest::Client,
}

impl TemporalGraphClient {
    fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            client: reqwest::Client::new(),
        }
    }
    
    async fn health_check(&self) -> Result<Value, reqwest::Error> {
        let response = self.client.get(&format!("{}/health", self.base_url))
            .send()
            .await?
            .json::<Value>()
            .await?;
            
        Ok(response)
    }
    
    async fn create_node(&self, data: Value) -> Result<Value, reqwest::Error> {
        let response = self.client.post(&format!("{}/nodes", self.base_url))
            .json(&data)
            .send()
            .await?
            .json::<Value>()
            .await?;
            
        Ok(response)
    }
    
    async fn get_node(&self, id: &str) -> Result<Value, reqwest::Error> {
        let response = self.client.get(&format!("{}/nodes/{}", self.base_url, id))
            .send()
            .await?
            .json::<Value>()
            .await?;
            
        Ok(response)
    }
    
    async fn create_edge(&self, data: Value) -> Result<Value, reqwest::Error> {
        let response = self.client.post(&format!("{}/edges", self.base_url))
            .json(&data)
            .send()
            .await?
            .json::<Value>()
            .await?;
            
        Ok(response)
    }
    
    async fn query_knowledge(&self, query: Value) -> Result<Value, reqwest::Error> {
        let response = self.client.post(&format!("{}/knowledge/query", self.base_url))
            .json(&query)
            .send()
            .await?
            .json::<Value>()
            .await?;
            
        Ok(response)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Ensure the server is running on localhost:3000 before running this example
    let client = TemporalGraphClient::new("http://localhost:3000");
    
    println!("🚀 Interacting with Temporal Knowledge Graph API...");
    
    // Check API health
    match client.health_check().await {
        Ok(health) => println!("✅ API Health: {}", health["status"]),
        Err(e) => {
            println!("❌ API Health check failed: {}", e);
            println!("Make sure the server is running with `cargo run` in another terminal");
            return Ok(());
        }
    }
    
    // Generate UUIDs for our entities
    let person_id = Uuid::new_v4().to_string();
    let org_id = Uuid::new_v4().to_string();
    
    println!("📝 Creating a person node...");
    
    // Create a person node
    let person_node = json!({
        "id": person_id,
        "entity_type": "Person",
        "label": "Jane Smith",
        "properties": {
            "name": "Jane Smith",
            "age": 28,
            "email": "jane.smith@example.com",
            "skills": ["Rust", "GraphQL", "Machine Learning"]
        },
        "valid_time": {
            "start": Utc::now().to_rfc3339(),
            "end": null
        }
    });
    
    let person_result = client.create_node(person_node).await?;
    println!("✅ Created Person node: {}", person_result["id"]);
    
    println!("📝 Creating an organization node...");
    
    // Create an organization node
    let org_node = json!({
        "id": org_id,
        "entity_type": "Organization",
        "label": "Tech Innovators",
        "properties": {
            "name": "Tech Innovators Inc.",
            "founded": 2015,
            "location": "San Francisco, CA",
            "industry": "Technology",
            "employees": 250
        },
        "valid_time": {
            "start": Utc::now().to_rfc3339(),
            "end": null
        }
    });
    
    let org_result = client.create_node(org_node).await?;
    println!("✅ Created Organization node: {}", org_result["id"]);
    
    println!("🔗 Creating an edge between person and organization...");
    
    // Create an employment relationship
    let edge_id = Uuid::new_v4().to_string();
    let edge = json!({
        "id": edge_id,
        "source_id": person_id,
        "target_id": org_id,
        "label": "EMPLOYED_AT",
        "properties": {
            "role": "Senior Developer",
            "department": "Engineering",
            "start_date": "2020-03-15",
            "salary": 120000
        },
        "valid_time": {
            "start": Utc::now().to_rfc3339(),
            "end": null
        }
    });
    
    let edge_result = client.create_edge(edge).await?;
    println!("✅ Created EMPLOYED_AT relationship: {}", edge_result["id"]);
    
    println!("🔍 Retrieving the person node...");
    
    // Get the person node details
    let person = client.get_node(&person_id).await?;
    println!("👤 Person: {} ({})", 
        person["label"], 
        person["properties"]["name"]
    );
    
    println!("🔍 Running a knowledge query...");
    
    // Query for employment relationships
    let query = json!({
        "filter": {
            "entity_type": "Person",
            "properties": {
                "skills": {
                    "contains": "Rust"
                }
            }
        },
        "include_edges": true,
        "time_point": Utc::now().to_rfc3339()
    });
    
    let query_result = client.query_knowledge(query).await?;
    println!("✅ Query returned {} results", query_result["results"].as_array().unwrap_or(&vec![]).len());
    
    // Pretty print the query results
    if let Some(results) = query_result["results"].as_array() {
        for (i, result) in results.iter().enumerate() {
            println!("  Result #{}: {}", i + 1, result["label"]);
            
            if let Some(props) = result["properties"].as_object() {
                println!("  Properties:");
                for (key, value) in props {
                    println!("    {}: {}", key, value);
                }
            }
            
            if let Some(edges) = result["edges"].as_array() {
                println!("  Relationships:");
                for edge in edges {
                    println!("    {} -> {}", edge["label"], edge["target"]["label"]);
                }
            }
            
            println!();
        }
    }
    
    println!("\n✨ API client example completed successfully!");
    
    Ok(())
} 
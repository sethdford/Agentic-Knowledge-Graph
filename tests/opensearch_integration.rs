use chrono::Utc;
use opensearch::{
    http::transport::{SingleNodeConnectionPool, Transport},
    indices::{IndicesCreateParts, IndicesDeleteParts},
    BulkParts, OpenSearch, BulkOperation,
    params::Refresh,
};
use graph::{
    error::{Error, Result},
    memory::{MemoryEntry, MemoryOperations},
    types::EntityType,
};
use serde_json::{json, Value};
use std::{collections::HashMap, time::Duration};
use url::Url;
use uuid::Uuid;

async fn setup_test_memory() -> Result<OpenSearch> {
    let url = Url::parse("http://localhost:9200").unwrap();
    let pool = SingleNodeConnectionPool::new(url);
    let transport = Transport::single_node("http://localhost:9200")?;
    let client = OpenSearch::new(transport);

    // Delete test index if it exists
    let _ = client
        .indices()
        .delete(IndicesDeleteParts::Index(&["test-memories"]))
        .send()
        .await;

    // Create test index with mapping
    client
        .indices()
        .create(IndicesCreateParts::Index("test-memories"))
        .body(json!({
            "mappings": {
                "properties": {
                    "id": { "type": "keyword" },
                    "created_at": { "type": "date" },
                    "updated_at": { "type": "date" },
                    "content": { "type": "text" },
                    "metadata": { "type": "object" },
                    "embedding": { "type": "dense_vector", "dims": 384 },
                    "node_type": { "type": "keyword" }
                }
            }
        }))
        .send()
        .await?;

    Ok(client)
}

#[tokio::test]
async fn test_basic_memory_operations() -> Result<()> {
    let client = setup_test_memory().await?;

    let entry = MemoryEntry {
        id: Uuid::new_v4().to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        content: "Test memory".to_string(),
        metadata: HashMap::from([("test".to_string(), json!("metadata"))]),
        embedding: Some(vec![0.1; 384]),
        node_type: Some(EntityType::Node),
    };

    // Store entry
    let body = json!({
        "id": entry.id,
        "created_at": entry.created_at,
        "updated_at": entry.updated_at,
        "content": entry.content,
        "metadata": entry.metadata,
        "embedding": entry.embedding,
        "node_type": entry.node_type.as_ref().unwrap().to_string()
    });

    client
        .index(opensearch::IndexParts::Index("test-memories"))
        .body(body)
        .refresh(Refresh::True)
        .send()
        .await?;

    // Verify storage
    let response = client
        .search(opensearch::SearchParts::Index(&["test-memories"]))
        .body(json!({
            "query": {
                "match_all": {}
            }
        }))
        .send()
        .await?;

    let hits = response.json::<Value>().await?;
    let total = hits["hits"]["total"]["value"].as_i64().unwrap();
    assert_eq!(total, 1);

    Ok(())
}

#[tokio::test]
async fn test_vector_similarity_search() -> Result<()> {
    let client = setup_test_memory().await?;
    
    // Store two entries with different embeddings
    let entries = vec![
        MemoryEntry {
            id: Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            content: "First memory".to_string(),
            metadata: HashMap::from([("test".to_string(), json!("metadata"))]),
            embedding: Some(vec![1.0; 384]),
            node_type: Some(EntityType::Node),
        },
        MemoryEntry {
            id: Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            content: "Second memory".to_string(),
            metadata: HashMap::from([("test".to_string(), json!("metadata"))]),
            embedding: Some(vec![0.5; 384]),
            node_type: Some(EntityType::Node),
        },
    ];

    // Store entries
    let mut bulk_operations: Vec<BulkOperation<Value>> = Vec::new();
    for entry in &entries {
        let doc = json!({
            "id": entry.id,
            "created_at": entry.created_at,
            "updated_at": entry.updated_at,
            "content": entry.content,
            "metadata": entry.metadata,
            "embedding": entry.embedding,
            "node_type": entry.node_type.as_ref().unwrap().to_string()
        });
        
        let operation: BulkOperation<Value> = BulkOperation::index(doc)
            .id(&entry.id)
            .into();
        bulk_operations.push(operation);
    }

    client
        .bulk(BulkParts::None)
        .body(bulk_operations)
        .refresh(Refresh::True)
        .send()
        .await?;

    // Search for similar vectors
    let response = client
        .search(opensearch::SearchParts::Index(&["test-memories"]))
        .body(json!({
            "query": {
                "script_score": {
                    "query": { "match_all": {} },
                    "script": {
                        "source": "cosineSimilarity(params.query_vector, 'embedding') + 1.0",
                        "params": {
                            "query_vector": vec![1.0; 384]
                        }
                    }
                }
            },
            "size": 2
        }))
        .send()
        .await?;

    let hits = response.json::<Value>().await?;
    let total = hits["hits"]["total"]["value"].as_i64().unwrap();
    assert_eq!(total, 2);

    Ok(())
}

#[tokio::test]
async fn test_temporal_queries() -> Result<()> {
    let client = setup_test_memory().await?;
    
    let entry = MemoryEntry {
        id: Uuid::new_v4().to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        content: "Temporal memory".to_string(),
        metadata: HashMap::from([("test".to_string(), json!("metadata"))]),
        embedding: Some(vec![0.1; 384]),
        node_type: Some(EntityType::Node),
    };

    let body = serde_json::to_string(&json!({
        "id": entry.id,
        "created_at": entry.created_at,
        "updated_at": entry.updated_at,
        "content": entry.content,
        "metadata": entry.metadata,
        "embedding": entry.embedding,
        "node_type": entry.node_type.as_ref().unwrap().to_string(),
    })).unwrap();

    client
        .index(opensearch::IndexParts::Index("test-memories"))
        .body(body)
        .refresh(Refresh::True)
        .send()
        .await?;

    let response = client
        .search(opensearch::SearchParts::Index(&["test-memories"]))
        .body(json!({
            "query": {
                "range": {
                    "created_at": {
                        "gte": "now-1h",
                        "lte": "now"
                    }
                }
            }
        }))
        .send()
        .await?;

    let hits = response.json::<Value>().await?;
    let total = hits["hits"]["total"]["value"].as_i64().unwrap();
    assert_eq!(total, 1);

    Ok(())
}

#[tokio::test]
async fn test_metadata_operations() -> Result<()> {
    let client = setup_test_memory().await?;
    
    let entry = MemoryEntry {
        id: Uuid::new_v4().to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        content: "Memory with metadata".to_string(),
        metadata: HashMap::from([("key".to_string(), json!("value"))]),
        embedding: Some(vec![0.1; 384]),
        node_type: Some(EntityType::Node),
    };

    let body = serde_json::to_string(&json!({
        "id": entry.id,
        "created_at": entry.created_at,
        "updated_at": entry.updated_at,
        "content": entry.content,
        "metadata": entry.metadata,
        "embedding": entry.embedding,
        "node_type": entry.node_type.as_ref().unwrap().to_string(),
    })).unwrap();

    client
        .index(opensearch::IndexParts::Index("test-memories"))
        .body(body)
        .refresh(Refresh::True)
        .send()
        .await?;

    let response = client
        .search(opensearch::SearchParts::Index(&["test-memories"]))
        .body(json!({
            "query": {
                "match": {
                    "metadata.key": "value"
                }
            }
        }))
        .send()
        .await?;

    let hits = response.json::<Value>().await?;
    let total = hits["hits"]["total"]["value"].as_i64().unwrap();
    assert_eq!(total, 1);

    Ok(())
}

#[tokio::test]
async fn test_bulk_operations() -> Result<()> {
    let client = setup_test_memory().await?;
    
    let entries: Vec<_> = (0..5).map(|i| MemoryEntry {
        id: Uuid::new_v4().to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        content: format!("Bulk memory {}", i),
        metadata: HashMap::from([("test".to_string(), json!("metadata"))]),
        embedding: Some(vec![0.1; 384]),
        node_type: Some(EntityType::Node),
    }).collect();

    // Store entries
    let mut bulk_operations: Vec<BulkOperation<Value>> = Vec::new();
    for entry in &entries {
        let doc = json!({
            "id": entry.id,
            "created_at": entry.created_at,
            "updated_at": entry.updated_at,
            "content": entry.content,
            "metadata": entry.metadata,
            "embedding": entry.embedding,
            "node_type": entry.node_type.as_ref().unwrap().to_string()
        });
        
        let operation: BulkOperation<Value> = BulkOperation::index(doc)
            .id(&entry.id)
            .into();
        bulk_operations.push(operation);
    }

    client
        .bulk(BulkParts::None)
        .body(bulk_operations)
        .refresh(Refresh::True)
        .send()
        .await?;

    // Verify bulk storage
    let response = client
        .search(opensearch::SearchParts::Index(&["test-memories"]))
        .body(json!({
            "query": {
                "match_all": {}
            }
        }))
        .send()
        .await?;

    let hits = response.json::<Value>().await?;
    let total = hits["hits"]["total"]["value"].as_i64().unwrap();
    assert_eq!(total, 5);

    Ok(())
} 
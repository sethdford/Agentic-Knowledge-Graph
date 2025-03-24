use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use opensearch::{
    auth::Credentials,
    http::transport::{SingleNodeConnectionPool, Transport, TransportBuilder},
    BulkOperation,
    BulkParts, 
    IndexParts, 
    OpenSearch, 
    SearchParts,
    DeleteParts,
    DeleteByQueryParts,
    http::StatusCode,
    indices::{IndicesCreateParts, IndicesExistsParts, IndicesDeleteParts},
    GetParts,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::RwLock;
use url::Url;
use uuid::Uuid;

use crate::{
    error::{Error, Result},
    Config,
    types::{TemporalRange, EntityType, Timestamp},
};

const DEFAULT_INDEX: &str = "memory";

/// Represents a memory entry in the vector store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub content: String,
    pub metadata: HashMap<String, Value>,
    pub embedding: Option<Vec<f32>>,
    pub node_type: Option<EntityType>,
}

impl MemoryEntry {
    pub fn new(id: String, content: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            created_at: now,
            updated_at: now,
            content,
            metadata: HashMap::new(),
            embedding: None,
            node_type: None,
        }
    }
}

/// Memory operations trait defining the interface for vector storage and retrieval
#[async_trait]
pub trait MemoryOperations: Send + Sync {
    /// Store a new memory entry
    async fn store(&self, entry: MemoryEntry) -> Result<()>;
    
    /// Retrieve similar memories based on vector similarity
    async fn search_similar(&self, query_vector: &[f32], limit: usize) -> Result<Vec<MemoryEntry>>;
    
    /// Get memory by ID
    async fn get(&self, id: &str) -> Result<Option<MemoryEntry>>;
    
    /// Delete a memory entry
    async fn delete(&self, id: &str) -> Result<()>;
}

/// OpenSearch implementation of MemoryOperations
pub struct OpenSearchMemory {
    client: OpenSearch,
    index: String,
}

impl OpenSearchMemory {
    /// Create a new OpenSearchMemory instance
    pub async fn new(config: &Config) -> Result<Self> {
        let url = Url::parse(&config.memory_url)
            .map_err(|e| Error::OpenSearch(e.to_string()))?;
        let conn_pool = SingleNodeConnectionPool::new(url);
        let mut transport_builder = TransportBuilder::new(conn_pool);
        
        if !config.memory_username.is_empty() {
            transport_builder = transport_builder.auth(Credentials::Basic(
                config.memory_username.clone(),
                config.memory_password.clone()
            ));
        }

        let transport = transport_builder.build()
            .map_err(|e| Error::OpenSearch(e.to_string()))?;
        let client = OpenSearch::new(transport);
        let index = DEFAULT_INDEX.to_string();

        let memory = Self { client, index };
        memory.initialize().await?;
        Ok(memory)
    }
    
    /// Initialize the OpenSearch index with proper mappings
    pub async fn initialize(&self) -> Result<()> {
        // Check if index exists
        let exists = self.client
            .indices()
            .exists(IndicesExistsParts::Index(&[&self.index]))
            .send()
            .await?
            .status_code()
            .is_success();

        if exists {
            return Ok(());
        }

        let mapping = json!({
            "mappings": {
                "properties": {
                    "id": { "type": "keyword" },
                    "created_at": { "type": "date" },
                    "updated_at": { "type": "date" },
                    "content": { "type": "text" },
                    "embedding": { "type": "dense_vector", "dims": 768 },
                    "metadata": { "type": "object" },
                    "node_type": { "type": "keyword" }
                }
            }
        });
        
        let response = self.client
            .indices()
            .create(IndicesCreateParts::Index(&self.index))
            .body(mapping)
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;
            
        if !response.status_code().is_success() {
            return Err(Error::OpenSearch("Failed to initialize index".to_string()));
        }
            
        Ok(())
    }
    
    /// Convert vector to OpenSearch format
    fn format_vector(vector: &[f32]) -> String {
        vector
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    pub async fn index_document(&self, document: Value) -> Result<()> {
        let response = self.client
            .index(IndexParts::Index(&self.index))
            .body(document)
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        if !response.status_code().is_success() {
            return Err(Error::OpenSearch(format!("Failed to index document: {}", response.status_code())));
        }

        Ok(())
    }

    pub async fn search_documents(&self, query: Value) -> Result<Vec<Value>> {
        let response = self.client
            .search(SearchParts::Index(&[&self.index]))
            .body(query)
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        if !response.status_code().is_success() {
            return Err(Error::OpenSearch(format!("Failed to search documents: {}", response.status_code())));
        }

        let search_response: Value = response.json().await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        let hits = search_response["hits"]["hits"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .map(|hit| hit["_source"].clone())
            .collect();

        Ok(hits)
    }

    pub async fn get(&self, id: &str) -> Result<Option<MemoryEntry>> {
        let response = self.client
            .get(GetParts::IndexId(&self.index, id))
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        if response.status_code() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status_code().is_success() {
            return Err(Error::OpenSearch(format!("Failed to get document: {}", response.status_code())));
        }

        let search_response: Value = response.json().await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        if let Some(hits) = search_response["hits"]["hits"].as_array() {
            if let Some(hit) = hits.first() {
                if let Some(_) = hit["_source"].as_object() {
                    let entry: MemoryEntry = serde_json::from_value(hit["_source"].clone())
                        .map_err(|e| Error::Serialization(e.to_string()))?;
                    return Ok(Some(entry));
                }
            }
        }

        Ok(None)
    }

    fn parse_memory_entry(&self, source: &Value) -> Result<MemoryEntry> {
        let metadata = source["metadata"].as_object()
            .ok_or_else(|| Error::OpenSearch("Invalid metadata format".to_string()))?
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let node_type = source.get("node_type")
            .and_then(|v| v.as_str())
            .map(|s| EntityType::from_str(s))
            .transpose()?;

        Ok(MemoryEntry {
            id: source["id"].as_str()
                .ok_or_else(|| Error::OpenSearch("Missing id field".to_string()))?
                .to_string(),
            created_at: chrono::DateTime::parse_from_rfc3339(
                source["created_at"].as_str()
                    .ok_or_else(|| Error::OpenSearch("Missing created_at field".to_string()))?
            )?.with_timezone(&chrono::Utc),
            updated_at: chrono::DateTime::parse_from_rfc3339(
                source["updated_at"].as_str()
                    .ok_or_else(|| Error::OpenSearch("Missing updated_at field".to_string()))?
            )?.with_timezone(&chrono::Utc),
            content: source["content"].as_str()
                .ok_or_else(|| Error::OpenSearch("Missing content field".to_string()))?
                .to_string(),
            metadata,
            embedding: source.get("embedding")
                .and_then(|e| e.as_array())
                .map(|arr| arr.iter()
                    .filter_map(|v| v.as_f64())
                    .map(|v| v as f32)
                    .collect()
                ),
            node_type,
        })
    }

    async fn delete_document(&self, index: &str, id: &str) -> Result<()> {
        let request = self.client
            .delete(DeleteParts::IndexId(index, id));
            
        request.send().await.map_err(|e| Error::OpenSearch(e.to_string()))?;
        Ok(())
    }

    pub async fn store(&self, entry: MemoryEntry) -> Result<()> {
        let document = json!({
            "id": entry.id,
            "embedding": entry.embedding,
            "metadata": entry.metadata,
            "created_at": entry.created_at.to_rfc3339(),
            "updated_at": entry.updated_at.to_rfc3339(),
            "content": entry.content,
            "node_type": entry.node_type.map(|nt| nt.to_string()),
        });

        let response = self.client
            .index(IndexParts::Index(&self.index))
            .body(document)
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        if !response.status_code().is_success() {
            return Err(Error::OpenSearch("Failed to store entry".to_string()));
        }

        Ok(())
    }

    /// Store multiple entries in a single bulk request
    pub async fn bulk_store(&self, entries: Vec<MemoryEntry>) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let mut bulk_operations: Vec<BulkOperation<Value>> = Vec::with_capacity(entries.len());
        for entry in entries {
            let operation = BulkOperation::index(json!({
                "id": entry.id,
                "created_at": entry.created_at,
                "updated_at": entry.updated_at,
                "content": entry.content,
                "metadata": entry.metadata,
                "embedding": entry.embedding,
                "node_type": entry.node_type
            }))
            .id(&entry.id)
            .into();
            bulk_operations.push(operation);
        }

        let response = self.client
            .bulk(BulkParts::Index(&self.index))
            .body(bulk_operations)
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        if !response.status_code().is_success() {
            return Err(Error::OpenSearch(format!("Failed to bulk store documents: {}", response.status_code())));
        }

        Ok(())
    }

    async fn parse_search_results(&self, response_body: Value) -> Result<Vec<MemoryEntry>> {
        let hits = response_body["hits"]["hits"]
            .as_array()
            .ok_or_else(|| Error::OpenSearch("Invalid response format".to_string()))?;

        let mut results = Vec::new();
        for hit in hits {
            let source = hit["_source"].clone();
            let metadata = source["metadata"]
                .as_object()
                .map(|obj| {
                    obj.iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<HashMap<String, Value>>()
                })
                .unwrap_or_default();

            let node_type = if let Some(type_str) = source.get("node_type").and_then(|v| v.as_str()) {
                match EntityType::from_str(type_str) {
                    Ok(nt) => Some(nt),
                    Err(e) => return Err(Error::OpenSearch(e.to_string())),
                }
            } else {
                None
            };

            let entry = MemoryEntry {
                id: source["id"].as_str()
                    .ok_or_else(|| Error::OpenSearch("Missing id field".to_string()))?
                    .to_string(),
                embedding: serde_json::from_value(source["embedding"].clone())
                    .map_err(|e| Error::OpenSearch(e.to_string()))?,
                metadata,
                created_at: DateTime::parse_from_rfc3339(source["created_at"].as_str().unwrap_or_default())
                    .map_err(|e| Error::OpenSearch(e.to_string()))?
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(source["updated_at"].as_str().unwrap_or_default())
                    .map_err(|e| Error::OpenSearch(e.to_string()))?
                    .with_timezone(&Utc),
                content: source["content"].as_str().unwrap_or_default().to_string(),
                node_type,
            };
            results.push(entry);
        }

        Ok(results)
    }

    pub async fn search(&self, query: &Value) -> Result<Vec<MemoryEntry>> {
        let response = self.client
            .search(SearchParts::Index(&[&self.index]))
            .body(query)
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        if !response.status_code().is_success() {
            return Err(Error::OpenSearch("Search failed".to_string()));
        }

        let response_body = response.json::<Value>().await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        self.parse_search_results(response_body).await
    }

    /// Search by ID
    pub async fn search_by_id(&self, id: &str) -> Result<Option<MemoryEntry>> {
        let query = json!({
            "query": {
                "term": {
                    "id": id
                }
            }
        });

        let response = self.client
            .search(SearchParts::Index(&[&self.index]))
            .body(query)
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        if !response.status_code().is_success() {
            return Err(Error::OpenSearch(format!("Failed to search documents: {}", response.status_code())));
        }

        let search_response: Value = response.json().await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        if let Some(hits) = search_response["hits"]["hits"].as_array() {
            if let Some(hit) = hits.first() {
                if let Some(_) = hit["_source"].as_object() {
                    let entry: MemoryEntry = serde_json::from_value(hit["_source"].clone())
                        .map_err(|e| Error::Serialization(e.to_string()))?;
                    return Ok(Some(entry));
                }
            }
        }

        Ok(None)
    }

    /// Delete entries by query
    pub async fn delete_by_query(&self, query: &Value) -> Result<usize> {
        let response = self.client
            .delete_by_query(DeleteByQueryParts::Index(&[&self.index]))
            .body(query)
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;
            
        let response_body = response.json::<Value>().await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        let deleted = response_body["deleted"]
            .as_u64()
            .ok_or_else(|| Error::OpenSearch("Invalid response format".to_string()))?;

        Ok(deleted as usize)
    }

    /// Bulk delete memory entries in OpenSearch
    pub async fn bulk_delete(&self, entries: &[MemoryEntry]) -> Result<usize> {
        if entries.is_empty() {
            return Ok(0);
        }

        let mut bulk_operations: Vec<BulkOperation<Value>> = Vec::with_capacity(entries.len());
        for entry in entries {
            let operation = BulkOperation::delete(&entry.id).into();
            bulk_operations.push(operation);
        }

        let response = self.client
            .bulk(BulkParts::Index(&self.index))
            .body(bulk_operations)
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;
        
        if !response.status_code().is_success() {
            return Err(Error::OpenSearch("Bulk delete operation failed".to_string()));
        }
        
        let response_body = response.json::<Value>().await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;
        
        if let Some(errors) = response_body["errors"].as_bool() {
            if errors {
                return Err(Error::OpenSearch("Bulk delete operation had errors".to_string()));
            }
        }
        
        Ok(entries.len())
    }

    /// Ensure the memory index exists, create it if it doesn't
    async fn ensure_index_exists(&self) -> Result<()> {
        let exists = self.client
            .indices()
            .exists(IndicesExistsParts::Index(&[&self.index]))
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        if !exists.status_code().is_success() {
            // Create index with mappings
            let mappings = json!({
                "mappings": {
                    "properties": {
                        "id": { "type": "keyword" },
                        "created_at": { "type": "date" },
                        "updated_at": { "type": "date" },
                        "content": { "type": "text" },
                        "metadata": { "type": "object", "dynamic": true },
                        "embedding": { "type": "dense_vector", "dims": 1536 }
                    }
                }
            });

            self.client
                .indices()
                .create(IndicesCreateParts::Index(&self.index))
                .body(mappings)
                .send()
                .await
                .map_err(|e| Error::OpenSearch(e.to_string()))?;
        }

        Ok(())
    }

    /// Delete all entries
    pub async fn clear(&self) -> Result<()> {
        let response = self.client
            .delete_by_query(DeleteByQueryParts::Index(&[&self.index]))
            .body(json!({
                "query": {
                    "match_all": {}
                }
            }))
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        if !response.status_code().is_success() {
            return Err(Error::OpenSearch(format!("Failed to clear index: {}", response.status_code())));
        }

        Ok(())
    }

    pub async fn search_with_metadata(&self, query: &Value) -> Result<Vec<MemoryEntry>> {
        let response = self.client
            .search(SearchParts::Index(&[&self.index]))
            .body(query)
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        if !response.status_code().is_success() {
            return Err(Error::OpenSearch("Search failed".to_string()));
        }

        let response_body = response.json::<Value>().await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        self.parse_search_results(response_body).await
    }
}

#[async_trait]
impl MemoryOperations for OpenSearchMemory {
    async fn store(&self, entry: MemoryEntry) -> Result<()> {
        let document = json!({
            "id": entry.id,
            "embedding": entry.embedding,
            "metadata": entry.metadata,
            "created_at": entry.created_at.to_rfc3339(),
            "updated_at": entry.updated_at.to_rfc3339(),
            "content": entry.content,
            "node_type": entry.node_type.map(|nt| nt.to_string()),
        });

        let response = self.client
            .index(IndexParts::IndexId(&self.index, &entry.id))
            .body(document)
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        if !response.status_code().is_success() {
            return Err(Error::OpenSearch("Failed to store entry".to_string()));
        }

        Ok(())
    }

    async fn search_similar(&self, query_vector: &[f32], limit: usize) -> Result<Vec<MemoryEntry>> {
        let query = json!({
            "size": limit,
            "query": {
                "script_score": {
                    "query": { "match_all": {} },
                    "script": {
                        "source": "cosineSimilarity(params.query_vector, 'embedding') + 1.0",
                        "params": {
                            "query_vector": query_vector
                        }
                    }
                }
            }
        });

        let response = self.client
            .search(SearchParts::Index(&[&self.index]))
            .body(query)
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        if !response.status_code().is_success() {
            return Err(Error::OpenSearch("Failed to search similar entries".to_string()));
        }

        let response_body = response.json::<Value>().await
            .map_err(|e| Error::Serialization(e.to_string()))?;

        let hits = response_body["hits"]["hits"]
            .as_array()
            .ok_or_else(|| Error::OpenSearch("Invalid response format".to_string()))?;

        let entries = hits
            .iter()
            .filter_map(|hit| {
                let source = hit["_source"].clone();
                serde_json::from_value::<MemoryEntry>(source).ok()
            })
            .collect();

        Ok(entries)
    }

    async fn get(&self, id: &str) -> Result<Option<MemoryEntry>> {
        self.get(id).await
    }

    async fn delete(&self, id: &str) -> Result<()> {
        self.delete_document("zep_memories", id).await?;
        Ok(())
    }
}

/// Memory system for efficient vector storage and retrieval
pub struct MemorySystem {
    /// OpenSearch client
    client: Arc<OpenSearch>,
    /// Index name for vector storage
    index: String,
    /// Dimension of vector embeddings
    embedding_dim: usize,
}

#[async_trait]
pub trait Memory {
    /// Store a memory entry
    async fn store(&self, entry: MemoryEntry) -> Result<()>;
    
    /// Store multiple memory entries in bulk
    async fn store_bulk(&self, entries: Vec<MemoryEntry>) -> Result<()>;
    
    /// Search for similar memories
    async fn search_similar(&self, embedding: Vec<f32>, k: usize, filter: Option<Value>) -> Result<Vec<MemoryEntry>>;
    
    /// Get memories by node type
    async fn get_by_node_type(&self, node_type: EntityType, limit: usize) -> Result<Vec<MemoryEntry>>;
    
    /// Get memories by time range
    async fn get_by_time_range(&self, range: TemporalRange, limit: usize) -> Result<Vec<MemoryEntry>>;
    
    /// Get memories for a specific node
    async fn get_for_node(&self, node_id: Uuid, limit: usize) -> Result<Vec<MemoryEntry>>;
    
    /// Get memories for a specific edge
    async fn get_for_edge(&self, source_id: Uuid, target_id: Uuid, limit: usize) -> Result<Vec<MemoryEntry>>;
}

#[async_trait]
impl Memory for MemorySystem {
    async fn store(&self, entry: MemoryEntry) -> Result<()> {
        let document = json!({
            "id": entry.id,
            "embedding": entry.embedding,
            "metadata": entry.metadata,
            "created_at": entry.created_at.to_rfc3339(),
            "updated_at": entry.updated_at.to_rfc3339(),
            "content": entry.content,
            "node_type": entry.node_type.map(|nt| nt.to_string()),
        });

        let response = self.client
            .index(IndexParts::IndexId(&*self.index, &entry.id))
            .body(document)
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        if !response.status_code().is_success() {
            return Err(Error::OpenSearch("Failed to store entry".to_string()));
        }

        Ok(())
    }

    async fn store_bulk(&self, entries: Vec<MemoryEntry>) -> Result<()> {
        let mut bulk_operations: Vec<BulkOperation<Value>> = Vec::with_capacity(entries.len());
        for entry in entries {
            let operation = BulkOperation::index(json!({
                "id": entry.id,
                "embedding": entry.embedding,
                "metadata": entry.metadata,
                "created_at": entry.created_at.to_rfc3339(),
                "updated_at": entry.updated_at.to_rfc3339(),
                "content": entry.content,
                "node_type": entry.node_type.map(|nt| nt.to_string()),
            }))
            .id(&entry.id)
            .into();
            bulk_operations.push(operation);
        }

        let response = self.client
            .bulk(BulkParts::Index(&self.index))
            .body(bulk_operations)
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        if !response.status_code().is_success() {
            return Err(Error::OpenSearch("Failed to bulk store entries".to_string()));
        }

        Ok(())
    }

    async fn search_similar(&self, embedding: Vec<f32>, k: usize, filter: Option<Value>) -> Result<Vec<MemoryEntry>> {
        let mut query = json!({
            "size": k,
            "query": {
                "script_score": {
                    "query": { "match_all": {} },
                    "script": {
                        "source": "cosineSimilarity(params.query_vector, 'embedding') + 1.0",
                        "params": {
                            "query_vector": embedding
                        }
                    }
                }
            }
        });

        if let Some(f) = filter {
            query["query"]["script_score"]["query"] = json!({
                "bool": {
                    "must": [
                        { "match_all": {} },
                        f
                    ]
                }
            });
        }

        let response = self.client
            .search(SearchParts::Index(&[&*self.index]))
            .body(query)
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        let response_body = response.json::<Value>().await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        let hits = response_body["hits"]["hits"]
            .as_array()
            .ok_or_else(|| Error::OpenSearch("Invalid response format".to_string()))?;

        let mut results = Vec::new();
        for hit in hits {
            let source = hit["_source"].clone();
            let metadata = source["metadata"]
                .as_object()
                .map(|obj| {
                    obj.iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<HashMap<String, Value>>()
                })
                .unwrap_or_default();

            let node_type = source.get("node_type")
                .and_then(|v| v.as_str())
                .map(|s| EntityType::from_str(s))
                .transpose()
                .map_err(|e| Error::OpenSearch(e.to_string()))?;

            let entry = MemoryEntry {
                id: source["id"].as_str()
                    .ok_or_else(|| Error::OpenSearch("Missing id field".to_string()))?
                    .to_string(),
                embedding: serde_json::from_value(source["embedding"].clone())
                    .map_err(|e| Error::OpenSearch(e.to_string()))?,
                metadata,
                created_at: DateTime::parse_from_rfc3339(source["created_at"].as_str().unwrap_or_default())
                    .map_err(|e| Error::OpenSearch(e.to_string()))?
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(source["updated_at"].as_str().unwrap_or_default())
                    .map_err(|e| Error::OpenSearch(e.to_string()))?
                    .with_timezone(&Utc),
                content: source["content"].as_str().unwrap_or_default().to_string(),
                node_type,
            };
            results.push(entry);
        }

        Ok(results)
    }

    async fn get_by_node_type(&self, node_type: EntityType, limit: usize) -> Result<Vec<MemoryEntry>> {
        let query = json!({
            "size": limit,
            "query": {
                "term": {
                    "node_type": node_type.to_string()
                }
            }
        });

        self.search_with_query(query).await
    }

    async fn get_by_time_range(&self, range: TemporalRange, limit: usize) -> Result<Vec<MemoryEntry>> {
        let mut range_query = json!({});

        if let Some(start) = range.start {
            range_query["gte"] = json!(start.0.to_rfc3339());
        }

        if let Some(end) = range.end {
            range_query["lte"] = json!(end.0.to_rfc3339());
        }

        let query = json!({
            "size": limit,
            "query": {
                "range": {
                    "created_at": range_query
                }
            }
        });

        self.search_with_query(query).await
    }

    async fn get_for_node(&self, node_id: Uuid, limit: usize) -> Result<Vec<MemoryEntry>> {
        let query = json!({
            "size": limit,
            "query": {
                "bool": {
                    "should": [
                        { "term": { "source_id": node_id.to_string() } },
                        { "term": { "target_id": node_id.to_string() } }
                    ]
                }
            }
        });

        self.search_with_query(query).await
    }

    async fn get_for_edge(&self, source_id: Uuid, target_id: Uuid, limit: usize) -> Result<Vec<MemoryEntry>> {
        let query = json!({
            "size": limit,
            "query": {
                "bool": {
                    "must": [
                        { "term": { "source_id": source_id.to_string() } },
                        { "term": { "target_id": target_id.to_string() } }
                    ]
                }
            }
        });

        self.search_with_query(query).await
    }
}

impl MemorySystem {
    /// Create a new OpenSearch client from config
    pub async fn create_client(config: &Config) -> Result<Arc<OpenSearch>> {
        let url = Url::parse(&config.opensearch_endpoint)
            .map_err(|e| Error::OpenSearch(format!("Invalid OpenSearch URL: {}", e)))?;
        
        let conn_pool = SingleNodeConnectionPool::new(url);
        let transport = if !config.memory_username.is_empty() && !config.memory_password.is_empty() {
            let credentials = Credentials::Basic(
                config.memory_username.clone(),
                config.memory_password.clone(),
            );
            TransportBuilder::new(conn_pool)
                .auth(credentials)
                .build()
                .map_err(|e| Error::OpenSearch(e.to_string()))?
        } else {
            TransportBuilder::new(conn_pool)
                .build()
                .map_err(|e| Error::OpenSearch(e.to_string()))?
        };
        
        Ok(Arc::new(OpenSearch::new(transport)))
    }

    /// Create a new memory system
    pub async fn new(client: Arc<OpenSearch>, index: String, embedding_dim: usize) -> Result<Self> {
        // Create index if it doesn't exist
        let index_exists = client
            .indices()
            .exists(IndicesExistsParts::Index(&[&index]))
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?
            .status_code()
            .is_success();

        if !index_exists {
            let mapping = json!({
                "mappings": {
                    "properties": {
                        "id": { "type": "keyword" },
                        "embedding": {
                            "type": "dense_vector",
                            "dims": embedding_dim,
                            "index": true,
                            "similarity": "cosine"
                        },
                        "metadata": { "type": "object" },
                        "created_at": { "type": "date" },
                        "node_type": { "type": "keyword" },
                        "source_id": { "type": "keyword" },
                        "target_id": { "type": "keyword" }
                    }
                }
            });

            client
                .indices()
                .create(IndicesCreateParts::Index(&index))
                .body(mapping)
                .send()
                .await
                .map_err(|e| Error::OpenSearch(e.to_string()))?;
        }

        Ok(Self {
            client,
            index,
            embedding_dim,
        })
    }

    /// Search with a specific query
    async fn search_with_query(&self, query: Value) -> Result<Vec<MemoryEntry>> {
        let response = self.client
            .search(SearchParts::Index(&[&self.index]))
            .body(query)
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        let response_body = response.json::<Value>().await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        let hits = response_body["hits"]["hits"]
            .as_array()
            .ok_or_else(|| Error::OpenSearch("Invalid response format".to_string()))?;

        let mut results = Vec::new();
        for hit in hits {
            let source = hit["_source"].clone();
            let metadata = source["metadata"]
                .as_object()
                .map(|obj| {
                    obj.iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<HashMap<String, Value>>()
                })
                .unwrap_or_default();

            let node_type = source.get("node_type")
                .and_then(|v| v.as_str())
                .map(|s| EntityType::from_str(s))
                .transpose()
                .map_err(|e| Error::OpenSearch(e.to_string()))?;

            let entry = MemoryEntry {
                id: source["id"].as_str()
                    .ok_or_else(|| Error::OpenSearch("Missing id field".to_string()))?
                    .to_string(),
                embedding: serde_json::from_value(source["embedding"].clone())
                    .map_err(|e| Error::OpenSearch(e.to_string()))?,
                metadata,
                created_at: DateTime::parse_from_rfc3339(source["created_at"].as_str().unwrap_or_default())
                    .map_err(|e| Error::OpenSearch(e.to_string()))?
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(source["updated_at"].as_str().unwrap_or_default())
                    .map_err(|e| Error::OpenSearch(e.to_string()))?
                    .with_timezone(&Utc),
                content: source["content"].as_str().unwrap_or_default().to_string(),
                node_type,
            };
            results.push(entry);
        }

        Ok(results)
    }

    /// Create a new memory entry
    pub fn new_entry(
        id: String,
        content: String,
        embedding: Option<Vec<f32>>,
        metadata: HashMap<String, Value>,
        node_type: Option<EntityType>,
    ) -> MemoryEntry {
        MemoryEntry {
            id,
            content,
            embedding,
            metadata,
            node_type,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Ensure the memory index exists
    async fn ensure_index(&self) -> Result<()> {
        let exists = self.client
            .indices()
            .exists(IndicesExistsParts::Index(&[&self.index]))
            .send()
            .await
            .map_err(|e| Error::OpenSearch(e.to_string()))?;

        if !exists.status_code().is_success() {
            let mappings = json!({
                "properties": {
                    "id": { "type": "keyword" },
                    "created_at": { "type": "date" },
                    "updated_at": { "type": "date" },
                    "content": { "type": "text" },
                    "metadata": { "type": "object" },
                    "embedding": {
                        "type": "dense_vector",
                        "dims": self.embedding_dim,
                        "index": true,
                        "similarity": "cosine"
                    },
                    "node_type": { "type": "keyword" }
                }
            });

            self.client
                .indices()
                .create(IndicesCreateParts::Index(&self.index))
                .body(json!({
                    "settings": {
                        "number_of_shards": 1,
                        "number_of_replicas": 1
                    },
                    "mappings": mappings
                }))
                .send()
                .await
                .map_err(|e| Error::OpenSearch(e.to_string()))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use crate::types::Timestamp;
    use opensearch::http::transport::Transport;

    #[tokio::test]
    async fn test_memory_operations() {
        let transport = Transport::single_node("http://localhost:9200").unwrap();
        let client = Arc::new(OpenSearch::new(transport));
        let memory = MemorySystem::new(client, "test_memory".to_string(), 384).await.unwrap();
        
        // Clear any existing data
        memory.ensure_index().await.unwrap();
        
        let now = Utc::now();
        let entry = MemoryEntry {
            id: "test".to_string(),
            created_at: now,
            updated_at: now,
            content: "test memory".to_string(),
            metadata: HashMap::new(),
            embedding: Some(vec![0.1; 384]),
            node_type: Some(EntityType::Person),
        };

        // Store entry
        assert!(memory.store(entry.clone()).await.is_ok());

        // Test similar search
        let similar = memory.search_similar(vec![0.1; 384], 1, None).await.unwrap();
        assert_eq!(similar.len(), 1);
        assert_eq!(similar[0].id, entry.id);

        // Test get by node type
        let by_type = memory.get_by_node_type(EntityType::Person, 1).await.unwrap();
        assert_eq!(by_type.len(), 1);
        assert_eq!(by_type[0].id, entry.id);

        // Test get by time range
        let range = TemporalRange {
            start: Some(Timestamp(now - chrono::Duration::hours(1))),
            end: Some(Timestamp(now + chrono::Duration::hours(1))),
        };
        let by_time = memory.get_by_time_range(range, 1).await.unwrap();
        assert_eq!(by_time.len(), 1);
        assert_eq!(by_time[0].id, entry.id);
    }

    #[test]
    fn test_memory_entry() {
        let now = Utc::now();
        let entry = MemoryEntry {
            id: "test".to_string(),
            created_at: now,
            updated_at: now,
            content: "test memory".to_string(),
            metadata: HashMap::new(),
            embedding: None,
            node_type: None,
        };

        assert_eq!(entry.id, "test");
        assert_eq!(entry.content, "test memory");
        assert!(entry.metadata.is_empty());
        assert!(entry.embedding.is_none());
    }
} 
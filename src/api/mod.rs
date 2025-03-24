use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use uuid::Uuid;
use aws_config;
use aws_sdk_dynamodb::Client as DynamoClient;
use aws_sdk_neptune::Client as NeptuneClient;
use opensearch::{
    OpenSearch,
    http::transport::{SingleNodeConnectionPool, Transport, TransportBuilder},
    auth::Credentials,
};
use url::Url;

use crate::{
    error::{Error, Result},
    graph::neptune::NeptuneGraph,
    memory::MemorySystem,
    rag::{RAGSystem, RAG, RAGConfig},
    temporal::{DynamoDBTemporal, Temporal, new_temporal_dynamodb},
    types::{Node, Edge, EntityId, Timestamp, TemporalRange, NodeId, EntityType, Properties, EdgeId},
    Context,
    Config,
    aws::dynamodb::DynamoDBClient,
};

/// Request to store information in the graph
#[derive(Debug, Clone, Deserialize)]
pub struct StoreRequest {
    /// Content to store
    pub content: String,
}

/// Request to query knowledge from the graph
#[derive(Debug, Clone, Deserialize)]
pub struct QueryRequest {
    /// Query text
    pub query: String,
    /// Optional timestamp to query at
    pub timestamp: Option<DateTime<Utc>>,
    /// Optional time window in seconds
    pub time_window: Option<i64>,
}

/// Response from a knowledge query
#[derive(Debug, Clone, Serialize)]
pub struct QueryResponse {
    /// Query results
    pub results: Vec<QueryResult>,
    /// Relevant context
    pub context: Vec<String>,
}

/// Single result from a knowledge query
#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    /// Node ID
    pub id: Uuid,
    /// Content
    pub content: String,
    /// Confidence score
    pub confidence: f64,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// API state
pub struct ApiState {
    /// Application context
    pub context: Arc<Context>,
    /// Graph store
    pub graph: Arc<NeptuneGraph>,
    /// Memory store
    pub memory: Arc<MemorySystem>,
    /// RAG system
    pub rag: Arc<RAGSystem>,
    /// Temporal store for nodes
    pub temporal: Arc<DynamoDBTemporal<Node, DynamoClient>>,
    /// Temporal store for edges
    pub edge_temporal: Arc<DynamoDBTemporal<Edge, DynamoClient>>,
}

/// Create a new API router
pub fn new_router(
    context: Arc<Context>,
    graph: Arc<NeptuneGraph>,
    memory: Arc<MemorySystem>,
    rag: Arc<RAGSystem>,
    temporal: Arc<DynamoDBTemporal<Node, DynamoClient>>,
    edge_temporal: Arc<DynamoDBTemporal<Edge, DynamoClient>>,
) -> Router {
    let state = Arc::new(ApiState {
        context,
        graph,
        memory,
        rag,
        temporal,
        edge_temporal,
    });

    Router::new()
        .route("/store", post(store_information))
        .route("/query", post(query_knowledge))
        .route("/node/:id", get(get_node))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

/// Store information in the graph
async fn store_information(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<StoreRequest>,
) -> impl IntoResponse {
    match state.rag.update_graph(&request.content, Utc::now()).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Query knowledge from the graph
async fn query_knowledge(
    State(state): State<Arc<ApiState>>,
    Json(request): Json<QueryRequest>,
) -> impl IntoResponse {
    let timestamp = request.timestamp.unwrap_or_else(|| Utc::now());
    let time_window = request.time_window.unwrap_or(3600); // Default 1 hour
    
    // Get relevant context using RAG
    let context = match state.rag.extract_entities(&request.query).await {
        Ok(entities) => entities.into_iter().map(|e| e.text).collect(),
        Err(_) => vec![],
    };
    
    // Get temporal nodes in range
    let start_time = timestamp - Duration::seconds(time_window);
    let mut results = Vec::new();
    
    // Query each entity ID from the context
    for entity_text in context.iter() {
        // Create an entity ID for the query
        let entity_id = EntityId::from(entity_text.clone());
        
        // Query the temporal store
        if let Ok(temporal_results) = state.temporal.query_between(
            &entity_id,
            start_time,
            timestamp,
        ).await {
            for result in temporal_results {
                let node = result.data;
                results.push(QueryResult {
                    id: node.id.0,
                    content: node.label,
                    confidence: 1.0, // TODO: Implement proper confidence scoring
                    timestamp: result.timestamp.0,
                });
            }
        }
    }
        
    (
        StatusCode::OK,
        Json(QueryResponse {
            results,
            context,
        })
    )
}

/// Get a specific node by ID
async fn get_node(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let entity_id = EntityId {
        entity_type: EntityType::Node,
        id,
    };
    
    match state.temporal.query_latest(&entity_id).await {
        Ok(Some(result)) => (StatusCode::OK, Json(result.data)),
        Ok(None) => {
            let error_node = Node {
                id: NodeId(Uuid::new_v4()),
                entity_type: EntityType::Node,
                label: "Node not found".to_string(),
                properties: Properties::new(),
                valid_time: TemporalRange::from_now(),
                transaction_time: TemporalRange::from_now(),
            };
            (StatusCode::NOT_FOUND, Json(error_node))
        },
        Err(_) => {
            let error_node = Node {
                id: NodeId(Uuid::new_v4()),
                entity_type: EntityType::Node,
                label: "Failed to get node".to_string(),
                properties: Properties::new(),
                valid_time: TemporalRange::from_now(),
                transaction_time: TemporalRange::from_now(),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(error_node))
        }
    }
}

impl ApiState {
    pub async fn new(config: Config) -> Result<Self> {
        // Initialize AWS config
        let aws_config = aws_config::from_env()
            .region(aws_sdk_dynamodb::config::Region::new(config.aws_region.clone()))
            .load()
            .await;

        // Create AWS clients
        let dynamo_client = Arc::new(DynamoClient::new(&aws_config));
        let neptune_client = Arc::new(NeptuneClient::new(&aws_config));

        // Initialize context
        let context = Arc::new(Context::new(config.clone()));

        // Initialize graph store
        let graph = Arc::new(NeptuneGraph::new(&config).await?);

        // Initialize OpenSearch client for memory store
        let url = Url::parse(&config.memory_url)?;
        let pool = SingleNodeConnectionPool::new(url);
        let transport = TransportBuilder::new(pool)
            .auth(Credentials::Basic(
                config.memory_username.clone(),
                config.memory_password.clone(),
            ))
            .build()
            .map_err(|e| Error::OpenSearch(e.to_string()))?;
        let opensearch_client = Arc::new(OpenSearch::new(transport));

        // Initialize memory store
        let memory = Arc::new(MemorySystem::new(
            opensearch_client,
            "memories".to_string(),
            384, // Standard embedding dimension
        ).await?);

        // Initialize temporal stores
        let temporal = Arc::new(DynamoDBTemporal::<Node, _>::new(
            dynamo_client.clone(),
            config.temporal_table.clone(),
        ));

        let edge_temporal = Arc::new(DynamoDBTemporal::<Edge, _>::new(
            dynamo_client.clone(),
            config.temporal_table.clone(),
        ));

        // Initialize RAG system
        let rag = Arc::new(RAGSystem::new(
            RAGConfig::default(),
            memory.clone(),
            temporal.clone(),
            edge_temporal.clone(),
        ));

        Ok(Self {
            context,
            graph,
            memory,
            rag,
            temporal,
            edge_temporal,
        })
    }
}

pub struct GraphAPI {
    temporal: Arc<DynamoDBTemporal<Node, aws_sdk_dynamodb::Client>>,
    edge_temporal: Arc<DynamoDBTemporal<Edge, aws_sdk_dynamodb::Client>>,
}

impl GraphAPI {
    pub async fn new(config: &Config) -> Result<Self> {
        let temporal = Arc::new(new_temporal_dynamodb(config).await?);
        let edge_temporal = Arc::new(new_temporal_dynamodb(config).await?);

        Ok(Self {
            temporal,
            edge_temporal,
        })
    }

    pub async fn add_node(&self, node_id: NodeId, node: Node, valid_time: TemporalRange) -> Result<()> {
        let entity_id = EntityId {
            entity_type: EntityType::Node,
            id: node_id.0.to_string(),
        };
        Temporal::store(&*self.temporal, entity_id, node, valid_time).await
    }

    pub async fn add_edge(&self, edge_id: EdgeId, edge: Edge, valid_time: TemporalRange) -> Result<()> {
        let entity_id = EntityId {
            entity_type: EntityType::Edge,
            id: edge_id.0.to_string(),
        };
        Temporal::store(&*self.edge_temporal, entity_id, edge, valid_time).await
    }
} 
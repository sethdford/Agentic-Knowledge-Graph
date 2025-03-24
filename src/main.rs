use axum::{routing::{get, post}, Router};
use std::sync::Arc;
use tokio;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tower_http::trace::TraceLayer;
use aws_config;
use aws_sdk_dynamodb::Client as DynamoClient;
use aws_sdk_neptune::Client as NeptuneClient;
use opensearch::{
    OpenSearch,
    http::transport::{SingleNodeConnectionPool, Transport, TransportBuilder},
    auth::Credentials,
};
use url::Url;

use graph::{
    api,
    mcp,
    graph::neptune::NeptuneGraph,
    memory::{OpenSearchMemory, MemorySystem},
    rag::{RAGSystem, RAGConfig},
    temporal::DynamoDBTemporal,
    types::{Node, Edge},
    Config, Context,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load config
    let config = Config::from_env()?;
    let context = Arc::new(Context::new(config.clone()));

    // Initialize AWS config
    let aws_config = aws_config::from_env()
        .region(aws_sdk_dynamodb::config::Region::new(config.aws_region.clone()))
        .load()
        .await;

    // Create AWS clients
    let dynamo_client = Arc::new(DynamoClient::new(&aws_config));
    let neptune_client = Arc::new(NeptuneClient::new(&aws_config));

    // Initialize OpenSearch client
    let url = Url::parse(&config.memory_url)?;
    let pool = SingleNodeConnectionPool::new(url);
    let transport = TransportBuilder::new(pool)
        .auth(Credentials::Basic(
            config.memory_username.clone(),
            config.memory_password.clone(),
        ))
        .build()?;
    let opensearch_client = Arc::new(OpenSearch::new(transport));

    // Initialize graph store
    let graph = Arc::new(NeptuneGraph::new(&config).await?);

    // Initialize temporal stores
    let temporal = Arc::new(DynamoDBTemporal::<Node, _>::new(
        dynamo_client.clone(),
        config.temporal_table.clone(),
    ));

    let temporal_edges = Arc::new(DynamoDBTemporal::<Edge, _>::new(
        dynamo_client.clone(),
        config.temporal_table.clone(),
    ));

    // Initialize memory store
    let memory = Arc::new(MemorySystem::new(
        opensearch_client,
        "memories".to_string(),
        384,
    ).await?);

    // Initialize RAG system
    let rag = Arc::new(RAGSystem::new(
        RAGConfig::default(),
        memory.clone(),
        temporal.clone(),
        temporal_edges.clone(),
    ));

    // Create router with API routes
    let app = Router::new()
        .merge(api::new_router(
            context,
            graph,
            memory,
            rag,
            temporal,
            temporal_edges,
        ))
        .layer(TraceLayer::new_for_http());

    // Start server
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("Listening on {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
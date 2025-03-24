use aws_config::meta::region::RegionProviderChain;
use aws_sdk_dynamodb::Client as DynamoDbClient;
use aws_sdk_opensearch::Client as OpenSearchClient;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_sqs::Client as SqsClient;
use aws_sdk_dynamodb::types::{AttributeDefinition, KeySchemaElement, ScalarAttributeType, KeyType, BillingMode};
use aws_sdk_sqs::types::QueueAttributeName;
use std::sync::Arc;
use aws_config::Region;

use crate::{Context, Error, Result};

pub mod dynamodb;

/// AWS service clients wrapper
#[derive(Clone)]
pub struct AwsClients {
    pub dynamodb: Arc<DynamoDbClient>,
    pub opensearch: Arc<OpenSearchClient>,
    pub s3: Arc<S3Client>,
    pub sqs: Arc<SqsClient>,
}

impl AwsClients {
    /// Create new AWS service clients
    pub async fn new(context: &Context) -> Result<Self> {
        let region = Region::new(context.config.aws_region.clone());
        let region_provider = RegionProviderChain::first_try(region);
            
        let shared_config = aws_config::from_env()
            .region(region_provider)
            .load()
            .await;
            
        Ok(Self {
            dynamodb: Arc::new(DynamoDbClient::new(&shared_config)),
            opensearch: Arc::new(OpenSearchClient::new(&shared_config)),
            s3: Arc::new(S3Client::new(&shared_config)),
            sqs: Arc::new(SqsClient::new(&shared_config)),
        })
    }
}

/// DynamoDB table initialization
pub async fn init_dynamodb(client: &DynamoDbClient, table_name: &str) -> Result<()> {
    let table_exists = client
        .describe_table()
        .table_name(table_name)
        .send()
        .await
        .is_ok();
        
    if !table_exists {
        let id_attr = AttributeDefinition::builder()
            .attribute_name("id")
            .attribute_type(ScalarAttributeType::S)
            .build()
            .unwrap();

        let key_schema = KeySchemaElement::builder()
            .attribute_name("id")
            .key_type(KeyType::Hash)
            .build()
            .unwrap();

        client
            .create_table()
            .table_name(table_name)
            .key_schema(key_schema)
            .attribute_definitions(id_attr)
            .billing_mode(BillingMode::PayPerRequest)
            .send()
            .await
            .map_err(|e| Error::AwsError(format!("Failed to create DynamoDB table: {}", e)))?;
    }
    
    Ok(())
}

/// S3 bucket initialization
pub async fn init_s3(client: &S3Client, bucket_name: &str) -> Result<()> {
    let bucket_exists = client
        .head_bucket()
        .bucket(bucket_name)
        .send()
        .await
        .is_ok();
        
    if !bucket_exists {
        client
            .create_bucket()
            .bucket(bucket_name)
            .send()
            .await
            .map_err(|e| Error::AwsError(format!("Failed to create S3 bucket: {}", e)))?;
    }
    
    Ok(())
}

/// SQS queue initialization
pub async fn init_sqs(client: &SqsClient, queue_name: &str) -> Result<String> {
    let mut attributes = std::collections::HashMap::new();
    attributes.insert(
        QueueAttributeName::VisibilityTimeout,
        "60".to_string(),
    );
    attributes.insert(
        QueueAttributeName::MessageRetentionPeriod,
        "86400".to_string(),
    );

    let queue_url = client
        .create_queue()
        .queue_name(queue_name)
        .set_attributes(Some(attributes))
        .send()
        .await
        .map_err(|e| Error::AwsError(format!("Failed to create SQS queue: {}", e)))?
        .queue_url()
        .ok_or_else(|| Error::AwsError("No queue URL returned".to_string()))?
        .to_string();
        
    Ok(queue_url)
}

/// AWS resource initialization
pub async fn init_resources(clients: &AwsClients, context: &Context) -> Result<()> {
    // Initialize DynamoDB table
    init_dynamodb(&clients.dynamodb, &context.config.dynamodb_table).await?;
    
    // Initialize S3 bucket
    init_s3(&clients.s3, &context.config.s3_bucket).await?;
    
    // Initialize SQS queue and update config with queue URL
    let queue_name = context.config.sqs_queue_url.split('/').last()
        .ok_or_else(|| Error::AwsError("Invalid SQS queue URL".to_string()))?;
    init_sqs(&clients.sqs, queue_name).await?;
    
    Ok(())
} 
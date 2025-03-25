use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Mutex;

/// Configuration for AWS services and application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// AWS Region
    pub aws_region: String,
    
    /// Neptune endpoint URL
    pub neptune_endpoint: String,
    
    /// Neptune port
    pub neptune_port: u16,
    
    /// Neptune IAM authentication enabled
    pub neptune_iam_auth: bool,
    
    /// OpenSearch endpoint URL
    pub opensearch_endpoint: String,
    
    /// DynamoDB table name for metadata
    pub dynamodb_table: String,
    
    /// DynamoDB table for temporal data
    pub temporal_table: String,
    
    /// S3 bucket for raw data storage
    pub s3_bucket: String,
    
    /// SQS queue URL for async processing
    pub sqs_queue_url: String,
    
    /// Memory service URL
    pub memory_url: String,
    
    /// Memory service username
    pub memory_username: String,
    
    /// Memory service password
    pub memory_password: String,
    
    /// Maximum number of retries for AWS operations
    pub max_retries: u32,
    
    /// Connection timeout in seconds
    pub connection_timeout: u32,
    
    /// Maximum connections in the pool
    pub max_connections: u32,
    
    /// Entity extraction confidence threshold
    pub entity_extraction_confidence: f32,
    
    /// Relationship detection confidence threshold
    pub relationship_detection_confidence: f32,
    
    /// Maximum context window size
    pub max_context_window: usize,
    
    /// Batch size for processing
    pub batch_size: usize,
    
    /// Memory size for embedding models
    pub memory_size: usize,
}

impl Config {
    /// Create a new configuration from environment variables
    pub fn from_env() -> Result<Self, env::VarError> {
        Ok(Self {
            aws_region: env::var("AWS_REGION")?,
            neptune_endpoint: env::var("NEPTUNE_ENDPOINT")?,
            neptune_port: env::var("NEPTUNE_PORT").unwrap_or_else(|_| "8182".to_string()).parse().unwrap_or(8182),
            neptune_iam_auth: env::var("NEPTUNE_IAM_AUTH").unwrap_or_else(|_| "false".to_string()).parse().unwrap_or(false),
            opensearch_endpoint: env::var("OPENSEARCH_ENDPOINT")?,
            dynamodb_table: env::var("DYNAMODB_TABLE")?,
            temporal_table: env::var("TEMPORAL_TABLE")?,
            s3_bucket: env::var("S3_BUCKET")?,
            sqs_queue_url: env::var("SQS_QUEUE_URL")?,
            memory_url: env::var("MEMORY_URL")?,
            memory_username: env::var("MEMORY_USERNAME").unwrap_or_default(),
            memory_password: env::var("MEMORY_PASSWORD").unwrap_or_default(),
            max_retries: env::var("MAX_RETRIES").unwrap_or_else(|_| "3".to_string()).parse().unwrap_or(3),
            connection_timeout: env::var("CONNECTION_TIMEOUT").unwrap_or_else(|_| "30".to_string()).parse().unwrap_or(30),
            max_connections: env::var("MAX_CONNECTIONS").unwrap_or_else(|_| "100".to_string()).parse().unwrap_or(100),
            entity_extraction_confidence: env::var("ENTITY_EXTRACTION_CONFIDENCE").unwrap_or_else(|_| "0.7".to_string()).parse().unwrap_or(0.7),
            relationship_detection_confidence: env::var("RELATIONSHIP_DETECTION_CONFIDENCE").unwrap_or_else(|_| "0.7".to_string()).parse().unwrap_or(0.7),
            max_context_window: env::var("MAX_CONTEXT_WINDOW").unwrap_or_else(|_| "512".to_string()).parse().unwrap_or(512),
            batch_size: env::var("BATCH_SIZE").unwrap_or_else(|_| "32".to_string()).parse().unwrap_or(32),
            memory_size: env::var("MEMORY_SIZE").unwrap_or_else(|_| "384".to_string()).parse().unwrap_or(384),
        })
    }

    /// Create a new configuration for testing
    pub fn for_testing() -> Self {
        Self {
            aws_region: "us-east-1".to_string(),
            neptune_endpoint: "localhost".to_string(),
            neptune_port: 8182,
            neptune_iam_auth: false,
            opensearch_endpoint: "http://localhost:9200".to_string(),
            dynamodb_table: "test-table".to_string(),
            temporal_table: "test-temporal-table".to_string(),
            s3_bucket: "test-bucket".to_string(),
            sqs_queue_url: "http://localhost:4566/000000000000/test-queue".to_string(),
            memory_url: "http://localhost:9200".to_string(),
            memory_username: "".to_string(),
            memory_password: "".to_string(),
            max_retries: 3,
            connection_timeout: 5,
            max_connections: 10,
            entity_extraction_confidence: 0.7,
            relationship_detection_confidence: 0.7,
            max_context_window: 512,
            batch_size: 32,
            memory_size: 384,
        }
    }

    /// Create a new configuration with custom Neptune settings
    pub fn new(neptune_endpoint: String, max_connections: u32, connection_timeout: u32) -> Self {
        Self {
            aws_region: "us-east-1".to_string(),
            neptune_endpoint,
            neptune_port: 8182,
            neptune_iam_auth: false,
            opensearch_endpoint: "http://localhost:9200".to_string(),
            dynamodb_table: "graph-table".to_string(),
            temporal_table: "graph-temporal-table".to_string(),
            s3_bucket: "graph-bucket".to_string(),
            sqs_queue_url: "http://localhost:4566/000000000000/graph-queue".to_string(),
            memory_url: "http://localhost:9200".to_string(),
            memory_username: "".to_string(),
            memory_password: "".to_string(),
            max_retries: 3,
            connection_timeout,
            max_connections,
            entity_extraction_confidence: 0.7,
            relationship_detection_confidence: 0.7,
            max_context_window: 512,
            batch_size: 32,
            memory_size: 384,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            aws_region: "us-east-1".to_string(),
            neptune_endpoint: "localhost".to_string(),
            neptune_port: 8182,
            neptune_iam_auth: false,
            opensearch_endpoint: "http://localhost:9200".to_string(),
            dynamodb_table: "graph-table".to_string(),
            temporal_table: "graph-temporal-table".to_string(),
            s3_bucket: "graph-bucket".to_string(),
            sqs_queue_url: "http://localhost:4566/000000000000/graph-queue".to_string(),
            memory_url: "http://localhost:9200".to_string(),
            memory_username: "".to_string(),
            memory_password: "".to_string(),
            max_retries: 3,
            connection_timeout: 30,
            max_connections: 50,
            entity_extraction_confidence: 0.7,
            relationship_detection_confidence: 0.7,
            max_context_window: 512,
            batch_size: 32,
            memory_size: 384,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    // This mutex ensures tests don't run in parallel to avoid env var conflicts
    lazy_static::lazy_static! {
        static ref TEST_MUTEX: Mutex<()> = Mutex::new(());
    }

    #[test]
    fn test_config_from_env() {
        let _lock = TEST_MUTEX.lock().unwrap();
        
        // Save current environment variables
        let save_aws_region = env::var("AWS_REGION").ok();
        let save_neptune_endpoint = env::var("NEPTUNE_ENDPOINT").ok();
        let save_opensearch_endpoint = env::var("OPENSEARCH_ENDPOINT").ok();
        let save_dynamodb_table = env::var("DYNAMODB_TABLE").ok();
        let save_temporal_table = env::var("TEMPORAL_TABLE").ok();
        let save_s3_bucket = env::var("S3_BUCKET").ok();
        let save_sqs_queue_url = env::var("SQS_QUEUE_URL").ok();
        let save_memory_url = env::var("MEMORY_URL").ok();
        let save_max_retries = env::var("MAX_RETRIES").ok();
        let save_connection_timeout = env::var("CONNECTION_TIMEOUT").ok();
        let save_max_connections = env::var("MAX_CONNECTIONS").ok();

        // Set test environment variables
        env::set_var("AWS_REGION", "us-west-2");
        env::set_var("NEPTUNE_ENDPOINT", "test-neptune:8182");
        env::set_var("OPENSEARCH_ENDPOINT", "test-opensearch:9200");
        env::set_var("DYNAMODB_TABLE", "test-table");
        env::set_var("TEMPORAL_TABLE", "test-temporal-table");
        env::set_var("S3_BUCKET", "test-bucket");
        env::set_var("SQS_QUEUE_URL", "test-queue");
        env::set_var("MEMORY_URL", "http://localhost:9200");
        env::set_var("MAX_RETRIES", "5");
        env::set_var("CONNECTION_TIMEOUT", "10");
        env::set_var("MAX_CONNECTIONS", "50");

        let config = Config::from_env().unwrap();
        assert_eq!(config.aws_region, "us-west-2");
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.connection_timeout, 10);
        assert_eq!(config.max_connections, 50);

        // Restore environment variables
        if let Some(v) = save_aws_region { env::set_var("AWS_REGION", v); } else { env::remove_var("AWS_REGION"); }
        if let Some(v) = save_neptune_endpoint { env::set_var("NEPTUNE_ENDPOINT", v); } else { env::remove_var("NEPTUNE_ENDPOINT"); }
        if let Some(v) = save_opensearch_endpoint { env::set_var("OPENSEARCH_ENDPOINT", v); } else { env::remove_var("OPENSEARCH_ENDPOINT"); }
        if let Some(v) = save_dynamodb_table { env::set_var("DYNAMODB_TABLE", v); } else { env::remove_var("DYNAMODB_TABLE"); }
        if let Some(v) = save_temporal_table { env::set_var("TEMPORAL_TABLE", v); } else { env::remove_var("TEMPORAL_TABLE"); }
        if let Some(v) = save_s3_bucket { env::set_var("S3_BUCKET", v); } else { env::remove_var("S3_BUCKET"); }
        if let Some(v) = save_sqs_queue_url { env::set_var("SQS_QUEUE_URL", v); } else { env::remove_var("SQS_QUEUE_URL"); }
        if let Some(v) = save_memory_url { env::set_var("MEMORY_URL", v); } else { env::remove_var("MEMORY_URL"); }
        if let Some(v) = save_max_retries { env::set_var("MAX_RETRIES", v); } else { env::remove_var("MAX_RETRIES"); }
        if let Some(v) = save_connection_timeout { env::set_var("CONNECTION_TIMEOUT", v); } else { env::remove_var("CONNECTION_TIMEOUT"); }
        if let Some(v) = save_max_connections { env::set_var("MAX_CONNECTIONS", v); } else { env::remove_var("MAX_CONNECTIONS"); }
    }

    #[test]
    fn test_config_defaults() {
        let _lock = TEST_MUTEX.lock().unwrap();
        
        // Save current environment variables
        let save_aws_region = env::var("AWS_REGION").ok();
        let save_neptune_endpoint = env::var("NEPTUNE_ENDPOINT").ok();
        let save_opensearch_endpoint = env::var("OPENSEARCH_ENDPOINT").ok();
        let save_dynamodb_table = env::var("DYNAMODB_TABLE").ok();
        let save_temporal_table = env::var("TEMPORAL_TABLE").ok();
        let save_s3_bucket = env::var("S3_BUCKET").ok();
        let save_sqs_queue_url = env::var("SQS_QUEUE_URL").ok();
        let save_memory_url = env::var("MEMORY_URL").ok();
        let save_max_retries = env::var("MAX_RETRIES").ok();
        let save_connection_timeout = env::var("CONNECTION_TIMEOUT").ok();
        let save_max_connections = env::var("MAX_CONNECTIONS").ok();

        // Set only required variables
        env::set_var("AWS_REGION", "us-west-2");
        env::set_var("NEPTUNE_ENDPOINT", "test-neptune:8182");
        env::set_var("OPENSEARCH_ENDPOINT", "test-opensearch:9200");
        env::set_var("DYNAMODB_TABLE", "test-table");
        env::set_var("TEMPORAL_TABLE", "test-temporal-table");
        env::set_var("S3_BUCKET", "test-bucket");
        env::set_var("SQS_QUEUE_URL", "test-queue");
        env::set_var("MEMORY_URL", "http://localhost:9200");
        
        // Clear optional variables
        env::remove_var("MAX_RETRIES");
        env::remove_var("CONNECTION_TIMEOUT");
        env::remove_var("MAX_CONNECTIONS");

        let config = Config::from_env().unwrap();
        assert_eq!(config.max_retries, 3); // Default value
        assert_eq!(config.connection_timeout, 30); // Default value
        assert_eq!(config.max_connections, 100); // Default value

        // Restore environment variables
        if let Some(v) = save_aws_region { env::set_var("AWS_REGION", v); } else { env::remove_var("AWS_REGION"); }
        if let Some(v) = save_neptune_endpoint { env::set_var("NEPTUNE_ENDPOINT", v); } else { env::remove_var("NEPTUNE_ENDPOINT"); }
        if let Some(v) = save_opensearch_endpoint { env::set_var("OPENSEARCH_ENDPOINT", v); } else { env::remove_var("OPENSEARCH_ENDPOINT"); }
        if let Some(v) = save_dynamodb_table { env::set_var("DYNAMODB_TABLE", v); } else { env::remove_var("DYNAMODB_TABLE"); }
        if let Some(v) = save_temporal_table { env::set_var("TEMPORAL_TABLE", v); } else { env::remove_var("TEMPORAL_TABLE"); }
        if let Some(v) = save_s3_bucket { env::set_var("S3_BUCKET", v); } else { env::remove_var("S3_BUCKET"); }
        if let Some(v) = save_sqs_queue_url { env::set_var("SQS_QUEUE_URL", v); } else { env::remove_var("SQS_QUEUE_URL"); }
        if let Some(v) = save_memory_url { env::set_var("MEMORY_URL", v); } else { env::remove_var("MEMORY_URL"); }
        if let Some(v) = save_max_retries { env::set_var("MAX_RETRIES", v); } else { env::remove_var("MAX_RETRIES"); }
        if let Some(v) = save_connection_timeout { env::set_var("CONNECTION_TIMEOUT", v); } else { env::remove_var("CONNECTION_TIMEOUT"); }
        if let Some(v) = save_max_connections { env::set_var("MAX_CONNECTIONS", v); } else { env::remove_var("MAX_CONNECTIONS"); }
    }

    #[test]
    fn test_config_for_testing() {
        let config = Config::for_testing();
        assert_eq!(config.aws_region, "us-east-1");
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.connection_timeout, 5);
        assert_eq!(config.max_connections, 10);
    }
} 
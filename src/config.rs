use serde::{Deserialize, Serialize};
use std::env;

/// Configuration for AWS services and application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// AWS Region
    pub aws_region: String,
    
    /// Neptune endpoint URL
    pub neptune_endpoint: String,
    
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
}

impl Config {
    /// Create a new configuration from environment variables
    pub fn from_env() -> Result<Self, env::VarError> {
        Ok(Self {
            aws_region: env::var("AWS_REGION")?,
            neptune_endpoint: env::var("NEPTUNE_ENDPOINT")?,
            opensearch_endpoint: env::var("OPENSEARCH_ENDPOINT")?,
            dynamodb_table: env::var("DYNAMODB_TABLE")?,
            temporal_table: env::var("TEMPORAL_TABLE")?,
            s3_bucket: env::var("S3_BUCKET")?,
            sqs_queue_url: env::var("SQS_QUEUE_URL")?,
            memory_url: env::var("MEMORY_URL")?,
            memory_username: env::var("MEMORY_USERNAME").unwrap_or_default(),
            memory_password: env::var("MEMORY_PASSWORD").unwrap_or_default(),
            max_retries: env::var("MAX_RETRIES")?.parse().unwrap_or(3),
            connection_timeout: env::var("CONNECTION_TIMEOUT")?.parse().unwrap_or(30),
            max_connections: env::var("MAX_CONNECTIONS")?.parse().unwrap_or(100),
        })
    }

    /// Create a new configuration for testing
    pub fn for_testing() -> Self {
        Self {
            aws_region: "us-east-1".to_string(),
            neptune_endpoint: "localhost:8182".to_string(),
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
        }
    }

    /// Create a new configuration with custom Neptune settings
    pub fn new(neptune_endpoint: String, max_connections: u32, connection_timeout: u32) -> Self {
        Self {
            aws_region: "us-east-1".to_string(),
            neptune_endpoint,
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
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            aws_region: "us-east-1".to_string(),
            neptune_endpoint: "localhost:8182".to_string(),
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_config_from_env() {
        // Set test environment variables
        env::set_var("AWS_REGION", "us-west-2");
        env::set_var("NEPTUNE_ENDPOINT", "test-neptune:8182");
        env::set_var("OPENSEARCH_ENDPOINT", "test-opensearch:9200");
        env::set_var("DYNAMODB_TABLE", "test-table");
        env::set_var("TEMPORAL_TABLE", "test-temporal-table");
        env::set_var("S3_BUCKET", "test-bucket");
        env::set_var("SQS_QUEUE_URL", "test-queue");
        env::set_var("MAX_RETRIES", "5");
        env::set_var("CONNECTION_TIMEOUT", "10");
        env::set_var("MAX_CONNECTIONS", "50");

        let config = Config::from_env().unwrap();
        assert_eq!(config.aws_region, "us-west-2");
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.connection_timeout, 10);
        assert_eq!(config.max_connections, 50);
    }

    #[test]
    fn test_config_defaults() {
        // Set only required variables
        env::set_var("AWS_REGION", "us-west-2");
        env::set_var("NEPTUNE_ENDPOINT", "test-neptune:8182");
        env::set_var("OPENSEARCH_ENDPOINT", "test-opensearch:9200");
        env::set_var("DYNAMODB_TABLE", "test-table");
        env::set_var("TEMPORAL_TABLE", "test-temporal-table");
        env::set_var("S3_BUCKET", "test-bucket");
        env::set_var("SQS_QUEUE_URL", "test-queue");
        
        // Clear optional variables
        env::remove_var("MAX_RETRIES");
        env::remove_var("CONNECTION_TIMEOUT");
        env::remove_var("MAX_CONNECTIONS");

        let config = Config::from_env().unwrap();
        assert_eq!(config.max_retries, 3); // Default value
        assert_eq!(config.connection_timeout, 30); // Default value
        assert_eq!(config.max_connections, 100); // Default value
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
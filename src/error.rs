use thiserror::Error;
use std::io;
use gremlin_client::GValue;
use url;
use chrono;
use opensearch;
use aws_sdk_dynamodb::{
    error::SdkError,
    Error as DynamoDbError,
};

/// Custom error types for the Zep temporal knowledge graph
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("OpenSearch error: {0}")]
    OpenSearch(String),

    #[error("Neptune error: {0}")]
    Neptune(String),

    #[error("DynamoDB error: {0}")]
    DynamoDB(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Parse error: {0}")]
    Parse(#[from] chrono::ParseError),

    #[error("Value conversion error: {0}")]
    ValueConversion(String),

    #[error("Invalid temporal range: {0}")]
    InvalidTemporalRange(String),

    #[error("Entity not found: {0}")]
    EntityNotFound(String),

    #[error("Invalid data format: {0}")]
    InvalidDataFormat(String),

    #[error("Operation failed: {0}")]
    OperationFailed(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Gremlin error: {0}")]
    Gremlin(String),

    #[error("JSON error: {0}")]
    Json(String),

    #[error("UUID error: {0}")]
    Uuid(String),

    #[error("Backoff error: {0}")]
    Backoff(String),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Edge not found: {0}")]
    EdgeNotFound(String),

    #[error("Connection pool error: {0}")]
    ConnectionPool(String),

    #[error("Retry error: {0}")]
    Retry(String),

    #[error("Temporal overlap detected: {0}")]
    TemporalOverlap(String),

    #[error("Version not found: {0}")]
    VersionNotFound(String),

    #[error("Invalid temporal operation: {0}")]
    InvalidTemporalOperation(String),

    #[error("Temporal consistency violation: {0}")]
    TemporalConsistencyViolation(String),

    #[error("Transaction time inconsistency: {0}")]
    TransactionTimeInconsistency(String),

    #[error("AWS error: {0}")]
    AwsError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Neptune connection error: {0}")]
    NeptuneConnection(String),

    #[error("Neptune query error: {0}")]
    NeptuneQuery(String),

    #[error("Neptune response parsing error: {0}")]
    NeptuneResponseParsing(String),

    #[error("Neptune transaction error: {0}")]
    NeptuneTransaction(String),

    #[error("Invalid ID: {0}")]
    InvalidId(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid entity type: {0}")]
    InvalidEntityType(String),

    #[error("Other error: {0}")]
    Other(String),
}

/// Result type alias using our custom Error
pub type Result<T> = std::result::Result<T, Error>;

impl From<opensearch::Error> for Error {
    fn from(err: opensearch::Error) -> Self {
        Error::OpenSearch(err.to_string())
    }
}

impl From<gremlin_client::GremlinError> for Error {
    fn from(err: gremlin_client::GremlinError) -> Self {
        Error::ValueConversion(err.to_string())
    }
}

impl<E> From<SdkError<E>> for Error
where
    E: Into<DynamoDbError>,
{
    fn from(err: SdkError<E>) -> Self {
        Error::DynamoDB(err.to_string())
    }
}

impl<E: std::fmt::Display> From<backoff::Error<E>> for Error {
    fn from(err: backoff::Error<E>) -> Self {
        match err {
            backoff::Error::Permanent(e) => Error::Backoff(e.to_string()),
            backoff::Error::Transient { err, retry_after: _ } => Error::Retry(format!("Transient error: {}", err)),
        }
    }
}

impl From<uuid::Error> for Error {
    fn from(err: uuid::Error) -> Self {
        Error::Uuid(err.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}

impl From<GValue> for Error {
    fn from(value: GValue) -> Self {
        Error::Gremlin(format!("Failed to convert GValue: {:?}", value))
    }
}

impl From<url::ParseError> for Error {
    fn from(err: url::ParseError) -> Self {
        Error::OpenSearch(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::DatabaseError("test error".to_string());
        assert_eq!(err.to_string(), "Database error: test error");
    }

    #[test]
    fn test_error_conversion() {
        let err = Error::Json("invalid json".to_string());
        assert!(matches!(err, Error::Json(_)));
    }

    #[test]
    fn test_retry_error_conversion() {
        let original = Error::NeptuneConnection("connection failed".to_string());
        let retry_err = backoff::Error::Transient {
            err: original.to_string(),
            retry_after: None
        };
        let err: Error = retry_err.into();
        assert!(matches!(err, Error::Retry(_)));
    }

    #[test]
    fn test_permanent_error_conversion() {
        let original = Error::NodeNotFound("test".to_string());
        let permanent_err = backoff::Error::Permanent(original.to_string());
        let err: Error = permanent_err.into();
        assert!(matches!(err, Error::Backoff(_)));
    }
} 
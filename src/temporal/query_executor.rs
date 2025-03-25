use std::sync::Arc;
use tokio::sync::mpsc;
use futures::stream::{self, StreamExt};
use serde::{de::DeserializeOwned, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use async_trait::async_trait;
use aws_sdk_dynamodb::Client as DynamoClient;
use serde_json;

use crate::{
    error::{Error, Result},
    types::{EntityId, EntityType, Timestamp, TemporalQueryResult},
    aws::dynamodb::DynamoDBClient,
    temporal::{
        dynamodb::{DynamoDBTemporal, QueryResult},
        query::OptimizedQuery,
        query_builder::{PropertyFilter, PropertyOperator, RelationshipDirection, RelationshipFilter, TemporalQueryBuilder},
    },
};

/// Configuration for query execution
#[derive(Debug, Clone)]
pub struct QueryExecutorConfig {
    /// Maximum number of concurrent queries
    pub max_concurrent_queries: usize,
    /// Batch size for relationship queries
    pub relationship_batch_size: usize,
    /// Enable parallel execution
    pub enable_parallel: bool,
}

impl Default for QueryExecutorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_queries: 10,
            relationship_batch_size: 100,
            enable_parallel: true,
        }
    }
}

/// Query executor for handling complex temporal queries
#[derive(Clone)]
pub struct TemporalQueryExecutor<T, C>
where
    T: DeserializeOwned + Serialize + Send + Sync + 'static,
    C: DynamoDBClient + Send + Sync + 'static,
{
    temporal: Arc<DynamoDBTemporal<T, C>>,
    config: QueryExecutorConfig,
}

impl<T, C> TemporalQueryExecutor<T, C>
where
    T: DeserializeOwned + Serialize + Send + Sync + 'static,
    C: DynamoDBClient + Send + Sync + 'static,
{
    /// Create a new query executor
    pub fn new(temporal: Arc<DynamoDBTemporal<T, C>>, config: QueryExecutorConfig) -> Self {
        Self { temporal, config }
    }

    /// Execute a query with optimizations
    pub async fn execute(&self, builder: &TemporalQueryBuilder) -> Result<QueryResult<T>> {
        // Check if the query has relationship filters
        if !builder.relationship_filters.is_empty() {
            self.execute_relationship_query(builder).await
        } else {
            // Convert TemporalQueryBuilder to OptimizedQuery
            // Clone the builder to avoid ownership issues
            let optimized_query = builder.clone().build()?;
            let query_output = self.temporal.execute_query(&optimized_query).await?;
            
            // Process the query results
            let timestamp = match (builder.point_in_time, builder.time_range.as_ref()) {
                (Some(ts), _) => ts,
                (_, Some((start, _))) => *start,
                _ => Utc::now(),
            };
            
            let results = self.temporal.process_query_results(query_output, timestamp).await?;
            
            Ok(QueryResult {
                items: results,
                last_evaluated_key: None,
            })
        }
    }

    /// Execute a query with relationship filters in parallel
    async fn execute_relationship_query(&self, builder: &TemporalQueryBuilder) -> Result<QueryResult<T>> {
        // Clone relationship filters for the iterator
        let relationship_filters: Vec<_> = builder.relationship_filters.clone();
        let (tx, mut rx) = mpsc::channel(self.config.max_concurrent_queries);

        // Process relationship filters in parallel
        let mut tasks = Vec::new();
        for chunk in relationship_filters.chunks(self.config.relationship_batch_size) {
            let tx = tx.clone();
            let builder = builder.clone();
            let temporal = Arc::clone(&self.temporal);
            let chunk_vec = chunk.to_vec(); // Clone the chunk for the task

            let task = tokio::spawn(async move {
                for filter in chunk_vec {
                    match temporal.execute_relationship_filter(&builder, &filter).await {
                        Ok(result) => {
                            if let Err(e) = tx.send(Ok(result)).await {
                                return Err(Error::Internal(e.to_string()));
                            }
                        },
                        Err(e) => {
                            if let Err(send_err) = tx.send(Err(e)).await {
                                return Err(Error::Internal(send_err.to_string()));
                            }
                        }
                    }
                }
                Ok::<_, Error>(())
            });
            tasks.push(task);
        }

        // Close sender
        drop(tx);

        // Collect results
        let mut results = Vec::new();
        let mut last_evaluated_key = None;

        // Process results as they arrive
        while let Some(result) = rx.recv().await {
            let result = result?;
            results.extend(result);
            // Note: last_evaluated_key handling removed since Vec<TemporalQueryResult<T>> doesn't have it
        }

        // Wait for all tasks to complete
        for task in tasks {
            task.await.map_err(|e| Error::Internal(e.to_string()))??;
        }

        Ok(QueryResult {
            items: results,
            last_evaluated_key,
        })
    }

    /// Execute a batch of queries in parallel
    pub async fn execute_batch(&self, builders: Vec<TemporalQueryBuilder>) -> Result<Vec<QueryResult<T>>> {
        if !self.config.enable_parallel {
            // Execute sequentially if parallel execution is disabled
            let mut results = Vec::new();
            for builder in builders {
                results.push(self.execute(&builder).await?);
            }
            return Ok(results);
        }

        // Execute queries in parallel with bounded concurrency
        let mut results = Vec::with_capacity(builders.len());
        
        // Execute in chunks for bounded concurrency
        for chunk in builders.chunks(self.config.max_concurrent_queries) {
            let mut futures = Vec::with_capacity(chunk.len());
            
            for builder in chunk {
                let builder_clone = builder.clone();
                let executor = self.clone();
                futures.push(async move { executor.execute(&builder_clone).await });
            }
            
            // Execute all futures in this chunk
            let chunk_results = futures::future::join_all(futures).await;
            for result in chunk_results {
                results.push(result?);
            }
        }
        
        Ok(results)
    }
}

/// Interface for query execution
#[async_trait]
pub trait QueryExecutorTrait<T: Send> {
    /// Execute a temporal query
    async fn execute(&self, query: &TemporalQueryBuilder) -> Result<QueryResult<T>>;
    
    /// Execute a batch of queries
    async fn execute_batch(&self, queries: Vec<TemporalQueryBuilder>) -> Result<Vec<QueryResult<T>>>;
    
    /// Execute a property filter
    async fn execute_property_filter(&self, query: &TemporalQueryBuilder, filter: &PropertyFilter) -> Result<QueryResult<T>>;
    
    /// Execute a relationship filter
    async fn execute_relationship_filter(&self, query: &TemporalQueryBuilder, filter: &RelationshipFilter) -> Result<QueryResult<T>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temporal::TemporalOperation;
    
    #[tokio::test]
    async fn test_query_executor_config() {
        let config = QueryExecutorConfig::default();
        assert_eq!(config.max_concurrent_queries, 10);
        assert_eq!(config.relationship_batch_size, 100);
        assert_eq!(config.enable_parallel, true);
    }
} 
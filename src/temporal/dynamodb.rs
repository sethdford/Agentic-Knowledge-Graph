use std::sync::Arc;
use aws_sdk_dynamodb::{
    types::{AttributeValue, Select},
    Client as DynamoClient,
};
use crate::aws::dynamodb::DynamoDBClient;
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;
use uuid::Uuid;
use std::str::FromStr;

use crate::{
    error::{Error, Result},
    types::{
        EntityId, TemporalRange, Timestamp,
        Node, Edge, EntityType, NodeId, EdgeId,
    },
};

use super::{
    ConsistencyCheckResult, ConsistencyChecker, Temporal, TemporalIndexEntry, TemporalQueryResult,
    query::OptimizedQuery,
    query_builder::TemporalQueryBuilder,
    graph::TemporalGraph,
};

/// DynamoDB-backed temporal implementation
pub struct DynamoDBTemporal<T, C: DynamoDBClient + Send + Sync + 'static> {
    /// DynamoDB client
    client: Arc<C>,
    /// Table name
    table_name: String,
    /// Consistency checker
    checker: ConsistencyChecker,
    /// Type marker
    _marker: std::marker::PhantomData<T>,
}

impl<T, C> DynamoDBTemporal<T, C>
where
    T: Serialize + DeserializeOwned + Send + Sync,
    C: DynamoDBClient + Send + Sync + 'static,
{
    /// Create a new DynamoDB temporal instance
    pub fn new(client: Arc<C>, table_name: String) -> Self {
        Self {
            client,
            table_name,
            checker: ConsistencyChecker::new(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Store an item in DynamoDB
    pub async fn store(&self, entity_id: &EntityId, temporal_range: &TemporalRange, data: &T) -> Result<()> {
        // Validate temporal range
        if temporal_range.start.is_none() || temporal_range.end.is_none() {
            return Err(Error::InvalidTemporalRange("Both start and end times must be present".to_string()));
        }

        let version_id = Uuid::new_v4().to_string();
        let serialized_data = serde_json::to_string(data)
            .map_err(|e| Error::Serialization(e.to_string()))?;

        let item = HashMap::from([
            ("entity_id".to_string(), AttributeValue::S(entity_id.id.to_string())),
            ("entity_type".to_string(), AttributeValue::S(entity_id.entity_type.to_string())),
            ("valid_time_start".to_string(), AttributeValue::S(temporal_range.start.unwrap().0.to_rfc3339())),
            ("valid_time_end".to_string(), AttributeValue::S(temporal_range.end.unwrap().0.to_rfc3339())),
            ("transaction_time_start".to_string(), AttributeValue::S(Utc::now().to_rfc3339())),
            ("version_id".to_string(), AttributeValue::S(version_id)),
            ("data".to_string(), AttributeValue::S(serialized_data)),
        ]);

        self.client.put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await?;

        Ok(())
    }

    /// Query items from DynamoDB
    pub async fn query_items<U>(&self, query: &OptimizedQuery) -> Result<Vec<HashMap<String, AttributeValue>>>
    where
        U: DeserializeOwned,
    {
        let mut builder = self.client.query()
            .table_name(&self.table_name)
            .key_condition_expression(&query.key_condition);

        if let Some(filter_expr) = &query.filter_expression {
            builder = builder.filter_expression(filter_expr);
        }

        if let Some(obj) = query.expression_values.as_object() {
            for (k, v) in obj {
                let attr_value = match v {
                    serde_json::Value::String(s) => AttributeValue::S(s.clone()),
                    serde_json::Value::Number(n) => AttributeValue::N(n.to_string()),
                    _ => continue,
                };
                builder = builder.expression_attribute_values(k, attr_value);
            }
        }

        let result = builder.send().await?;
        Ok(result.items.unwrap_or_default())
    }

    /// Scan items from DynamoDB
    pub async fn scan_items<U>(&self, filter: Option<String>, values: Option<HashMap<String, AttributeValue>>) -> Result<Vec<U>>
    where
        U: DeserializeOwned,
    {
        let mut builder = self.client.scan()
            .table_name(&self.table_name);

        if let Some(filter_expr) = filter {
            builder = builder.filter_expression(filter_expr);
        }

        if let Some(expr_values) = values {
            for (k, v) in expr_values {
                builder = builder.expression_attribute_values(k, v);
            }
        }

        let result = builder.send().await?;
        
        let items = result.items.unwrap_or_default();
        let mut results = Vec::with_capacity(items.len());

        for item in items {
            if let Some(data) = item.get("data") {
                if let Ok(json_str) = data.as_s() {
                    if let Ok(deserialized) = serde_json::from_str(json_str) {
                        results.push(deserialized);
                    }
                }
            }
        }

        Ok(results)
    }

    /// Convert data to DynamoDB attributes
    async fn data_to_attributes(&self, data: &T) -> Result<AttributeValue> {
        let json = serde_json::to_string(data)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        Ok(AttributeValue::S(json))
    }

    /// Convert DynamoDB attributes to data
    async fn attributes_to_data(&self, attr: AttributeValue) -> Result<T> {
        let json = attr.as_s()
            .map_err(|_| Error::Serialization("Invalid data format".to_string()))?;
        serde_json::from_str(json)
            .map_err(|e| Error::Serialization(e.to_string()))
    }

    /// Create a temporal index entry from DynamoDB attributes
    async fn create_index_entry(
        &self,
        entity_id: EntityId,
        attrs: &HashMap<String, AttributeValue>,
    ) -> Result<TemporalIndexEntry> {
        let valid_start = attrs.get("valid_time_start")
            .and_then(|v| v.as_n().ok())
            .and_then(|n| n.parse::<i64>().ok())
            .ok_or_else(|| Error::Serialization("Missing valid_time_start".to_string()))?;

        let valid_end = attrs.get("valid_time_end")
            .and_then(|v| v.as_n().ok())
            .and_then(|n| n.parse::<i64>().ok())
            .ok_or_else(|| Error::Serialization("Missing valid_time_end".to_string()))?;

        let tx_start = attrs.get("transaction_time_start")
            .and_then(|v| v.as_n().ok())
            .and_then(|n| n.parse::<i64>().ok())
            .ok_or_else(|| Error::Serialization("Missing transaction_time_start".to_string()))?;

        let tx_end = attrs.get("transaction_time_end")
            .and_then(|v| v.as_n().ok())
            .and_then(|n| n.parse::<i64>().ok())
            .map(|ts| DateTime::from_timestamp(ts, 0).unwrap());

        let version = attrs.get("version_id")
            .and_then(|v| v.as_s().ok())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| Error::Serialization("Missing version_id".to_string()))?;

        Ok(TemporalIndexEntry {
            entity_id,
            valid_time_start: DateTime::from_timestamp(valid_start, 0).unwrap(),
            valid_time_end: DateTime::from_timestamp(valid_end, 0).unwrap(),
            transaction_time_start: DateTime::from_timestamp(tx_start, 0).unwrap(),
            transaction_time_end: tx_end,
            version_id: version,
        })
    }

    /// Get all temporal index entries from the database
    async fn get_all_entries(&self) -> Result<Vec<TemporalIndexEntry>> {
        let items = self.scan_items::<serde_json::Value>(None, None).await?;
        
        let mut entries = Vec::new();
        for item in items {
            let entity_id_str = item.get("entity_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Serialization("Missing entity_id".to_string()))?;
            
            let entity_type_str = item.get("entity_type")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Serialization("Missing entity_type".to_string()))?;

            let entity_type = EntityType::from_str(entity_type_str)
                .map_err(|e| Error::Serialization(e.to_string()))?;
            
            let entity_id = EntityId {
                entity_type,
                id: entity_id_str.to_string(),
            };
            
            let valid_time_start = item.get("valid_time_start")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Serialization("Missing valid_time_start".to_string()))?;
            
            let valid_time_end = item.get("valid_time_end")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Serialization("Missing valid_time_end".to_string()))?;
            
            let transaction_time_start = item.get("transaction_time_start")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Serialization("Missing transaction_time_start".to_string()))?;
            
            let version_id = item.get("version_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Serialization("Missing version_id".to_string()))?;
            
            let version_id = Uuid::parse_str(version_id)
                .map_err(|e| Error::Serialization(e.to_string()))?;
            
            let valid_time_start = DateTime::parse_from_rfc3339(valid_time_start)
                .map_err(|e| Error::Serialization(format!("{:?}", e)))?
                .with_timezone(&Utc);
            
            let valid_time_end = DateTime::parse_from_rfc3339(valid_time_end)
                .map_err(|e| Error::Serialization(format!("{:?}", e)))?
                .with_timezone(&Utc);
            
            let transaction_time_start = DateTime::parse_from_rfc3339(transaction_time_start)
                .map_err(|e| Error::Serialization(format!("{:?}", e)))?
                .with_timezone(&Utc);
            
            let entry = TemporalIndexEntry {
                entity_id,
                valid_time_start,
                valid_time_end,
                transaction_time_start,
                transaction_time_end: None,
                version_id,
            };
            
            entries.push(entry);
        }
        
        Ok(entries)
    }

    /// Get the table name
    pub fn table_name(&self) -> &str {
        &self.table_name
    }

    /// Query with options
    pub async fn query_with_options(
        &self,
        query: OptimizedQuery,
        last_evaluated_key: Option<HashMap<String, AttributeValue>>,
    ) -> Result<QueryResult<T>> {
        let mut request = self.client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression(&query.key_condition)
            .select(Select::AllAttributes);

        if let Some(filter) = &query.filter_expression {
            request = request.filter_expression(filter);
        }

        let values = query.expression_values;
        if let serde_json::Value::Object(map) = values {
            let mut expr_values = HashMap::new();
            for (key, value) in map {
                expr_values.insert(
                    key,
                    match value {
                        serde_json::Value::String(s) => AttributeValue::S(s),
                        serde_json::Value::Number(n) => AttributeValue::N(n.to_string()),
                        _ => continue,
                    },
                );
            }
            request = request.set_expression_attribute_values(Some(expr_values));
        }

        if let Some(key) = last_evaluated_key {
            request = request.set_exclusive_start_key(Some(key));
        }

        let result = request
            .send()
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        let items = result.items.unwrap_or_default();
        let mut results = Vec::with_capacity(items.len());

        for item in items {
            let data = self.attributes_to_data(
                item.get("data")
                    .ok_or_else(|| Error::Serialization("Missing data".to_string()))?
                    .clone(),
            ).await?;

            let timestamp = item.get("valid_time_start")
                .and_then(|v| v.as_n().ok())
                .and_then(|n| n.parse::<i64>().ok())
                .map(|ts| DateTime::from_timestamp(ts, 0).unwrap())
                .ok_or_else(|| Error::Serialization("Missing valid_time_start".to_string()))?;

            let version_id = item.get("version_id")
                .and_then(|v| v.as_s().ok())
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or_else(|| Error::Serialization("Missing version_id".to_string()))?;

            results.push(TemporalQueryResult::new(
                data,
                Timestamp(timestamp),
                version_id,
            ));
        }

        Ok(QueryResult {
            items: results,
            last_evaluated_key: result.last_evaluated_key,
        })
    }

    /// Execute a query built with TemporalQueryBuilder
    pub async fn execute_query(&self, builder: TemporalQueryBuilder) -> Result<Vec<TemporalQueryResult<T>>> {
        let query = builder.build()?;
        let items = self.query_items::<T>(&query).await?;
        let mut results = Vec::new();

        for item in items {
            let entity_id = EntityId::new(
                serde_json::from_str(
                    item.get("entity_type")
                        .ok_or_else(|| Error::Serialization("Missing entity_type".to_string()))?
                        .as_s()
                        .map_err(|e| Error::Serialization(format!("{:?}", e)))?
                )?,
                item.get("entity_id")
                    .ok_or_else(|| Error::Serialization("Missing entity_id".to_string()))?
                    .as_s()
                    .map_err(|e| Error::Serialization(format!("{:?}", e)))?
                    .to_string(),
            );

            let entry = self.create_index_entry(entity_id.clone(), &item).await?;
            let data = self.attributes_to_data(
                item.get("data")
                    .ok_or_else(|| Error::Serialization("Missing data".to_string()))?
                    .clone()
            ).await?;

            results.push(TemporalQueryResult::new(
                data,
                Timestamp(entry.valid_time_start),
                entry.version_id,
            ));
        }

        Ok(results)
    }
}

/// Result of a query operation
pub struct QueryResult<T> {
    /// Query result items
    pub items: Vec<TemporalQueryResult<T>>,
    /// Last evaluated key for pagination
    pub last_evaluated_key: Option<HashMap<String, AttributeValue>>,
}

#[async_trait]
impl<T, C> Temporal for DynamoDBTemporal<T, C>
where
    T: DeserializeOwned + Serialize + Send + Sync + 'static,
    C: DynamoDBClient + Send + Sync + 'static,
{
    type Data = T;

    async fn store(
        &self,
        entity_id: EntityId,
        data: Self::Data,
        valid_time: TemporalRange,
    ) -> Result<()> {
        let valid_start = valid_time.start.ok_or_else(|| 
            Error::InvalidTemporalRange("Missing valid time start".to_string()))?;
        let valid_end = valid_time.end.ok_or_else(|| 
            Error::InvalidTemporalRange("Missing valid time end".to_string()))?;

        // Validate temporal consistency
        self.checker
            .validate_range(&entity_id, valid_start.0, valid_end.0)
            .await?;

        let now = Utc::now();
        let version_id = Uuid::new_v4();
        let data_attr = self.data_to_attributes(&data).await?;

        let item = HashMap::from([
            ("entity_id".to_string(), AttributeValue::S(entity_id.id.to_string())),
            ("valid_time_start".to_string(), AttributeValue::N(valid_start.0.timestamp().to_string())),
            ("valid_time_end".to_string(), AttributeValue::N(valid_end.0.timestamp().to_string())),
            ("transaction_time_start".to_string(), AttributeValue::N(now.timestamp().to_string())),
            ("version_id".to_string(), AttributeValue::S(version_id.to_string())),
            ("data".to_string(), data_attr),
        ]);

        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await
            .map_err(|e| Error::DynamoDB(e.to_string()))?;

        Ok(())
    }

    async fn query_at(
        &self,
        entity_id: &EntityId,
        timestamp: DateTime<Utc>,
    ) -> Result<Vec<TemporalQueryResult<Self::Data>>> {
        let query = OptimizedQuery::new(self.table_name.clone())
            .with_key_condition("entity_id = :eid AND valid_time_start <= :ts AND valid_time_end >= :ts".to_string())
            .with_values(json!({
                ":eid": entity_id.id.to_string(),
                ":ts": timestamp.timestamp().to_string(),
            }));

        let items = self.query_items::<T>(&query).await?;
        let mut results = Vec::new();

        for item in items {
            let entry = self.create_index_entry(entity_id.clone(), &item).await?;
            let data = self.attributes_to_data(
                item.get("data")
                    .ok_or_else(|| Error::Serialization("Missing data".to_string()))?
                    .clone()
            ).await?;

            results.push(TemporalQueryResult::new(
                data,
                Timestamp(entry.valid_time_start),
                entry.version_id,
            ));
        }

        Ok(results)
    }

    async fn query_between(
        &self,
        entity_id: &EntityId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TemporalQueryResult<Self::Data>>> {
        if start > end {
            return Err(Error::InvalidTemporalRange(
                "Start time must be before end time".to_string(),
            ));
        }

        let query = OptimizedQuery::new(self.table_name.clone())
            .with_key_condition("entity_id = :eid AND valid_time_start <= :end AND valid_time_end >= :start".to_string())
            .with_values(json!({
                ":eid": entity_id.id.to_string(),
                ":start": start.timestamp().to_string(),
                ":end": end.timestamp().to_string(),
            }));

        let items = self.query_items::<T>(&query).await?;
        let mut results = Vec::new();

        for item in items {
            let entry = self.create_index_entry(entity_id.clone(), &item).await?;
            let data = self.attributes_to_data(
                item.get("data")
                    .ok_or_else(|| Error::Serialization("Missing data".to_string()))?
                    .clone()
            ).await?;

            results.push(TemporalQueryResult::new(
                data,
                Timestamp(entry.valid_time_start),
                entry.version_id,
            ));
        }

        Ok(results)
    }

    async fn query_evolution(
        &self,
        entity_id: &EntityId,
        range: &TemporalRange,
    ) -> Result<Vec<TemporalQueryResult<Self::Data>>> {
        let start = range.start.ok_or_else(|| 
            Error::InvalidTemporalRange("Missing range start".to_string()))?;
        let end = range.end.ok_or_else(|| 
            Error::InvalidTemporalRange("Missing range end".to_string()))?;

        let query = OptimizedQuery::new(self.table_name.clone())
            .with_key_condition("entity_id = :eid AND valid_time_start BETWEEN :start AND :end".to_string())
            .with_values(json!({
                ":eid": entity_id.id.to_string(),
                ":start": start.0.timestamp().to_string(),
                ":end": end.0.timestamp().to_string(),
            }))
            .with_scan_direction(true);

        let items = self.query_items::<T>(&query).await?;
        let mut results = Vec::new();

        for item in items {
            let entry = self.create_index_entry(entity_id.clone(), &item).await?;
            let data = self.attributes_to_data(
                item.get("data")
                    .ok_or_else(|| Error::Serialization("Missing data".to_string()))?
                    .clone()
            ).await?;

            results.push(TemporalQueryResult::new(
                data,
                Timestamp(entry.valid_time_start),
                entry.version_id,
            ));
        }

        Ok(results)
    }

    async fn query_latest(
        &self,
        entity_id: &EntityId,
    ) -> Result<Option<TemporalQueryResult<Self::Data>>> {
        let query = OptimizedQuery::new(self.table_name.clone())
            .with_key_condition("entity_id = :eid AND transaction_time_end IS NULL".to_string())
            .with_values(json!({
                ":eid": entity_id.id.to_string(),
            }))
            .with_limit(1)
            .with_scan_direction(false);

        let items = self.query_items::<T>(&query).await?;
        
        if let Some(item) = items.first() {
            let entry = self.create_index_entry(entity_id.clone(), item).await?;
            let data = self.attributes_to_data(
                item.get("data")
                    .ok_or_else(|| Error::Serialization("Missing data".to_string()))?
                    .clone()
            ).await?;

            Ok(Some(TemporalQueryResult::new(
                data,
                Timestamp(entry.valid_time_start),
                entry.version_id,
            )))
        } else {
            Ok(None)
        }
    }

    async fn validate_consistency(&self) -> Result<ConsistencyCheckResult> {
        let entries = self.get_all_entries().await?;
        self.checker.check_consistency(&entries).await
    }
}

#[async_trait]
impl<T, C> TemporalGraph for DynamoDBTemporal<T, C>
where
    T: DeserializeOwned + Serialize + Send + Sync + 'static,
    C: DynamoDBClient + Send + Sync + 'static,
{
    async fn get_nodes_at(&self, timestamp: DateTime<Utc>, node_type: Option<EntityType>) -> Result<Vec<Node>> {
        let results = self.query_at(&EntityId {
            entity_type: EntityType::Node,
            id: Uuid::new_v4().to_string()
        }, timestamp).await?;
        Ok(results.into_iter()
            .filter_map(|r| if let Ok(node) = serde_json::from_value(serde_json::to_value(r.data).unwrap()) {
                Some(node)
            } else {
                None
            })
            .collect())
    }

    async fn get_edges_at(&self, timestamp: DateTime<Utc>, source_id: Option<Uuid>, target_id: Option<Uuid>) -> Result<Vec<Edge>> {
        let mut query = OptimizedQuery::new(self.table_name().to_string())
            .with_key_condition("valid_time_start <= :ts AND valid_time_end >= :ts".to_string())
            .with_values(serde_json::json!({
                ":ts": timestamp.timestamp().to_string(),
            }));

        let mut filter_conditions = Vec::new();
        let mut filter_values = serde_json::Map::new();

        if let Some(sid) = source_id {
            filter_conditions.push("source_id = :sid");
            filter_values.insert(":sid".to_string(), serde_json::Value::String(sid.to_string()));
        }

        if let Some(tid) = target_id {
            filter_conditions.push("target_id = :tid");
            filter_values.insert(":tid".to_string(), serde_json::Value::String(tid.to_string()));
        }

        if !filter_conditions.is_empty() {
            query = query
                .with_filter(filter_conditions.join(" AND "))
                .with_values(serde_json::Value::Object(filter_values));
        }

        let result = self.query_with_options(query, None).await?;
        let edges: Vec<Edge> = result.items.into_iter()
            .filter_map(|r| serde_json::from_value(serde_json::to_value(r.data).unwrap()).ok())
            .collect();
        Ok(edges)
    }

    async fn get_nodes_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        node_type: Option<EntityType>,
    ) -> Result<Vec<Node>> {
        let results = self.query_between(&EntityId {
            entity_type: EntityType::Node,
            id: Uuid::new_v4().to_string()
        }, start, end).await?;
        Ok(results.into_iter()
            .filter_map(|r| if let Ok(node) = serde_json::from_value(serde_json::to_value(r.data).unwrap()) {
                Some(node)
            } else {
                None
            })
            .collect())
    }

    async fn get_edges_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<Edge>> {
        let results = self.query_between(&EntityId {
            entity_type: EntityType::Edge,
            id: Uuid::new_v4().to_string()
        }, start, end).await?;
        Ok(results.into_iter()
            .filter_map(|r| if let Ok(edge) = serde_json::from_value(serde_json::to_value(r.data).unwrap()) {
                Some(edge)
            } else {
                None
            })
            .collect())
    }

    async fn get_node_evolution(&self, node_id: NodeId, time_range: &TemporalRange) -> Result<Vec<Node>> {
        let entity_id = EntityId {
            id: node_id.0.to_string(),
            entity_type: EntityType::Node,
        };
        let mut query = OptimizedQuery::new(self.table_name().to_string())
            .with_key_condition("entity_id = :eid".to_string())
            .with_values(serde_json::json!({
                ":eid": entity_id.to_string(),
            }));

        if let (Some(start), Some(end)) = (time_range.start, time_range.end) {
            query = query
                .with_filter("valid_time_start <= :end AND valid_time_end >= :start".to_string())
                .with_values(serde_json::json!({
                    ":start": start.0.timestamp().to_string(),
                    ":end": end.0.timestamp().to_string(),
                }));
        }

        let result = self.query_with_options(query, None).await?;
        let nodes: Vec<Node> = result.items.into_iter()
            .filter_map(|r| serde_json::from_value(serde_json::to_value(r.data).unwrap()).ok())
            .collect();
        Ok(nodes)
    }

    async fn get_edge_evolution(&self, edge_id: EdgeId, time_range: &TemporalRange) -> Result<Vec<Edge>> {
        let entity_id = EntityId {
            id: edge_id.0.to_string(),
            entity_type: EntityType::Edge,
        };
        let mut query = OptimizedQuery::new(self.table_name().to_string())
            .with_key_condition("entity_id = :eid".to_string())
            .with_values(serde_json::json!({
                ":eid": entity_id.to_string(),
            }));

        if let (Some(start), Some(end)) = (time_range.start, time_range.end) {
            query = query
                .with_filter("valid_time_start <= :end AND valid_time_end >= :start".to_string())
                .with_values(serde_json::json!({
                    ":start": start.0.timestamp().to_string(),
                    ":end": end.0.timestamp().to_string(),
                }));
        }

        let result = self.query_with_options(query, None).await?;
        let edges: Vec<Edge> = result.items.into_iter()
            .filter_map(|r| serde_json::from_value(serde_json::to_value(r.data).unwrap()).ok())
            .collect();
        Ok(edges)
    }

    async fn store(&self, entity_id: EntityId, data: impl Serialize + Send + Sync, valid_time: TemporalRange) -> Result<()> {
        let data_value = serde_json::to_value(&data)?;
        let typed_data: T = serde_json::from_value(data_value)?;
        
        let version_id = Uuid::new_v4();
        let now = Utc::now();
        
        let mut item = HashMap::new();
        item.insert("entity_id".to_string(), AttributeValue::S(entity_id.id.to_string()));
        item.insert("entity_type".to_string(), AttributeValue::S(entity_id.entity_type.to_string()));
        item.insert("version_id".to_string(), AttributeValue::S(version_id.to_string()));
        item.insert("valid_time_start".to_string(), AttributeValue::N(valid_time.start.map_or(0, |t| t.0.timestamp()).to_string()));
        item.insert("valid_time_end".to_string(), AttributeValue::N(valid_time.end.map_or(i64::MAX, |t| t.0.timestamp()).to_string()));
        item.insert("transaction_time_start".to_string(), AttributeValue::N(now.timestamp().to_string()));
        item.insert("data".to_string(), self.data_to_attributes(&typed_data).await?);
        
        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;
            
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    struct TestData {
        value: String,
    }

    #[tokio::test]
    async fn test_dynamodb_temporal() {
        // Test implementation
    }
} 
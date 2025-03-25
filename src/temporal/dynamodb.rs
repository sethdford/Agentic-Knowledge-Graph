use std::sync::Arc;
use aws_sdk_dynamodb::{
    types::{AttributeValue, Select},
    Client as DynamoClient,
    operation::query::QueryOutput,
};
use crate::aws::dynamodb::DynamoDBClient;
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::json;
use uuid::Uuid;
use std::str::FromStr;
use std::collections::HashSet;
use crate::types;

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
    query_builder::{
        TemporalQueryBuilder,
        PropertyOperator,
        RelationshipDirection,
        PropertyFilter,
        RelationshipFilter,
    },
    graph::{TemporalGraph, StorableData},
    RelationshipType,
};

use crate::temporal::{
    TemporalOperation,
    query::optimize_temporal_query,
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
    T: DeserializeOwned + Serialize + Send + Sync + 'static,
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
            .table_name(&self.table_name);

        if let Some(key_cond) = &query.key_condition {
            builder = builder.key_condition_expression(key_cond);
        }

        if let Some(filter) = &query.filter_expression {
            builder = builder.filter_expression(filter);
        }

        if let Some(values) = &query.expression_values {
            if let serde_json::Value::Object(obj) = values {
            for (k, v) in obj {
                let attr_value = match v {
                    serde_json::Value::String(s) => AttributeValue::S(s.clone()),
                    serde_json::Value::Number(n) => AttributeValue::N(n.to_string()),
                    _ => continue,
                };
                builder = builder.expression_attribute_values(k, attr_value);
                }
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
            .select(Select::AllAttributes);

        if let Some(key_cond) = query.key_condition {
            request = request.key_condition_expression(key_cond);
        }

        if let Some(filter) = query.filter_expression {
            request = request.filter_expression(filter);
        }

        if let Some(values) = query.expression_values {
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
        }

        if let Some(key) = last_evaluated_key {
            request = request.set_exclusive_start_key(Some(key));
        }

        if let Some(limit) = query.limit {
            request = request.set_limit(Some(limit as i32));
        }

        if let Some(scan_direction) = query.scan_direction {
            request = request.set_scan_index_forward(Some(scan_direction));
        }

        let result = request
            .send()
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        let items = result.items.unwrap_or_default();
        let mut results = Vec::with_capacity(items.len());

        for item in items {
            if let Some(data_attr) = item.get("data") {
                let data = self.attributes_to_data(data_attr.clone()).await?;
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
        }

        Ok(QueryResult {
            items: results,
            last_evaluated_key: result.last_evaluated_key,
        })
    }

    /// Execute a query built with TemporalQueryBuilder
    pub async fn execute_query(&self, query: &OptimizedQuery) -> Result<QueryOutput> {
        let mut builder = self.client.query()
            .table_name(&self.table_name)
            .scan_index_forward(query.scan_direction.unwrap_or(true));

        if let Some(key_condition) = &query.key_condition {
            builder = builder.key_condition_expression(key_condition);
        }

        if let Some(filter_expression) = &query.filter_expression {
            builder = builder.filter_expression(filter_expression);
        }

        if let Some(values) = &query.expression_values {
            let mut attribute_values = HashMap::new();
            if let Some(obj) = values.as_object() {
                for (k, v) in obj {
                    attribute_values.insert(
                        k.clone(),
                        AttributeValue::S(v.as_str().unwrap_or_default().to_string()),
                    );
                }
            }
            builder = builder.set_expression_attribute_values(Some(attribute_values));
        }

        if let Some(limit) = query.limit {
            builder = builder.limit(limit);
        }

        builder.send()
            .await
            .map_err(|e| Error::DynamoDB(e.to_string()))
    }

    /// Execute a query with relationship filters
    pub async fn execute_relationship_query(
        &self,
        builder: TemporalQueryBuilder,
    ) -> Result<QueryResult<T>> {
        let query = builder.build()?;
        let result = self.execute_query(&query).await?;
        let mut results = Vec::new();
        let mut last_evaluated_key = None;

        // First query the relationships table
        let relationship_query = self.build_relationship_query(&query)?;
        let result = self.execute_query(&relationship_query).await?;

        // Get the related entity IDs
        let mut entity_ids = HashSet::new();
        if let Some(items) = &result.items {
            for item in items {
                if let Some(source_attr) = item.get("source_id") {
                    if let Ok(source_id) = source_attr.as_s() {
                        entity_ids.insert(source_id.to_string());
                    }
                }
                if let Some(target_attr) = item.get("target_id") {
                    if let Ok(target_id) = target_attr.as_s() {
                        entity_ids.insert(target_id.to_string());
                    }
                }
            }
        }

        // Query the main table for each entity
        for entity_id in entity_ids {
            let entity_query = OptimizedQuery::new(self.table_name.clone())
                .with_key_condition(format!("entity_id = :entity_id"))
                .with_values(serde_json::json!({
                    ":entity_id": entity_id
                }));

            let result = self.execute_query(&entity_query).await?;
            if let Some(items) = result.items {
                for item in items {
                    if let Ok(deserialized) = self.deserialize_item(&item).await {
                        results.push(TemporalQueryResult::new(
                            deserialized,
                            Timestamp(Utc::now()),
                            Uuid::new_v4(),
                        ));
                    }
                }
            }
            if result.last_evaluated_key.is_some() {
                last_evaluated_key = result.last_evaluated_key.clone();
            }
        }

        Ok(QueryResult {
            items: results,
            last_evaluated_key,
        })
    }

    /// Build a query for the relationships table
    fn build_relationship_query(&self, query: &OptimizedQuery) -> Result<OptimizedQuery> {
        let mut relationship_query = OptimizedQuery::new(self.table_name.clone());
        
        // Copy relevant conditions and values
        if let Some(key_condition) = &query.key_condition {
            relationship_query = relationship_query.with_key_condition(key_condition.clone());
        }
        
        if let Some(filter) = &query.filter_expression {
            relationship_query = relationship_query.with_filter(filter.clone());
        }
        
        if let Some(values) = &query.expression_values {
            relationship_query = relationship_query.with_values(values.clone());
        }
        
        Ok(relationship_query)
    }

    /// Deserialize an item from DynamoDB
    pub async fn deserialize_item(&self, item: &HashMap<String, AttributeValue>) -> Result<T> {
        let data = item.get("data")
            .ok_or_else(|| Error::DynamoDB("Missing data field".to_string()))?;
        
        let data_str = data.as_s()
            .map_err(|e| Error::DynamoDB(format!("Failed to get data string: {:?}", e)))?;
        
        serde_json::from_str(data_str)
            .map_err(|e| Error::DynamoDB(format!("Failed to deserialize data: {}", e)))
    }

    /// Execute a query with a relationship filter
    pub async fn execute_relationship_filter(
        &self,
        builder: &TemporalQueryBuilder,
        filter: &RelationshipFilter,
    ) -> Result<Vec<TemporalQueryResult<T>>> {
        // Check if entity_id is specified in the builder
        let entity_id = builder.entity_id.as_ref().ok_or_else(|| {
            Error::InvalidQueryFormat("Entity ID is required for relationship filters".to_string())
        })?;
        
        // Check if timestamp is specified
        let timestamp = match (builder.point_in_time, builder.time_range) {
            (Some(point), _) => point,
            (_, Some((start, _))) => start,
            _ => return Err(Error::InvalidQueryFormat(
                "Either point_in_time or time_range must be specified".to_string()
            )),
        };
        
        // Build optimized query for relationship table
        let relationship_query = self.build_relationship_query(&OptimizedQuery::new(self.table_name.clone()))?;
        
        // Execute the query
        let result = self.execute_query(&relationship_query).await?;
        
        // Process the results
        let mut results = Vec::new();
        
        if let Some(items) = &result.items {
            for item in items {
                // Process source_id relationship
                if let Some(source_attr) = item.get("source_id") {
                    if let Ok(source_id) = source_attr.as_s() {
                        let entity_type = if let Some(et) = &filter.target_entity_type {
                            et.clone()
                        } else {
                            EntityType::Node
                        };
                        let entity_id = EntityId::new(entity_type, source_id);
                        
                        // Create a query for this entity
                        let mut query = OptimizedQuery::new(self.table_name.clone())
                            .with_key_condition(format!("entity_id = :entity_id"))
                            .with_values(serde_json::json!({
                                ":entity_id": entity_id.to_string()
                            }));
                        
                        // Execute the query
                        if let Ok(entity_result) = self.execute_query(&query).await {
                            if let Some(entity_items) = entity_result.items {
                                for entity_item in &entity_items {
                                    if let Ok(deserialized) = self.deserialize_item(entity_item).await {
                                        let version_id = Uuid::new_v4();
                                        results.push(TemporalQueryResult::new(
                                            deserialized,
                                            Timestamp(timestamp),
                                            version_id,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
                
                // Process target_id relationship
                if let Some(target_attr) = item.get("target_id") {
                    if let Ok(target_id) = target_attr.as_s() {
                        let entity_type = if let Some(et) = &filter.target_entity_type {
                            et.clone()
                        } else {
                            EntityType::Node
                        };
                        let entity_id = EntityId::new(entity_type, target_id);
                        
                        // Create a query for this entity
                        let mut query = OptimizedQuery::new(self.table_name.clone())
                            .with_key_condition(format!("entity_id = :entity_id"))
                            .with_values(serde_json::json!({
                                ":entity_id": entity_id.to_string()
                            }));
                        
                        // Execute the query
                        if let Ok(entity_result) = self.execute_query(&query).await {
                            if let Some(entity_items) = entity_result.items {
                                for entity_item in &entity_items {
                                    if let Ok(deserialized) = self.deserialize_item(entity_item).await {
                                        let version_id = Uuid::new_v4();
                                        results.push(TemporalQueryResult::new(
                                            deserialized,
                                            Timestamp(timestamp),
                                            version_id,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(results)
    }

    /// Execute a query with a property filter
    pub async fn execute_property_filter(
        &self,
        builder: &TemporalQueryBuilder,
        filter: &PropertyFilter,
    ) -> Result<Vec<TemporalQueryResult<T>>> {
        // Check if entity_id is specified in the builder
        let entity_id = builder.entity_id.as_ref().ok_or_else(|| {
            Error::InvalidQueryFormat("Entity ID is required for property filters".to_string())
        })?;
        
        // Check if timestamp is specified
        let timestamp = match (builder.point_in_time, builder.time_range) {
            (Some(point), _) => point,
            (_, Some((start, _))) => start,
            _ => return Err(Error::InvalidQueryFormat(
                "Either point_in_time or time_range must be specified".to_string()
            )),
        };
        
        // Build the condition expression
        let property_name = format!("#{}", filter.property_name);
        let property_value = ":propertyValue";
        
        let mut expression_attribute_names = HashMap::new();
        expression_attribute_names.insert(format!("#{}", filter.property_name), filter.property_name.clone());
        
        let mut expression_attribute_values = HashMap::new();
        expression_attribute_values.insert(":propertyValue".to_string(), AttributeValue::S(filter.value.to_string()));
        
        // Build the condition expression based on the operator
        let condition = self.build_condition(&property_name, &filter.operator, &property_value);
        
        // Create the query with optimized builder
        let mut query = OptimizedQuery::new(self.table_name.clone())
            .with_key_condition(format!("entity_id = :entity_id"))
            .with_filter(condition)
            .with_values(serde_json::json!({
                ":entity_id": entity_id.to_string(),
                ":propertyValue": filter.value.clone(),
            }));
        
        // Execute the query
        let result = self.execute_query(&query).await?;
        
        // Process the results
        let mut results = Vec::new();
        
        if let Some(items) = &result.items {
            for item in items {
                if let Ok(deserialized) = self.deserialize_item(item).await {
                    let version_id = Uuid::new_v4();
                    results.push(TemporalQueryResult::new(
                        deserialized,
                        Timestamp(timestamp),
                        version_id,
                    ));
                }
            }
        }
        
        Ok(results)
    }

    fn build_condition(&self, property_name: &str, operator: &PropertyOperator, property_value: &str) -> String {
        match operator {
            PropertyOperator::Equal => format!("{} = {}", property_name, property_value),
            PropertyOperator::NotEqual => format!("{} <> {}", property_name, property_value),
            PropertyOperator::GreaterThan => format!("{} > {}", property_name, property_value),
            PropertyOperator::GreaterThanOrEqual => format!("{} >= {}", property_name, property_value),
            PropertyOperator::LessThan => format!("{} < {}", property_name, property_value),
            PropertyOperator::LessThanOrEqual => format!("{} <= {}", property_name, property_value),
            PropertyOperator::Contains => format!("contains({}, {})", property_name, property_value),
            PropertyOperator::StartsWith => format!("begins_with({}, {})", property_name, property_value),
            PropertyOperator::EndsWith => format!("ends_with({}, {})", property_name, property_value),
            PropertyOperator::In => format!("{} IN ({})", property_name, property_value),
            PropertyOperator::NotIn => format!("NOT {} IN ({})", property_name, property_value),
        }
    }

    async fn store_temporal(&self, entity_id: EntityId, data: T, valid_time: TemporalRange) -> Result<()> {
        let json_data = serde_json::to_string(&data)
            .map_err(|e| Error::Serialization(format!("Failed to serialize data: {}", e)))?;
        
        let mut item = HashMap::new();
        item.insert("entity_id".to_string(), AttributeValue::S(entity_id.to_string()));
        item.insert("valid_time_start".to_string(), AttributeValue::N(valid_time.start.unwrap_or_default().0.timestamp().to_string()));
        item.insert("valid_time_end".to_string(), AttributeValue::N(valid_time.end.unwrap_or_default().0.timestamp().to_string()));
        item.insert("data".to_string(), AttributeValue::S(json_data));
        item.insert("version_id".to_string(), AttributeValue::S(Uuid::new_v4().to_string()));
        
        self.client.put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await
            .map_err(|e| Error::DynamoDB(e.to_string()))?;
        
        Ok(())
    }

    /// Process query results and convert to TemporalQueryResult objects
    pub async fn process_query_results(&self, response: QueryOutput, timestamp: DateTime<Utc>) -> Result<Vec<TemporalQueryResult<T>>> {
        let mut results = Vec::new();
        
        if let Some(items) = response.items {
            for item in items {
                if let Some(data_attr) = item.get("data") {
                    if let Ok(data_str) = data_attr.as_s() {
                        match serde_json::from_str::<T>(data_str) {
                            Ok(deserialized) => {
                                // Get or generate version ID
                                let version_id = if let Some(version_attr) = item.get("version_id") {
                                    if let Ok(version_str) = version_attr.as_s() {
                                        Uuid::parse_str(version_str).unwrap_or_else(|_| Uuid::new_v4())
                                    } else {
                                        Uuid::new_v4()
                                    }
                                } else {
                                    Uuid::new_v4()
                                };
                                
                                results.push(TemporalQueryResult::new(
                                    deserialized,
                                    Timestamp(timestamp),
                                    version_id,
                                ));
                            },
                            Err(e) => {
                                // Log deserialization error but continue processing other items
                                eprintln!("Failed to deserialize item: {}", e);
                            }
                        }
                    }
                }
            }
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
        self.store_temporal(entity_id, data, valid_time).await
    }

    async fn query_at(
        &self,
        entity_id: &EntityId,
        timestamp: DateTime<Utc>,
    ) -> Result<Vec<TemporalQueryResult<T>>> {
        let query = optimize_temporal_query(
            &TemporalOperation::At(timestamp),
            self.table_name.clone(),
        )?;

        let response = self.execute_query(&query).await?;
        self.process_query_results(response, timestamp).await
    }

    async fn query_between(
        &self,
        entity_id: &EntityId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TemporalQueryResult<T>>> {
        let query = optimize_temporal_query(
            &TemporalOperation::Between(start, end),
            self.table_name.clone(),
        )?;

        let response = self.execute_query(&query).await?;
        self.process_query_results(response, start).await
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
    ) -> Result<Option<TemporalQueryResult<T>>> {
        let query = optimize_temporal_query(
            &TemporalOperation::Latest,
            self.table_name.clone(),
        )?;

        let response = self.execute_query(&query).await?;
        let results = self.process_query_results(response, Utc::now()).await?;
        Ok(results.into_iter().next())
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
        let results = self.query_at(&EntityId {
            entity_type: EntityType::Edge,
            id: Uuid::new_v4().to_string()
        }, timestamp).await?;
        Ok(results.into_iter()
            .filter_map(|r| if let Ok(edge) = serde_json::from_value(serde_json::to_value(r.data).unwrap()) {
                Some(edge)
            } else {
                None
            })
            .collect())
    }

    async fn get_nodes_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        node_type: Option<EntityType>,
    ) -> Result<Vec<Node>> {
        let results = self.query_between(
            &EntityId {
            entity_type: EntityType::Node,
            id: Uuid::new_v4().to_string()
            },
            start,
            end,
        ).await?;
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
        let results = self.query_between(
            &EntityId {
            entity_type: EntityType::Edge,
            id: Uuid::new_v4().to_string()
            },
            start,
            end,
        ).await?;
        Ok(results.into_iter()
            .filter_map(|r| if let Ok(edge) = serde_json::from_value(serde_json::to_value(r.data).unwrap()) {
                Some(edge)
            } else {
                None
            })
            .collect())
    }

    async fn store(&self, entity_id: EntityId, data: Box<dyn StorableData>, valid_time: TemporalRange) -> Result<()> {
        if let Some(node) = data.as_any().downcast_ref::<Node>() {
            let node = node.clone();
            let json_data = serde_json::to_string(&node)
                .map_err(|e| Error::Serialization(format!("Failed to serialize node: {}", e)))?;
            
            let mut item = HashMap::new();
            item.insert("entity_id".to_string(), AttributeValue::S(entity_id.to_string()));
            item.insert("valid_time_start".to_string(), AttributeValue::N(valid_time.start.unwrap_or_default().0.timestamp().to_string()));
            item.insert("valid_time_end".to_string(), AttributeValue::N(valid_time.end.unwrap_or_default().0.timestamp().to_string()));
            item.insert("data".to_string(), AttributeValue::S(json_data));
            item.insert("version_id".to_string(), AttributeValue::S(Uuid::new_v4().to_string()));
            
            self.client.put_item()
                .table_name(&self.table_name)
                .set_item(Some(item))
                .send()
                .await
                .map_err(|e| Error::DynamoDB(e.to_string()))?;
            
            Ok(())
        } else if let Some(edge) = data.as_any().downcast_ref::<Edge>() {
            let edge = edge.clone();
            let json_data = serde_json::to_string(&edge)
                .map_err(|e| Error::Serialization(format!("Failed to serialize edge: {}", e)))?;
            
            let mut item = HashMap::new();
            item.insert("entity_id".to_string(), AttributeValue::S(entity_id.to_string()));
            item.insert("valid_time_start".to_string(), AttributeValue::N(valid_time.start.unwrap_or_default().0.timestamp().to_string()));
            item.insert("valid_time_end".to_string(), AttributeValue::N(valid_time.end.unwrap_or_default().0.timestamp().to_string()));
            item.insert("data".to_string(), AttributeValue::S(json_data));
            item.insert("version_id".to_string(), AttributeValue::S(Uuid::new_v4().to_string()));
            
            self.client.put_item()
                .table_name(&self.table_name)
                .set_item(Some(item))
                .send()
                .await
                .map_err(|e| Error::DynamoDB(e.to_string()))?;
            
            Ok(())
        } else {
            Err(Error::InvalidDataType("Unsupported data type".to_string()))
        }
    }

    async fn get_node_evolution(&self, node_id: NodeId, time_range: &TemporalRange) -> Result<Vec<Node>> {
        let results = self.query_evolution(
            &EntityId {
            entity_type: EntityType::Node,
            id: node_id.0.to_string()
            },
            time_range,
        ).await?;
        Ok(results.into_iter()
            .filter_map(|r| if let Ok(node) = serde_json::from_value(serde_json::to_value(r.data).unwrap()) {
                Some(node)
            } else {
                None
            })
            .collect())
    }

    async fn get_edge_evolution(&self, edge_id: EdgeId, time_range: &TemporalRange) -> Result<Vec<Edge>> {
        let results = self.query_evolution(
            &EntityId {
            entity_type: EntityType::Edge,
            id: edge_id.0.to_string()
            },
            time_range,
        ).await?;
        Ok(results.into_iter()
            .filter_map(|r| if let Ok(edge) = serde_json::from_value(serde_json::to_value(r.data).unwrap()) {
                Some(edge)
            } else {
                None
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temporal::query_builder::{PropertyOperator, RelationshipDirection};

    #[derive(Debug, Serialize, Deserialize)]
    struct TestData {
        value: String,
    }

    #[tokio::test]
    async fn test_dynamodb_temporal() {
        // Test implementation
    }

    fn build_condition(property_name: &str, operator: &PropertyOperator, property_value: &str) -> String {
        match operator {
            PropertyOperator::Equal => format!("{} = {}", property_name, property_value),
            PropertyOperator::NotEqual => format!("{} <> {}", property_name, property_value),
            PropertyOperator::GreaterThan => format!("{} > {}", property_name, property_value),
            PropertyOperator::GreaterThanOrEqual => format!("{} >= {}", property_name, property_value),
            PropertyOperator::LessThan => format!("{} < {}", property_name, property_value),
            PropertyOperator::LessThanOrEqual => format!("{} <= {}", property_name, property_value),
            PropertyOperator::Contains => format!("contains({}, {})", property_name, property_value),
            PropertyOperator::StartsWith => format!("begins_with({}, {})", property_name, property_value),
            PropertyOperator::EndsWith => format!("ends_with({}, {})", property_name, property_value),
            PropertyOperator::In => format!("{} IN ({})", property_name, property_value),
            PropertyOperator::NotIn => format!("NOT {} IN ({})", property_name, property_value),
        }
    }
} 
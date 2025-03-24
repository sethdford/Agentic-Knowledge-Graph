use aws_sdk_neptune::Client as NeptuneClient;
use async_trait::async_trait;
use backoff::{ExponentialBackoff, future::retry};
use gremlin_client::{GremlinClient, ConnectionOptions, TlsOptions, Vertex, Edge as GremlinEdge, GValue, ToGValue, GResultSet};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use futures::TryStreamExt;
use serde_json::json;
use uuid::Uuid;
use std::str::FromStr;
use gremlin_client::GremlinError;
use tokio::time::sleep;

use crate::{
    error::{Error, Result},
    types::{Node, Edge, NodeId, EdgeId, TemporalRange, Properties, EntityType, LocalResultSet, FromLocalResultSet, Timestamp, GID},
    config::Config,
};

use super::{Graph, query};

/// Neptune implementation of the Graph trait
pub struct NeptuneGraph {
    client: Arc<GremlinClient>,
    config: Arc<Config>,
}

impl NeptuneGraph {
    /// Create a new NeptuneGraph instance with connection pooling and retry logic
    pub async fn new(config: &Config) -> Result<Self> {
        let options = ConnectionOptions::builder()
            .host(&config.neptune_endpoint)
            .port(8182)
            .ssl(true)
            .pool_size(config.max_connections)
            .build();

        let client = GremlinClient::connect(options)
            .map_err(|e| Error::Neptune(format!("Failed to connect: {}", e)))?;

        info!("Connected to Neptune at {}", config.neptune_endpoint);
        
        Ok(Self {
            client: Arc::new(client),
            config: Arc::new(config.clone()),
        })
    }

    /// Helper to validate temporal range
    fn validate_temporal_range(&self, range: &TemporalRange) -> Result<()> {
        if let (Some(start), Some(end)) = (range.start, range.end) {
            if start.0 > end.0 {
                return Err(Error::InvalidTemporalRange(
                    "Start time must be before end time".to_string()
                ));
            }
        }
        Ok(())
    }

    /// Safely converts a gremlin_client::GID to a String
    fn gid_to_string(id: &gremlin_client::GID) -> Result<String> {
        match id {
            gremlin_client::GID::String(s) => Ok(s.clone()),
            gremlin_client::GID::Int32(i) => Ok(i.to_string()),
            gremlin_client::GID::Int64(i) => Ok(i.to_string()),
            _ => Err(Error::Neptune("Unsupported GID format".to_string())),
        }
    }

    /// Parse a vertex into a Node
    fn parse_vertex(&self, vertex: &Vertex) -> Result<Node> {
        // Extract ID safely
        let id_str = Self::gid_to_string(vertex.id())?;
        let node_id = NodeId(Uuid::from_str(&id_str)?);
        
        // Extract properties
        let mut properties = HashMap::new();
        
        // Iterate through all property keys and values
        for (key, vertex_properties) in vertex.iter() {
            if !vertex_properties.is_empty() {
                // Take the first property value for each key
                let property = &vertex_properties[0];
                properties.insert(key.clone(), Self::convert_gvalue_to_json(property.value()));
            }
        }
        
        let label = vertex.label().to_string();
        
        // Determine entity type from properties
        let entity_type = if let Some(value) = properties.get("entity_type") {
            let entity_type_str = value.as_str()
                .ok_or_else(|| Error::Neptune("Invalid entity_type format".to_string()))?;
            match entity_type_str {
                "Person" => EntityType::Person,
                "Organization" => EntityType::Organization,
                "Location" => EntityType::Location,
                "Event" => EntityType::Event,
                "Topic" => EntityType::Topic,
                "Document" => EntityType::Document,
                "Vertex" => EntityType::Vertex,
                other => EntityType::Custom(other.to_string()),
            }
        } else {
            EntityType::Vertex
        };
        
        // Extract temporal range properties
        let valid_time = vertex.property("valid_time")
            .map(|prop| Self::convert_gvalue_to_json(&prop.value()))
            .ok_or_else(|| Error::Neptune("Missing valid_time property".to_string()))?;
        let valid_time: TemporalRange = serde_json::from_value(valid_time)?;
        
        let transaction_time = vertex.property("transaction_time")
            .map(|prop| Self::convert_gvalue_to_json(&prop.value()))
            .ok_or_else(|| Error::Neptune("Missing transaction_time property".to_string()))?;
        let transaction_time: TemporalRange = serde_json::from_value(transaction_time)?;
        
        Ok(Node {
            id: node_id,
            entity_type,
            label,
            properties: Properties(properties),
            valid_time,
            transaction_time,
        })
    }

    /// Convert a GValue to a serde_json::Value
    fn convert_gvalue_to_json(value: &GValue) -> serde_json::Value {
        match value {
            GValue::String(s) => serde_json::Value::String(s.clone()),
            GValue::Int32(i) => serde_json::Value::Number(serde_json::Number::from(*i as i64)),
            GValue::Int64(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
            GValue::Float(f) => {
                if let Some(n) = serde_json::Number::from_f64(*f as f64) {
                    serde_json::Value::Number(n)
                } else {
                    serde_json::Value::Null
                }
            },
            GValue::Double(d) => {
                if let Some(n) = serde_json::Number::from_f64(*d) {
                    serde_json::Value::Number(n)
                } else {
                    serde_json::Value::Null
                }
            },
            GValue::List(list) => serde_json::Value::Array(
                list.iter().map(|v| Self::convert_gvalue_to_json(v)).collect()
            ),
            GValue::Map(_) => serde_json::Value::Object(serde_json::Map::new()), // Simplified for now
            _ => serde_json::Value::Null,
        }
    }

    /// Parse a Gremlin edge into an Edge
    fn parse_edge(&self, edge: &GremlinEdge) -> Result<Edge> {
        // Extract ID safely
        let id_str = Self::gid_to_string(edge.id())?;
        let edge_id = EdgeId(Uuid::from_str(&id_str)?);
        
        // Extract source and target IDs
        let source_id_str = Self::gid_to_string(edge.out_v().id())?;
        let target_id_str = Self::gid_to_string(edge.in_v().id())?;
        
        let source_id = NodeId(Uuid::from_str(&source_id_str)?);
        let target_id = NodeId(Uuid::from_str(&target_id_str)?);
        
        // Extract properties
        let mut properties = HashMap::new();
        
        // Iterate through all properties
        for (key, property) in edge.iter() {
            properties.insert(key.clone(), Self::convert_gvalue_to_json(property.value()));
        }
        
        // Extract valid_time and transaction_time separately for special handling
        let valid_time = if let Some(valid_time_prop) = edge.property("valid_time") {
            let valid_time_value = Self::convert_gvalue_to_json(valid_time_prop.value());
            let valid_time_str = valid_time_value.as_str()
                .ok_or_else(|| Error::Neptune("Invalid valid_time format".to_string()))?;
            serde_json::from_str::<TemporalRange>(valid_time_str)?
        } else {
            // Default to unbounded range if not specified
            TemporalRange::unbounded()
        };
        
        let transaction_time = if let Some(tx_time_prop) = edge.property("transaction_time") {
            let tx_time_value = Self::convert_gvalue_to_json(tx_time_prop.value());
            let tx_time_str = tx_time_value.as_str()
                .ok_or_else(|| Error::Neptune("Invalid transaction_time format".to_string()))?;
            serde_json::from_str::<TemporalRange>(tx_time_str)?
        } else {
            // Default to unbounded range if not specified
            TemporalRange::unbounded()
        };
        
        Ok(Edge {
            id: edge_id,
            source_id,
            target_id,
            label: edge.label().to_string(),
            properties: Properties(properties),
            valid_time,
            transaction_time,
        })
    }
}

#[async_trait]
impl Graph for NeptuneGraph {
    async fn create_node(&self, node: Node) -> Result<NodeId> {
        self.validate_temporal_range(&node.valid_time)?;
        self.validate_temporal_range(&node.transaction_time)?;

        let query = query::create_node(&node);
        let params = &[
            ("id", &node.id as &dyn ToGValue),
            ("label", &node.label as &dyn ToGValue),
            ("entity_type", &node.entity_type.to_string() as &dyn ToGValue),
            ("properties", &node.properties as &dyn ToGValue),
            ("valid_time", &node.valid_time as &dyn ToGValue),
            ("transaction_time", &node.transaction_time as &dyn ToGValue),
        ];

        self.execute_query::<NodeId>(&query, params).await
    }

    async fn get_node(&self, id: NodeId) -> Result<Node> {
        let query = query::get_node(id);
        let params = &[("id", &id as &dyn ToGValue)];

        self.execute_query::<Node>(&query, params).await
    }

    async fn update_node(&self, node: Node) -> Result<()> {
        self.validate_temporal_range(&node.valid_time)?;
        self.validate_temporal_range(&node.transaction_time)?;

        let query = query::update_node(&node);
        let params = &[
            ("id", &node.id as &dyn ToGValue),
            ("label", &node.label as &dyn ToGValue),
            ("entity_type", &node.entity_type.to_string() as &dyn ToGValue),
            ("properties", &node.properties as &dyn ToGValue),
            ("valid_time", &node.valid_time as &dyn ToGValue),
            ("transaction_time", &node.transaction_time as &dyn ToGValue),
        ];

        self.execute_query::<()>(&query, params).await
    }

    async fn delete_node(&self, id: NodeId) -> Result<()> {
        let query = query::delete_node(id);
        let params = &[("id", &id as &dyn ToGValue)];

        self.execute_query::<()>(&query, params).await
    }

    async fn create_edge(&self, edge: Edge) -> Result<EdgeId> {
        self.validate_temporal_range(&edge.valid_time)?;
        self.validate_temporal_range(&edge.transaction_time)?;

        let query = query::create_edge(&edge);
        let params = &[
            ("id", &edge.id as &dyn ToGValue),
            ("source_id", &edge.source_id as &dyn ToGValue),
            ("target_id", &edge.target_id as &dyn ToGValue),
            ("label", &edge.label as &dyn ToGValue),
            ("properties", &edge.properties as &dyn ToGValue),
            ("valid_time", &edge.valid_time as &dyn ToGValue),
            ("transaction_time", &edge.transaction_time as &dyn ToGValue),
        ];

        self.execute_query::<EdgeId>(&query, params).await
    }

    async fn get_edge(&self, id: EdgeId) -> Result<Edge> {
        let query = query::get_edge(id);
        let params = &[("id", &id as &dyn ToGValue)];

        self.execute_query::<Edge>(&query, params).await
    }

    async fn update_edge(&self, edge: Edge) -> Result<()> {
        self.validate_temporal_range(&edge.valid_time)?;
        self.validate_temporal_range(&edge.transaction_time)?;

        let query = query::update_edge(&edge);
        let params = &[
            ("id", &edge.id as &dyn ToGValue),
            ("source_id", &edge.source_id as &dyn ToGValue),
            ("target_id", &edge.target_id as &dyn ToGValue),
            ("label", &edge.label as &dyn ToGValue),
            ("properties", &edge.properties as &dyn ToGValue),
            ("valid_time", &edge.valid_time as &dyn ToGValue),
            ("transaction_time", &edge.transaction_time as &dyn ToGValue),
        ];

        self.execute_query::<()>(&query, params).await
    }

    async fn delete_edge(&self, id: EdgeId) -> Result<()> {
        let query = query::delete_edge(id);
        let params = &[("id", &id as &dyn ToGValue)];

        self.execute_query::<()>(&query, params).await
    }

    async fn get_edges_for_node(&self, node_id: NodeId, temporal_range: Option<TemporalRange>) -> Result<Vec<Edge>> {
        let query = query::get_edges_for_node(node_id, temporal_range);
        self.execute_query::<Vec<Edge>>(&query, &[]).await
    }

    async fn get_connected_nodes(&self, node_id: NodeId, temporal_range: Option<TemporalRange>) -> Result<Vec<Node>> {
        let query = query::get_connected_nodes(node_id, temporal_range);
        self.execute_query::<Vec<Node>>(&query, &[]).await
    }

    async fn get_nodes_by_label(&self, label: &str) -> Result<Vec<Node>> {
        let query = query::get_nodes_by_label(label);
        let params = &[("label", &label as &dyn ToGValue)];

        self.execute_query::<Vec<Node>>(&query, params).await
    }

    async fn get_edges_by_label(&self, label: &str) -> Result<Vec<Edge>> {
        let query = query::get_edges_by_label(label);
        let params = &[("label", &label as &dyn ToGValue)];

        self.execute_query::<Vec<Edge>>(&query, params).await
    }

    async fn get_edges_between(&self, from: NodeId, to: NodeId) -> Result<Vec<Edge>> {
        let query = query::get_edges_between(&from, &to);
        let params = &[
            ("from", &from as &dyn ToGValue),
            ("to", &to as &dyn ToGValue),
        ];

        self.execute_query::<Vec<Edge>>(&query, params).await
    }

    async fn get_edges_from(&self, from: NodeId) -> Result<Vec<Edge>> {
        let query = query::get_edges_from(&from);
        let params = &[("from", &from as &dyn ToGValue)];

        self.execute_query::<Vec<Edge>>(&query, params).await
    }

    async fn get_edges_to(&self, to: NodeId) -> Result<Vec<Edge>> {
        let query = query::get_edges_to(&to);
        let params = &[("to", &to as &dyn ToGValue)];

        self.execute_query::<Vec<Edge>>(&query, params).await
    }

    async fn get_vertex(&self, id: &str) -> Result<Option<Node>> {
        let query = query::get_vertex(id);
        let params = &[("id", &id as &dyn ToGValue)];

        self.execute_query::<Option<Node>>(&query, params).await
    }

    async fn execute_gremlin_query(&self, query: &str, params: &[(&str, &dyn ToGValue)]) -> Result<GResultSet> {
        match self.client.execute(query, params) {
            Ok(result) => Ok(result),
            Err(err) => Err(Error::Neptune(format!("Failed to execute Gremlin query: {}", err))),
        }
    }

    async fn execute_query_with_retry<T>(&self, query: &str, params: &[(&str, &dyn ToGValue)]) -> Result<T>
    where
        T: FromLocalResultSet,
    {
        use backoff::{backoff::Backoff, ExponentialBackoff};
        
        let max_retries = 3;
        let mut backoff = ExponentialBackoff::default();
        
        for _ in 0..max_retries {
            match self.client.execute(query, params) {
                Ok(result_set) => {
                    // Extract the results from GResultSet and convert to Vec<GValue>
                    let mut results: Vec<GValue> = Vec::new();
                    for result in result_set {
                        match result {
                            Ok(value) => results.push(value),
                            Err(e) => log::warn!("Error processing result: {}", e),
                        }
                    }
                    
                    // Create LocalResultSet from the results
                    let local_result_set = LocalResultSet(results);
                    
                    return T::from_local_result_set(local_result_set);
                }
                Err(e) => {
                    if let Some(duration) = backoff.next_backoff() {
                        warn!("Query execution failed, retrying in {:?}: {}", duration, e);
                        tokio::time::sleep(duration).await;
                    } else {
                        return Err(Error::Neptune(format!("Query execution failed after retries: {}", e)));
                    }
                }
            }
        }
        
        Err(Error::Neptune("Maximum number of retries reached".to_string()))
    }

    async fn execute_query<T>(&self, query: &str, params: &[(&str, &dyn ToGValue)]) -> Result<T>
    where
        T: FromLocalResultSet,
    {
        self.execute_query_with_retry::<T>(query, params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path};

    async fn setup_mock_server() -> MockServer {
        let mock_server = MockServer::start().await;
        
        Mock::given(method("POST"))
            .and(path("/gremlin"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "requestId": "test-id",
                "status": {
                    "message": "",
                    "code": 200
                },
                "result": {
                    "data": [{
                        "id": "test-id",
                        "label": "test",
                        "properties": {}
                    }]
                }
            })))
            .mount(&mock_server)
            .await;
            
        mock_server
    }

    #[tokio::test]
    async fn test_create_node_with_mock() {
        let mock_server = setup_mock_server().await;
        let config = Config {
            neptune_endpoint: mock_server.uri(),
            ..Config::for_testing()
        };
        
        let graph = NeptuneGraph::new(&config).await.unwrap();
        let now = Timestamp(Utc::now());
        let node = Node {
            id: NodeId(uuid::Uuid::new_v4()),
            entity_type: EntityType::Event,
            label: "test".to_string(),
            properties: Properties(HashMap::new()),
            valid_time: TemporalRange {
                start: Some(now),
                end: None,
            },
            transaction_time: TemporalRange {
                start: Some(now),
                end: None,
            },
        };

        let result = graph.create_node(node.clone()).await;
        assert!(result.is_ok());
    }
} 